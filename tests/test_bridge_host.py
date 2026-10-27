"""Bridge host selection and the setup-hook output: WSL-only gateway detection."""

import json
import subprocess

import pytest
from click.testing import CliRunner

from sts_sim import _platform
from sts_sim.bridge import client
from sts_sim.bridge.cli import main
from sts_sim import server

GATEWAY_ROUTE = "default via 172.26.176.1 dev eth0 proto kernel\n"


@pytest.fixture
def host_env(monkeypatch):
    """No WSL signal, no override, and `ip route` reporting a gateway."""
    monkeypatch.delenv("STS2_BRIDGE_HOST", raising=False)
    monkeypatch.delenv("WSL_DISTRO_NAME", raising=False)
    monkeypatch.setattr(_platform, "_read_proc_version", lambda: "Linux version 6.1")
    monkeypatch.setattr(subprocess, "check_output", lambda *a, **kw: GATEWAY_ROUTE)
    return monkeypatch


def test_native_linux_uses_loopback_even_with_a_default_route(host_env):
    assert client._default_bridge_host() == "127.0.0.1"


def test_wsl_env_uses_the_default_route_gateway(host_env):
    host_env.setenv("WSL_DISTRO_NAME", "Ubuntu")
    assert client._default_bridge_host() == "172.26.176.1"


def test_wsl_proc_version_uses_the_default_route_gateway(host_env):
    host_env.setattr(
        _platform, "_read_proc_version", lambda: "Linux ...-microsoft-standard-WSL2"
    )
    assert client._default_bridge_host() == "172.26.176.1"


def test_wsl_falls_back_to_loopback_when_gateway_lookup_fails(host_env):
    host_env.setenv("WSL_DISTRO_NAME", "Ubuntu")

    def boom(*a, **kw):
        raise FileNotFoundError("ip")

    host_env.setattr(subprocess, "check_output", boom)
    assert client._default_bridge_host() == "127.0.0.1"


@pytest.mark.parametrize("wsl", [False, True])
def test_explicit_host_wins(host_env, wsl):
    if wsl:
        host_env.setenv("WSL_DISTRO_NAME", "Ubuntu")
    host_env.setenv("STS2_BRIDGE_HOST", "10.1.2.3")
    assert client._default_bridge_host() == "10.1.2.3"


def test_empty_explicit_host_is_ignored(host_env):
    host_env.setenv("STS2_BRIDGE_HOST", "")
    assert client._default_bridge_host() == "127.0.0.1"


@pytest.mark.parametrize(
    "wsl,expected",
    [(False, "127.0.0.1"), (True, "0.0.0.0")],
)
def test_server_default_host_follows_wsl(host_env, wsl, expected):
    if wsl:
        host_env.setenv("WSL_DISTRO_NAME", "Ubuntu")
    assert server._default_host() == expected


# ─── setup-hook ───────────────────────────────────────────────────────────────


def _install(tmp_path, app):
    result = CliRunner().invoke(
        main, ["setup-hook", "--project-dir", str(tmp_path), "--app", app]
    )
    assert result.exit_code == 0, result.output


def test_claude_hook_exports_the_host_only_on_wsl(tmp_path):
    _install(tmp_path, "claude-code")
    settings = json.loads((tmp_path / ".claude" / "settings.json").read_text())
    command = settings["SessionStart"][0]["hooks"][0]["command"]
    export = command.index("export STS2_BRIDGE_HOST")
    guard = command.index("microsoft")
    assert command.startswith("if ") and guard < export
    assert "WSL_DISTRO_NAME" in command[:export]
    assert "fi;" in command[export:]


def _run_hook_probe(tmp_path, env_extra):
    """Run the installed hook command with the sts2 binary swapped for an echo."""
    import os
    import subprocess

    _install(tmp_path, "claude-code")
    settings = json.loads((tmp_path / ".claude" / "settings.json").read_text())
    command = settings["SessionStart"][0]["hooks"][0]["command"]
    head = command.rsplit("; ", 1)[0]
    probe = head + '; echo "HOST=${STS2_BRIDGE_HOST-unset}"'
    env = {
        k: v
        for k, v in os.environ.items()
        if k not in ("STS2_BRIDGE_HOST", "WSL_DISTRO_NAME")
    }
    env.update(env_extra)
    out = subprocess.run(["sh", "-c", probe], env=env, capture_output=True, text=True)
    return out.stdout.strip()


def test_claude_hook_keeps_an_existing_host_on_wsl(tmp_path):
    out = _run_hook_probe(
        tmp_path, {"WSL_DISTRO_NAME": "Ubuntu", "STS2_BRIDGE_HOST": "10.9.9.9"}
    )
    assert out == "HOST=10.9.9.9"


def test_opencode_plugin_exports_the_host_only_on_wsl(tmp_path):
    _install(tmp_path, "opencode")
    plugin = (tmp_path / ".opencode" / "plugins" / "sts2.ts").read_text()
    export = plugin.index("output.env.STS2_BRIDGE_HOST")
    guard = plugin.index("microsoft")
    assert guard < export
    assert "WSL_DISTRO_NAME" in plugin[:export]
