"""Compare a game fixture entry with the sim's ``card_spec``.

Each game var is read off the sim's effects by a ``Probe``: a default per var
kind, or a per-card entry in ``CARD_PROBES`` (named vars, and cards the sim
models with a different op). ``UNMAPPED`` marks a var the sim can't expose
(e.g. a binary status whose number lives in the engine's reaction); those
cards are checked for text only.
"""

from __future__ import annotations

import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any

Effects = Sequence[Mapping[str, Any]]

_DAMAGE_OPS = (
    "DealDamage",
    "DealDamageToAllEnemies",
    "DealDamageToRandomEnemy",
    "DealDamageRepeated",
)
_STATUS_OPS = ("ApplyStatusToSelf", "ApplyStatusToTarget")
_COUNTED_STATUSES = ("Vulnerable", "Weak")


@dataclass(frozen=True)
class Probe:
    describe: str
    read: Callable[[Effects], Any] | None  # None: unmapped


UNMAPPED = Probe("unmapped", None)


def _one(values: list[int]) -> Any:
    distinct = sorted(set(values))
    if not distinct:
        return None
    return distinct[0] if len(distinct) == 1 else tuple(distinct)


def field(op: str, key: str) -> Probe:
    return Probe(f"{op}.{key}", lambda fx: _one([e[key] for e in fx if e["op"] == op]))


def fields(*pairs: tuple[str, str]) -> Probe:
    def read(fx: Effects) -> Any:
        return _one([e[k] for op, k in pairs for e in fx if e["op"] == op])

    return Probe(" | ".join(f"{op}.{k}" for op, k in pairs), read)


def count_status(status: str, op: str = "ApplyStatusToTarget") -> Probe:
    def read(fx: Effects) -> Any:
        n = sum(1 for e in fx if e["op"] == op and e["status"] == status)
        return n or None

    return Probe(f"count of {op}({status})", read)


class _Binary:
    def __repr__(self) -> str:
        return "<binary status>"


# A status op was found but carries no amount: the var is unmapped.
BINARY = _Binary()


def status_amount(status: str) -> Probe:
    def read(fx: Effects) -> Any:
        amounts = [
            e["amount"] for e in fx if e["op"] in _STATUS_OPS and e["status"] == status
        ]
        if amounts and all(a is None for a in amounts):
            return BINARY
        return _one([a for a in amounts if a is not None])

    return Probe(f"{status} amount", read)


def negated(probe: Probe) -> Probe:
    assert probe.read is not None
    inner = probe.read

    def read(fx: Effects) -> Any:
        v = inner(fx)
        return -v if isinstance(v, int) else v

    return Probe(f"-({probe.describe})", read)


def _damage(fx: Effects) -> Any:
    return _one([e["amount"] for e in fx if e["op"] in _DAMAGE_OPS])


def _hits(fx: Effects) -> Any:
    repeated = [e for e in fx if e["op"] == "DealDamageRepeated"]
    if repeated:
        # Binary sources (e.g. "lost HP this turn") reach their full count.
        return _one([e["hits_base"] + e["hits_per_unit"] for e in repeated])
    n = sum(1 for e in fx if e["op"] in _DAMAGE_OPS)
    return n or None


DAMAGE = Probe("every damage op amount", _damage)
BLOCK = fields(("GainBlock", "amount"), ("GainBlockScaled", "base"))
CARDS = field("DrawCards", "amount")
ENERGY = fields(("GainEnergy", "amount"), ("GainEnergyScaled", "per_unit"))
HP_LOSS = field("LoseHp", "amount")
HEAL = field("Heal", "amount")
CALC_BASE = fields(("DealDamageScaled", "base"), ("GainBlockScaled", "base"))
CALC_EXTRA = fields(("DealDamageScaled", "per_unit"), ("GainBlockScaled", "per_unit"))
HITS = Probe("hit count", _hits)


def default_probe(name: str, var: Mapping[str, Any]) -> Probe:
    cls = var["class"]
    if "formula_sha256" in var:
        return UNMAPPED
    m = re.fullmatch(r"PowerVar<(\w+)Power>", cls)
    if m:
        status = m.group(1)
        if status in _COUNTED_STATUSES:
            return count_status(status)
        return status_amount(status)
    return {
        "DamageVar": DAMAGE,
        "BlockVar": BLOCK,
        "CardsVar": CARDS,
        "EnergyVar": ENERGY,
        "HpLossVar": HP_LOSS,
        "HealVar": HEAL,
        "CalculationBaseVar": CALC_BASE,
        "ExtraDamageVar": CALC_EXTRA,
        "CalculationExtraVar": CALC_EXTRA,
        "RepeatVar": HITS,
    }.get(cls, UNMAPPED)


def _uppercut(fx: Effects) -> Any:
    weak = count_status("Weak").read
    vuln = count_status("Vulnerable").read
    assert weak and vuln
    return _one([v for v in (weak(fx), vuln(fx)) if v is not None])


# The number lives in the engine's reaction for these, so there is nothing
# on the card's effects to compare.
def _in_engine(why: str) -> Probe:
    return Probe(f"unmapped: {why}", None)


# Per-card overrides, keyed by game class then var name. Informed by each
# card's template, e.g. Uppercut "Apply {Power} Weak. Apply {Power} Vulnerable".
CARD_PROBES: dict[str, dict[str, Probe]] = {
    "Colossus": {"Colossus": status_amount("Colossus")},
    "CrimsonMantle": {
        "CrimsonMantlePower": _in_engine(
            "the status amount is the HP-loss counter; Block is in the reaction"
        )
    },
    "DemonForm": {
        "StrengthPower": _in_engine("binary status; Strength is in the reaction")
    },
    "Dominate": {
        "StrengthPerVulnerable": _in_engine(
            "GainStrengthEqualToTargetVulnerable is 1 per stack"
        )
    },
    "FiendFire": {"Damage": field("DealDamageScaled", "per_unit")},
    "FightMe": {
        "StrengthPower": Probe(
            "self Strength amount",
            lambda fx: _one(
                [
                    e["amount"]
                    for e in fx
                    if e["op"] == "ApplyStatusToSelf" and e["status"] == "Strength"
                ]
            ),
        ),
        "EnemyStrength": Probe(
            "target Strength amount",
            lambda fx: _one(
                [
                    e["amount"]
                    for e in fx
                    if e["op"] == "ApplyStatusToTarget" and e["status"] == "Strength"
                ]
            ),
        ),
    },
    "Mangle": {"StrengthLoss": negated(status_amount("Strength"))},
    "OneTwoPunch": {"Attacks": count_status("OneTwoPunch", "ApplyStatusToSelf")},
    "PactsEnd": {"Cards": _in_engine("the Exhaust Pile threshold is a play condition")},
    "Pyre": {"Energy": _in_engine("binary status; energy is in the reaction")},
    "SecondWind": {"Block": field("ExhaustAllFromHand", "gain_block_per_card")},
    "SetupStrike": {"StrengthPower": status_amount("StrengthThisTurn")},
    "TearAsunder": {
        # Hits = CalculationBase + CalculationExtra * (1 + times HP was lost);
        # Repeat is declared but not read by the card.
        "Repeat": _in_engine("declared but unused by the card"),
        "CalculationBase": _in_engine("folded into DealDamageRepeated.hits_base"),
        "CalculationExtra": field("DealDamageRepeated", "hits_per_unit"),
    },
    "Uppercut": {"Power": Probe("count of Weak and of Vulnerable", _uppercut)},
    "Vicious": {"Cards": status_amount("Vicious")},
}


@dataclass(frozen=True)
class Formula:
    """A calculated var, described by hand; the game's source is never stored."""

    formula: str
    scale_source: str | None  # the sim's ScaleSource, None if not modelled
    sha256: str  # the fixture's formula_sha256 this entry was written against


FORMULAS: dict[str, Formula] = {
    "AshenStrike": Formula(
        "base + per_unit × cards in the Exhaust Pile",
        "ExhaustPileSize",
        "821e8724e5d358d38be7c1ac68138ba7f8ca271e553dc53e3719dc148c126640",
    ),
    "BodySlam": Formula(
        "damage = current Block",
        "CurrentBlock",
        "f801b3eddf3e0747c8bf12e13fbd1b4d7ac2c2045c375351ce8ce503815dbc21",
    ),
    "Bully": Formula(
        "base + per_unit × Vulnerable on the target",
        "VulnerableStacksOnTarget",
        "3f026a337a8aa08303c63821650920c24fd52a0f88bc013f468dfa3151e1ae19",
    ),
    "ExpectAFight": Formula(
        "energy = Attacks in Hand",
        None,
        "e496879d47c8dbb7b591cfe53a4dc5411fb22e6ef5e15fae917d5fef4c47f92c",
    ),
    "PerfectedStrike": Formula(
        "base + per_unit × Strike-tagged cards in all piles",
        "StrikeCardsInDeck",
        "a104403b3ad5f43c85d8194baee079218d4384a78da372b0bb6828f79f3ae319",
    ),
    "TearAsunder": Formula(
        "hits = 1 + times the player lost HP this combat",
        "DamageTakenThisCombat",
        "50e27420ded39cc2741438f317d2b2aeb199e1525e8d41b043d25a48e43bfea8",
    ),
}

# (game class, field) -> follow-up issue. A listed pair must still fail.
KNOWN_FAILURES: dict[tuple[str, str], str] = {
    ("OneTwoPunch", "upgraded.var.Attacks"): "HOL-116",
    ("DrumOfBattle", "type"): "HOL-117",
    ("DrumOfBattle", "var.Energy"): "HOL-117",
    ("DrumOfBattle", "upgraded.var.Energy"): "HOL-117",
}

_TARGETED = {
    "AnyEnemy": True,
    "Self": False,
    "AllEnemies": False,
    "AnyAlly": False,
    "None": False,
}


@dataclass(frozen=True)
class Mismatch:
    card: str
    field: str
    game: Any
    sim: Any

    def __str__(self) -> str:
        return f"{self.card}: {self.field}: game {self.game!r} != sim {self.sim!r}"


def probe_for(card: str, name: str, var: Mapping[str, Any]) -> Probe:
    return CARD_PROBES.get(card, {}).get(name) or default_probe(name, var)


def normalise_text(text: str) -> str:
    text = text.replace("“", '"').replace("”", '"').replace("‘", "'").replace("’", "'")
    lines = [re.sub(r"[ \t]+", " ", line).strip() for line in text.split("\n")]
    return "\n".join(lines).strip()


def _rarity(game: str) -> str:
    return "Starter" if game == "Basic" else game


def _formula_mismatches(
    entry: Mapping[str, Any], spec: Mapping[str, Any]
) -> list[Mismatch]:
    card = entry["class"]
    out = []
    for name, var in entry["vars"].items():
        if "formula_sha256" not in var or card not in FORMULAS:
            continue
        f = FORMULAS[card]
        if f.sha256 != var["formula_sha256"]:
            out.append(
                Mismatch(
                    card,
                    f"formula.{name}",
                    "formula changed in game: re-check the description",
                    f.formula,
                )
            )
        sources = sorted(
            {
                e["source"]
                for e in spec["effects"]
                if "source" in e and e["op"] != "PlayTopOfDeckScaled"
            }
        )
        sim = sources[0] if len(sources) == 1 else (tuple(sources) or None)
        if sim != f.scale_source:
            out.append(
                Mismatch(card, f"formula.{name}.scale_source", f.scale_source, sim)
            )
    return out


def compare(
    entry: Mapping[str, Any], spec: Mapping[str, Any], spec_upgraded: Mapping[str, Any]
) -> list[Mismatch]:
    """Every field where the sim disagrees with the game fixture entry."""
    card = entry["class"]
    out: list[Mismatch] = []

    def check(field_name: str, game: Any, sim: Any) -> None:
        if game != sim:
            out.append(Mismatch(card, field_name, game, sim))

    x = entry["x_cost"]
    check("cost", -1 if x else entry["cost"], spec["cost"])
    check("upgraded.cost", -1 if x else entry["upgraded_cost"], spec_upgraded["cost"])
    check("type", entry["type"], spec["card_type"])
    check("rarity", _rarity(entry["rarity"]), spec["rarity"])
    targeted = _TARGETED.get(entry["target"])
    if targeted is not None:
        check("targeted", targeted, spec["targeted"])
    elif entry["target"] != "RandomEnemy":
        out.append(Mismatch(card, "targeted", entry["target"], spec["targeted"]))
    check("keywords", entry["keywords"], spec["keywords"])
    check("upgraded.keywords", entry["upgraded_keywords"], spec_upgraded["keywords"])
    check(
        "text",
        normalise_text(entry["text"]["base"]),
        normalise_text(spec["description"]),
    )
    check(
        "upgraded.text",
        normalise_text(entry["text"]["upgraded"]),
        normalise_text(spec_upgraded["description"]),
    )
    if spec["effects"]:
        for name, var in entry["vars"].items():
            probe = probe_for(card, name, var)
            if probe.read is None:
                continue
            base, upgraded = (
                probe.read(spec["effects"]),
                probe.read(spec_upgraded["effects"]),
            )
            if base is BINARY or upgraded is BINARY:
                continue
            check(f"var.{name}", var["base"], base)
            check(f"upgraded.var.{name}", var["upgraded"], upgraded)
        out += _formula_mismatches(entry, spec)
    return out


def check(
    entry: Mapping[str, Any],
    spec: Mapping[str, Any],
    spec_upgraded: Mapping[str, Any],
    known: Mapping[tuple[str, str], str] = KNOWN_FAILURES,
) -> tuple[list[Mismatch], list[str]]:
    """Unexpected mismatches, and known failures for this card that now pass."""
    found = compare(entry, spec, spec_upgraded)
    unexpected = [m for m in found if (m.card, m.field) not in known]
    fields = {m.field for m in found}
    stale = [
        f"{card}: {f} matches now; remove it from KNOWN_FAILURES ({issue})"
        for (card, f), issue in sorted(known.items())
        if card == entry["class"] and f not in fields
    ]
    return unexpected, stale


def unmapped_vars(entry: Mapping[str, Any], spec: Mapping[str, Any]) -> list[str]:
    """Non-formula vars the sim's numbers can't be checked against."""
    out = []
    for name, var in entry["vars"].items():
        if "formula_sha256" in var:
            continue
        probe = probe_for(entry["class"], name, var)
        if probe.read is None or probe.read(spec["effects"]) is BINARY:
            out.append(name)
    return out
