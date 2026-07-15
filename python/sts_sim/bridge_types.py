"""Typed dataclasses for bridge API response shapes.

Parse functions convert raw bridge JSON dicts into typed objects so callers
avoid stringly-typed dict access. bridge.py's from_combat() and diff() still
accept typed objects at the CombatSnapshot / CardPiles boundary.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


# ---------------------------------------------------------------------------
# Shared building blocks
# ---------------------------------------------------------------------------


@dataclass
class Power:
    """A status/power entry on a player or monster."""

    name: str  # bridge class name (e.g. "VulnerablePower")
    amount: int = 1
    type: str = ""  # "Buff", "Debuff", "Special"


@dataclass
class IntentDetail:
    """One intent component within a monster move."""

    type: str  # e.g. "Attack", "Defend", "Buff"
    damage: int = 0
    hits: int = 1
    total_damage: int = 0


@dataclass
class Intent:
    """A monster's next move, decomposed into individual intents."""

    move_id: str = ""
    intents: list[IntentDetail] = field(default_factory=list)


# ---------------------------------------------------------------------------
# get_combat_state()
# ---------------------------------------------------------------------------


@dataclass
class HandCard:
    index: int
    name: str
    upgraded: bool = False
    cost: int = 0
    type: str = ""  # "Attack", "Skill", "Power", "Status"
    can_play: bool = True
    unplayable_reason: str = ""
    target_type: str = ""  # "AnyEnemy", "None", "AnyAlly", …
    valid_targets: list[int] | None = None


@dataclass
class PlayerCombatState:
    hp: int
    max_hp: int
    block: int
    energy: int
    hand: list[HandCard] = field(default_factory=list)
    powers: list[Power] = field(default_factory=list)
    character: str = ""
    max_energy: int = 3
    draw_pile_count: int = 0
    discard_pile_count: int = 0
    exhaust_pile_count: int = 0


@dataclass
class EnemyState:
    hp: int
    max_hp: int
    block: int
    name: str
    is_alive: bool = True
    intent: Intent | None = None
    powers: list[Power] = field(default_factory=list)
    entity_id: str = ""
    index: int = 0


@dataclass
class CombatSnapshot:
    player: PlayerCombatState
    enemies: list[EnemyState] = field(default_factory=list)
    in_combat: bool = True
    screen: str = "COMBAT_PLAYER_TURN"
    round: int = 0
    is_player_turn: bool = True


# ---------------------------------------------------------------------------
# get_card_piles()
# ---------------------------------------------------------------------------


@dataclass
class CardEntry:
    name: str
    upgraded: bool = False
    index: int = 0
    type: str = ""  # "Attack", "Skill", "Power", "Status"
    energy_cost: int = 0


@dataclass
class CardPile:
    cards: list[CardEntry] = field(default_factory=list)


@dataclass
class CardPiles:
    draw_pile: CardPile = field(default_factory=CardPile)
    discard_pile: CardPile = field(default_factory=CardPile)
    exhaust_pile: CardPile = field(default_factory=CardPile)
    hand: CardPile = field(default_factory=CardPile)


# ---------------------------------------------------------------------------
# get_available_actions()
# ---------------------------------------------------------------------------


@dataclass
class BridgeAction:
    action: str = ""  # "play_card", "end_turn", "event_option", "console", …
    card_index: int = -1
    card_name: str = ""
    target_index: int = -1
    target_name: str = ""
    label: str = ""  # for event_option, shop actions, etc.
    description: str = ""  # for console and descriptive actions


@dataclass
class AvailableActions:
    screen: str = "UNKNOWN"
    screen_source: str = ""
    room_type: str = ""
    screen_context_type: str = ""
    actions: list[BridgeAction] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Parse functions
# ---------------------------------------------------------------------------


def _parse_power(raw: dict) -> Power:
    name = raw.get("name") or raw.get("id") or raw.get("power_id", "")
    amount = int(raw.get("stacks", raw.get("amount", 1)))
    return Power(name=name, amount=amount, type=raw.get("type", ""))


def _parse_intent_detail(raw: dict) -> IntentDetail:
    return IntentDetail(
        type=raw.get("type", ""),
        damage=raw.get("damage", 0),
        hits=raw.get("hits", 1),
        total_damage=raw.get("total_damage", 0),
    )


def _parse_intent(raw: Any) -> Intent | None:
    if raw is None:
        return None
    if isinstance(raw, str):
        return Intent(move_id=raw)
    if not isinstance(raw, dict):
        return None
    return Intent(
        move_id=raw.get("move_id", ""),
        intents=[
            _parse_intent_detail(i)
            for i in raw.get("intents", [])
            if isinstance(i, dict)
        ],
    )


def _parse_hand_card(raw: dict) -> HandCard:
    return HandCard(
        index=raw.get("index", 0),
        name=raw.get("name", ""),
        upgraded=bool(raw.get("upgraded", False)),
        cost=raw.get("energy_cost", raw.get("cost", 0)),
        type=raw.get("type", ""),
        can_play=bool(raw.get("can_play", True)),
        unplayable_reason=raw.get("unplayable_reason") or "",
        target_type=raw.get("target_type", ""),
        valid_targets=raw.get("valid_targets"),
    )


def parse_combat_snapshot(raw: dict) -> CombatSnapshot:
    """Parse a raw get_combat_state() payload into a CombatSnapshot."""
    players = raw.get("players", [])
    p = players[0] if players else {}

    # draw_pile / discard_pile / exhaust_pile are counts (int) in combat state,
    # not card lists — full lists come from get_card_piles().
    def _pile_count(val: Any) -> int:
        return val if isinstance(val, int) else 0

    player = PlayerCombatState(
        hp=p.get("hp", 0),
        max_hp=p.get("max_hp", p.get("hp", 0)),
        block=p.get("block", 0),
        energy=p.get("energy", 3),
        max_energy=p.get("max_energy", 3),
        character=p.get("character") or "",
        hand=[_parse_hand_card(c) for c in p.get("hand", []) if isinstance(c, dict)],
        powers=[_parse_power(pw) for pw in p.get("powers", []) if isinstance(pw, dict)],
        draw_pile_count=_pile_count(p.get("draw_pile")),
        discard_pile_count=_pile_count(p.get("discard_pile")),
        exhaust_pile_count=_pile_count(p.get("exhaust_pile")),
    )

    enemies = [
        EnemyState(
            hp=e.get("hp", 0),
            max_hp=e.get("max_hp", e.get("hp", 0)),
            block=e.get("block", 0),
            name=e.get("name", ""),
            is_alive=bool(e.get("is_alive", True)),
            intent=_parse_intent(e.get("intent")),
            powers=[
                _parse_power(pw) for pw in e.get("powers", []) if isinstance(pw, dict)
            ],
            entity_id=e.get("entity_id", ""),
            index=e.get("index", 0),
        )
        for e in raw.get("enemies", [])
        if isinstance(e, dict)
    ]

    return CombatSnapshot(
        player=player,
        enemies=enemies,
        in_combat=bool(raw.get("in_combat", True)),
        screen=raw.get("screen", "COMBAT_PLAYER_TURN"),
        round=raw.get("round", 0),
        is_player_turn=bool(raw.get("is_player_turn", True)),
    )


def _parse_card_entry(raw: Any) -> CardEntry:
    if isinstance(raw, str):
        # Plain STS2 ID string (legacy format)
        return CardEntry(name=raw)
    return CardEntry(
        name=raw.get("name", ""),
        upgraded=bool(raw.get("upgraded", False)),
        index=raw.get("index", 0),
        type=raw.get("type", ""),
        energy_cost=raw.get("energy_cost", 0),
    )


def _parse_card_pile(raw: Any) -> CardPile:
    if isinstance(raw, list):
        return CardPile(cards=[_parse_card_entry(c) for c in raw])
    if isinstance(raw, dict):
        return CardPile(cards=[_parse_card_entry(c) for c in raw.get("cards", [])])
    return CardPile()


def parse_card_piles(raw: dict) -> CardPiles:
    """Parse a raw get_card_piles() payload into a CardPiles."""
    return CardPiles(
        draw_pile=_parse_card_pile(raw.get("draw_pile", {})),
        discard_pile=_parse_card_pile(raw.get("discard_pile", {})),
        exhaust_pile=_parse_card_pile(raw.get("exhaust_pile", {})),
        hand=_parse_card_pile(raw.get("hand", {})),
    )


def _parse_bridge_action(raw: dict) -> BridgeAction:
    return BridgeAction(
        action=raw.get("action", ""),
        card_index=raw.get("card_index", -1),
        card_name=raw.get("card_name", ""),
        target_index=raw.get("target_index", -1),
        target_name=raw.get("target_name") or "",
        label=raw.get("label") or "",
        description=raw.get("description") or "",
    )


def parse_available_actions(raw: dict) -> AvailableActions:
    """Parse a raw get_available_actions() payload into AvailableActions."""
    return AvailableActions(
        screen=raw.get("screen", "UNKNOWN"),
        screen_source=raw.get("screen_source", ""),
        room_type=raw.get("room_type") or "",
        screen_context_type=raw.get("screen_context_type") or "",
        actions=[
            _parse_bridge_action(a)
            for a in raw.get("actions", [])
            if isinstance(a, dict)
        ],
    )
