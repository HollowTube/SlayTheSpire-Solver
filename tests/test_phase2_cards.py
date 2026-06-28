"""Behavioural tests for Phase 2 Ironclad cards: hand/pile manipulation
(exhaust-from-hand, draw, discard-to-draw-pile, hand-size-scaled damage, and
adding generated cards to hand)."""

from conftest import make_state
from sts_sim import CombatState, Monster, PlayCardAction, SelectTargetAction, apply


# ── Cinder ───────────────────────────────────────────────────────────────────


def test_cinder_deals_18_damage_and_exhausts_a_random_hand_card():
    # Per the decompiled source, Cinder costs 2, deals 18 damage to a chosen
    # enemy, then exhausts a random card from hand.
    state = make_state(hand=["Cinder", "Defend"])

    awaiting_target = apply(state, PlayCardAction("Cinder"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 18
    # Cinder itself goes to discard (not Exhaust); "Defend" is the only other
    # card in hand, so it must be the one exhausted.
    assert resolved.hand == []
    assert "Defend" in resolved.exhaust_pile
    assert "Cinder" in resolved.discard_pile


# ── TrueGrit ─────────────────────────────────────────────────────────────────


def test_true_grit_gains_7_block_and_exhausts_a_random_hand_card():
    # Per the decompiled source, base (non-upgraded) TrueGrit costs 1, gains
    # 7 block, and exhausts a random card from hand.
    state = make_state(hand=["TrueGrit", "Defend"])

    resolved = apply(state, PlayCardAction("TrueGrit"))

    assert resolved.player_block == state.player_block + 7
    assert resolved.hand == []
    assert "Defend" in resolved.exhaust_pile
    assert "TrueGrit" in resolved.discard_pile


# ── BurningPact ──────────────────────────────────────────────────────────────


def test_burning_pact_exhausts_a_random_hand_card_and_draws_2():
    # Per the decompiled source, BurningPact costs 1; the player chooses 1
    # card from hand to exhaust (modeled as random) and draws 2 cards.
    state = make_state(
        hand=["BurningPact", "Defend"],
        draw_pile=["Strike", "Strike"],
    )

    resolved = apply(state, PlayCardAction("BurningPact"))

    assert "Defend" in resolved.exhaust_pile
    assert "BurningPact" in resolved.discard_pile
    assert resolved.hand.count("Strike") == 2


# ── Thrash ───────────────────────────────────────────────────────────────────


def test_thrash_deals_8_damage_and_exhausts_a_random_attack_from_hand():
    # Per the decompiled source, Thrash costs 1, deals 4 damage twice (8
    # total) to a chosen enemy, then exhausts a random Attack card from hand.
    # We skip the "absorb that card's damage" mechanic (per-card-instance
    # state, not modeled) and document it as a simplification.
    state = make_state(hand=["Thrash", "Strike", "Defend"])

    awaiting_target = apply(state, PlayCardAction("Thrash"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 8
    # Only "Strike" (an Attack) is eligible to be exhausted; "Defend" (a
    # Skill) must remain in hand.
    assert "Strike" in resolved.exhaust_pile
    assert "Defend" in resolved.hand
    assert "Thrash" in resolved.discard_pile


# ── SecondWind ───────────────────────────────────────────────────────────────


def test_second_wind_exhausts_non_attacks_and_gains_5_block_each():
    # Per the decompiled source, SecondWind costs 1; for each non-Attack card
    # in hand, exhaust it and gain 5 block (total = 5 * count). Attack cards
    # remain in hand.
    state = make_state(hand=["SecondWind", "Defend", "Rage", "Strike"])

    resolved = apply(state, PlayCardAction("SecondWind"))

    assert resolved.player_block == state.player_block + 10
    assert "Defend" in resolved.exhaust_pile
    assert "Rage" in resolved.exhaust_pile
    assert "Strike" in resolved.hand
    assert "SecondWind" in resolved.discard_pile


# ── Headbutt ─────────────────────────────────────────────────────────────────


def test_headbutt_deals_9_damage_and_returns_a_discarded_card_to_top_of_draw():
    # Per the decompiled source, Headbutt costs 1, deals 9 damage to a chosen
    # enemy, then the player picks a card from the discard pile to put on top
    # of the draw pile (modeled as random — see TrueGrit/BurningPact). By the
    # time this resolves, Headbutt itself is already in the discard pile (it
    # moved there when played, like every non-exhausting card), so — matching
    # real Slay the Spire — it's a valid candidate to retrieve.
    state = make_state(
        hand=["Headbutt"],
        draw_pile=["Strike"],
        discard_pile=["Iron Wave"],
    )

    awaiting_target = apply(state, PlayCardAction("Headbutt"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 9
    # One card moved from discard pile to the top of draw pile; the other
    # stays in discard.
    assert len(resolved.discard_pile) == 1
    # "draw_cards" pops from the end of `draw_pile`, so the top card is the
    # last element.
    assert sorted(resolved.discard_pile + [resolved.draw_pile[-1]]) == [
        "Headbutt",
        "Iron Wave",
    ]
    assert resolved.draw_pile[0] == "Strike"


# ── FiendFire ────────────────────────────────────────────────────────────────


def test_fiend_fire_deals_7_per_card_in_hand_and_exhausts_hand_then_itself():
    # Per the decompiled source, FiendFire costs 2, exhausts (itself), deals
    # 7 damage to a chosen enemy once per card remaining in hand (counted
    # BEFORE FiendFire exhausts the rest of the hand), and exhausts every
    # other card in hand.
    state = make_state(hand=["FiendFire", "Strike", "Defend"])

    awaiting_target = apply(state, PlayCardAction("FiendFire"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    # 2 cards remaining in hand (Strike, Defend) -> 2 * 7 = 14 damage.
    assert state.monsters[0].hp - resolved.monsters[0].hp == 14
    assert resolved.hand == []
    assert "Strike" in resolved.exhaust_pile
    assert "Defend" in resolved.exhaust_pile
    assert "FiendFire" in resolved.exhaust_pile


# ── InfernalBlade ────────────────────────────────────────────────────────────


def test_infernal_blade_adds_a_random_attack_to_hand_and_exhausts():
    # Per the decompiled source, InfernalBlade costs 1 and Exhausts. It adds a
    # random Attack card from the Ironclad's card pool to hand. We model the
    # pool as a hardcoded list of currently-implemented Attack cards (a
    # documented simplification — the real pool is the player's full unlocked
    # card pool, and the generated card is also "free this turn", which we
    # don't model).
    state = make_state(hand=["InfernalBlade"])

    resolved = apply(state, PlayCardAction("InfernalBlade"))

    assert len(resolved.hand) == 1
    assert "InfernalBlade" in resolved.exhaust_pile
