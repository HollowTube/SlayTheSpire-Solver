#!/usr/bin/env python3
"""Scrape in-game card descriptions for all cards in data/cards.toml.

Requires:
  - STS2 bridge running (set STS2_BRIDGE_HOST if not localhost)
  - GodotExplorer mod active in game (port 27020)
  - Game in an active combat (use `sts2 dev fight NIBBIT` to start one)

Usage:
  # Scrape all cards, save to scripts/card_descriptions.json
  python scripts/scrape_card_descriptions.py

  # Only scrape cards missing from the output file (resume / patch update)
  python scripts/scrape_card_descriptions.py --missing-only

  # Scrape a specific card
  python scripts/scrape_card_descriptions.py --card SHOCKWAVE

Output format (scripts/card_descriptions.json):
  {
    "SHOCKWAVE": {
      "base": "Apply 3 Weak and Vulnerable to ALL enemies.\\nExhaust.",
      "upgraded": "Apply 5 Weak and Vulnerable to ALL enemies.\\nExhaust."
    },
    ...
  }

Protocol note:
  GodotExplorer uses JSON-RPC 2.0 with method "tools/call" wrapping.
  The sts2-modding MCP server's godot_explorer_client.py has the reference
  implementation; this script replicates that protocol directly so it can
  run without the MCP server in the loop.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

# Try to import from the sts2-modding MCP package (preferred — authoritative protocol).
# Fall back to a minimal inline implementation if the package isn't on the path.
try:
    sys.path.insert(0, str(Path.home() / "projects/sts2-modding-mcp"))
    from sts2mcp.godot_explorer_client import get_property as _explorer_get_property
    _USE_MCP_CLIENT = True
except ImportError:
    _USE_MCP_CLIENT = False
    import socket as _socket

BRIDGE_HOST = os.environ.get("STS2_BRIDGE_HOST", "127.0.0.1")
EXPLORER_PORT = 27020
HAND_BASE = (
    "/root/Game/RootSceneContainer/Run/RoomContainer"
    "/CombatRoom/CombatUi/Hand/CardHolderContainer"
)
ENERGY_PATH = "res://images/packed/sprite_fonts/ironclad_energy_icon.png"

REPO_ROOT = Path(__file__).parent.parent
OUT_FILE = REPO_ROOT / "scripts" / "card_descriptions.json"
CARDS_TOML = REPO_ROOT / "data" / "cards.toml"


# ── Fallback explorer client (mirrors sts2mcp.godot_explorer_client) ─────────

def _explorer_get_property_fallback(path: str, prop: str) -> str:
    """Minimal inline client matching GodotExplorer's tools/call protocol."""
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "get_property", "arguments": {"path": path, "property": prop}},
    }
    payload = json.dumps(request) + "\n"
    try:
        with _socket.create_connection((BRIDGE_HOST, EXPLORER_PORT), timeout=15) as sock:
            sock.settimeout(15)
            sock.sendall(payload.encode("utf-8"))
            data = b""
            while True:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                data += chunk
                if b"\n" in data:
                    break
        resp = json.loads(data.decode("utf-8-sig").strip())
        result = resp.get("result", {})
        content = result.get("content", []) if isinstance(result, dict) else []
        texts = [c.get("text", "") for c in content if isinstance(c, dict)]
        return "\n".join(texts)
    except Exception as e:
        return f"ERROR:{e}"


def explorer_get_property(path: str, prop: str) -> str:
    if _USE_MCP_CLIENT:
        raw = _explorer_get_property(path, prop)
        return str(raw) if raw is not None else ""
    return _explorer_get_property_fallback(path, prop)


# ── Card list from data/cards.toml ────────────────────────────────────────────

def load_card_ids() -> list[tuple[str, str]]:
    """Return [(variant, sts2_id), ...] for every card in data/cards.toml."""
    content = CARDS_TOML.read_text()
    entries = []
    current: dict = {}
    for line in content.splitlines():
        line = line.strip()
        if line == "[[cards]]":
            if current:
                entries.append(current)
            current = {}
        elif "=" in line and not line.startswith("#"):
            key, _, val = line.partition("=")
            current[key.strip()] = val.strip().strip('"')
    if current:
        entries.append(current)
    return [(e["variant"], e["sts2_id"]) for e in entries if "variant" in e and "sts2_id" in e]


# ── Text cleaning ─────────────────────────────────────────────────────────────

def sanitize(text: str) -> str:
    if not text:
        return ""
    # Strip "text = " prefix from get_property return value
    if text.startswith("text = "):
        text = text[7:]
    # Replace energy image path
    n = text.count(ENERGY_PATH)
    if n:
        text = text.replace(ENERGY_PATH * n, "[E]" * n)
    text = text.replace(ENERGY_PATH, "[E]")
    # Replace [img][E][/img] shorthand
    text = re.sub(r"\[img\]\[E\]\[/img\]", "[E]", text)
    # Strip BBCode formatting but keep content
    text = re.sub(r"\[center\]|\[/center\]|\[gold\]|\[/gold\]|\[b\]|\[/b\]", "", text)
    # Strip trailing tooltip "(keyword explanation)" on its own line
    text = re.sub(r"\n\([^)]+\)$", "", text.strip())
    return text.strip()


# ── sts2 CLI wrapper ──────────────────────────────────────────────────────────

def sts2(*args: str) -> str:
    env = {**os.environ, "STS2_BRIDGE_HOST": BRIDGE_HOST}
    result = subprocess.run(
        ["sts2", *args],
        capture_output=True, text=True, timeout=10, env=env,
    )
    return result.stdout.strip()


def get_hand_size() -> int:
    out = sts2("combat")
    m = re.search(r"^hand\[(\d+)\]", out, re.MULTILINE)
    return int(m.group(1)) if m else 0


def get_desc(sts2_id: str) -> str:
    path = f"{HAND_BASE}/NHandCardHolder-CARD_{sts2_id}/Card/CardContainer/DescriptionLabel"
    raw = explorer_get_property(path, "text")
    return sanitize(raw)


# ── Main scrape loop ──────────────────────────────────────────────────────────

def scrape(card_ids: list[tuple[str, str]], results: dict, verbose: bool = True) -> dict:
    hand_size = get_hand_size()
    if verbose:
        print(f"Hand size before scraping: {hand_size}")

    for i, (variant, sts2_id) in enumerate(card_ids):
        if sts2_id in results:
            if verbose:
                print(f"[{i+1}/{len(card_ids)}] {variant} — skip")
            continue

        if verbose:
            print(f"[{i+1}/{len(card_ids)}] {variant} ({sts2_id})", end="  ", flush=True)

        # Add card, retry description up to 3x with increasing delay
        sts2("dev", "card", sts2_id, "hand")
        time.sleep(0.8)

        base = ""
        for attempt in range(3):
            base = get_desc(sts2_id)
            if base:
                break
            time.sleep(0.6 * (attempt + 1))

        # Upgrade and read upgraded description
        sts2("dev", "upgrade", str(hand_size))
        time.sleep(0.5)
        upgraded = get_desc(sts2_id)

        # Remove card
        sts2("dev", "remove", sts2_id, "hand")
        time.sleep(0.3)

        if verbose:
            print(f"base={base[:60]!r}" + ("..." if len(base) > 60 else ""))
            if upgraded != base:
                print(f"  {'':>{len(variant)+len(sts2_id)+6}}upg ={upgraded[:60]!r}")
            if not base:
                print(f"  WARNING: empty description for {sts2_id}")

        results[sts2_id] = {"base": base, "upgraded": upgraded}

    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--missing-only", action="store_true",
                        help="Only scrape cards absent from the output file")
    parser.add_argument("--card", metavar="STS2_ID",
                        help="Scrape a single card by its STS2 ID (e.g. SHOCKWAVE)")
    parser.add_argument("--out", type=Path, default=OUT_FILE,
                        help=f"Output JSON file (default: {OUT_FILE})")
    args = parser.parse_args()

    all_cards = load_card_ids()
    print(f"Loaded {len(all_cards)} cards from {CARDS_TOML.name}")

    # Load existing results
    results: dict = {}
    if args.out.exists():
        with open(args.out) as f:
            results = json.load(f)
        print(f"Loaded {len(results)} existing entries from {args.out.name}")

    if args.card:
        target = [(v, s) for v, s in all_cards if s == args.card.upper()]
        if not target:
            print(f"ERROR: {args.card!r} not found in {CARDS_TOML.name}")
            sys.exit(1)
        # Force re-scrape even if already present
        results.pop(args.card.upper(), None)
        to_scrape = target
    elif args.missing_only:
        to_scrape = [(v, s) for v, s in all_cards if s not in results or not results[s].get("base")]
        print(f"Cards to scrape: {len(to_scrape)}")
    else:
        to_scrape = all_cards

    results = scrape(to_scrape, results)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"\nSaved {len(results)} entries to {args.out}")


if __name__ == "__main__":
    main()
