"""TCP/JSON analysis server: the bridge between an external process (e.g. an
in-game overlay mod) and this package's `evaluate`/`mcts.action_values`.

Protocol: line-delimited JSON over TCP. Each request is a single JSON object
on its own line; each response is a single JSON object on its own line.

Request (`cmd: "analyze"`):
    {"cmd": "analyze", "iterations": 200, "seed": 12345,
     "state": {"player": {...}, "hand": [...], "draw_pile": [...],
               "discard_pile": [...], "exhaust_pile": [...], "turn": 3,
               "monsters": [{...}, ...]}}

Response:
    {"legal_actions": [...], "values": {action: float}, "state_value": float,
     "expected_hp_lost": float, "action_hp_lost": {action: float}}

Request (`cmd: "deck_baseline"`):
    {"cmd": "deck_baseline", "deck": [...],
     "monsters": [{"name": "Fuzzy Wurm Crawler", "intent": "ACID_GOOP",
                   "statuses": [["Vulnerable", 1]]}],
     "seeds": 30, "iterations": 200}

    Legacy form (single named encounter, still supported):
    {"cmd": "deck_baseline", "deck": [...], "monster": "fuzzy-wurm-crawler",
     "seeds": 30, "iterations": 200}

Response:
    {"mean_hp_lost": float, "win_rate": float}

`deck_baseline` is a "deck vs. monster, before any cards are drawn" figure:
it runs `sts_sim.bench.run_deck` over `seeds` fresh fights (full player HP,
the monster's canonical starting HP/stats, the given `deck` reshuffled per
seed) and reports the average HP lost over a full fight under MCTS play. This
is independent of any specific draw, unlike `expected_hp_lost` (which is
anchored to the current, already-drawn state) - intended as a one-time
per-fight baseline computed when combat starts.

`state_value`/`values` are `evaluate`'s `player_fraction - monsters_fraction`
(see `src/lib.rs`), roughly -1..1. `action_hp_lost` converts each action's
value into an absolute HP figure the same way: the player's
fraction-remaining is approximated as `clamp(value, 0, 1)` (a winning value
~= player_fraction since monsters_fraction ~= 0; a non-positive value is
treated as "dead, 0 HP remaining"), then `hp_lost = player_hp -
remaining_fraction * player_max_hp`. This is a quick per-action ranking
signal, not a calibrated HP estimate — `action_values` averages over MCTS
rollouts whose tails play uniformly at random, which inflates the figure well
above what skilled play would actually lose.

`expected_hp_lost` (the top-line, "if I keep playing this out" number) is
instead `simulate_hp_lost` averaged over a few seeds: it plays the fight to
completion choosing each move via `mcts_search` against the true state
(non-clairvoyant - it never sees future draws), then reports the player's
actual HP lost. That mirrors a chess engine's principal-variation eval rather
than a single rollout-poisoned position value.

`build_state` is a pure function from the JSON `state` dict to a
`CombatState`, reusing the snapshot-reconstruction constructor parameters
(`player_block`, `player_statuses`, `turn`, the card piles, and per-monster
`block`/`statuses`/`intent`/`last_move`/`move_streak`) so a mid-fight game
state can be reconstructed faithfully, not just a fresh turn-0 combat.
"""

import argparse
import json
import socketserver

from . import CombatState, Monster, apply, evaluate, legal_actions, simulate_hp_lost
from . import mcts as _mcts

DEFAULT_PORT = 8765

# Number of independent simulate_hp_lost playouts averaged into the top-line
# expected_hp_lost. Each playout re-runs MCTS search at every decision until
# the fight ends, so this multiplies the request's cost roughly
# `playouts * (decisions per fight)`-fold over a single analyze call.
DEFAULT_PLAYOUTS = 3

# Number of independent seeds (fresh shuffles/draws) averaged into
# "deck_baseline"'s mean_hp_lost. run_deck's fight_outcomes_per_fight path
# runs all seeds in one rayon-parallel Rust call, so 30 seeds is still under
# 2s at the default iteration count.
DEFAULT_DECK_BASELINE_SEEDS = 30


def _statuses(entries):
    """Translate a JSON list of `[name, amount]` pairs into the list of
    `(name, amount)` tuples the `CombatState`/`Monster` constructors expect."""
    return [(name, amount) for name, amount in entries]


# The bridge mod sends each monster's `intent` as the raw STS2 MoveState id
# (e.g. "ACID_GOOP", "BUTT_MOVE"), but src/monsters.rs's `monster_move` and
# `select_next_intent` key on the display-style move names it generates
# itself (e.g. "Acid Goop", "Butt"). Without this translation, the FIRST
# simulated turn for a freshly-reconstructed monster never matches any
# `monster_move` arm, so that turn's attack is silently dropped (0 damage)
# before `select_next_intent` falls back to its own correctly-named moves for
# subsequent turns. Keyed by sts_sim monster name (post-NameMap.MonsterNameMap
# translation), then raw STS2 move id -> monsters.rs move name.
INTENT_NAME_MAP = {
    "Fuzzy Wurm Crawler": {
        "FIRST_ACID_GOOP": "Acid Goop",
        "ACID_GOOP": "Acid Goop",
        "INHALE": "Inhale",
    },
    "Nibbit": {
        "BUTT_MOVE": "Butt",
        "SLICE_MOVE": "Hesitant Slice",
        "HISS_MOVE": "Hiss",
    },
    "Shrinker Beetle": {
        "SHRINKER_MOVE": "Shrink",
        "CHOMP_MOVE": "Chomp",
        "STOMP_MOVE": "Stomp",
    },
    "Leaf Slime (S)": {
        "BUTT_MOVE": "Tackle",
        "GOOP_MOVE": "Goop",
    },
    "Leaf Slime (M)": {
        "CLUMP_SHOT": "ClumpShot",
        "STICKY_SHOT": "StickyShot",
    },
    "Twig Slime (S)": {
        "BUTT_MOVE": "Tackle",
    },
    "Twig Slime (M)": {
        "CLUMP_SHOT_MOVE": "ClumpShot",
        "STICKY_SHOT_MOVE": "StickyShot",
    },
    "Byrdonis": {
        "SWOOP_MOVE": "Swoop",
        "PECK_MOVE": "Peck",
    },
    "Inklet": {
        "JAB_MOVE": "Jab",
        "PIERCING_GAZE_MOVE": "Piercing Gaze",
        "WHIRLWIND_MOVE": "Windup Punch",
    },
    "Vantom": {
        "INK_BLOT_MOVE": "Ink Blot",
        "INKY_LANCE_MOVE": "Inky Lance",
        "DISMEMBER_MOVE": "Dismember",
        "PREPARE_MOVE": "Prepare",
    },
}


def _translate_intent(monster_name, intent):
    """Map a raw STS2 move id to the move name monsters.rs expects, for the
    monster names listed in `INTENT_NAME_MAP`. Unmapped monsters/intents pass
    through unchanged (monsters.rs's `monster_move` returns `None` for them
    either way, same as before this translation existed)."""
    if intent is None:
        return None
    return INTENT_NAME_MAP.get(monster_name, {}).get(intent, intent)


def build_state(payload):
    """Reconstruct a `CombatState` from an "analyze" request's `state` dict."""
    state = payload["state"]
    player = state["player"]
    monsters = [
        Monster(
            hp=m["hp"],
            attack=m.get("attack", 0),
            max_hp=m.get("max_hp"),
            name=m.get("name"),
            block=m.get("block", 0),
            statuses=_statuses(m.get("statuses", [])),
            intent=_translate_intent(m.get("name"), m.get("intent")),
            last_move=m.get("last_move"),
            move_streak=m.get("move_streak", 0),
        )
        for m in state["monsters"]
    ]
    return CombatState(
        player_hp=player["hp"],
        player_energy=player["energy"],
        monsters=monsters,
        seed=payload.get("seed", 0),
        hand=state.get("hand", []),
        player_max_hp=player.get("max_hp"),
        player_max_energy=player.get("max_energy"),
        player_block=player.get("block", 0),
        player_statuses=_statuses(player.get("statuses", [])),
        turn=state.get("turn", 0),
        draw_pile=state.get("draw_pile", []),
        discard_pile=state.get("discard_pile", []),
        exhaust_pile=state.get("exhaust_pile", []),
    )


def _expected_hp_lost(value, state):
    """Convert an `evaluate`-style value (`player_fraction -
    monsters_fraction`, roughly -1..1) into an absolute expected-HP-lost
    figure — see the module docstring for the approximation used."""
    remaining_fraction = min(max(value, 0.0), 1.0)
    expected_final_hp = remaining_fraction * state.player_max_hp
    return max(state.player_hp - expected_final_hp, 0.0)


def _simulated_expected_hp_lost(state, iterations, seed, playouts):
    """Average `simulate_hp_lost` over `playouts` seeds derived from `seed`:
    each playout plays the fight to completion picking every move via
    `mcts_search` against the true state (non-clairvoyant), then reports the
    player's actual HP lost. See the module docstring for why this — rather
    than `_expected_hp_lost` over `state_value` — is the top-line figure."""
    losses = [
        simulate_hp_lost(state, iterations=iterations, seed=seed + i)
        for i in range(playouts)
    ]
    return sum(losses) / len(losses)


def handle_request(payload):
    """Dispatch one decoded request to its handler and return a dict to be
    JSON-encoded as the response. `cmd` is dispatched explicitly so future
    commands can be added without changing the wire format of `"analyze"`."""
    cmd = payload.get("cmd")
    if cmd == "analyze":
        state = build_state(payload)
        iterations = payload.get("iterations", _mcts.DEFAULT_ITERATIONS)
        seed = payload.get("seed", 0)
        playouts = payload.get("playouts", DEFAULT_PLAYOUTS)
        actions = legal_actions(state)
        values = _mcts.action_values(state, iterations=iterations)
        state_value = max(values.values()) if values else evaluate(state)
        # Per-target greedy evaluation for single-target cards in multi-monster
        # fights. MCTS action_values already fold in the optimal target
        # internally; this tells the player *which* target that is. Uses
        # evaluate() (one-step greedy) rather than another MCTS pass — the
        # ranking across targets is what matters, not calibrated HP estimates.
        from . import PlayCardAction, SelectTargetAction

        target_values = {}
        if len(state.monsters) > 1:
            for action in actions:
                if not isinstance(action, PlayCardAction):
                    continue
                try:
                    mid = apply(state, action)
                except Exception:
                    continue
                sub = legal_actions(mid)
                if not sub or not isinstance(sub[0], SelectTargetAction):
                    continue
                tv = {}
                for sub_action in sub:
                    try:
                        tv[str(sub_action)] = evaluate(apply(mid, sub_action))
                    except Exception:
                        pass
                if tv:
                    target_values[str(action)] = tv
        return {
            "legal_actions": [str(a) for a in actions],
            "values": {str(a): v for a, v in values.items()},
            "state_value": state_value,
            "expected_hp_lost": _simulated_expected_hp_lost(
                state, iterations, seed, playouts
            ),
            "action_hp_lost": {
                str(a): _expected_hp_lost(v, state) for a, v in values.items()
            },
            "target_values": target_values,
        }
    if cmd == "deck_baseline":
        from .bench import run_deck
        from .scenarios import (
            IRONCLAD_STARTING_DECK,
            MONSTER_STARTING_HP,
            PLAYER_STARTING_HP,
        )

        monsters_list = payload.get("monsters")
        if monsters_list is not None:
            # Dynamic monsters protocol: build a scenario_fn closure from the
            # live monster list rather than a named Encounter key. Each entry
            # must have "name" (sts_sim monster name) and may have "intent"
            # (raw STS2 move id) and "statuses" ([[name, amount], ...]).
            monster_specs = []
            for m in monsters_list:
                name = m["name"]
                hp = MONSTER_STARTING_HP.get(name)
                if hp is None:
                    raise ValueError(
                        f"unknown monster name for deck_baseline: {name!r}"
                    )
                monster_specs.append(
                    {
                        "name": name,
                        "hp": hp,
                        "intent": _translate_intent(name, m.get("intent")),
                        "statuses": _statuses(m.get("statuses", [])),
                    }
                )

            def _scenario_fn(seed, deck):
                return CombatState(
                    player_hp=PLAYER_STARTING_HP,
                    player_energy=3,
                    monsters=[
                        Monster(
                            hp=spec["hp"],
                            attack=0,
                            name=spec["name"],
                            intent=spec["intent"],
                            statuses=spec["statuses"],
                        )
                        for spec in monster_specs
                    ],
                    seed=seed,
                    deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
                )

            result = run_deck(
                payload.get("deck"),
                scenario_fn=_scenario_fn,
                seeds=payload.get("seeds", DEFAULT_DECK_BASELINE_SEEDS),
                iterations=payload.get("iterations", _mcts.DEFAULT_ITERATIONS),
            )
        else:
            result = run_deck(
                payload.get("deck"),
                monster=payload["monster"],
                seeds=payload.get("seeds", DEFAULT_DECK_BASELINE_SEEDS),
                iterations=payload.get("iterations", _mcts.DEFAULT_ITERATIONS),
            )
        return {
            "mean_hp_lost": result.mean_hp_lost,
            "win_rate": result.win_rate,
        }
    raise ValueError(f"unknown cmd: {cmd!r}")


class _Handler(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            line = line.decode("utf-8").strip()
            if not line:
                continue
            try:
                response = handle_request(json.loads(line))
            except Exception as exc:  # noqa: BLE001 - report to client, don't crash
                response = {"error": str(exc)}
            self.wfile.write((json.dumps(response) + "\n").encode("utf-8"))


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def make_server(host="127.0.0.1", port=DEFAULT_PORT):
    """Construct (but don't start) a `Server` bound to `(host, port)`. Pass
    `port=0` to let the OS assign a free port (useful for tests) — read the
    actual port back via `server.server_address`."""
    return Server((host, port), _Handler)


def serve(host="127.0.0.1", port=DEFAULT_PORT):
    """Bind and serve forever. The CLI entry point."""
    with make_server(host, port) as server:
        host, port = server.server_address
        print(f"sts_sim analysis server listening on {host}:{port}")
        server.serve_forever()


def main():
    parser = argparse.ArgumentParser(description="sts_sim TCP/JSON analysis server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    args = parser.parse_args()
    serve(args.host, args.port)


if __name__ == "__main__":
    main()
