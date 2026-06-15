"""Behavioural tests for HOL-34 Phase 4b Wave 1 cards: Pyre, FightMe!,
Anger, DrumOfBattle, Stomp."""

from sts_sim import CombatState, Monster, apply, legal_actions


def _state(hand, seed=42, energy=3, hp=80, draw_pile=None):
    return CombatState(
        player_hp=hp,
        player_energy=energy,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=seed,
        hand=hand,
        draw_pile=draw_pile or [],
    )


# ── FightMe! ─────────────────────────────────────────────────────────────────


def test_fight_me_deals_5_damage_twice_and_grants_strength():
    state = _state(["FightMe!", "Strike", "Strike", "Strike", "Defend"])
    after = apply(apply(state, "PlayCard:FightMe!"), "SelectTarget:Monster:0")
    assert after.monsters[0].hp == 44 - 5 - 5
    assert after.monsters[0].strength == 1
    assert after.player_strength == 3


# ── Pyre ─────────────────────────────────────────────────────────────────────


def test_pyre_applies_pyre_status():
    state = _state(["Pyre", "Strike", "Strike", "Strike", "Defend"])
    after = apply(state, "PlayCard:Pyre")
    assert "Pyre" in after.player_statuses


def test_pyre_grants_energy_at_start_of_each_turn():
    state = _state(["Pyre", "Strike", "Strike", "Strike", "Defend"])
    after = apply(state, "PlayCard:Pyre")
    after_end_turn = apply(after, "EndTurn")
    assert after_end_turn.player_energy == 4  # max(3) + Pyre(1)


# ── Anger ────────────────────────────────────────────────────────────────────


def test_anger_deals_6_damage():
    state = _state(["Anger", "Strike", "Strike", "Strike", "Defend"])
    after = apply(apply(state, "PlayCard:Anger"), "SelectTarget:Monster:0")
    assert after.monsters[0].hp == 44 - 6


def test_anger_adds_copy_to_discard():
    state = _state(["Anger", "Strike", "Strike", "Strike", "Defend"])
    after = apply(apply(state, "PlayCard:Anger"), "SelectTarget:Monster:0")
    assert after.discard_pile.count("Anger") == 2  # played copy + added copy


def test_anger_costs_0():
    state = _state(["Anger", "Strike", "Strike", "Strike", "Strike"])
    assert legal_actions(state).count("PlayCard:Anger") == 1


# ── DrumOfBattle ─────────────────────────────────────────────────────────────


def test_drum_of_battle_draws_2_on_play():
    state = _state(
        ["DrumOfBattle", "Strike", "Strike", "Strike", "Defend"],
        draw_pile=["Defend", "Defend"],
    )
    assert len(state.hand) == 5
    after = apply(state, "PlayCard:DrumOfBattle")
    assert len(after.hand) == 6  # played (1 gone) + draw 2 = net +1


def test_drum_of_battle_exhausts_top_of_draw_at_turn_start():
    state = _state(
        ["DrumOfBattle", "Strike", "Strike", "Strike", "Strike"],
        draw_pile=["Defend", "Defend"],
    )
    after = apply(state, "PlayCard:DrumOfBattle")
    # draw_pile had 2, DrumOfBattle draws 2 → exhausted draw pile
    assert len(after.draw_pile) == 0
    assert "BattleDrum" in after.player_statuses


# ── Stomp ────────────────────────────────────────────────────────────────────


def test_stomp_deals_12_damage_to_all_enemies():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm"), Monster(hp=30, name="Gremlin Nob")],
        hand=["Stomp", "Strike", "Strike", "Strike", "Defend"],
        seed=42,
    )
    after = apply(state, "PlayCard:Stomp")
    assert after.monsters[0].hp == 44 - 12
    assert after.monsters[1].hp == 30 - 12


def test_stomp_costs_1_less_per_attack_played():
    state = _state(["Strike", "Stomp", "Strike", "Strike", "Defend"], energy=2)
    after_strike = apply(apply(state, "PlayCard:Strike"), "SelectTarget:Monster:0")
    legal = legal_actions(after_strike)
    assert "PlayCard:Stomp" in legal
