from sts_sim import apply, is_terminal, legal_actions
from sts_sim.scenarios import (
    IRONCLAD_STARTING_DECK,
    PLAYER_STARTING_HP,
    ironclad_starter_deck_vs_placeholder_monster,
)


def test_the_canonical_scenario_loads_with_the_documented_starting_loadout():
    state = ironclad_starter_deck_vs_placeholder_monster(seed=42)

    assert state.player_hp == PLAYER_STARTING_HP
    assert sorted(state.hand) == sorted(IRONCLAD_STARTING_DECK)


def test_the_canonical_scenario_is_playable_through_the_existing_interface():
    state = ironclad_starter_deck_vs_placeholder_monster(seed=42)

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
    state = ironclad_starter_deck_vs_placeholder_monster(seed=42)

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

    # Greedy play exhausts the hand in its listed order — all 5 Strikes (6
    # dmg each, monster not yet Vulnerable) land before Bash (8 dmg, then
    # applies Vulnerable too late to amplify anything further this fight) —
    # pinning the final HP down to a concrete, wiki-consistent number.
    assert state.monster_hp == 44 - (5 * 6 + 8)
