"""Behavioural tests for HOL-50: card piles as `Vec<CardInstance>` rather than
`Vec<String>`. Upgraded cards round-trip as `"Name+"` through the
constructor, `legal_actions`, `apply`, and every pile getter, while
`card_data` lookups (cost/effects) still key off the base name."""

from sts_sim import (
    CombatState,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
    legal_actions,
)


def make_state(hand, seed=42, player_energy=3, **kwargs):
    return CombatState(
        player_hp=80,
        player_energy=player_energy,
        monsters=[Monster(hp=44, attack=0)],
        seed=seed,
        hand=list(hand),
        **kwargs,
    )


def test_upgraded_card_round_trips_through_hand_and_legal_actions():
    state = make_state(hand=["Strike+", "Defend"])

    assert state.hand == ["Strike+", "Defend"]
    assert "PlayCard:Strike+" in legal_actions(state)


def test_playing_an_upgraded_card_uses_base_card_data_and_lands_in_discard():
    state = make_state(hand=["Strike+"])

    after_play = apply(state, PlayCardAction("Strike+"))
    after = apply(after_play, SelectTargetAction(0))

    # Strike+ deals 9 damage (HOL-53's UpgradeDelta mechanism applies the
    # upgrade); the `+` doesn't break the card_data lookup.
    assert after.monsters[0].hp == 44 - 9
    assert after.discard_pile == ["Strike+"]
    assert after.hand == []


def test_playing_one_copy_leaves_the_other_copys_upgrade_level_intact():
    state = make_state(hand=["Strike", "Strike+"], player_energy=4)

    after = apply(state, PlayCardAction("Strike"))

    assert after.hand == ["Strike+"]
    assert after.discard_pile == ["Strike"]


def test_upgraded_targeted_card_goes_through_select_target():
    state = make_state(hand=["Bash+"])

    after_play = apply(state, PlayCardAction("Bash+"))
    assert after_play.pending == "SelectTarget"
    assert legal_actions(after_play) == ["SelectTarget:Monster:0"]

    after_target = apply(after_play, SelectTargetAction(0))
    assert after_target.discard_pile == ["Bash+"]
    assert after_target.monsters[0].hp < 44
