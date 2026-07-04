"""Tests for exhaust-pile aware cards: PactsEnd and HowlFromBeyond."""

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    apply,
    legal_actions,
)


# ── PactsEnd ──────────────────────────────────────────────────────────────────


def test_pacts_end_not_legal_when_exhaust_pile_has_less_than_3(make_state):
    """PactsEnd is suppressed from legal_actions when < 3 cards in exhaust pile."""
    state = make_state(
        hand=["PactsEnd"],
        exhaust_pile=["Strike", "Defend"],
    )
    actions = legal_actions(state)
    assert "PlayCard:PactsEnd" not in actions


def test_pacts_end_is_legal_when_exhaust_pile_has_3(make_state):
    """PactsEnd appears in legal_actions when exactly 3 cards in exhaust pile."""
    state = make_state(
        hand=["PactsEnd"],
        exhaust_pile=["Strike", "Defend", "Bash"],
    )
    actions = legal_actions(state)
    assert "PlayCard:PactsEnd" in actions


def test_pacts_end_is_legal_when_exhaust_pile_has_more_than_3(make_state):
    """PactsEnd appears in legal_actions when > 3 cards in exhaust pile."""
    state = make_state(
        hand=["PactsEnd"],
        exhaust_pile=["Strike", "Defend", "Bash", "Tremble"],
    )
    actions = legal_actions(state)
    assert "PlayCard:PactsEnd" in actions


def test_pacts_end_deals_17_aoe(make_state):
    """PactsEnd deals 17 damage to all enemies."""
    state = make_state(
        monsters=[Monster(hp=44, attack=6), Monster(hp=30, attack=4)],
        hand=["PactsEnd"],
        exhaust_pile=["Strike", "Defend", "Bash"],
    )
    # Non-targeted AoE; resolves immediately on play.
    resolved = apply(state, PlayCardAction("PactsEnd"))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 17
    assert state.monsters[1].hp - resolved.monsters[1].hp == 17


# ── HowlFromBeyond ────────────────────────────────────────────────────────────


def test_howl_from_beyond_deals_16_aoe_and_exhausts(make_state):
    """HowlFromBeyond deals 16 AoE damage and appears in exhaust_pile after play."""
    state = make_state(
        monsters=[Monster(hp=44, attack=6), Monster(hp=30, attack=4)],
        hand=["HowlFromBeyond"],
    )
    resolved = apply(state, PlayCardAction("HowlFromBeyond"))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 16
    assert state.monsters[1].hp - resolved.monsters[1].hp == 16
    assert "HowlFromBeyond" in resolved.exhaust_pile


def test_howl_from_beyond_autoplays_from_exhaust_at_turn_start():
    """HowlFromBeyond auto-plays from exhaust pile at start of next turn."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["HowlFromBeyond"],
    )
    # Play HowlFromBeyond. It should deal 16 damage and exhaust.
    played = apply(state, PlayCardAction("HowlFromBeyond"))
    assert played.monsters[0].hp == 44 - 16  # 28
    assert "HowlFromBeyond" in played.exhaust_pile

    # End turn. On the next turn start, HowlFromBeyond auto-plays from
    # exhaust, dealing another 16 before the hand is drawn.
    after_turn = apply(played, EndTurnAction())
    assert after_turn.monsters[0].hp == 28 - 16  # 12
