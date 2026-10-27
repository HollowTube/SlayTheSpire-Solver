"""Host-platform checks shared by the bridge client and the analysis server."""

import os
import pathlib


def _read_proc_version() -> str:
    try:
        return pathlib.Path("/proc/version").read_text()
    except OSError:
        return ""


def is_wsl() -> bool:
    return (
        "WSL_DISTRO_NAME" in os.environ or "microsoft" in _read_proc_version().lower()
    )
