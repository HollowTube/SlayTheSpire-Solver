"""Coverage of the parity check: which cards are checked how far. Never fails."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from typing import Any

from .mapping import FORMULAS, KNOWN_FAILURES, unmapped_vars

Spec = Callable[[str], Mapping[str, Any]]

BUCKETS = (
    "fully checked",
    "text only (var not mapped)",
    "stubs (no sim effects; metadata and text only)",
    "behaviour upgrades (IsUpgraded branches)",
    "formula vars (checked by hash and ScaleSource)",
    "pool cards missing from the registry",
    "known failures",
)


def registry_join(fixture: Mapping[str, Any]) -> tuple[dict[str, str], list[str]]:
    """fixture key -> sim display name, and the pool keys with no registry row."""
    from ..sim._registry import CARD_ROWS

    by_sts2 = {row["sts2_id"]: row["display"] for row in CARD_ROWS}
    joined = {key: by_sts2[key] for key in fixture["cards"] if key in by_sts2}
    missing = sorted(key for key in fixture["cards"] if key not in by_sts2)
    return joined, missing


def coverage(fixture: Mapping[str, Any], card_spec: Spec) -> dict[str, list[str]]:
    joined, missing = registry_join(fixture)
    out: dict[str, list[str]] = {b: [] for b in BUCKETS}
    for key, entry in sorted(fixture["cards"].items()):
        card = entry["class"]
        if entry["uses_is_upgraded"]:
            note = "" if entry["has_on_upgrade"] else " (no OnUpgrade)"
            out[BUCKETS[3]].append(card + note)
        formulas = [n for n, v in entry["vars"].items() if "formula_sha256" in v]
        if formulas:
            f = FORMULAS.get(card)
            desc = f"{f.formula} -> {f.scale_source}" if f else "no mapping entry"
            out[BUCKETS[4]].append(f"{card}.{formulas[0]}: {desc}")
        if key not in joined:
            continue
        spec = card_spec(joined[key])
        if not spec["effects"]:
            out[BUCKETS[2]].append(card)
            continue
        unmapped = unmapped_vars(entry, spec)
        if unmapped:
            out[BUCKETS[1]].append(f"{card} ({', '.join(unmapped)})")
        else:
            out[BUCKETS[0]].append(card)
    out[BUCKETS[5]] = [fixture["cards"][k]["class"] for k in missing]
    out[BUCKETS[6]] = [
        f"{c}.{f} -> {issue}" for (c, f), issue in sorted(KNOWN_FAILURES.items())
    ]
    return out


def format_report(buckets: Mapping[str, list[str]], title: str) -> str:
    lines = [title]
    for name, cards in buckets.items():
        lines.append(f"  {name}: {len(cards)}")
        lines += [f"    {c}" for c in cards]
    return "\n".join(lines)
