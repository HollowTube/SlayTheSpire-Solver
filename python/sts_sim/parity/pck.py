"""Read one file out of a Godot 4 ``.pck`` (format v3) without running the game."""

from __future__ import annotations

import struct
from pathlib import Path
from typing import BinaryIO

_MAGIC = 0x43504447  # "GDPC"
_DIR_ENCRYPTED = 1
_FILE_ENCRYPTED = 1


class PckError(Exception):
    pass


def _norm(path: str) -> str:
    return path.removeprefix("res://").lstrip("/")


def _read(f: BinaryIO, fmt: str) -> tuple[int, ...]:
    size = struct.calcsize(fmt)
    data = f.read(size)
    if len(data) != size:
        raise PckError("truncated PCK")
    return struct.unpack(fmt, data)


def read_pck_file(pck: str | Path, inner: str) -> bytes:
    """Return the bytes of ``inner`` (e.g. ``localization/eng/cards.json``)."""
    want = _norm(inner)
    with open(pck, "rb") as f:
        magic, version, _major, _minor, _patch = _read(f, "<IIIII")
        if magic != _MAGIC:
            raise PckError(f"{pck}: not a Godot PCK (magic {magic:#x})")
        if version != 3:
            raise PckError(
                f"{pck}: PCK format version {version} is not supported (want 3)"
            )
        flags, file_base, dir_offset = _read(f, "<IQQ")
        if flags & _DIR_ENCRYPTED:
            raise PckError(f"{pck}: the PCK directory is encrypted")
        # A relative file base is relative to the pack's start, which is 0 for a
        # standalone .pck, so both flag states read the same way here.
        f.seek(dir_offset)
        (count,) = _read(f, "<I")
        for _ in range(count):
            (path_len,) = _read(f, "<I")
            path = f.read(path_len).rstrip(b"\0").decode("utf-8")
            offset, size = _read(f, "<QQ")
            f.read(16)  # md5
            (file_flags,) = _read(f, "<I")
            if _norm(path) != want:
                continue
            if file_flags & _FILE_ENCRYPTED:
                raise PckError(f"{pck}: {path} is encrypted")
            f.seek(file_base + offset)
            data = f.read(size)
            if len(data) != size:
                raise PckError(f"{pck}: {path} is truncated")
            return data
    raise PckError(f"{pck}: no file {want!r} in the PCK directory")
