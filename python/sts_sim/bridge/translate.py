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

from typing import Any, Protocol, Sequence

from .. import CombatState, EndTurnAction, Monster, PlayCardAction, SelectTargetAction
from .. import names as _names
from .types import (
    AvailableActions,
    CardEntry,
    CardPiles,
    CombatSnapshot,
    Intent,
    Power,
)
from ..names import StatusName

# Derived from StatusName.bridge_classes — edit data/statuses.toml, not this line.
STATUS_MAP: dict[str, str] = {
    bc: s.value for s in StatusName for bc in s.bridge_classes
}


class _CardLike(Protocol):
    name: str
    upgraded: bool


def _map_statuses(powers: list[Power]) -> list[tuple[str, int]]:
    """Convert typed Power list to (status_name, stacks) pairs the sim knows."""
    result = []
    for p in powers:
        mapped = STATUS_MAP.get(p.name)
        if mapped:
            result.append((mapped, p.amount))
    return result


def _card_list(cards: Sequence[_CardLike]) -> list[str]:
    """Convert a typed card list to sim card strings (e.g. ``"Strike"`` / ``"Strike+"``).

    Upgrade encoding: append ``"+"`` for each upgrade level, matching the
    sim's ``CardInstance::as_str`` format.
    """
    result = []
    for c in cards:
        sim_name = _names.card(c.name)
        if c.upgraded:
            sim_name += "+"
        result.append(sim_name)
    return result


def from_combat(
    combat: CombatSnapshot,
    player_state: dict | None = None,
    card_piles: CardPiles | None = None,
) -> CombatState:
    """Build a CombatState from a parsed CombatSnapshot.

    Args:
        combat: Parsed combat snapshot (from parse_combat_snapshot()).
        player_state: Optional raw get_player_state() payload — used to
                      reconstruct the draw pile from the full deck when
                      card_piles is not available.
        card_piles: Optional parsed CardPiles — gives the actual
                    draw/discard/exhaust card lists (more accurate).

    Returns:
        A CombatState with all deterministic fields populated. Without
        card_piles the draw pile order is arbitrary; with it the order matches
        the live game.
    """
    p = combat.player

    hand = _card_list(p.hand)
    discard = _card_list(card_piles.discard_pile.cards) if card_piles else []
    exhaust = _card_list(card_piles.exhaust_pile.cards) if card_piles else []

    if card_piles:
        draw = _card_list(card_piles.draw_pile.cards)
    elif player_state:
        ps_players = player_state.get("players", [])
        ps_p = ps_players[0] if ps_players else {}
        deck_raw = ps_p.get("deck", [])
        deck_entries = [
            CardEntry(name=c.get("name", ""), upgraded=bool(c.get("upgraded", False)))
            for c in deck_raw
        ]
        draw_raw_all = _card_list(deck_entries)
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

    p_statuses = _map_statuses(p.powers)

    monsters = []
    for e in combat.enemies:
        if not e.is_alive:
            continue
        sim_name = _names.monster(e.name)
        intent_str = _fmt_intent(e.intent)
        m_statuses = _map_statuses(e.powers)
        monsters.append(
            Monster(
                hp=e.hp,
                max_hp=e.max_hp,
                block=e.block,
                attack=0,
                name=sim_name,
                intent=intent_str,
                statuses=m_statuses,
            )
        )

    return CombatState(
        player_hp=p.hp,
        player_max_hp=p.max_hp,
        player_block=p.block,
        player_energy=p.energy,
        player_statuses=p_statuses,
        monsters=monsters,
        hand=hand,
        draw_pile=draw,
        discard_pile=discard,
        exhaust_pile=exhaust,
        seed=0,
    )


def _fmt_intent(intent: Intent | None) -> str | None:
    """Convert a typed Intent into a sim-compatible intent string."""
    if intent is None:
        return None
    if not intent.intents:
        return intent.move_id or None
    parts = []
    for i in intent.intents:
        if i.type == "Attack":
            parts.append(
                f"Attack({i.damage}×{i.hits})" if i.hits > 1 else f"Attack({i.damage})"
            )
        else:
            parts.append(i.type)
    return " + ".join(parts) if parts else intent.move_id or None


def sim_action_to_bridge(
    action: PlayCardAction | EndTurnAction | SelectTargetAction,
    snapshot: CombatSnapshot,
    avail: AvailableActions,
) -> tuple[str, dict]:
    """Translate a typed sim action to a (method, kwargs) pair for bridge_client.

    Returns one of:
        ("end_turn", {})
        ("play_card", {"card_index": int, "target_index": int})
        ("unknown", {})  — if the card is not found in the live hand
    """
    if isinstance(action, EndTurnAction):
        return "end_turn", {}

    if isinstance(action, PlayCardAction):
        target_card = next(
            (
                c
                for c in snapshot.player.hand
                if _names.card(c.name) == action.card_name
                and c.upgraded == action.upgraded
            ),
            None,
        )
        if target_card is None:
            return "unknown", {}
        bridge_act = next(
            (a for a in avail.actions if a.card_index == target_card.index), None
        )
        return "play_card", {
            "card_index": target_card.index,
            "target_index": bridge_act.target_index if bridge_act else -1,
        }

    return "unknown", {}


def diff(
    predicted: CombatState,
    actual: CombatSnapshot,
    card_piles: CardPiles | None = None,
) -> dict[str, Any]:
    """Compare a sim-predicted CombatState against a parsed combat snapshot.

    Args:
        predicted: Sim CombatState after applying the action.
        actual: Parsed CombatSnapshot (from parse_combat_snapshot()).
        card_piles: Optional parsed CardPiles for pile comparisons.
                    Without it, pile fields are marked skipped.

    Each entry is one of:
        {"match": True, "sim": v, "game": v}
        {"match": False, "sim": v, "game": v}
        {"skipped": True, "reason": str}
    """
    result: dict[str, Any] = {}

    ap = actual.player

    result["player_hp"] = _cmp(predicted.player_hp, ap.hp)
    result["player_block"] = _cmp(predicted.player_block, ap.block)
    result["player_energy"] = _cmp(predicted.player_energy, ap.energy)

    actual_enemies = [e for e in actual.enemies if e.is_alive]
    for i, (sim_m, act_e) in enumerate(zip(predicted.monsters, actual_enemies)):
        prefix = f"enemy[{i}]"
        result[f"{prefix}.hp"] = _cmp(sim_m.hp, act_e.hp)
        result[f"{prefix}.block"] = _cmp(sim_m.block, act_e.block)
        result[f"{prefix}.intent"] = {"skipped": True, "reason": "non-deterministic"}

    result["hand_cards"] = _cmp_pile(predicted.hand, _card_list(ap.hand))

    if card_piles:
        result["discard_pile"] = _cmp_pile(
            predicted.discard_pile, _card_list(card_piles.discard_pile.cards)
        )
        result["draw_pile"] = _cmp_pile(
            predicted.draw_pile, _card_list(card_piles.draw_pile.cards)
        )
        result["exhaust_pile"] = _cmp_pile(
            predicted.exhaust_pile, _card_list(card_piles.exhaust_pile.cards)
        )
    else:
        result["discard_pile"] = {"skipped": True, "reason": "card_piles not provided"}
        result["draw_pile"] = {"skipped": True, "reason": "card_piles not provided"}
        result["exhaust_pile"] = {"skipped": True, "reason": "card_piles not provided"}

    sim_p_statuses = _count_statuses(predicted.player_statuses)
    act_p_statuses = _statuses_from_powers(ap.powers)
    for status in sim_p_statuses.keys() | act_p_statuses.keys():
        result[f"player.{status}"] = _cmp(
            sim_p_statuses.get(status, 0), act_p_statuses.get(status, 0)
        )

    for i, (sim_m, act_e) in enumerate(zip(predicted.monsters, actual_enemies)):
        sim_m_statuses = _count_statuses(sim_m.statuses)
        act_m_statuses = _statuses_from_powers(act_e.powers)
        for status in sim_m_statuses.keys() | act_m_statuses.keys():
            result[f"enemy[{i}].{status}"] = _cmp(
                sim_m_statuses.get(status, 0), act_m_statuses.get(status, 0)
            )

    return result


def _count_statuses(statuses: list[str]) -> dict[str, int]:
    """Convert sim's flat status name list to {name: stacks}.

    Binary statuses (Vulnerable, Weak, Frail) are stored as repeated entries;
    counting them gives stacks. Strength appears once per (Strength, n) variant
    and is best queried via player_strength / Monster.strength instead.
    """
    result: dict[str, int] = {}
    for s in statuses:
        result[s] = result.get(s, 0) + 1
    return result


def _statuses_from_powers(powers: list[Power]) -> dict[str, int]:
    """Convert typed Power list to {sim_status_name: stacks} dict."""
    out: dict[str, int] = {}
    for p in powers:
        mapped = STATUS_MAP.get(p.name)
        if mapped:
            out[mapped] = out.get(mapped, 0) + p.amount
    return out


def _cmp(sim_val: Any, game_val: Any) -> dict[str, Any]:
    match = sim_val == game_val
    return {"match": match, "sim": sim_val, "game": game_val}


def _cmp_pile(sim_pile: list[str], act_pile: list[str]) -> dict[str, Any]:
    """Compare two card piles as sorted multisets (order-independent)."""
    s, g = sorted(sim_pile), sorted(act_pile)
    return {"match": s == g, "sim": s, "game": g}
