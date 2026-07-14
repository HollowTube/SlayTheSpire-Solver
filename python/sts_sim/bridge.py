"""Translate bridge JSON snapshots into sim CombatState objects.

The bridge exposes the live game state via get_combat_state() and
get_player_state(). This module converts that JSON into a CombatState
that the sim can advance with apply_action().

Gaps (accepted, documented):
  - draw_pile order: unknown (hidden info). Populated as deck minus
    hand/discard in arbitrary order — draw outcomes will diverge.
  - RNG state: not recoverable. Predictions depending on random rolls
    will diverge.
  - Powers/statuses: partially mapped; unknown ones are silently dropped.
"""

from __future__ import annotations

from typing import Any

from . import CombatState, Monster
from . import names as _names
from .names import StatusName

# Derived from StatusName.bridge_classes — edit data/statuses.toml, not this line.
STATUS_MAP: dict[str, str] = {
    bc: s.value for s in StatusName for bc in s.bridge_classes
}


def _map_statuses(powers: list[dict]) -> list[tuple[str, int]]:
    """Convert bridge powers list to (status_name, stacks) pairs the sim knows."""
    result = []
    for p in powers:
        name = p.get("name") or p.get("id") or p.get("power_id", "")
        stacks = int(p.get("stacks", p.get("amount", 1)))
        mapped = STATUS_MAP.get(name)
        if mapped:
            result.append((mapped, stacks))
    return result


def _card_list(cards: list[dict]) -> list[str]:
    """Convert a bridge card list to sim card strings (e.g. ``"Strike"`` / ``"Strike+"``).

    Upgrade encoding: append ``"+"`` for each upgrade level, matching the
    sim's ``CardInstance::as_str`` format.
    """
    result = []
    for c in cards:
        sim_name = _names.card(c.get("name", ""))
        if c.get("upgraded"):
            sim_name += "+"
        result.append(sim_name)
    return result


def from_combat(combat: dict, player_state: dict | None = None) -> CombatState:
    """Build a CombatState from bridge get_combat_state() output.

    Args:
        combat: Result of bridge_client.get_combat_state() (already unwrapped
                from the ``result`` envelope).
        player_state: Optional result of get_player_state() — used to populate
                      the draw pile from the full deck when combat doesn't
                      include it.

    Returns:
        A CombatState with all deterministic fields populated. Draw pile
        order is arbitrary; RNG state is fresh (seed=0).
    """
    players = combat.get("players", [])
    p = players[0] if players else {}

    hand_raw = p.get("hand", [])
    hand = _card_list(hand_raw)

    discard_raw = combat.get("discard_pile", p.get("discard", []))
    discard = _card_list(discard_raw)

    # Draw pile: use combat draw_pile if available, else reconstruct from deck
    draw_raw = combat.get("draw_pile", p.get("draw", []))
    if draw_raw:
        draw = _card_list(draw_raw)
    elif player_state:
        ps_players = player_state.get("players", [])
        ps_p = ps_players[0] if ps_players else {}
        deck_raw = ps_p.get("deck", [])
        draw_raw_all = _card_list(deck_raw)
        # best-effort: subtract hand+discard counts
        hand_counts: dict[str, int] = {}
        for c in hand + discard:
            hand_counts[c] = hand_counts.get(c, 0) + 1
        draw = []
        for c in draw_raw_all:
            if hand_counts.get(c, 0) > 0:
                hand_counts[c] -= 1
            else:
                draw.append(c)
    else:
        draw = []

    # Player statuses
    p_statuses = _map_statuses(p.get("powers", []))

    # Monsters
    monsters = []
    for e in combat.get("enemies", []):
        if not e.get("is_alive", True):
            continue
        sim_name = _names.monster(e.get("name", ""))
        intent_raw = e.get("intent")
        intent_str = _fmt_intent(intent_raw)
        m_statuses = _map_statuses(e.get("powers", []))
        monsters.append(
            Monster(
                hp=e.get("hp", 0),
                max_hp=e.get("max_hp", e.get("hp", 0)),
                block=e.get("block", 0),
                attack=0,
                name=sim_name,
                intent=intent_str,
                statuses=m_statuses,
            )
        )

    return CombatState(
        player_hp=p.get("hp", 0),
        player_max_hp=p.get("max_hp", p.get("hp", 0)),
        player_block=p.get("block", 0),
        player_energy=p.get("energy", 3),
        player_statuses=p_statuses,
        monsters=monsters,
        hand=hand,
        draw_pile=draw,
        discard_pile=discard,
        seed=0,
    )


def _fmt_intent(intent: Any) -> str | None:
    """Convert bridge intent dict to a sim-compatible intent string."""
    if intent is None:
        return None
    if isinstance(intent, str):
        return intent
    if not isinstance(intent, dict):
        return str(intent)
    move_id = intent.get("move_id", "")
    intents = intent.get("intents", [])
    if not intents:
        return move_id or None
    parts = []
    for i in intents:
        t = i.get("type", "")
        if t == "Attack":
            dmg = i.get("damage", 0)
            hits = i.get("hits", 1)
            parts.append(f"Attack({dmg}×{hits})" if hits > 1 else f"Attack({dmg})")
        else:
            parts.append(t)
    return " + ".join(parts) if parts else move_id or None


def diff(predicted: CombatState, actual: dict) -> dict[str, Any]:
    """Compare a sim-predicted CombatState against a bridge combat snapshot.

    Returns a dict of field comparisons. Fields that are non-deterministic
    (draw results, enemy intent selection) are marked ``skipped``.

    Each entry is one of:
        {"match": True, "sim": v, "game": v}
        {"match": False, "sim": v, "game": v}
        {"skipped": True, "reason": str}
    """
    result: dict[str, Any] = {}

    actual_players = actual.get("players", [{}])
    ap = actual_players[0] if actual_players else {}

    # Player HP
    result["player_hp"] = _cmp(predicted.player_hp, ap.get("hp"))
    result["player_block"] = _cmp(predicted.player_block, ap.get("block", 0))
    result["player_energy"] = _cmp(predicted.player_energy, ap.get("energy"))

    # Enemies
    actual_enemies = [e for e in actual.get("enemies", []) if e.get("is_alive", True)]
    for i, (sim_m, act_e) in enumerate(zip(predicted.monsters, actual_enemies)):
        prefix = f"enemy[{i}]"
        result[f"{prefix}.hp"] = _cmp(sim_m.hp, act_e.get("hp"))
        result[f"{prefix}.block"] = _cmp(sim_m.block, act_e.get("block", 0))
        result[f"{prefix}.intent"] = {"skipped": True, "reason": "non-deterministic"}

    # Hand (compare as sorted multiset — order within hand is stable after set_hand)
    result["hand_cards"] = _cmp_pile(predicted.hand, _card_list(ap.get("hand", [])))

    # Discard pile
    act_discard_raw = actual.get("discard_pile", ap.get("discard", []))
    result["discard_pile"] = _cmp_pile(
        predicted.discard_pile, _card_list(act_discard_raw)
    )

    # Draw pile (order unknown — compare as multiset)
    act_draw_raw = actual.get("draw_pile", ap.get("draw", []))
    result["draw_pile"] = _cmp_pile(predicted.draw_pile, _card_list(act_draw_raw))

    # Exhaust pile
    act_exhaust_raw = actual.get("exhaust_pile", ap.get("exhaust", []))
    result["exhaust_pile"] = _cmp_pile(
        predicted.exhaust_pile, _card_list(act_exhaust_raw)
    )

    # Player statuses
    sim_p_statuses = dict(predicted.player_statuses)
    act_p_statuses = _statuses_from_powers(ap.get("powers", []))
    for status in sim_p_statuses.keys() | act_p_statuses.keys():
        result[f"player.{status}"] = _cmp(
            sim_p_statuses.get(status, 0), act_p_statuses.get(status, 0)
        )

    # Enemy statuses
    for i, (sim_m, act_e) in enumerate(zip(predicted.monsters, actual_enemies)):
        sim_m_statuses = dict(sim_m.statuses)
        act_m_statuses = _statuses_from_powers(act_e.get("powers", []))
        for status in sim_m_statuses.keys() | act_m_statuses.keys():
            result[f"enemy[{i}].{status}"] = _cmp(
                sim_m_statuses.get(status, 0), act_m_statuses.get(status, 0)
            )

    return result


def _statuses_from_powers(powers: list[dict]) -> dict[str, int]:
    """Convert bridge powers list to {sim_status_name: stacks} dict."""
    out: dict[str, int] = {}
    for p in powers:
        name = p.get("name") or p.get("id") or p.get("power_id", "")
        mapped = STATUS_MAP.get(name)
        if mapped:
            out[mapped] = out.get(mapped, 0) + int(p.get("stacks", p.get("amount", 1)))
    return out


def _cmp(sim_val: Any, game_val: Any) -> dict[str, Any]:
    match = sim_val == game_val
    return {"match": match, "sim": sim_val, "game": game_val}


def _cmp_pile(sim_pile: list[str], act_pile: list[str]) -> dict[str, Any]:
    """Compare two card piles as sorted multisets (order-independent)."""
    s, g = sorted(sim_pile), sorted(act_pile)
    return {"match": s == g, "sim": s, "game": g}
