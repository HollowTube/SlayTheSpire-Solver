"""Behavioural tests for Phase 4 Wave 2 Ironclad cards: GameEvent::CardExhausted
and the persistent powers that react to it, plus Barricade's surgical
block-clear change."""

from sts_sim import CombatState, EndTurnAction, Monster, PlayCardAction, apply


def make_state(hand=("Strike",), seed=42, player_energy=3, **kwargs):
    return CombatState(
        player_hp=80,
        player_energy=player_energy,
        monsters=[Monster(hp=44, attack=0)],
        seed=seed,
        hand=list(hand),
        **kwargs,
    )


# ── DarkEmbrace ──────────────────────────────────────────────────────────────


def test_dark_embrace_draws_a_card_when_a_card_is_exhausted():
    # Impervious has the Exhaust keyword. Playing it while
    # Dark Embrace is active should draw 1 extra card -> hand size unchanged
    # (-1 for playing Impervious, +1 from Dark Embrace's draw).
    state = make_state(
        hand=["DarkEmbrace", "Impervious"],
        player_energy=4,
        draw_pile=["Strike"] * 5,
    )

    after_dark_embrace = apply(state, PlayCardAction("DarkEmbrace"))
    hand_size_before = len(after_dark_embrace.hand)

    after_impervious = apply(after_dark_embrace, PlayCardAction("Impervious"))

    assert "Impervious" in after_impervious.exhaust_pile
    assert len(after_impervious.hand) == hand_size_before


# ── FeelNoPain ───────────────────────────────────────────────────────────────


def test_feel_no_pain_gains_3_block_when_a_card_is_exhausted():
    state = make_state(
        hand=["FeelNoPain", "Impervious"],
        player_energy=4,
        draw_pile=["Strike"] * 5,
    )

    after_feel_no_pain = apply(state, PlayCardAction("FeelNoPain"))

    after_impervious = apply(after_feel_no_pain, PlayCardAction("Impervious"))

    # Impervious itself grants 30 Block, plus 3 from Feel No Pain.
    assert "Impervious" in after_impervious.exhaust_pile
    assert after_impervious.player_block == 33


# ── Barricade ────────────────────────────────────────────────────────────────


def test_barricade_keeps_block_through_turn_start():
    state = make_state(hand=["Barricade", "ShrugItOff"], player_energy=4)

    after_barricade = apply(state, PlayCardAction("Barricade"))
    after_block = apply(after_barricade, PlayCardAction("ShrugItOff"))
    assert after_block.player_block == 8

    after_turn = apply(after_block, EndTurnAction())
    assert after_turn.player_block == 8


def test_without_barricade_block_clears_at_turn_start():
    state = make_state(hand=["ShrugItOff"])

    after_block = apply(state, PlayCardAction("ShrugItOff"))
    assert after_block.player_block == 8

    after_turn = apply(after_block, EndTurnAction())
    assert after_turn.player_block == 0
