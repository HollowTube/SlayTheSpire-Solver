"""Typed dataclasses for bridge API response shapes.

Parse functions convert raw bridge JSON dicts into typed objects so callers
avoid stringly-typed dict access. bridge.py's from_combat() and diff() still
accept raw dicts — they are the canonical bridge→sim translation boundary.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class HandCard:
    index: int
    name: str
    upgraded: bool = False
    cost: int = 0


@dataclass
class Power:
    """A status/power entry on a player or monster."""

    name: str  # bridge class name (e.g. "VulnerablePower", "Vulnerable")
    amount: int = 1  # stacks (uses stacks → amount → 1 fallback chain)


@dataclass
class EnemyState:
    hp: int
    max_hp: int
    block: int
    name: str
    is_alive: bool = True
    intent: Any = None
    powers: list[Power] = field(default_factory=list)


@dataclass
class PlayerCombatState:
    hp: int
    max_hp: int
    block: int
    energy: int
    hand: list[HandCard] = field(default_factory=list)
    powers: list[Power] = field(default_factory=list)


@dataclass
class CombatSnapshot:
    player: PlayerCombatState
    enemies: list[EnemyState] = field(default_factory=list)


@dataclass
class BridgeAction:
    card_index: int
    card_name: str
    target_index: int = -1


@dataclass
class AvailableActions:
    screen: str = "UNKNOWN"
    actions: list[BridgeAction] = field(default_factory=list)


@dataclass
class CardEntry:
    name: str
    upgraded: bool = False


@dataclass
class CardPile:
    cards: list[CardEntry] = field(default_factory=list)


@dataclass
class CardPiles:
    draw_pile: CardPile = field(default_factory=CardPile)
    discard_pile: CardPile = field(default_factory=CardPile)
    exhaust_pile: CardPile = field(default_factory=CardPile)


# ---------------------------------------------------------------------------
# Parse functions
# ---------------------------------------------------------------------------


def _parse_power(raw: dict) -> Power:
    # Bridge varies the key: name, id, or power_id
    name = raw.get("name") or raw.get("id") or raw.get("power_id", "")
    amount = int(raw.get("stacks", raw.get("amount", 1)))
    return Power(name=name, amount=amount)


def _parse_hand_card(raw: dict) -> HandCard:
    return HandCard(
        index=raw.get("index", 0),
        name=raw.get("name", ""),
        upgraded=bool(raw.get("upgraded", False)),
        cost=raw.get("cost", 0),
    )


def parse_combat_snapshot(raw: dict) -> CombatSnapshot:
    """Parse a raw get_combat_state() payload into a CombatSnapshot."""
    players = raw.get("players", [])
    p = players[0] if players else {}

    player = PlayerCombatState(
        hp=p.get("hp", 0),
        max_hp=p.get("max_hp", p.get("hp", 0)),
        block=p.get("block", 0),
        energy=p.get("energy", 3),
        hand=[_parse_hand_card(c) for c in p.get("hand", [])],
        powers=[_parse_power(pw) for pw in p.get("powers", [])],
    )

    enemies = [
        EnemyState(
            hp=e.get("hp", 0),
            max_hp=e.get("max_hp", e.get("hp", 0)),
            block=e.get("block", 0),
            name=e.get("name", ""),
            is_alive=bool(e.get("is_alive", True)),
            intent=e.get("intent"),
            powers=[_parse_power(pw) for pw in e.get("powers", [])],
        )
        for e in raw.get("enemies", [])
    ]

    return CombatSnapshot(player=player, enemies=enemies)


def _parse_card_entry(raw: dict) -> CardEntry:
    return CardEntry(
        name=raw.get("name", ""),
        upgraded=bool(raw.get("upgraded", False)),
    )


def _parse_card_pile(raw: dict) -> CardPile:
    return CardPile(cards=[_parse_card_entry(c) for c in raw.get("cards", [])])


def parse_card_piles(raw: dict) -> CardPiles:
    """Parse a raw get_card_piles() payload into a CardPiles."""
    return CardPiles(
        draw_pile=_parse_card_pile(raw.get("draw_pile", {})),
        discard_pile=_parse_card_pile(raw.get("discard_pile", {})),
        exhaust_pile=_parse_card_pile(raw.get("exhaust_pile", {})),
    )


def _parse_bridge_action(raw: dict) -> BridgeAction:
    return BridgeAction(
        card_index=raw.get("card_index", 0),
        card_name=raw.get("card_name", ""),
        target_index=raw.get("target_index", -1),
    )


def parse_available_actions(raw: dict) -> AvailableActions:
    """Parse a raw get_available_actions() payload into AvailableActions."""
    return AvailableActions(
        screen=raw.get("screen", "UNKNOWN"),
        actions=[_parse_bridge_action(a) for a in raw.get("actions", [])],
    )
