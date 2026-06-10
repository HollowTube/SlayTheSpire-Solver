import random
from collections import Counter

from sts_sim import apply, is_terminal, legal_actions, reward
from sts_sim.scenarios import (
    IRONCLAD_STARTING_DECK,
    JAW_WORM_STARTING_HP,
    PLAYER_STARTING_HP,
    ironclad_starter_deck_vs_jaw_worm,
)


def test_the_canonical_scenario_loads_with_the_documented_starting_loadout():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    assert state.player_hp == PLAYER_STARTING_HP
    # Per HOL-13: combat starts with the deck shuffled into the draw pile and
    # a real opening hand dealt from it — not the whole deck dumped into hand.
    assert len(state.hand) == 5
    assert sorted(state.hand + state.draw_pile) == sorted(IRONCLAD_STARTING_DECK)
    assert state.discard_pile == []
    assert state.monsters[0].name == "Jaw Worm"
    assert state.monsters[0].hp == JAW_WORM_STARTING_HP
    # Per the Slay the Spire wiki, Jaw Worm always opens combat with Chomp.
    assert state.monsters[0].intent == "Chomp"


def test_the_canonical_scenario_is_playable_through_the_existing_interface():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    assert "PlayCard:Strike" in legal_actions(state)
    assert "EndTurn" in legal_actions(state)

    awaiting_target = apply(state, "PlayCard:Strike")
    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert resolved.monsters[0].hp == state.monsters[0].hp - 6


def test_a_full_playthrough_plays_every_starting_card_in_some_sequence():
    # Per HOL-10's AC: "A full playthrough of the starter deck (all 10
    # starting cards playable in some sequence) completes without errors."
    # Per HOL-13, the deck now cycles through hand -> discard -> (reshuffled)
    # draw pile across many turns rather than sitting in hand all at once —
    # so "every starting card played" becomes "every named card played at
    # least as many times as it appears in the deck" (HOL-13's AC: "playing a
    # card moves it to discard_pile rather than vanishing", so a card that
    # cycles back can legitimately be played again before the deck is
    # "exhausted" in this sense).
    #
    # Greedily play the cheapest-affordable card each turn (so energy is never
    # wasted), targeting the monster whenever asked, and end the turn once
    # nothing more is affordable — proving the whole engine (draw/discard/
    # reshuffle/play/target) runs without error and without going terminal
    # before the full deck has cycled through at least once.
    #
    # Unlike the placeholder monster HOL-10 was written against, Jaw Worm's
    # AI-driven block/Strength make the exact final HP an emergent property
    # of its random move sequence rather than a fixed wiki number — so this
    # test only pins down what the AC actually requires (every card played,
    # fight still ongoing), leaving "did the random fight end" to the
    # dedicated terminal-state tests below.
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)
    deck_counts = Counter(IRONCLAD_STARTING_DECK)

    played_counts = Counter()
    while not played_counts >= deck_counts:
        playable = [a for a in legal_actions(state) if a.startswith("PlayCard:")]
        if not playable:
            state = apply(state, "EndTurn")
            assert not is_terminal(state)
            continue
        action = playable[0]
        played_counts[action.removeprefix("PlayCard:")] += 1
        state = apply(state, action)
        if state.pending == "SelectTarget":
            state = apply(state, "SelectTarget:Monster:0")

    # Pinned to seed=42, where the greedy line doesn't end the fight early —
    # in principle an unlucky seed could roll enough Jaw Worm damage to kill
    # the player before the whole deck has cycled through, but 42 is
    # known-safe here.
    assert not is_terminal(state)


def play_randomly_to_terminal(state, seed):
    rng = random.Random(seed)
    while not is_terminal(state):
        state = apply(state, rng.choice(legal_actions(state)))
    return state


def test_complete_random_fights_against_the_jaw_worm_reach_correctly_shaped_terminal_states():
    # Per HOL-11's AC: "Multiple complete simulated fights (via scripted or
    # random play) reach correct terminal win/loss states with
    # correctly-shaped reward values." Drive several fights to completion via
    # uniformly-random legal actions (a maximally-dumb "scripted" policy that
    # nonetheless must terminate correctly either way), and check that
    # `is_terminal`/`reward` agree on which side actually won.
    #
    # Per HOL-13, drawing a real 5-card hand each turn (rather than holding
    # the whole deck from turn one) makes wins far more common under random
    # play — re-sampled so seed=11 ends in the player's death and the rest in
    # the Jaw Worm's, guaranteeing this exercises *both* terminal outcomes
    # rather than asserting only one.
    for seed in (0, 1, 2, 3, 11):
        state = ironclad_starter_deck_vs_jaw_worm(seed=seed)

        final = play_randomly_to_terminal(state, seed=seed)

        assert is_terminal(final)
        r = reward(final)
        assert -1.0 <= r <= 1.0
        if final.player_hp <= 0:
            assert r < 0
        else:
            assert final.monsters[0].hp <= 0
            assert r > 0


def test_random_play_can_both_win_and_lose_against_the_jaw_worm():
    # A sanity check on the sample above: win-detection isn't dead code — a
    # 44 HP Jaw Worm is genuinely beatable by the 38-raw-damage starter hand
    # when Bash's Vulnerable lands before the Strikes that benefit from it.
    outcomes = set()
    for seed in (0, 1, 2, 3, 11):
        state = ironclad_starter_deck_vs_jaw_worm(seed=seed)
        final = play_randomly_to_terminal(state, seed=seed)
        outcomes.add(final.monsters[0].hp <= 0)

    assert outcomes == {True, False}
