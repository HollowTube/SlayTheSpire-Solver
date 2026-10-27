"""PCK reader tests. All PCK files here are synthetic, built in-test."""

from __future__ import annotations

import hashlib
import struct
from pathlib import Path

import pytest

from sts_sim.parity.pck import PckError, read_pck_file

MAGIC = 0x43504447
DIR_ENCRYPTED = 1
REL_FILEBASE = 2


def _build_pck(
    path: Path,
    files: dict[str, bytes],
    *,
    flags: int = REL_FILEBASE,
    file_flags: int = 0,
    version: int = 3,
) -> None:
    header_len = 20 + 12 + 8 + 16 * 4
    blobs = b""
    offsets = {}
    for name, data in files.items():
        offsets[name] = len(blobs)
        blobs += data
    file_base = header_len
    dir_offset = header_len + len(blobs)
    directory = struct.pack("<I", len(files))
    for name, data in files.items():
        raw = name.encode()
        raw += b"\0" * (-len(raw) % 4)
        directory += struct.pack("<I", len(raw)) + raw
        directory += struct.pack("<QQ", offsets[name], len(data))
        directory += hashlib.md5(data).digest()
        directory += struct.pack("<I", file_flags)
    header = struct.pack("<IIIII", MAGIC, version, 4, 5, 1)
    header += struct.pack("<IQQ", flags, file_base, dir_offset)
    header += b"\0" * 64
    path.write_bytes(header + blobs + directory)


def test_reads_one_file_by_exact_path(tmp_path: Path) -> None:
    pck = tmp_path / "game.pck"
    _build_pck(
        pck,
        {
            "localization/eng/cards.json": b'{"A.title": "A"}',
            "localization/eng/cards.json.bak": b"nope",
        },
    )
    assert read_pck_file(pck, "localization/eng/cards.json") == b'{"A.title": "A"}'


def test_accepts_res_prefix_on_either_side(tmp_path: Path) -> None:
    pck = tmp_path / "game.pck"
    _build_pck(pck, {"res://localization/eng/cards.json": b"x"})
    assert read_pck_file(pck, "localization/eng/cards.json") == b"x"
    assert read_pck_file(pck, "res://localization/eng/cards.json") == b"x"


def test_missing_file_fails_naming_it(tmp_path: Path) -> None:
    pck = tmp_path / "game.pck"
    _build_pck(pck, {"a.json": b"x"})
    with pytest.raises(PckError, match="b.json"):
        read_pck_file(pck, "b.json")


def test_rejects_encrypted_directory(tmp_path: Path) -> None:
    pck = tmp_path / "game.pck"
    _build_pck(pck, {"a.json": b"x"}, flags=DIR_ENCRYPTED)
    with pytest.raises(PckError, match="encrypted"):
        read_pck_file(pck, "a.json")


def test_rejects_encrypted_file(tmp_path: Path) -> None:
    pck = tmp_path / "game.pck"
    _build_pck(pck, {"a.json": b"x"}, file_flags=1)
    with pytest.raises(PckError, match="encrypted"):
        read_pck_file(pck, "a.json")


def test_rejects_non_pck(tmp_path: Path) -> None:
    pck = tmp_path / "game.pck"
    pck.write_bytes(b"not a pck at all, just bytes" * 4)
    with pytest.raises(PckError, match="not a Godot PCK"):
        read_pck_file(pck, "a.json")


def test_rejects_unsupported_version(tmp_path: Path) -> None:
    pck = tmp_path / "game.pck"
    _build_pck(pck, {"a.json": b"x"}, version=2)
    with pytest.raises(PckError, match="version 2"):
        read_pck_file(pck, "a.json")
