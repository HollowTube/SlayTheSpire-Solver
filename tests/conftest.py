from sts_sim import CombatState, Monster


def make_state(hand=(), seed=42, player_hp=80, player_energy=3, monsters=None, **kwargs):
    return CombatState(
        player_hp=player_hp,
        player_energy=player_energy,
        monsters=monsters if monsters is not None else [Monster(hp=44, attack=6)],
        seed=seed,
        hand=list(hand),
        **kwargs,
    )
