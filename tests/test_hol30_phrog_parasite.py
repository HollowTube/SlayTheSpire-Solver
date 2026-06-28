from sts_sim import CombatState, EndTurnAction, Monster, PlayCardAction, apply


# ── helpers ──────────────────────────────────────────────────────────────────


def _phrog(seed=42):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=64, name="Phrog Parasite")],
        seed=seed,
        hand=[],
    )


# ── Infection card ───────────────────────────────────────────────────────────


def test_infection_is_a_status_card():
    """Infection is a Status card: it has a card_data entry (bypasses the
    'unknown card' error) and exhausts on play."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=64, name="Phrog Parasite")],
        hand=["Infection"],
        seed=0,
    )
    assert "Infection" in state.hand
    a1 = apply(state, PlayCardAction("Infection"))
    # Status cards exhaust on play
    assert "Infection" not in a1.hand
    assert "Infection" in a1.exhaust_pile


# ── Status::Stun ─────────────────────────────────────────────────────────────


def test_stunned_monster_skips_turn():
    """A monster with Status::Stun skips its turn entirely."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[
            Monster(
                hp=64,
                name="Phrog Parasite",
                statuses=[("Stun", 1)],
            )
        ],
        hand=[],
        seed=0,
    )
    a1 = apply(state, EndTurnAction())
    # Stunned monster doesn't deal damage, doesn't change intent
    assert a1.player_hp == 80
    # Stun is consumed after the skip
    assert not any(
        s == ("Stun", 0) or (isinstance(s, tuple) and s[0] == "Stun")
        for s in a1.monsters[0].statuses
    )


# ── Phrog Parasite basic moves ───────────────────────────────────────────────


def test_phrog_opens_with_infect():
    assert _phrog().monsters[0].intent == "Infect"


def test_infect_applies_3_infection():
    state = _phrog()
    a1 = apply(state, EndTurnAction())
    # Infect pushes 3 Infection cards into player's discard; since the draw
    # pile is empty, they get reshuffled into the hand on the next turn draw.
    infections = [c for c in a1.hand if c == "Infection"]
    assert len(infections) == 3


def test_lash_deals_16_damage():
    state = _phrog()
    a1 = apply(state, EndTurnAction())  # Infect
    assert a1.monsters[0].intent == "Lash"
    a2 = apply(a1, EndTurnAction())
    # Lash: 4 hits of 4 damage = 16
    assert a1.player_hp - a2.player_hp == 16


def test_phrog_alternates_infect_and_lash():
    state = _phrog()
    a1 = apply(state, EndTurnAction())
    assert a1.monsters[0].intent == "Lash"
    a2 = apply(a1, EndTurnAction())
    assert a2.monsters[0].intent == "Infect"


# ── Wriggler basic moves ──────────────────────────────────────────────────────


def _wriggler(seed=42):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=21, name="Wriggler")],
        seed=seed,
        hand=[],
    )


def test_wriggler_opens_with_nasty_bite():
    """Single Wriggler (odd index) opens with Nasty Bite."""
    assert _wriggler().monsters[0].intent == "Nasty Bite"


def test_nasty_bite_deals_6_damage():
    state = _wriggler()
    a1 = apply(state, EndTurnAction())
    assert a1.player_hp == 74  # 80 - 6
    assert a1.monsters[0].intent == "Wriggle"


def test_wriggle_applies_1_infection_and_2_strength():
    state = _wriggler(seed=99)
    a1 = apply(state, EndTurnAction())  # Nasty Bite
    assert a1.monsters[0].intent == "Wriggle"
    # Wriggle: 1 Infection + self +2 Strength
    a2 = apply(a1, EndTurnAction())
    infections = [c for c in a2.hand if c == "Infection"]
    assert len(infections) == 1
    # Strength(2) appears as 1 "Strength" entry in Python
    strength_count = sum(1 for s in a2.monsters[0].statuses if s == "Strength")
    assert strength_count == 1


# ── Phrog Parasite death → Wriggler spawn ────────────────────────────────────


def test_infested_phrog_dies_and_spawns_4_stunned_wrigglers():
    """Killing an Infested Phrog spawns 4 Wrigglers with Stun. The dead Phrog
    stays in the list (hp ≤ 0) and the new Wrigglers appear alongside it."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[
            Monster(
                hp=0,  # already dead
                name="Phrog Parasite",
                statuses=[("Infested", 4)],
            )
        ],
        seed=0,
        hand=[],
    )
    assert len(state.monsters) == 1
    # EndTurn triggers Infested spawn (dead monster's Infested fires during
    # the monster turn loop before the hp ≤ 0 skip)
    a1 = apply(state, EndTurnAction())
    wrigglers = [m for m in a1.monsters if m.name == "Wriggler"]
    assert len(wrigglers) == 4
    # All start with Stun
    for w in wrigglers:
        assert "Stun" in w.statuses
    # Stunned Wrigglers don't act on their first turn
    a2 = apply(a1, EndTurnAction())
    assert a2.player_hp == 80
