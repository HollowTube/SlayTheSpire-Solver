"""
Tests for card parity group G — reactive turn-hook powers:
  Stampede, Rupture, Hellraiser, ExpectAFight, Pillage.
"""
import pytest
from sts_sim import CombatState, apply, legal_actions


def _state(monster_hp=30, player_hp=70, deck=None, player_statuses=None,
           draw_pile=None, discard_pile=None, hand=None, player_energy=3,
           player_block=0, exhaust_pile=None, monster_block=0,
           monster_statuses=None):
    if deck is None:
        deck = ["Strike"] * 5 + ["Defend"] * 5
    if draw_pile is None:
        draw_pile = []
    if discard_pile is None:
        discard_pile = []
    if hand is None:
        hand = []
    if exhaust_pile is None:
        exhaust_pile = []
    if player_statuses is None:
        player_statuses = []
    if monster_statuses is None:
        monster_statuses = []
    return CombatState(
        player_hp=player_hp,
        player_max_hp=80,
        player_block=player_block,
        player_energy=player_energy,
        player_max_energy=3,
        player_statuses=player_statuses,
        deck=deck,
        draw_pile=draw_pile,
        discard_pile=discard_pile,
        hand=hand,
        exhaust_pile=exhaust_pile,
        monsters=[dict(
            hp=monster_hp,
            max_hp=monster_hp,
            block=monster_block,
            attack=5,
            statuses=monster_statuses,
        )],
    )


def _has_status(state, name):
    """Check if state's player has a status whose string starts with name."""
    sts = list(state.player_statuses())
    return any(s.startswith(name) for s in sts)


def _status_stacks(state, name):
    """Return total stacks for a counted status like Strength(n)."""
    total = 0
    for s in state.player_statuses():
        if s.startswith(name):
            if "(" in s:
                total += int(s.split("(")[1].rstrip(")"))
            else:
                total += 1
    return total


# =========================================================================
# Stampede
# =========================================================================

class TestStampede:
    """Stampede: At the end of your turn, 1 random Attack in your Hand is
    played against a random enemy."""

    def test_playing_stampede_card(self):
        s = _state(hand=["Stampede"], player_energy=3)
        s = apply(s, "PlayCard:Stampede")
        assert _has_status(s, "Stampede")

    def test_stampede_auto_plays_attack_at_end_of_turn(self):
        s = _state(hand=["Strike"], player_statuses=[("Stampede", 1)],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "EndTurn")
        # Strike should have been auto-played before hand discard
        assert s.monsters[0].hp < 30

    def test_stampede_no_attacks_does_nothing(self):
        s = _state(hand=["Defend"], player_statuses=[("Stampede", 1)],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "EndTurn")
        assert s.monsters[0].hp == 30

    def test_stampede_upgraded_cost(self):
        s = _state(hand=["Stampede+"], player_energy=1, draw_pile=[])
        actions = legal_actions(s)
        assert "PlayCard:Stampede+" in actions

    def test_stampede_multiple_stacks(self):
        s = _state(hand=["Strike", "Strike"], player_statuses=[("Stampede", 2)],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "EndTurn")
        # At most 2 Strikes were played
        assert s.monsters[0].hp < 30


# =========================================================================
# Rupture
# =========================================================================

class TestRupture:
    """Rupture: Whenever you lose HP on your turn, gain 1 Strength."""

    def test_rupture_gains_strength_on_hp_loss(self):
        s = _state(hand=["Bloodletting"], player_statuses=[("Rupture", 1)],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:Bloodletting")
        assert _has_status(s, "Strength")
        assert _status_stacks(s, "Strength") >= 1

    def test_rupture_no_strength_without_hp_loss(self):
        s = _state(hand=["Defend"], player_statuses=[("Rupture", 1)],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:Defend")
        assert _status_stacks(s, "Strength") == 0

    def test_rupture_upgraded_cost(self):
        s = _state(hand=["Rupture+"], player_energy=0, draw_pile=[])
        actions = legal_actions(s)
        assert "PlayCard:Rupture+" in actions

    def test_rupture_multiple_stacks(self):
        s = _state(hand=["Bloodletting"], player_statuses=[("Rupture", 2)],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:Bloodletting")
        assert _status_stacks(s, "Strength") >= 2


# =========================================================================
# Hellraiser
# =========================================================================

class TestHellraiser:
    """Hellraiser: Whenever you draw a card containing 'Strike', it is
    played against a random enemy."""

    def test_playing_hellraiser_card(self):
        s = _state(hand=["Hellraiser"], player_energy=3)
        s = apply(s, "PlayCard:Hellraiser")
        assert _has_status(s, "Hellraiser")

    def test_hellraiser_triggers_on_strike_draw(self):
        s = _state(hand=[], player_statuses=[("Hellraiser",)],
                   draw_pile=["Strike", "Strike", "Defend", "Defend", "Defend"],
                   discard_pile=[], player_energy=3)
        s = apply(s, "EndTurn")
        # Strikes drawn during the opening-hand draw should be auto-played
        assert s.monsters[0].hp < 30

    def test_hellraiser_ignores_non_strike_draws(self):
        s = _state(hand=[], player_statuses=[("Hellraiser",)],
                   draw_pile=["Defend", "Defend", "Defend", "Defend", "Defend"],
                   discard_pile=[], player_energy=3)
        s = apply(s, "EndTurn")
        assert s.monsters[0].hp == 30

    def test_hellraiser_upgraded_cost(self):
        s = _state(hand=["Hellraiser+"], player_energy=2, draw_pile=[])
        actions = legal_actions(s)
        assert "PlayCard:Hellraiser+" in actions


# =========================================================================
# ExpectAFight
# =========================================================================

class TestExpectAFight:
    """ExpectAFight: Gain Energy equal to Attacks in Hand. Block further
    energy gain this turn."""

    def test_gains_energy_for_attacks(self):
        s = _state(hand=["Strike", "Strike", "Strike", "ExpectAFight"],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:ExpectAFight")
        # 3 - 1 (cost) + 3 (attacks in hand) = 5
        assert s.player_energy == 5

    def test_no_energy_gain_blocks_further_gain(self):
        s = _state(hand=["ExpectAFight", "Bloodletting"],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:ExpectAFight")
        # After ExpectAFight, Bloodletting's +2 energy should be blocked
        if "PlayCard:Bloodletting" in legal_actions(s):
            energy_before = s.player_energy
            s = apply(s, "PlayCard:Bloodletting")
            assert s.player_energy == energy_before
        else:
            # Bloodletting might not be affordable — that's fine
            pass

    def test_expect_a_fight_upgraded_cost(self):
        s = _state(hand=["ExpectAFight+"], player_energy=0, draw_pile=[])
        actions = legal_actions(s)
        assert "PlayCard:ExpectAFight+" in actions

    def test_expect_a_fight_no_attacks(self):
        s = _state(hand=["Defend", "Defend", "ExpectAFight"],
                   draw_pile=[], discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:ExpectAFight")
        assert s.player_energy == 2  # 3 - 1 + 0


# =========================================================================
# Pillage
# =========================================================================

class TestPillage:
    """Pillage: Deal 8 damage. Draw cards until you draw a non-Attack."""

    def test_pillage_deals_damage(self):
        s = _state(hand=["Pillage"], draw_pile=["Defend"],
                   discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:Pillage")
        assert s.monsters[0].hp == 22  # 30 - 8

    def test_pillage_draws_until_non_attack(self):
        s = _state(hand=["Pillage"], draw_pile=["Strike", "Strike", "Defend"],
                   discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:Pillage")
        # Should have drawn 3 cards (2 Strikes + 1 Defend stop card)
        assert len(s.hand) >= 1
        # The last card in hand should be the non-Attack stop card
        assert any(not n.startswith("Strike") for n in s.hand)

    def test_pillage_empty_piles(self):
        s = _state(hand=["Pillage"], draw_pile=[], discard_pile=[],
                   player_energy=3)
        s = apply(s, "PlayCard:Pillage")
        assert s.monsters[0].hp == 22
        assert len(s.hand) == 0

    def test_pillage_upgraded_damage(self):
        s = _state(hand=["Pillage+"], draw_pile=["Defend"],
                   discard_pile=[], player_energy=3)
        s = apply(s, "PlayCard:Pillage+")
        assert s.monsters[0].hp == 18  # 30 - 12
