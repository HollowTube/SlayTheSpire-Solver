"""Behavioural tests for HOL-54 (slice 2, wave A): `UpgradeDelta` entries for
Aggression, AshenStrike, Bloodletting, BloodWall, Bludgeon, BodySlam, Break,
Breakthrough, Bully, BurningPact, Cinder, Colossus, and Conflagration. Each
test confirms the `"+"`-suffixed card resolves the upgraded value via `apply`,
matching STS2's `OnUpgrade()`."""

from sts_sim import CombatState, Monster, apply


def make_state(hand, seed=42, player_energy=3, monsters=None, **kwargs):
    return CombatState(
        player_hp=80,
        player_energy=player_energy,
        monsters=monsters or [Monster(hp=99, attack=0)],
        seed=seed,
        hand=list(hand),
        **kwargs,
    )


# ── Aggression ───────────────────────────────────────────────────────────────


def test_aggression_plus_still_installs_aggression_status():
    state = make_state(hand=["Aggression+"], player_energy=1)

    resolved = apply(state, "PlayCard:Aggression+")

    assert "Aggression" in resolved.player_statuses
    # Powers exhaust on play regardless of keywords.
    assert "Aggression+" in resolved.exhaust_pile


# ── AshenStrike ──────────────────────────────────────────────────────────────


def test_ashen_strike_plus_deals_6_plus_4_per_card_in_exhaust_pile():
    # 2 cards already in exhaust pile -> 6 + 4*2 = 14 damage.
    state = make_state(hand=["AshenStrike+"], exhaust_pile=["Tremble", "Impervious"])

    after_play = apply(state, "PlayCard:AshenStrike+")
    resolved = apply(after_play, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 14


# ── Bloodletting ─────────────────────────────────────────────────────────────


def test_bloodletting_plus_gains_3_energy():
    state = make_state(hand=["Bloodletting+"], player_energy=1)

    resolved = apply(state, "PlayCard:Bloodletting+")

    assert resolved.player_energy == 1 + 3
    assert resolved.player_hp == state.player_hp - 3


# ── BloodWall ────────────────────────────────────────────────────────────────


def test_blood_wall_plus_grants_20_block():
    state = make_state(hand=["BloodWall+"], player_energy=2)

    resolved = apply(state, "PlayCard:BloodWall+")

    assert resolved.player_block == 20
    assert resolved.player_hp == state.player_hp - 2


# ── Bludgeon ─────────────────────────────────────────────────────────────────


def test_bludgeon_plus_deals_42_damage():
    state = make_state(hand=["Bludgeon+"], player_energy=3)

    after_play = apply(state, "PlayCard:Bludgeon+")
    resolved = apply(after_play, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 42


# ── BodySlam ─────────────────────────────────────────────────────────────────


def test_body_slam_plus_costs_0_energy():
    state = make_state(hand=["BodySlam+"], player_energy=1, player_block=12)

    after_play = apply(state, "PlayCard:BodySlam+")
    resolved = apply(after_play, "SelectTarget:Monster:0")

    # BodySlam+ costs 0 energy, so the player's energy is unchanged.
    assert resolved.player_energy == 1
    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


# ── Break ────────────────────────────────────────────────────────────────────


def test_break_plus_deals_30_damage_and_applies_7_vulnerable():
    state = make_state(hand=["Break+"])

    after_play = apply(state, "PlayCard:Break+")
    resolved = apply(after_play, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 30
    assert resolved.monsters[0].statuses.count("Vulnerable") == 7


# ── Breakthrough ─────────────────────────────────────────────────────────────


def test_breakthrough_plus_deals_13_damage_to_all_enemies():
    state = make_state(
        hand=["Breakthrough+"],
        monsters=[Monster(hp=99, attack=0), Monster(hp=99, attack=0)],
    )

    resolved = apply(state, "PlayCard:Breakthrough+")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 13
    assert state.monsters[1].hp - resolved.monsters[1].hp == 13


# ── Bully ────────────────────────────────────────────────────────────────────


def test_bully_plus_deals_4_plus_3_per_vulnerable_stack_on_target():
    # Target has 3 stacks of Vulnerable -> base damage 4 + 3*3 = 13, then
    # the target's own Vulnerable amplifies it by 1.5x -> floor(13*1.5) = 19.
    state = make_state(
        hand=["Bully+"],
        monsters=[Monster(hp=99, attack=0, statuses=[("Vulnerable", 3)])],
        player_energy=0,
    )

    after_play = apply(state, "PlayCard:Bully+")
    resolved = apply(after_play, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 19


# ── BurningPact ──────────────────────────────────────────────────────────────


def test_burning_pact_plus_exhausts_a_card_and_draws_3():
    state = make_state(
        hand=["BurningPact+", "Strike"],
        draw_pile=["Defend", "Defend", "Defend", "Defend"],
        player_energy=1,
    )

    resolved = apply(state, "PlayCard:BurningPact+")

    # BurningPact+ exhausts a random card from hand, then draws 3.
    assert len(resolved.exhaust_pile) == 1
    assert len(resolved.hand) == 3


# ── Cinder ───────────────────────────────────────────────────────────────────


def test_cinder_plus_deals_24_damage():
    state = make_state(hand=["Cinder+"], player_energy=2)

    after_play = apply(state, "PlayCard:Cinder+")
    resolved = apply(after_play, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 24


# ── Colossus ─────────────────────────────────────────────────────────────────


def test_colossus_plus_grants_8_block():
    state = make_state(hand=["Colossus+"], player_energy=1)

    resolved = apply(state, "PlayCard:Colossus+")

    assert resolved.player_block == 8


# ── Conflagration ────────────────────────────────────────────────────────────


def test_conflagration_plus_deals_9_plus_3_per_attack_played_this_turn():
    state = make_state(
        hand=["Strike", "Strike", "Conflagration+"],
        monsters=[Monster(hp=99, attack=0), Monster(hp=99, attack=0)],
        player_energy=3,
    )

    after_strike_1 = apply(apply(state, "PlayCard:Strike"), "SelectTarget:Monster:0")
    after_strike_2 = apply(
        apply(after_strike_1, "PlayCard:Strike"), "SelectTarget:Monster:0"
    )

    resolved = apply(after_strike_2, "PlayCard:Conflagration+")

    # 9 + 3*2 = 15 damage to each enemy.
    assert after_strike_2.monsters[0].hp - resolved.monsters[0].hp == 15
    assert after_strike_2.monsters[1].hp - resolved.monsters[1].hp == 15
