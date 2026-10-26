"""Tests for card parity group H: Feed, Rampage, Armaments, Brand."""

from sts_sim import (
    EndTurnAction,
    ExhaustCardFromHandAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    UpgradeCardFromHandAction,
    apply,
    legal_actions,
)


# ── Feed ──────────────────────────────────────────────────────────────────────


def test_feed_deals_10_damage(make_state):
    """Feed deals 10 damage to a chosen enemy."""
    state = make_state(hand=["Feed"], monsters=[Monster(hp=44, attack=6)])

    after_play = apply(state, PlayCardAction("Feed"))
    # Feed is targeted — choose the only monster
    resolved = apply(after_play, SelectTargetAction(0))

    assert resolved.monsters[0].hp == 44 - 10


def test_feed_raises_max_hp_on_kill(make_state):
    """Feed raises player max HP by 3 when it kills the target."""
    state = make_state(
        hand=["Feed"],
        player_hp=80,
        monsters=[Monster(hp=5, attack=0)],
    )

    after_play = apply(state, PlayCardAction("Feed"))
    resolved = apply(after_play, SelectTargetAction(0))

    # Monster killed (5 HP ≤ 10 damage), max HP raised by 3
    assert resolved.monsters[0].hp <= 0
    assert resolved.player_max_hp == 80 + 3
    assert resolved.player_hp == 80  # current HP unchanged


def test_feed_no_hp_gain_when_not_fatal(make_state):
    """Feed does not raise max HP when the target survives."""
    state = make_state(
        hand=["Feed"],
        player_hp=80,
        monsters=[Monster(hp=44, attack=6)],
    )

    after_play = apply(state, PlayCardAction("Feed"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert resolved.monsters[0].hp == 34  # 44 - 10
    assert resolved.player_max_hp == 80  # unchanged


def test_feed_exhausts(make_state):
    """Feed has the Exhaust keyword — goes to exhaust pile, not discard."""
    state = make_state(hand=["Feed"])

    after_play = apply(state, PlayCardAction("Feed"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert "Feed" in resolved.exhaust_pile
    assert "Feed" not in resolved.discard_pile


def test_feed_plus_deals_12_damage(make_state):
    """Feed+ deals 12 damage."""
    state = make_state(hand=["Feed+"], monsters=[Monster(hp=44, attack=0)])

    after_play = apply(state, PlayCardAction("Feed+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert resolved.monsters[0].hp == 44 - 12


def test_feed_plus_raises_max_hp_by_4_on_kill(make_state):
    """Feed+ raises max HP by 4 on a kill."""
    state = make_state(
        hand=["Feed+"],
        player_hp=80,
        monsters=[Monster(hp=5, attack=0)],
    )

    after_play = apply(state, PlayCardAction("Feed+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert resolved.monsters[0].hp <= 0
    assert resolved.player_max_hp == 80 + 4


# ── Rampage ───────────────────────────────────────────────────────────────────


def test_rampage_deals_9_damage_first_play(make_state):
    """Rampage deals base 9 damage on first play."""
    state = make_state(hand=["Rampage"], monsters=[Monster(hp=44, attack=0)])

    after_play = apply(state, PlayCardAction("Rampage"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert resolved.monsters[0].hp == 44 - 9


def test_rampage_damage_increases_each_play(make_state):
    """Rampage damage increases by 5 each time it is played."""
    state = make_state(
        hand=["Rampage", "Rampage"],
        player_energy=3,
        monsters=[Monster(hp=100, attack=0)],
    )

    # First Rampage: 9 damage
    s1 = apply(state, PlayCardAction("Rampage"))
    s1 = apply(s1, SelectTargetAction(0))
    assert s1.monsters[0].hp == 100 - 9

    # Second Rampage: 9 + 5 = 14 damage
    s2 = apply(s1, PlayCardAction("Rampage"))
    s2 = apply(s2, SelectTargetAction(0))
    assert s2.monsters[0].hp == 100 - 9 - 14


def test_rampage_counter_persists_across_turns(make_state):
    """Rampage's combat bonus persists into the next turn."""
    state = make_state(
        hand=["Rampage"],
        player_energy=3,
        monsters=[Monster(hp=100, attack=0)],
    )

    # Play Rampage once: deals 9 damage, counter now +5
    s1 = apply(state, PlayCardAction("Rampage"))
    s1 = apply(s1, SelectTargetAction(0))

    # End turn so Rampage returns to hand via deck cycle
    s2 = apply(s1, EndTurnAction())

    # Find Rampage in hand and play it again: should deal 9 + 5 = 14
    hp_before = s2.monsters[0].hp
    s3 = apply(s2, PlayCardAction("Rampage"))
    s3 = apply(s3, SelectTargetAction(0))
    assert s3.monsters[0].hp == hp_before - 14


def test_rampage_plus_increment_is_9(make_state):
    """Rampage+ increases damage by 9 each play."""
    state = make_state(
        hand=["Rampage+", "Rampage+"],
        player_energy=3,
        monsters=[Monster(hp=100, attack=0)],
    )

    # First play: 9 damage
    s1 = apply(state, PlayCardAction("Rampage+"))
    s1 = apply(s1, SelectTargetAction(0))

    # Second play: 9 + 9 = 18 damage
    s2 = apply(s1, PlayCardAction("Rampage+"))
    s2 = apply(s2, SelectTargetAction(0))
    assert s2.monsters[0].hp == 100 - 9 - 18


# ── Armaments ─────────────────────────────────────────────────────────────────


def test_armaments_gains_5_block(make_state):
    """Armaments grants 5 Block."""
    state = make_state(hand=["Armaments", "Strike"])

    # Play Armaments → UpgradeCardFromHand pending → choose Strike
    s1 = apply(state, PlayCardAction("Armaments"))
    assert s1.player_block == 5


def test_armaments_prompts_upgrade(make_state):
    """Playing Armaments puts state into UpgradeFromHand pending."""
    state = make_state(hand=["Armaments", "Strike"])

    s1 = apply(state, PlayCardAction("Armaments"))

    assert s1.pending == "UpgradeFromHand"
    legal = legal_actions(s1)
    assert any("UpgradeCardFromHand:Strike" in str(a) for a in legal)


def test_armaments_upgrades_chosen_card(make_state):
    """Armaments upgrades the chosen card in hand."""
    state = make_state(hand=["Armaments", "Strike"])

    s1 = apply(state, PlayCardAction("Armaments"))
    s2 = apply(s1, UpgradeCardFromHandAction("Strike"))

    assert "Strike+" in s2.hand
    assert s2.pending is None


def test_armaments_only_shows_unupgraded_cards(make_state):
    """UpgradeFromHand actions only include unupgraded cards."""
    state = make_state(hand=["Armaments", "Strike+", "Defend"])

    s1 = apply(state, PlayCardAction("Armaments"))
    legal = legal_actions(s1)

    card_names = [str(a) for a in legal]
    assert any("UpgradeCardFromHand:Defend" in n for n in card_names)
    assert not any("UpgradeCardFromHand:Strike+" in n for n in card_names)


def test_armaments_exhausts(make_state):
    """Armaments exhausts itself."""
    state = make_state(hand=["Armaments", "Strike"])

    s1 = apply(state, PlayCardAction("Armaments"))
    s2 = apply(s1, UpgradeCardFromHandAction("Strike"))

    assert "Armaments" in s2.exhaust_pile


def test_armaments_plus_upgrades_all_cards(make_state):
    """Armaments+ upgrades ALL unupgraded cards in hand."""
    state = make_state(hand=["Armaments+", "Strike", "Defend"])

    resolved = apply(state, PlayCardAction("Armaments+"))

    # No pending decision — effect resolves immediately
    assert resolved.pending is None
    assert "Strike+" in resolved.hand
    assert "Defend+" in resolved.hand


def test_armaments_plus_no_prompt(make_state):
    """Armaments+ does not trigger a pending decision."""
    state = make_state(hand=["Armaments+", "Strike"])

    resolved = apply(state, PlayCardAction("Armaments+"))

    assert resolved.pending is None


# ── Brand ─────────────────────────────────────────────────────────────────────


def test_brand_loses_1_hp(make_state):
    """Brand causes the player to lose 1 HP."""
    state = make_state(hand=["Brand", "Strike"], player_hp=80)

    s1 = apply(state, PlayCardAction("Brand"))
    # After card play, HP loss has happened; still waiting for exhaust choice
    assert s1.player_hp == 79


def test_brand_prompts_exhaust(make_state):
    """Playing Brand puts state into ExhaustFromHand pending."""
    state = make_state(hand=["Brand", "Strike"])

    s1 = apply(state, PlayCardAction("Brand"))

    assert s1.pending == "ExhaustFromHand"
    legal = legal_actions(s1)
    assert any("ExhaustCardFromHand:Strike" in str(a) for a in legal)


def test_brand_exhausts_chosen_card(make_state):
    """Brand exhausts the chosen card from hand."""
    state = make_state(hand=["Brand", "Strike"])

    s1 = apply(state, PlayCardAction("Brand"))
    s2 = apply(s1, ExhaustCardFromHandAction("Strike"))

    assert "Strike" in s2.exhaust_pile
    assert "Strike" not in s2.hand


def test_brand_grants_1_strength(make_state):
    """Brand grants 1 Strength after the exhaust choice resolves."""
    state = make_state(hand=["Brand", "Strike"])

    s1 = apply(state, PlayCardAction("Brand"))
    s2 = apply(s1, ExhaustCardFromHandAction("Strike"))

    assert s2.player_strength == 1
    assert s2.pending is None


def test_brand_exhausts_itself(make_state):
    """Brand exhausts itself (has Exhaust keyword)."""
    state = make_state(hand=["Brand", "Strike"])

    s1 = apply(state, PlayCardAction("Brand"))
    s2 = apply(s1, ExhaustCardFromHandAction("Strike"))

    assert "Brand" in s2.exhaust_pile
    assert "Brand" not in s2.discard_pile


def test_brand_plus_grants_2_strength(make_state):
    """Brand+ grants 2 Strength after the exhaust choice."""
    state = make_state(hand=["Brand+", "Strike"])

    s1 = apply(state, PlayCardAction("Brand+"))
    s2 = apply(s1, ExhaustCardFromHandAction("Strike"))

    assert s2.player_strength == 2
