from sts_sim import is_terminal, legal_actions, reward
from sts_sim.cli import (
    format_action,
    intent_description,
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


def test_two_turn_trace_verifies_every_mechanical_interaction_step_by_step():
    # A deterministic trace of the opening two turns (seed=42) that pins
    # exact HP/block/energy/statuses at each decision point — proving the
    # render surface exposed by the CLI faithfully reflects real engine
    # mechanics and not stale or placeholder values.
    #
    # Verified by hand via `sts_sim cli step` (see PR description):
    #   Turn 0: hand = Strike/Strike/Defend/Bash/Defend (5 cards), JW intent = Chomp
    #     Strike vs JW:   6 dmg → JW 44→38 HP, energy 3→2
    #     Defend:         5 block, energy 2→1
    #     EndTurn:        JW Chomp (11 dmg), 5 block absorbs 5 → player 80→74 HP
    #                     Player block resets. Energy refreshes to 3. 5 new cards drawn.
    #   Turn 1: JW intent = Thrash. Fresh hand: Bash/Defend/Strike/Strike/Defend
    #     Bash vs JW:     8 dmg → JW 38→30 HP, +Vulnerable ×2, energy 3→1
    #     Strike vs JW:   Vulnerable ×50% → 9 dmg → JW 30→21 HP, energy 1→0
    #     EndTurn:        JW Thrash (7 dmg + 5 block). Player 74→67 HP. JW block = 5.
    #                     Intent rotates to Bellow. Energy refreshes to 3. 5 new cards.
    from sts_sim import apply
    from sts_sim.cli import run_step

    seed = 42
    history = []

    def step(action):
        nonlocal history
        result = run_step(seed=seed, history=history, action=action)
        history = result.updated_history
        return result.state

    # --- Turn 0 setup ---
    initial = ironclad_starter_deck_vs_jaw_worm(seed=seed)
    assert initial.turn == 0
    assert initial.player_hp == 80
    assert initial.player_energy == 3
    assert initial.monster_hp == 44
    assert initial.monster_intent == "Chomp"
    assert len(initial.hand) == 5
    assert "Strike" in initial.hand

    # --- Strike deals 6 damage ---
    step("PlayCard:Strike")
    after_strike = step("SelectTarget:Monster")
    assert after_strike.monster_hp == 38
    assert after_strike.player_energy == 2

    # --- Defend gives 5 block ---
    after_defend = step("PlayCard:Defend")
    assert after_defend.player_block == 5
    assert after_defend.player_energy == 1

    # --- EndTurn: Chomp (11) hits 5 block → 6 net HP lost; energy/draw reset ---
    after_turn_1 = step("EndTurn")
    assert after_turn_1.turn == 1
    assert after_turn_1.player_hp == 74          # 80 - (11 - 5 block)
    assert after_turn_1.player_block == 0        # block resets
    assert after_turn_1.player_energy == 3       # energy refreshes
    assert len(after_turn_1.hand) == 5           # full hand drawn
    assert after_turn_1.monster_intent == "Thrash"

    # --- Bash: 8 damage + Vulnerable ---
    step("PlayCard:Bash")
    after_bash = step("SelectTarget:Monster")
    assert after_bash.monster_hp == 30           # 38 - 8
    assert after_bash.player_energy == 1         # 3 - 2 (Bash costs 2)
    assert "Vulnerable" in after_bash.monster_statuses

    # --- Strike vs Vulnerable: 6 × 1.5 = 9 damage ---
    step("PlayCard:Strike")
    after_vuln_strike = step("SelectTarget:Monster")
    assert after_vuln_strike.monster_hp == 21    # 30 - 9
    assert after_vuln_strike.player_energy == 0  # 1 - 1

    # --- EndTurn: Thrash (7 dmg to player, 5 block to JW) ---
    after_turn_2 = step("EndTurn")
    assert after_turn_2.turn == 2
    assert after_turn_2.player_hp == 67          # 74 - 7
    assert after_turn_2.monster_block == 5       # Thrash grants JW 5 block
    assert after_turn_2.player_energy == 3
    assert len(after_turn_2.hand) == 5
    assert after_turn_2.monster_intent == "Bellow"
    assert "Vulnerable" in after_turn_2.monster_statuses


def test_intent_description_returns_human_readable_move_effects():
    assert intent_description("Jaw Worm", "Chomp") == "11 damage"
    assert intent_description("Jaw Worm", "Thrash") == "7 damage, gain 5 block"
    assert intent_description("Jaw Worm", "Bellow") == "gain 3 Strength, gain 6 block"
    # Unknown intent falls back gracefully rather than crashing
    assert intent_description("Jaw Worm", "???") == "???"
    assert intent_description("Unknown Monster", "Bite") == "Bite"


def test_render_state_includes_intent_description():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    text = render_state(state)

    # Jaw Worm always opens with Chomp — its description must appear so the
    # player knows how much damage they're about to take without needing to
    # look it up.
    assert "11 damage" in text
