from sts_sim import CombatState, Monster, PlayCardAction, apply


def make_state(hand, monsters=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=monsters or [Monster(hp=44, attack=6)],
        seed=42,
        hand=list(hand),
    )


def test_breakthrough_costs_1_hp_and_hits_all_enemies_for_9_without_a_target():
    state = make_state(
        hand=["Breakthrough"],
        monsters=[Monster(hp=44, attack=6), Monster(hp=30, attack=5)],
    )
    resolved = apply(state, PlayCardAction("Breakthrough"))
    assert resolved.pending is None
    assert resolved.player_hp == state.player_hp - 1
    assert resolved.monsters[0].hp == state.monsters[0].hp - 9
    assert resolved.monsters[1].hp == state.monsters[1].hp - 9
