"""Interactive and agent-driven CLI for playing fights against `sts_core`.

Per HOL-14: a pure consumer of the existing public engine interface
(`CombatState`/`apply`/`legal_actions`/`is_terminal`/`reward`/`evaluate`) —
no new Rust-side surface area. Rendering and menu-building are kept as pure
functions, separate from input handling, so a future webapp/TUI can reuse
them and so interactive/agent modes can never visually drift apart.
"""

from collections import Counter
from dataclasses import dataclass

import click

from . import apply, evaluate, is_terminal, legal_actions, reward
from .scenarios import (
    ironclad_starter_deck_vs_fuzzy_wurm_crawler,
    ironclad_starter_deck_vs_gremlin_nob,
    ironclad_starter_deck_vs_jaw_worm,
    ironclad_starter_deck_vs_nibbit,
)

_SCENARIOS = {
    "jaw-worm": ironclad_starter_deck_vs_jaw_worm,
    "gremlin-nob": ironclad_starter_deck_vs_gremlin_nob,
    "nibbit": ironclad_starter_deck_vs_nibbit,
    "fuzzy-wurm-crawler": ironclad_starter_deck_vs_fuzzy_wurm_crawler,
}

# Translates the engine's internal action-string vocabulary into labels a
# human can read without knowing the `"PlayCard:Strike"` format. The engine
# strings themselves are round-tripped to `apply` unchanged — the CLI never
# parses or constructs them.
_ACTION_LABELS = {
    "EndTurn": "End Turn",
}


# Per src/monsters.rs: base attack damage of each move, before Strength.
# Only moves that deal damage appear here; non-attacks have no entry.
_INTENT_BASE_DAMAGE = {
    ("Jaw Worm", "Chomp"): 11,
    ("Jaw Worm", "Thrash"): 7,
    ("Gremlin Nob", "Rush"): 14,
    ("Gremlin Nob", "Skull Bash"): 6,
    ("Nibbit", "Butt"): 12,
    ("Nibbit", "Hesitant Slice"): 6,
    ("Fuzzy Wurm Crawler", "Acid Goop"): 4,
}

# Human-readable description of each move's full effect list (display only).
_INTENT_DESCRIPTIONS = {
    ("Jaw Worm", "Chomp"): "11 damage",
    ("Jaw Worm", "Thrash"): "7 damage, gain 5 block",
    ("Jaw Worm", "Bellow"): "gain 3 Strength, gain 6 block",
    ("Gremlin Nob", "Bellow"): "gain Enrage 2",
    ("Gremlin Nob", "Rush"): "14 damage",
    ("Gremlin Nob", "Skull Bash"): "6 damage, apply 2 Vulnerable",
    ("Nibbit", "Butt"): "12 damage",
    ("Nibbit", "Hesitant Slice"): "6 damage, gain 5 block",
    ("Nibbit", "Hiss"): "gain 2 Strength",
    ("Fuzzy Wurm Crawler", "Acid Goop"): "4 damage",
    ("Fuzzy Wurm Crawler", "Inhale"): "gain 7 Strength",
}


def intent_description(monster_name, intent):
    """Return a plain-English description of what `intent` does.
    Falls back to the intent name for any pair not in the table."""
    return _INTENT_DESCRIPTIONS.get((monster_name, intent), intent)


def effective_intent_description(state):
    """Like intent_description but adjusts attack damage by the monster's
    current Strength so the displayed number matches what will actually land."""
    name = state.monster_name
    intent = state.monster_intent
    if not name or not intent:
        return intent or ""
    strength = state.monster_strength
    base = _INTENT_BASE_DAMAGE.get((name, intent), 0)
    desc = _INTENT_DESCRIPTIONS.get((name, intent), intent)
    if base > 0 and strength > 0:
        effective = base + strength
        desc = desc.replace(f"{base} damage", f"{effective} damage (+{strength} Str)")
    return desc


def format_action(action):
    """Translate one `legal_actions` string into a human-readable label."""
    if action in _ACTION_LABELS:
        return _ACTION_LABELS[action]
    if action.startswith("PlayCard:"):
        return f"Play {action.removeprefix('PlayCard:')}"
    if action.startswith("SelectTarget:"):
        return f"Target {action.removeprefix('SelectTarget:')}"
    return action


def _format_statuses(statuses):
    if not statuses:
        return "none"
    counts = Counter(statuses)
    return ", ".join(
        name if count == 1 else f"{name} x{count}" for name, count in counts.items()
    )


def render_state(state):
    """Render a `CombatState` as the human-readable text both CLI modes show.

    Surfaces everything HOL-14's user stories name as decision-relevant:
    HP/block/energy/hand/statuses for the player, and HP/block/intent/
    statuses for the monster — plus the turn number for orientation.
    """
    lines = [
        f"Turn {state.turn}",
        f"You: {state.player_hp} HP | Block: {state.player_block} | "
        f"Energy: {state.player_energy}",
        f"  Statuses: {_format_statuses(state.player_statuses)}",
        f"  Hand: {', '.join(state.hand) if state.hand else 'empty'}",
        f"{state.monster_name}: {state.monster_hp} HP | Block: {state.monster_block} | "
        f"Intent: {state.monster_intent}"
        + (
            f" ({effective_intent_description(state)})"
            if state.monster_intent and state.monster_name
            else ""
        ),
        f"  Statuses: {_format_statuses(state.monster_statuses)}",
    ]
    return "\n".join(lines)


def _menu_text(actions, values=None):
    lines = []
    for i, a in enumerate(actions):
        label = format_action(a)
        if values and a in values:
            v = values[a]
            annotation = f"  [{v:+.2f}]"
        else:
            annotation = ""
        lines.append(f"  {i + 1}. {label}{annotation}")
    return "\n".join(lines)


def prompt_for_choice(actions, input_fn, output_fn, values=None):
    """Read a 1-based menu choice from `input_fn`, re-prompting on bad input.

    `input_fn(prompt_text)` and `output_fn(message)` are injected so this —
    and the loop built on it — can be driven by scripted input in tests
    (per HOL-14's Testing Decisions) without touching real stdin/stdout.
    """
    output_fn("Choose an action:")
    output_fn(_menu_text(actions, values))
    while True:
        raw = input_fn("> ")
        try:
            choice = int(raw)
        except ValueError:
            output_fn(f"Invalid input: {raw!r} is not a number. Try again.")
            continue
        if not 1 <= choice <= len(actions):
            output_fn(
                f"Invalid choice: {choice} is not between 1 and {len(actions)}. Try again."
            )
            continue
        return actions[choice - 1]


def describe_turn_outcome(before, after):
    """One-line summary of what the monster did on its turn.

    Compares the player's HP/block before and after EndTurn to derive the
    net incoming damage and how much block absorbed, without needing a
    separate engine event stream.
    """
    name = before.monster_name or "Monster"
    intent = before.monster_intent or "?"
    desc = effective_intent_description(before)

    hp_lost = before.player_hp - after.player_hp
    block_before = before.player_block

    if hp_lost > 0:
        raw = hp_lost + block_before
        if block_before > 0:
            damage_line = (
                f"{raw} damage dealt → {block_before} blocked, {hp_lost} HP lost"
            )
        else:
            damage_line = f"{hp_lost} HP lost"
    elif block_before > 0:
        # Fully blocked — we know the attack was ≤ block_before
        damage_line = "fully blocked — no HP lost"
    else:
        damage_line = "no damage"

    return f"  {name} used {intent} ({desc}) → {damage_line}"


def _report_outcome(state, output_fn):
    outcome = "won" if state.monster_hp <= 0 else "lost"
    output_fn(f"You {outcome}! Final HP: {state.player_hp}")
    output_fn(f"Reward: {reward(state):.2f} (evaluate: {evaluate(state):.2f})")


def run_interactive(state, input_fn=input, output_fn=print, analysis=False):
    """The human-facing REPL core loop, per HOL-14 user stories 1-7.

    Render -> menu -> read+validate a numeric choice -> apply -> repeat
    until terminal, then report the outcome. Pure consumer of the public
    `CombatState`/`apply`/`legal_actions`/`is_terminal`/`reward`/`evaluate`
    interface — no engine-side seams of its own.

    When `analysis=True`, runs MCTS before each decision to annotate the
    menu with per-action value estimates and the overall state value.
    """
    from . import mcts as _mcts

    while not is_terminal(state):
        output_fn("")
        output_fn(render_state(state))
        actions = legal_actions(state)
        values = None
        if analysis:
            values = _mcts.action_values(state)
            state_value = max(values.values())
            output_fn(f"State value: {state_value:+.2f} (MCTS, optimal play)")
        action = prompt_for_choice(actions, input_fn, output_fn, values=values)
        prev = state
        state = apply(state, action)
        if action == "EndTurn":
            output_fn(describe_turn_outcome(prev, state))

    output_fn("")
    output_fn(render_state(state))
    _report_outcome(state, output_fn)
    return state


def replay_history(seed, history, monster="jaw-worm"):
    """Reconstruct the state reached by `history` from a fresh `seed`.

    Leans entirely on the property M1 already guarantees — `CombatState`
    embeds its own PRNG and `apply` is pure/deterministic, so `(seed,
    history)` fully reconstructs any reachable state — exactly the
    seed+history replay contract HOL-14 chose over persistent sessions.
    """
    scenario_fn = _SCENARIOS.get(monster)
    if scenario_fn is None:
        raise ValueError(
            f"unknown monster {monster!r}; choices: {', '.join(_SCENARIOS)}"
        )
    state = scenario_fn(seed=seed)
    for action in history:
        state = apply(state, action)
    return state


@dataclass
class StepResult:
    """Everything one agent-step invocation hands back, per user story 15:
    self-contained — the agent just echoes `updated_history` into the next
    call, never tracking or re-deriving it itself."""

    state: object
    rendered: str
    legal_actions: list
    updated_history: list
    action: str = ""
    previous_state: object = None


def run_step(seed, history, action, monster="jaw-worm"):
    """One non-blocking agent-mode step, per HOL-14 user story 14.

    Replays `history` from `seed`, applies `action`, and returns the
    resulting state, its render, its legal-action menu, and the updated
    history — all in one shot. No persistent process, no new engine state.
    """
    before = replay_history(seed, history, monster=monster)
    state = apply(before, action)
    return StepResult(
        state=state,
        rendered=render_state(state),
        legal_actions=legal_actions(state),
        updated_history=list(history) + [action],
        action=action,
        previous_state=before,
    )


def render_step_result(result):
    """Render a `StepResult` as the text the agent-mode `step` command prints."""
    lines = [result.rendered]
    if result.action == "EndTurn" and result.previous_state is not None:
        lines.append(describe_turn_outcome(result.previous_state, result.state))
    lines += ["", _menu_text(result.legal_actions)]
    if is_terminal(result.state):
        lines.append("")
        outcome = "won" if result.state.monster_hp <= 0 else "lost"
        lines.append(f"Fight over — you {outcome}! Reward: {reward(result.state):.2f}")
    lines.append("")
    lines.append(f"updated_history: {','.join(result.updated_history)}")
    return "\n".join(lines)


@click.group(invoke_without_command=True)
@click.option(
    "--seed",
    type=int,
    default=42,
    show_default=True,
    help="RNG seed for the canonical scenario (replayable).",
)
@click.option(
    "--monster",
    type=click.Choice(list(_SCENARIOS)),
    default="jaw-worm",
    show_default=True,
    help="Which enemy to fight.",
)
@click.option(
    "--analysis",
    is_flag=True,
    default=False,
    help="Show MCTS value estimates per action before each decision.",
)
@click.pass_context
def main(ctx, seed, monster, analysis):
    """Play a fight against the sts_sim simulator from the terminal."""
    if ctx.invoked_subcommand is None:
        state = _SCENARIOS[monster](seed=seed)
        run_interactive(state, analysis=analysis)


@main.command()
@click.option(
    "--seed", type=int, required=True, help="RNG seed identifying the scenario."
)
@click.option(
    "--monster",
    type=click.Choice(list(_SCENARIOS)),
    default="jaw-worm",
    show_default=True,
    help="Which enemy to fight.",
)
@click.option(
    "--history",
    default="",
    help="Comma-separated action history replayed before --action.",
)
@click.option(
    "--action", required=True, help="The new action to apply after replaying --history."
)
def step(seed, monster, history, action):
    """Agent mode: replay HISTORY from SEED, apply ACTION, print the result.

    Non-blocking and stateless — prints the resulting state, the new
    legal-actions menu, and `updated_history` to feed into the next
    invocation, then exits immediately (per HOL-14 user stories 14-15).
    """
    parsed_history = [a for a in history.split(",") if a]
    try:
        result = run_step(
            seed=seed, history=parsed_history, action=action, monster=monster
        )
    except ValueError as exc:
        raise click.UsageError(str(exc)) from exc
    click.echo(render_step_result(result))


if __name__ == "__main__":
    main()
