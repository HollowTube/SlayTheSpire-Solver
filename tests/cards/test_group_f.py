"""Tests for card parity group F: Havoc, BattleTrance, Whirlwind, Cascade."""

from sts_sim import (
    EndTurnAction,
    Monster,
    PlayCardAction,
    apply,
    legal_actions,
)


# ── Havoc ─────────────────────────────────────────────────────────────────────


def test_havoc_plays_top_card_and_exhausts_it(make_state):
    """Havoc plays the top card of the draw pile and exhausts it."""
    state = make_state(hand=["Havoc"], draw_pile=["Strike", "Defend"])

    resolved = apply(state, PlayCardAction("Havoc"))

    # Strike was on top of the draw pile and should be auto-played + exhausted.
    assert "Strike" in resolved.exhaust_pile
    # Strike's 6 damage should have been dealt.
    assert resolved.monsters[0].hp == 44 - 6
    # Havoc itself goes to discard pile (it's a skill without Exhaust).
    assert "Havoc" in resolved.discard_pile


def test_havoc_does_nothing_when_draw_and_discard_empty(make_state):
    """Havoc does nothing when both draw and discard piles are empty."""
    state = make_state(hand=["Havoc"])

    resolved = apply(state, PlayCardAction("Havoc"))

    # No cards were played, no exhaust.
    assert resolved.exhaust_pile == []
    assert resolved.discard_pile == ["Havoc"]
    assert resolved.monsters[0].hp == 44


def test_havoc_reshuffles_discard_when_draw_empty(make_state):
    """Havoc reshuffles discard into draw when draw pile is empty."""
    state = make_state(hand=["Havoc"], discard_pile=["Strike"])

    resolved = apply(state, PlayCardAction("Havoc"))

    # Strike was in discard, shuffled into draw, then auto-played and exhausted.
    assert "Strike" in resolved.exhaust_pile
    assert resolved.monsters[0].hp == 44 - 6


def test_havoc_costs_1_energy(make_state):
    """Normal Havoc costs 1 energy."""
    state = make_state(hand=["Havoc"], player_energy=1, draw_pile=["Strike"])

    resolved = apply(state, PlayCardAction("Havoc"))

    assert resolved.player_energy == 0


def test_havoc_plus_costs_0_energy(make_state):
    """Upgraded Havoc costs 0 energy."""
    state = make_state(hand=["Havoc+"], draw_pile=["Strike"])

    resolved = apply(state, PlayCardAction("Havoc+"))

    assert resolved.player_energy == 3  # unchanged
    assert "Strike" in resolved.exhaust_pile


def test_havoc_not_playable_without_energy(make_state):
    """Normal Havoc cannot be played with 0 energy."""
    state = make_state(hand=["Havoc"], player_energy=0, draw_pile=["Strike"])

    actions = legal_actions(state)
    assert "PlayCard:Havoc" not in actions


# ── BattleTrance ──────────────────────────────────────────────────────────────


def test_battle_trance_draws_3_and_applies_no_draw(make_state):
    """BattleTrance draws 3 cards and blocks further draws."""
    state = make_state(hand=["BattleTrance"], draw_pile=["Strike", "Defend", "Bash"])

    resolved = apply(state, PlayCardAction("BattleTrance"))

    # Drew 3 cards into hand.
    assert len(resolved.hand) == 3
    assert "Strike" in resolved.hand
    assert "Defend" in resolved.hand
    assert "Bash" in resolved.hand
    # NoDraw status should be applied.
    assert "NoDraw" in resolved.player_statuses


def test_battle_trance_blocks_subsequent_draws(make_state):
    """After BattleTrance, playing a card that draws should not draw."""
    state = make_state(
        hand=["BattleTrance", "Offering"],
        draw_pile=["Strike"] * 10,
    )

    # Play BattleTrance first — it draws 3 and applies NoDraw.
    after_bt = apply(state, PlayCardAction("BattleTrance"))
    # BattleTrance drew 3 Strikes, leaving hand [Offering, Strike, Strike, Strike].
    assert len(after_bt.hand) == 4  # Offering + 3 drawn from BT
    # Now play Offering - should NOT draw additional cards due to NoDraw.
    after_offering = apply(after_bt, PlayCardAction("Offering"))

    # Offering normally draws 3, but NoDraw blocks it.
    # Hand should have the 3 Strikes from BattleTrance, minus Offering.
    assert len(after_offering.hand) == 3


def test_no_draw_removed_at_end_of_turn(make_state):
    """NoDraw status is removed at the end of the player's turn."""
    state = make_state(hand=["BattleTrance"], draw_pile=["Strike"] * 10)

    after_bt = apply(state, PlayCardAction("BattleTrance"))
    assert "NoDraw" in after_bt.player_statuses

    after_turn = apply(after_bt, EndTurnAction())
    assert "NoDraw" not in after_turn.player_statuses


def test_no_draw_does_not_block_next_turn_draw(make_state):
    """The next turn's opening hand draw is not blocked by a previous NoDraw."""
    state = make_state(hand=["BattleTrance"], draw_pile=["Strike"] * 10)

    after_bt = apply(state, PlayCardAction("BattleTrance"))
    # End the turn — NoDraw should be removed, and next-turn 5-card draw occurs.
    after_turn = apply(after_bt, EndTurnAction())

    assert len(after_turn.hand) == 5


def test_battle_trance_plus_draws_4(make_state):
    """Upgraded BattleTrance draws 4 cards."""
    state = make_state(
        hand=["BattleTrance+"],
        draw_pile=["Strike", "Defend", "Bash", "Anger"],
    )

    resolved = apply(state, PlayCardAction("BattleTrance+"))

    assert len(resolved.hand) == 4
    assert "NoDraw" in resolved.player_statuses


# ── Whirlwind ─────────────────────────────────────────────────────────────────


def test_whirlwind_deals_5_damage_per_energy(make_state):
    """Whirlwind deals 5 damage per energy to all enemies."""
    state = make_state(
        hand=["Whirlwind"],
        player_energy=3,
        monsters=[Monster(hp=44, attack=6), Monster(hp=30, attack=4)],
    )

    resolved = apply(state, PlayCardAction("Whirlwind"))

    # 3 energy × 5 damage = 15 damage to each monster.
    assert resolved.monsters[0].hp == 44 - 15
    assert resolved.monsters[1].hp == 30 - 15
    assert resolved.player_energy == 0


def test_whirlwind_with_0_energy_deals_0_damage(make_state):
    """Whirlwind with 0 energy deals 0 damage — but is still legal to play."""
    state = make_state(
        hand=["Whirlwind"],
        player_energy=0,
        monsters=[Monster(hp=44, attack=6)],
    )

    actions = legal_actions(state)
    assert "PlayCard:Whirlwind" in actions

    resolved = apply(state, PlayCardAction("Whirlwind"))

    assert resolved.monsters[0].hp == 44  # no damage
    assert resolved.player_energy == 0


def test_whirlwind_always_legal_regardless_of_energy(make_state):
    """Whirlwind is always a legal action even with 0 energy."""
    state = make_state(hand=["Whirlwind"], player_energy=0)

    assert "PlayCard:Whirlwind" in legal_actions(state)


def test_whirlwind_plus_deals_8_damage_per_energy(make_state):
    """Upgraded Whirlwind deals 8 damage per energy."""
    state = make_state(
        hand=["Whirlwind+"],
        player_energy=2,
        monsters=[Monster(hp=44, attack=6)],
    )

    resolved = apply(state, PlayCardAction("Whirlwind+"))

    # 2 energy × 8 damage = 16 damage.
    assert resolved.monsters[0].hp == 44 - 16
    assert resolved.player_energy == 0


# ── Cascade ───────────────────────────────────────────────────────────────────


def test_cascade_plays_x_cards_from_draw_pile(make_state):
    """Cascade plays X cards from top of draw pile without exhausting them."""
    state = make_state(
        hand=["Cascade"],
        player_energy=3,
        draw_pile=["Strike", "Defend", "Bash", "Anger"],
        monsters=[Monster(hp=44, attack=6)],
    )

    resolved = apply(state, PlayCardAction("Cascade"))

    # 3 energy → 3 cards played from top.
    # Strike: 6 damage; Defend: 5 block; Bash: 8 damage + 2 Vulnerable.
    # Total damage: 6 + 8 = 14.
    assert resolved.monsters[0].hp == 44 - 14
    # Cards should go to discard, not exhaust.
    assert "Strike" in resolved.discard_pile
    assert "Defend" in resolved.discard_pile
    assert "Bash" in resolved.discard_pile
    assert resolved.player_energy == 0


def test_cascade_with_0_energy_plays_0_cards(make_state):
    """Cascade with 0 energy plays 0 cards."""
    state = make_state(
        hand=["Cascade"],
        player_energy=0,
        draw_pile=["Strike"],
        monsters=[Monster(hp=44, attack=6)],
    )

    resolved = apply(state, PlayCardAction("Cascade"))

    assert resolved.monsters[0].hp == 44
    assert "Strike" not in resolved.discard_pile
    assert "Strike" in resolved.draw_pile


def test_cascade_always_legal_regardless_of_energy(make_state):
    """Cascade is always a legal action even with 0 energy."""
    state = make_state(hand=["Cascade"], player_energy=0)

    assert "PlayCard:Cascade" in legal_actions(state)


def test_cascade_reshuffles_discard_when_draw_runs_out(make_state):
    """Cascade reshuffles discard into draw if it runs out mid-count."""
    state = make_state(
        hand=["Cascade"],
        player_energy=3,
        draw_pile=["Strike"],
        discard_pile=["Defend"],
        monsters=[Monster(hp=44, attack=6)],
    )

    resolved = apply(state, PlayCardAction("Cascade"))

    # Should play Strike (6 damage) and then Defend (5 block) after reshuffle.
    assert resolved.monsters[0].hp == 44 - 6
    assert "Strike" in resolved.discard_pile
    assert "Defend" in resolved.discard_pile


def test_cascade_plus_plays_x_plus_1_cards(make_state):
    """Upgraded Cascade plays X+1 cards."""
    state = make_state(
        hand=["Cascade+"],
        player_energy=2,
        draw_pile=["Strike", "Strike", "Strike", "Strike"],
        monsters=[Monster(hp=44, attack=6)],
    )

    resolved = apply(state, PlayCardAction("Cascade+"))

    # 2 energy + 1 bonus = 3 cards played.
    # 3 Strikes × 6 damage = 18 damage.
    assert resolved.monsters[0].hp == 44 - 18


def test_cascade_handles_empty_piles_gracefully(make_state):
    """Cascade stops early if both piles are empty."""
    state = make_state(
        hand=["Cascade"],
        player_energy=5,
        draw_pile=["Strike"],
        monsters=[Monster(hp=44, attack=6)],
    )

    resolved = apply(state, PlayCardAction("Cascade"))

    # Only 1 card in draw pile, so only 1 played even though 5 energy.
    assert resolved.monsters[0].hp == 44 - 6
    # Cascade itself goes to discard.
    assert "Cascade" in resolved.discard_pile
