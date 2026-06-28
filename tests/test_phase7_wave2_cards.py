from conftest import make_state
from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
    legal_actions,
)


def test_setup_strike_deals_7_and_grants_2_strength_this_turn_only():
    state = make_state(hand=["Setup Strike", "Strike"])
    after_setup = apply(
        apply(state, PlayCardAction("Setup Strike")), SelectTargetAction(0)
    )
    assert after_setup.monsters[0].hp == state.monsters[0].hp - 7
    assert "StrengthThisTurn" in after_setup.player_statuses

    after_strike = apply(
        apply(after_setup, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    # Strike (6) + 2 from StrengthThisTurn = 8.
    assert after_strike.monsters[0].hp == after_setup.monsters[0].hp - 8

    after_turn = apply(after_strike, EndTurnAction())
    assert "StrengthThisTurn" not in after_turn.player_statuses


def test_unrelenting_deals_12_and_makes_next_attack_free():
    state = make_state(hand=["Unrelenting", "Strike"], player_energy=1)
    after_unrelenting = apply(
        apply(state, PlayCardAction("Unrelenting")), SelectTargetAction(0)
    )
    assert after_unrelenting.monsters[0].hp == state.monsters[0].hp - 12
    assert "FreeAttack" in after_unrelenting.player_statuses
    assert after_unrelenting.player_energy == 0

    # Strike normally costs 1, but FreeAttack should make it cost 0 and be
    # consumed in the process.
    assert "PlayCard:Strike" in legal_actions(after_unrelenting)
    after_strike = apply(
        apply(after_unrelenting, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    assert after_strike.player_energy == 0
    assert "FreeAttack" not in after_strike.player_statuses


def test_evil_eye_grants_8_block_or_16_if_exhausted_a_card_this_turn():
    state = make_state(hand=["Evil Eye"])
    after_no_exhaust = apply(state, PlayCardAction("Evil Eye"))
    assert after_no_exhaust.player_block == 8

    state_with_exhaust = make_state(hand=["Evil Eye", "MoltenFist"])
    after_exhaust = apply(
        apply(
            apply(state_with_exhaust, PlayCardAction("MoltenFist")),
            SelectTargetAction(0),
        ),
        PlayCardAction("Evil Eye"),
    )
    assert after_exhaust.player_block == 16


def test_forgotten_ritual_grants_3_energy_and_exhausts():
    state = make_state(hand=["Forgotten Ritual"], player_energy=1)
    resolved = apply(state, PlayCardAction("Forgotten Ritual"))
    assert resolved.player_energy == 1 + 3
    assert "Forgotten Ritual" in resolved.exhaust_pile
    assert "Forgotten Ritual" not in resolved.hand
