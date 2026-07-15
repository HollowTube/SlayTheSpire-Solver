import json
from unittest.mock import MagicMock

import pytest

from sts_sim.bridge_cli import (
    _call,
    _card_name,
    _card_upgraded,
    _error,
    _first_player,
    _fmt_intent,
    main,
)
from sts_sim import bridge_client as bc


# ─── helpers ──────────────────────────────────────────────────────────────────


def _invoke(cmd, args, monkeypatch, return_value=None):
    """Invoke a Click command with mocked bridge data."""
    from click.testing import CliRunner

    ctx = MagicMock()
    ctx.obj = {"as_json": "--json" in args}

    if return_value is not None:
        monkeypatch.setattr(bc, cmd, lambda *a, **kw: return_value)

    runner = CliRunner()
    result = runner.invoke(main, args)
    return result


# ─── _first_player ────────────────────────────────────────────────────────────


def test_first_player_extracts_from_players_array():
    assert _first_player({"players": [{"hp": 80}]}) == {"hp": 80}


def test_first_player_returns_empty_dict_for_non_dict():
    assert _first_player("not a dict") == {}
    assert _first_player(None) == {}


def test_first_player_returns_empty_dict_when_no_players():
    assert _first_player({}) == {}
    assert _first_player({"players": []}) == {}


# ─── _call ────────────────────────────────────────────────────────────────────


def test_call_unwraps_result_wrapper():
    def fn():
        return {"result": {"hp": 80}}

    assert _call(fn) == {"hp": 80}


def test_call_returns_raw_dict_when_no_result_key():
    def fn():
        return {"hp": 80}

    assert _call(fn) == {"hp": 80}


def test_call_exits_on_error(monkeypatch):
    monkeypatch.setattr("sts_sim.bridge_cli.click.echo", lambda x: None)

    def fn():
        return {"result": {"error": "something broke"}}

    with pytest.raises(SystemExit) as exc:
        _call(fn)
    assert exc.value.code == 1


def test_call_exits_on_non_dict_return(monkeypatch):
    monkeypatch.setattr("sts_sim.bridge_cli.click.echo", lambda x: None)

    def fn():
        return "double-encoded string"

    with pytest.raises(SystemExit) as exc:
        _call(fn)
    assert exc.value.code == 1


# ─── player command ───────────────────────────────────────────────────────────


def test_player_shows_character_hp_gold_and_deck(monkeypatch):
    from click.testing import CliRunner

    data = {
        "players": [
            {
                "character": "Ironclad",
                "hp": 75,
                "max_hp": 80,
                "gold": 99,
                "deck": [
                    {"name": "Strike", "type": "Attack", "upgraded": False},
                    {"name": "Bash", "type": "Attack", "upgraded": True},
                ],
                "relics": [{"name": "BurningBlood", "rarity": "Starter"}],
                "potions": [{"slot": 0, "name": "empty"}],
            }
        ]
    }
    monkeypatch.setattr(bc, "get_player_state", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["player"])
    assert result.exit_code == 0
    assert "Ironclad" in result.output
    assert "75/80" in result.output
    assert "99" in result.output
    assert "Strike" in result.output
    assert "Bash" in result.output
    assert "BurningBlood" in result.output
    assert "empty" in result.output


def test_player_handles_flat_response(monkeypatch):
    """Some bridge versions may return flat dicts without the players array."""
    from click.testing import CliRunner

    data = {
        "character": "Silent",
        "current_hp": 70,
        "max_hp": 70,
        "gold": 50,
        "deck": [],
    }
    monkeypatch.setattr(bc, "get_player_state", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["player"])
    assert result.exit_code == 0
    assert "Silent" in result.output
    assert "70/70" in result.output


# ─── state command ────────────────────────────────────────────────────────────


def test_state_extracts_hp_and_gold_from_players_array(monkeypatch):
    from click.testing import CliRunner

    data = {
        "floor": 3,
        "act": 1,
        "seed": "12345",
        "players": [{"hp": 60, "max_hp": 80, "gold": 42}],
    }
    monkeypatch.setattr(bc, "get_run_state", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["state"])
    assert result.exit_code == 0
    assert "3" in result.output
    assert "1" in result.output
    assert "60/80" in result.output
    assert "42" in result.output
    assert "12345" in result.output


# ─── map command ────────────────────────────────────────────────────────────


def test_map_formats_nodes_as_row_col(monkeypatch):
    from click.testing import CliRunner

    data = {
        "floor": 2,
        "act": 1,
        "nodes": [
            {"row": 1, "col": 0, "type": "Monster", "available": True},
            {"row": 1, "col": 1, "type": "Shop", "available": False},
        ],
    }
    monkeypatch.setattr(bc, "get_map_state", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["map"])
    assert result.exit_code == 0
    assert "1,0" in result.output
    assert "1,1" in result.output
    assert "Monster" in result.output
    assert "Shop" in result.output


# ─── log command ──────────────────────────────────────────────────────────────


def test_log_uses_timestamp_field(monkeypatch):
    from click.testing import CliRunner

    data = {
        "entries": [
            {
                "id": 1,
                "timestamp": "12:34:56.789",
                "level": "Info",
                "message": "hello world",
            }
        ]
    }
    monkeypatch.setattr(bc, "get_game_log", lambda **kw: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["log", "--lines", "1"])
    assert result.exit_code == 0
    assert "12:34:56.789" in result.output
    assert "hello world" in result.output


def test_log_falls_back_to_time_field(monkeypatch):
    from click.testing import CliRunner

    data = {
        "entries": [
            {"time": "12:34:56", "message": "legacy format"},
        ]
    }
    monkeypatch.setattr(bc, "get_game_log", lambda **kw: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["log", "--lines", "1"])
    assert result.exit_code == 0
    assert "legacy format" in result.output


# ─── piles command ────────────────────────────────────────────────────────────


def test_piles_calls_get_card_piles_through_call(monkeypatch):
    """Ensure piles uses _call() (which unwraps {"result": ...}) instead of
    manual unwrapping that could diverge from _call()'s logic."""
    from click.testing import CliRunner

    data = {
        "hand": [
            {"name": "Strike", "type": "Attack", "energy_cost": 1, "upgraded": False}
        ],
        "draw_pile": [],
        "discard_pile": [],
        "exhaust_pile": [],
    }
    monkeypatch.setattr(bc, "get_card_piles", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["piles"])
    assert result.exit_code == 0
    assert "Strike" in result.output


def test_actions_table_includes_status_column(monkeypatch):
    from click.testing import CliRunner

    data = {
        "screen": "EVENT",
        "actions": [
            {"action": "event_option", "label": "Blessing A"},
            {"action": "event_option", "label": "Blessing B"},
            {"action": "event_proceed"},
        ],
    }
    monkeypatch.setattr(bc, "get_available_actions", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, ["actions"])
    assert result.exit_code == 0
    # Table header should include status column
    assert "actions[3]{n,action,label,status}:" in result.output
    # Blocked action should be marked
    assert "blocked" in result.output
    # Ready actions should be marked
    assert "ready" in result.output


# ─── send_request double-encoding ─────────────────────────────────────────────


def test_act_warns_on_blocked_action(monkeypatch):
    from click.testing import CliRunner

    actions = [
        {"action": "event_option", "label": "Blessing A"},
        {"action": "event_proceed"},
    ]
    monkeypatch.setattr(
        bc, "get_available_actions", lambda: {"result": {"actions": actions}}
    )

    runner = CliRunner()
    result = runner.invoke(main, ["act", "1"])
    assert result.exit_code == 1
    assert "error:" in result.output
    assert "not valid right now" in result.output


def test_act_executes_ready_action_normally(monkeypatch):
    from click.testing import CliRunner

    actions = [
        {"action": "event_option", "label": "Blessing A"},
        {"action": "event_proceed"},
    ]
    monkeypatch.setattr(
        bc, "get_available_actions", lambda: {"result": {"actions": actions}}
    )
    # act_and_wait returns a new state
    monkeypatch.setattr(
        bc,
        "act_and_wait",
        lambda *a, **kw: {"screen": "MAP", "combat": {}, "result": {}},
    )

    runner = CliRunner()
    result = runner.invoke(main, ["act", "0"])
    assert result.exit_code == 0
    assert "acted:" in result.output


def test_send_request_handles_double_encoded_json(monkeypatch):
    """If the bridge sends a JSON string instead of a JSON object,
    send_request should decode it one more level."""
    import socket

    inner = {"status": "ok", "version": "2.0.0"}
    double_encoded = json.dumps({"result": inner})

    fake_sock = MagicMock()
    fake_sock.recv = MagicMock(
        side_effect=[double_encoded.encode("utf-8") + b"\n", b""]
    )
    fake_sock.sendall = MagicMock()
    fake_sock.__enter__ = MagicMock(return_value=fake_sock)
    fake_sock.__exit__ = MagicMock(return_value=False)

    monkeypatch.setattr(socket, "create_connection", lambda *a, **kw: fake_sock)

    result = bc.send_request("ping")
    assert result == inner


# ─── formatting helpers ───────────────────────────────────────────────────────


def test_fmt_intent_parses_attack():
    assert (
        _fmt_intent({"intents": [{"type": "Attack", "damage": 6, "hits": 1}]})
        == "Attack(6)"
    )
    assert (
        _fmt_intent({"intents": [{"type": "Attack", "damage": 3, "hits": 2}]})
        == "Attack(3x2)"
    )


def test_fmt_intent_unknown():
    assert _fmt_intent(None) == "?"
    assert _fmt_intent("Debuff") == "Debuff"


def test_card_name_from_dict():
    assert _card_name({"name": "Strike"}) == "Strike"
    assert _card_name({"card_name": "Bash"}) == "Bash"


def test_card_name_from_string():
    assert _card_name("STRIKE_IRONCLAD") == "STRIKE_IRONCLAD"


def test_card_upgraded():
    assert _card_upgraded({"upgraded": True}) is True
    assert _card_upgraded("Bash+") is True
    assert _card_upgraded({"upgraded": False}) is False


def test_error_block():
    assert _error("boom") == "error: boom"
    assert _error("boom", "do this") == "error: boom\nhelp: do this"


# ─── _action_status (tracer bullet) ───────────────────────────────────────────


def test_action_status_event_proceed_blocked_when_blessings_available():
    from sts_sim.bridge_cli import _action_status

    actions = [
        {"action": "event_option"},
        {"action": "event_proceed"},
    ]
    assert _action_status({"action": "event_proceed"}, actions) == "blocked"


def test_action_status_event_proceed_ready_when_no_blessings():
    from sts_sim.bridge_cli import _action_status

    actions = [
        {"action": "event_proceed"},
        {"action": "console"},
    ]
    assert _action_status({"action": "event_proceed"}, actions) == "ready"


def test_action_status_card_confirm_blocked_until_selection():
    from sts_sim.bridge_cli import _action_status

    actions = [
        {"action": "card_select"},
        {"action": "card_confirm"},
    ]
    assert _action_status({"action": "card_confirm"}, actions) == "blocked"


def test_action_status_card_confirm_ready_when_no_selections():
    from sts_sim.bridge_cli import _action_status

    actions = [
        {"action": "card_confirm"},
    ]
    assert _action_status({"action": "card_confirm"}, actions) == "ready"


def test_action_status_default_ready():
    from sts_sim.bridge_cli import _action_status

    actions = [{"action": "play_card"}, {"action": "end_turn"}]
    assert _action_status({"action": "play_card"}, actions) == "ready"


# ─── phase-aware hints ──────────────────────────────────────────────────────────


def test_home_hints_neow_pick_phase():
    from sts_sim.bridge_cli import _home_hints

    actions = [
        {"action": "event_option", "label": "Pomander"},
        {"action": "event_option", "label": "Scissors"},
        {"action": "event_proceed"},
    ]
    hints = _home_hints("EVENT", "NEVENTROOM", actions)
    assert "pick a blessing" in " ".join(hints).lower()


def test_home_hints_neow_proceed_phase():
    from sts_sim.bridge_cli import _home_hints

    actions = [
        {"action": "event_proceed"},
        {"action": "console"},
    ]
    hints = _home_hints("EVENT", "NEVENTROOM", actions)
    assert "proceed" in " ".join(hints).lower()


def test_home_hints_card_selection_upgrade():
    from sts_sim.bridge_cli import _home_hints

    actions = [
        {"action": "card_select", "label": "Strike"},
        {"action": "card_select", "label": "Defend"},
        {"action": "card_confirm"},
        {"action": "card_skip"},
    ]
    hints = _home_hints("CARD_SELECTION", "CardUpgradeScreen", actions)
    assert "select" in " ".join(hints).lower()
    assert "confirm" in " ".join(hints).lower()


def test_home_hints_combat_turn():
    from sts_sim.bridge_cli import _home_hints

    actions = [
        {"action": "play_card", "label": "Strike"},
        {"action": "end_turn"},
    ]
    hints = _home_hints("COMBAT_PLAYER_TURN", "", actions)
    assert "act" in " ".join(hints).lower()


# ─── home view action preview ─────────────────────────────────────────────────


def test_main_shows_actions_in_combat(monkeypatch):
    from click.testing import CliRunner

    data = {
        "screen": "COMBAT_PLAYER_TURN",
        "combat": {
            "players": [
                {
                    "hp": 80,
                    "max_hp": 80,
                    "energy": 3,
                    "max_energy": 3,
                    "hand": [
                        {"name": "Strike", "energy_cost": 1},
                        {"name": "Defend", "energy_cost": 1},
                    ],
                }
            ],
            "enemies": [
                {"name": "Jaw Worm", "hp": 44, "max_hp": 44, "intent": "Chomp"}
            ],
        },
        "available_actions": {
            "actions": [
                {"action": "play_card", "card_name": "Strike"},
                {"action": "play_card", "card_name": "Defend"},
                {"action": "end_turn"},
            ]
        },
    }
    monkeypatch.setattr(bc, "get_full_state", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, [])
    assert result.exit_code == 0
    assert "actions[3]" in result.output
    assert "Strike" in result.output
    assert "Defend" in result.output
    assert "End turn" in result.output


def test_main_limits_actions_to_8(monkeypatch):
    from click.testing import CliRunner

    data = {
        "screen": "COMBAT_PLAYER_TURN",
        "combat": {
            "players": [
                {
                    "hp": 80,
                    "max_hp": 80,
                    "energy": 3,
                    "max_energy": 3,
                    "hand": [{"name": "Strike", "energy_cost": 1}],
                }
            ],
            "enemies": [
                {"name": "Jaw Worm", "hp": 44, "max_hp": 44, "intent": "Chomp"}
            ],
        },
        "available_actions": {
            "actions": [
                {"action": "play_card", "card_name": f"Card{i}"} for i in range(12)
            ]
        },
    }
    monkeypatch.setattr(bc, "get_full_state", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, [])
    assert result.exit_code == 0
    assert "actions[8]" in result.output
    assert "Card0" in result.output
    assert "Card7" in result.output
    # Card8 should be truncated
    assert "Card8" not in result.output


def test_main_shows_actions_in_event(monkeypatch):
    from click.testing import CliRunner

    data = {
        "screen": "EVENT",
        "available_actions": {
            "actions": [
                {"action": "event_option", "label": "Pomander"},
                {"action": "event_option", "label": "Scissors"},
                {"action": "event_proceed"},
            ]
        },
    }
    monkeypatch.setattr(bc, "get_full_state", lambda: {"result": data})

    runner = CliRunner()
    result = runner.invoke(main, [])
    assert result.exit_code == 0
    assert "actions[3]" in result.output
    assert "Pomander" in result.output
    assert "Scissors" in result.output
