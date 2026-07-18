"""Live integ tests for implemented Ironclad power cards.

Power cards disappear from hand on play and don't land in any standard pile.
Each test verifies the card's effect directly — no pile-content assertions.

Run with:
    pytest integtests/cards/test_powers.py -v -s -m live
"""

import time

import pytest

from sts_sim.bridge import client as bc

from sts_sim.sim.names import CardName
from integtests.conftest import CombatFixture, _combat

pytestmark = pytest.mark.live


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _end_turn_wait(settle: float = 10.0) -> None:
    """End the player's turn and wait until the next player turn begins."""
    bc.act_and_wait("end_turn", settle_timeout=settle)
    time.sleep(0.3)  # let turn-start effects settle


def _player_powers() -> dict[str, int]:
    """Return {bridge_power_name: amount} for every power on the player."""
    return {p.name: p.amount for p in _combat().player.powers}


# ---------------------------------------------------------------------------
# Immediate-effect power cards
# ---------------------------------------------------------------------------


def test_inflame():
    """Inflame: play → player gains +2 Strength immediately."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.INFLAME)
    assert fix.play("inflame")
    # Inflame applies Strength(2) directly; bridge reports it as "Strength"
    assert fix.has_power("Strength", target="player") == 2


def test_juggernaut():
    """Juggernaut: play, then gain Block via Defend → enemy takes damage."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.JUGGERNAUT, CardName.DEFEND)
    assert fix.play("juggernaut")
    hp_before = fix.enemy_hp()
    assert fix.play("defend")  # gain 5 block → Juggernaut fires
    assert fix.enemy_hp() < hp_before


def test_feel_no_pain():
    """FeelNoPain: play, exhaust a card via Tremble → player gains 3 Block."""
    fix = CombatFixture()
    fix.setup_fight()
    # TREMBLE (Skill, cost 1, Exhaust keyword) triggers CardExhausted
    fix.set_hand(CardName.FEEL_NO_PAIN, CardName.TREMBLE)
    assert fix.play("feelno")
    assert fix.play("tremble")  # exhausts → FeelNoPain fires
    assert fix.player_block() == 3


def test_dark_embrace():
    """DarkEmbrace: play, exhaust a card → player draws 1 card."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.DARK_EMBRACE, CardName.TREMBLE)
    fix.set_draw_pile(CardName.DEFEND)
    assert fix.play("dark")
    # Hand now has only [TREMBLE]; draw pile has [DEFEND]
    snap_before = _combat()
    hand_before = len(snap_before.player.hand)
    assert fix.play("tremble")  # exhausts → DarkEmbrace fires → draw DEFEND
    snap_after = _combat()
    # Tremble removed (−1) but draw triggered (+1) → same size
    assert len(snap_after.player.hand) == hand_before


def test_unmovable():
    """Unmovable: first block gain per turn is doubled."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.UNMOVABLE, CardName.DEFEND)
    assert fix.play("unmovable")
    assert fix.play("defend")  # normally 5 block → doubled to 10
    assert fix.player_block() == 10


def test_juggling():
    """Juggling: after playing 3 attacks, a copy of the 3rd is added to hand."""
    fix = CombatFixture()
    fix.setup_fight()
    # BLOODLETTING (cost 0, gains 2 Energy) lets us fit Juggling + 3 Strikes
    fix.set_hand(
        CardName.BLOODLETTING,
        CardName.JUGGLING,
        CardName.STRIKE,
        CardName.STRIKE,
        CardName.STRIKE,
    )
    assert fix.play("bloodletting")  # 0 cost → +2e = 5e total
    assert fix.play("juggling")  # 1e
    assert fix.play("strike")  # 1e — attack 1
    assert fix.play("strike")  # 1e — attack 2
    snap_before = _combat()
    assert fix.play("strike")  # 1e — attack 3 → Juggling fires
    snap_after = _combat()
    # A Strike copy should have been added — hand grows by 1 net (played 1, gained 1)
    assert len(snap_after.player.hand) == len(snap_before.player.hand)


# ---------------------------------------------------------------------------
# Turn-start / turn-end power cards  (require ending the current turn)
# ---------------------------------------------------------------------------


def test_demon_form():
    """DemonForm: play, end turn → player gains 2 Strength at turn start."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.DEMON_FORM)
    strength_before = fix.has_power("Strength", target="player")
    assert fix.play("demonform")
    _end_turn_wait()
    assert fix.has_power("Strength", target="player") >= strength_before + 2


def test_barricade():
    """Barricade: play → BarricadePower status is active on the player."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.BARRICADE)
    assert fix.play("barricade")
    # BarricadePower is installed immediately; block-persistence is a sim-level invariant.
    assert fix.has_power("Barricade", target="player") > 0


def test_crimson_mantle():
    """CrimsonMantle: at turn start → gains 8 Block."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.CRIMSON_MANTLE)
    assert fix.play("crimson")
    _end_turn_wait()
    # Enemies attacked in the gap before turn 2 starts, but CrimsonMantle fires
    # at turn START (after enemy attacks) → should see exactly 8 block freshly granted.
    assert fix.player_block() == 8


def test_inferno():
    """Inferno: at turn start → player loses 1 HP, all enemies take 6 damage."""
    fix = CombatFixture(fight_id="THE_KIN_BOSS")
    fix.setup_fight()
    fix.set_hand(CardName.INFERNO)
    snap_before = _combat()
    enemy_hp_before = [e.hp for e in snap_before.enemies]
    assert fix.play("inferno")
    _end_turn_wait()
    snap_after = _combat()
    # Inferno fires at turn START (after enemy attacks in the gap) → 6 AoE to all.
    # Kin minions have no block, so HP should drop exactly 6 each.
    assert all(
        snap_after.enemies[i].hp == enemy_hp_before[i] - 6
        for i in range(len(snap_before.enemies))
    )


def test_pyre():
    """Pyre: play, end turn → player starts next turn with 4 Energy."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.PYRE)
    assert fix.play("pyre")
    _end_turn_wait()
    assert _combat().player.energy == 4


def test_aggression():
    """Aggression: with a Strike in discard, end turn → Strike appears in hand."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.AGGRESSION)
    fix.set_discard_pile(CardName.STRIKE)
    assert fix.play("aggression")
    _end_turn_wait()
    snap = _combat()
    names = [c.name.lower() for c in snap.player.hand]
    assert any("strike" in n for n in names)


def test_corruption():
    """Corruption: play, end turn → Skill (Defend) costs 0 Energy next turn."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.CORRUPTION)
    fix.set_draw_pile(CardName.DEFEND)
    assert fix.play("corruption")  # costs 3 energy
    _end_turn_wait()
    # On new turn, energy = 3 and Defend should have been drawn
    snap_before = _combat()
    energy_before = snap_before.player.energy
    assert fix.play("defend")  # Corruption makes Skills cost 0
    assert _combat().player.energy == energy_before  # energy unchanged


def test_stone_armor():
    """StoneArmor: play → PlatingPower(4) status installed immediately."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.STONE_ARMOR)
    assert fix.play("stone")
    # PlatingPower is applied immediately on play (not deferred to turn end).
    assert fix.has_power("Plating", target="player") == 4


# ---------------------------------------------------------------------------
# Complex / reactive power cards — verify damage amplification or raw presence
# ---------------------------------------------------------------------------


def test_cruelty():
    """Cruelty: deal Strike to Vulnerable enemy → 75% amp instead of 50%."""
    fix = CombatFixture()
    fix.setup_fight()
    # Apply 2 Vulnerable to enemy via Bash, then play Strike with Cruelty
    # Energy budget: Cruelty(1) + Bash(2) = 3e.  Strike goes next turn.
    fix.set_hand(CardName.CRUELTY, CardName.BASH)
    fix.set_draw_pile(CardName.STRIKE)
    assert fix.play("cruelty")
    assert fix.play("bash")  # 8 dmg + 2 Vulnerable
    hp_after_bash = fix.enemy_hp()
    _end_turn_wait()
    # New turn: Defend was drawn; play Strike (should be in hand or set it)
    fix.set_hand(CardName.STRIKE)
    assert fix.play("strike")  # 6 × 1.75 = 10.5 → floor = 10 (Cruelty + Vulnerable)
    # Without Cruelty, Strike on 2-stack Vulnerable = 6 × 1.5 = 9
    assert hp_after_bash - fix.enemy_hp() == 10


def test_vicious():
    """Vicious: play → status is active on the player."""
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand(CardName.VICIOUS)
    assert fix.play("vicious")
    powers = _player_powers()
    assert any("icious" in k for k in powers), f"Vicious not found in {powers}"
