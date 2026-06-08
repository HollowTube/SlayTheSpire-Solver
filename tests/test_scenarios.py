import random

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
    assert sorted(state.hand) == sorted(IRONCLAD_STARTING_DECK)
    assert state.monster_name == "Jaw Worm"
    assert state.monster_hp == JAW_WORM_STARTING_HP
    # Per the Slay the Spire wiki, Jaw Worm always opens combat with Chomp.
    assert state.monster_intent == "Chomp"


def test_the_canonical_scenario_is_playable_through_the_existing_interface():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    assert "PlayCard:Strike" in legal_actions(state)
    assert "EndTurn" in legal_actions(state)

    awaiting_target = apply(state, "PlayCard:Strike")
    resolved = apply(awaiting_target, "SelectTarget:Monster")

    assert resolved.monster_hp == state.monster_hp - 6


def test_a_full_playthrough_plays_every_starting_card_in_some_sequence():
    # Per HOL-10's AC: "A full playthrough of the starter deck (all 10
    # starting cards playable in some sequence) completes without errors."
    # Greedily play the cheapest-affordable card each turn (so energy is
    # never wasted), targeting the monster whenever asked, and end the turn
    # once nothing more is affordable — proving the whole 10-card deck runs
    # through apply/legal_actions without error and without going terminal.
    #
    # Unlike the placeholder monster HOL-10 was written against, Jaw Worm's
    # AI-driven block/Strength make the exact final HP an emergent property
    # of its random move sequence rather than a fixed wiki number — so this
    # test only pins down what the AC actually requires (every card played,
    # fight still ongoing), leaving "did the random fight end" to the
    # dedicated terminal-state tests below.
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    played = []
    while state.hand:
        playable = [a for a in legal_actions(state) if a.startswith("PlayCard:")]
        if not playable:
            state = apply(state, "EndTurn")
            assert not is_terminal(state)
            continue
        action = playable[0]
        played.append(action)
        state = apply(state, action)
        if state.pending == "SelectTarget":
            state = apply(state, "SelectTarget:Monster")

    assert sorted(played) == sorted(f"PlayCard:{name}" for name in IRONCLAD_STARTING_DECK)
    # Pinned to seed=42, where the greedy line doesn't end the fight early —
    # in principle an unlucky seed could roll enough Jaw Worm damage to kill
    # the player before all 10 cards are played, but 42 is known-safe here.
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
    # seed=0 happens to end in the player's death and seed=19 in the Jaw
    # Worm's — sampled to guarantee this exercises *both* terminal outcomes
    # rather than asserting only the (much more common) loss.
    for seed in (0, 1, 2, 3, 19):
        state = ironclad_starter_deck_vs_jaw_worm(seed=seed)

        final = play_randomly_to_terminal(state, seed=seed)

        assert is_terminal(final)
        r = reward(final)
        assert -1.0 <= r <= 1.0
        if final.player_hp <= 0:
            assert r < 0
        else:
            assert final.monster_hp <= 0
            assert r > 0


def test_random_play_can_both_win_and_lose_against_the_jaw_worm():
    # A sanity check on the sample above: win-detection isn't dead code — a
    # 44 HP Jaw Worm is genuinely beatable by the 38-raw-damage starter hand
    # when Bash's Vulnerable lands before the Strikes that benefit from it.
    outcomes = set()
    for seed in (0, 1, 2, 3, 19):
        state = ironclad_starter_deck_vs_jaw_worm(seed=seed)
        final = play_randomly_to_terminal(state, seed=seed)
        outcomes.add(final.monster_hp <= 0)

    assert outcomes == {True, False}
