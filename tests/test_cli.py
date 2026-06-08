from sts_sim import is_terminal, legal_actions, reward
from sts_sim.cli import (
    format_action,
    prompt_for_choice,
    render_state,
    run_interactive,
)
from sts_sim.scenarios import ironclad_starter_deck_vs_jaw_worm


def test_format_action_translates_engine_strings_into_human_readable_labels():
    assert format_action("PlayCard:Strike") == "Play Strike"
    assert format_action("PlayCard:Bash") == "Play Bash"
    assert format_action("SelectTarget:Monster") == "Target Monster"
    assert format_action("EndTurn") == "End Turn"


def test_render_state_shows_the_information_a_player_needs_to_decide():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    text = render_state(state)

    # Per HOL-14's user stories: HP, block, energy, hand, monster
    # name/HP/intent, and turn number must all be visible at a glance.
    assert f"Turn {state.turn}" in text
    assert str(state.player_hp) in text
    assert str(state.player_energy) in text
    assert state.monster_name in text
    assert str(state.monster_hp) in text
    assert state.monster_intent in text
    for card in state.hand:
        assert card in text


def test_render_state_shows_status_effects_by_name_and_stack_count():
    from sts_sim import CombatState, apply

    state = CombatState(player_hp=80, player_energy=3, monster_hp=44, monster_attack=6,
                        seed=42, hand=["Bash"])
    struck = apply(apply(state, "PlayCard:Bash"), "SelectTarget:Monster")

    text = render_state(struck)

    # Per HOL-14 user story 12: statuses are shown by name (and stack count
    # for stacking statuses), not raw engine identifiers — Bash inflicts
    # Vulnerable on the monster, so its name must appear in the render.
    assert "Vulnerable" in text

def test_prompt_for_choice_returns_the_action_at_the_chosen_menu_number():
    actions = ["PlayCard:Strike", "EndTurn"]
    inputs = iter(["2"])

    chosen = prompt_for_choice(actions, input_fn=lambda _: next(inputs), output_fn=lambda _: None)

    assert chosen == "EndTurn"


def test_prompt_for_choice_rejects_invalid_input_and_reprompts():
    # Per HOL-14 user story 5: a typo (non-numeric, or a number outside the
    # menu's range) must be rejected with a clear message and re-prompted —
    # not crash the session or silently apply the wrong action.
    actions = ["PlayCard:Strike", "EndTurn"]
    inputs = iter(["banana", "0", "99", "1"])
    messages = []

    chosen = prompt_for_choice(actions, input_fn=lambda _: next(inputs), output_fn=messages.append)

    assert chosen == "PlayCard:Strike"
    # Three bad inputs ("banana", "0", "99") should each have produced some
    # rejection message before the valid "1" was accepted.
    assert sum("invalid" in m.lower() for m in messages) == 3


def test_run_interactive_plays_a_full_fight_to_a_coherent_terminal_outcome():
    # Per HOL-14 user story 13 / Testing Decisions: drive the interactive
    # loop with scripted input and assert it reaches a terminal state and
    # reports a coherent win/loss outcome — mirroring how test_mcts.py
    # integration-tests `search` against the fixed canonical scenario.
    #
    # Always choosing the first menu option greedily plays whatever's
    # cheapest/first and ends the turn once nothing else is legal — the same
    # "dumbest scripted policy that still completes" pattern the scenario
    # tests use, and it reliably reaches a terminal state under seed=42.
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)
    messages = []

    final_state = run_interactive(state, input_fn=lambda _: "1", output_fn=messages.append)

    assert is_terminal(final_state)
    r = reward(final_state)
    transcript = "\n".join(messages)
    # The session must report a clean conclusion: win/loss, final HP, reward.
    assert ("won" in transcript.lower()) or ("lost" in transcript.lower())
    assert str(final_state.player_hp) in transcript
    assert f"{r:.2f}"[:4] in transcript


def test_replay_history_reconstructs_state_by_reapplying_each_action_from_the_seed():
    # Per HOL-14 user story 14: agent step mode reconstructs the current
    # state from (seed, history) alone — replaying it must land on exactly
    # the state `apply` would reach by walking the same actions directly.
    from sts_sim import apply
    from sts_sim.cli import replay_history

    seed = 42
    state = ironclad_starter_deck_vs_jaw_worm(seed=seed)
    awaiting_target = apply(state, "PlayCard:Strike")
    expected = apply(awaiting_target, "SelectTarget:Monster")

    replayed = replay_history(seed, ["PlayCard:Strike", "SelectTarget:Monster"])

    assert replayed == expected


def test_run_step_applies_one_action_and_returns_state_menu_and_updated_history():
    # Per HOL-14 user stories 14-15: one invocation replays `history`,
    # applies `action`, and returns everything the next invocation needs —
    # including `updated_history` (history + action) so the agent never has
    # to track or re-derive it.
    from sts_sim.cli import run_step

    seed = 42
    result = run_step(seed=seed, history=[], action="EndTurn")

    assert result.updated_history == ["EndTurn"]
    assert is_terminal(result.state) is False
    assert result.legal_actions == legal_actions(result.state)
    assert "Turn" in result.rendered


def test_agent_step_mode_can_play_a_complete_fight_via_pure_history_replay():
    # Per HOL-14 user story 16: the concrete proof the seed+history replay
    # contract holds across a *whole* fight, not just one step — repeatedly
    # feed each response's `updated_history` back in as the next request's
    # history, picking the first legal action each time (mirroring the
    # "first legal action" scripted-policy pattern test_scenarios.py uses).
    from sts_sim.cli import run_step

    seed = 42
    history = []
    state = ironclad_starter_deck_vs_jaw_worm(seed=seed)
    while not is_terminal(state):
        action = legal_actions(state)[0]
        result = run_step(seed=seed, history=history, action=action)
        assert result.updated_history == history + [action]
        history = result.updated_history
        state = result.state

    assert is_terminal(state)
    assert reward(state) != 0
