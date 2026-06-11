"""Behavioural tests for Phase 0 Ironclad cards: trivial drop-ins composed
entirely of existing EffectOps (no engine changes)."""

from sts_sim import CombatState, Monster, apply


def make_state(hand=("Strike",), seed=42):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=seed,
        hand=list(hand),
    )


# ── Bludgeon ─────────────────────────────────────────────────────────────────


def test_bludgeon_deals_32_damage():
    state = make_state(hand=["Bludgeon"])

    awaiting_target = apply(state, "PlayCard:Bludgeon")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 32


# ── TwinStrike ───────────────────────────────────────────────────────────────


def test_twin_strike_deals_5_damage_twice():
    state = make_state(hand=["TwinStrike"])

    awaiting_target = apply(state, "PlayCard:TwinStrike")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 10


# ── Break ────────────────────────────────────────────────────────────────────


def test_break_deals_20_damage_and_applies_5_vulnerable():
    state = make_state(hand=["Break"])

    awaiting_target = apply(state, "PlayCard:Break")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 20
    assert resolved.monsters[0].statuses.count("Vulnerable") == 5


# ── ShrugItOff ───────────────────────────────────────────────────────────────


def test_shrug_it_off_gains_8_block_and_draws_1():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["ShrugItOff"],
        draw_pile=["Strike"],
    )

    resolved = apply(state, "PlayCard:ShrugItOff")

    assert resolved.player_block == state.player_block + 8
    assert "Strike" in resolved.hand


# ── Taunt ────────────────────────────────────────────────────────────────────


def test_taunt_gains_7_block_and_applies_vulnerable_to_target():
    state = make_state(hand=["Taunt"])

    awaiting_target = apply(state, "PlayCard:Taunt")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert resolved.player_block == state.player_block + 7
    assert "Vulnerable" in resolved.monsters[0].statuses


# ── Uppercut ─────────────────────────────────────────────────────────────────


def test_uppercut_deals_13_damage_and_applies_weak_and_vulnerable():
    state = make_state(hand=["Uppercut"])

    awaiting_target = apply(state, "PlayCard:Uppercut")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 13
    assert "Weak" in resolved.monsters[0].statuses
    assert "Vulnerable" in resolved.monsters[0].statuses
