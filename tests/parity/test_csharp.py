"""Extractor tests. All C# here is synthetic, written in the decompile's shapes."""

from __future__ import annotations

from pathlib import Path

import pytest

from sts_sim.parity.csharp import (
    ExtractError,
    VarClasses,
    extract_card,
    formula_sha256,
    load_typed_accessors,
    load_var_classes,
    pool_cards,
    slugify,
)

VAR_CLASSES = {
    "DamageVar.cs": """
public class DamageVar : DynamicVar
{
    public DamageVar(decimal damage, ValueProp props) : base("Damage", damage) { }
    public DamageVar(string name, decimal damage, ValueProp props) : base(name, damage) { }
}""",
    "PowerVar.cs": """
public class PowerVar<T> : DynamicVar where T : PowerModel
{
    public PowerVar(decimal powerAmount) : base(typeof(T).Name, powerAmount) { }
}""",
    "EnergyVar.cs": """
public class EnergyVar : DynamicVar
{
    public EnergyVar(int energy) : this("Energy", energy) { }
    public EnergyVar(string name, int energy) : base(name, energy) { }
}""",
    "CalculatedVar.cs": """
public class CalculatedVar : DynamicVar
{
    public CalculatedVar(string name) : base(name, 0m) { }
}""",
    "CalculatedDamageVar.cs": """
public class CalculatedDamageVar : CalculatedVar
{
    public CalculatedDamageVar(ValueProp props) : base("CalculatedDamage") { }
}""",
    "CalculationBaseVar.cs": """
public class CalculationBaseVar : DynamicVar
{
    public CalculationBaseVar(decimal baseValue) : base("CalculationBase", baseValue) { }
}""",
    "BoolVar.cs": """
public class BoolVar : DynamicVar
{
    public BoolVar(string name, bool value) : base(name, Convert.ToDecimal(value)) { }
}""",
    "DynamicVarSet.cs": """
public class DynamicVarSet
{
    public DamageVar Damage => (DamageVar)_vars["Damage"];
    public PowerVar<VulnerablePower> Vulnerable => (PowerVar<VulnerablePower>)_vars["VulnerablePower"];
}""",
}


@pytest.fixture
def var_dir(tmp_path: Path) -> Path:
    for name, src in VAR_CLASSES.items():
        (tmp_path / name).write_text(src)
    return tmp_path


@pytest.fixture
def classes(var_dir: Path) -> VarClasses:
    return load_var_classes(var_dir)


@pytest.fixture
def accessors(var_dir: Path) -> dict[str, str]:
    return load_typed_accessors(var_dir / "DynamicVarSet.cs")


def card(
    *,
    vars: str = "new DamageVar(8m, ValueProp.Move)",
    ctor: str = "base(2, CardType.Attack, CardRarity.Basic, TargetType.AnyEnemy)",
    upgrade: str = "base.DynamicVars.Damage.UpgradeValueBy(2m);",
    extra: str = "",
) -> str:
    return f"""
namespace MegaCrit.Sts2.Core.Models.Cards;
public sealed class Foo : CardModel
{{
    private const string _key = "Power";
    protected override IEnumerable<DynamicVar> CanonicalVars =>
        new global::_003C_003Ez__ReadOnlyArray<DynamicVar>(new DynamicVar[1] {{ {vars} }});
    {extra}
    public Foo() : {ctor} {{ }}
    protected override void OnUpgrade()
    {{
        {upgrade}
    }}
}}"""


def test_reads_constructor_vars_and_upgrades(
    classes: VarClasses, accessors: dict[str, str]
) -> None:
    entry = extract_card(card(), "Foo", classes, accessors)
    assert (entry["cost"], entry["type"], entry["rarity"], entry["target"]) == (
        2,
        "Attack",
        "Basic",
        "AnyEnemy",
    )
    assert entry["vars"] == {
        "Damage": {"class": "DamageVar", "base": 8, "upgraded": 10}
    }
    assert entry["has_on_upgrade"] and not entry["uses_is_upgraded"]


def test_var_names_from_typed_named_and_generic_constructors(
    classes: VarClasses, accessors: dict[str, str]
) -> None:
    src = card(
        vars='new PowerVar<VulnerablePower>(2m), new DamageVar("Hit", 3m, ValueProp.Move), '
        'new EnergyVar(1), new DynamicVar("Other", 4m), new DynamicVar(_key, 5m)',
        upgrade="base.DynamicVars.Vulnerable.UpgradeValueBy(1m);\n"
        'base.DynamicVars["Hit"].UpgradeValueBy(1m);\n'
        "base.DynamicVars[_key].UpgradeValueBy(2m);\n"
        "base.EnergyCost.UpgradeBy(-1);\n"
        "AddKeyword(CardKeyword.Innate);",
    )
    entry = extract_card(src, "Foo", classes, accessors)
    assert {n: (v["base"], v["upgraded"]) for n, v in entry["vars"].items()} == {
        "VulnerablePower": (2, 3),
        "Hit": (3, 4),
        "Energy": (1, 1),
        "Other": (4, 4),
        "Power": (5, 7),
    }
    assert entry["upgraded_cost"] == 1
    assert entry["upgraded_keywords"] == ["Innate"]


def test_formula_is_hashed_never_stored(
    classes: VarClasses, accessors: dict[str, str]
) -> None:
    lam = "(CardModel card, Creature? _) => card.Owner.Creature.Block"
    src = card(
        vars=f"new CalculationBaseVar(6m), new CalculatedDamageVar(ValueProp.Move).WithMultiplier({lam})",
        upgrade="",
    )
    var = extract_card(src, "Foo", classes, accessors)["vars"]["CalculatedDamage"]
    assert var == {
        "class": "CalculatedDamageVar",
        "formula_sha256": formula_sha256(lam),
        "base": 6,
        "upgraded": 6,
    }
    assert "Block" not in str(var)


def test_keywords_and_x_cost(classes: VarClasses, accessors: dict[str, str]) -> None:
    src = card(
        extra="public override IEnumerable<CardKeyword> CanonicalKeywords => "
        "new global::_003C_003Ez__ReadOnlyArray<CardKeyword>(new CardKeyword[2] "
        "{ CardKeyword.Exhaust, CardKeyword.Ethereal });\n"
        "protected override bool HasEnergyCostX => true;",
        upgrade="RemoveKeyword(CardKeyword.Exhaust);",
    )
    entry = extract_card(src, "Foo", classes, accessors)
    assert entry["keywords"] == ["Ethereal", "Exhaust"]
    assert entry["upgraded_keywords"] == ["Ethereal"]
    assert entry["x_cost"] is True


def test_is_upgraded_branch_is_flagged(
    classes: VarClasses, accessors: dict[str, str]
) -> None:
    src = card(extra="private bool Big => base.IsUpgraded;")
    assert extract_card(src, "Foo", classes, accessors)["uses_is_upgraded"] is True


@pytest.mark.parametrize(
    ("kwargs", "construct"),
    [
        (
            {"upgrade": "base.DynamicVars.Damage.SetValue(2m);"},
            "unrecognised OnUpgrade statement",
        ),
        (
            {"upgrade": "if (x) { base.EnergyCost.UpgradeBy(-1); }"},
            "unrecognised OnUpgrade statement",
        ),
        (
            {"vars": "new BlockVar(5m, ValueProp.Move)"},
            "unrecognised var constructor BlockVar",
        ),
        (
            {"vars": 'new BoolVar("Flag", true)'},
            "unrecognised BoolVar constructor argument",
        ),
        ({"vars": "MakeVar()"}, "unrecognised var construct"),
        (
            {"ctor": "base(2, CardType.Attack, CardRarity.Basic)"},
            "unrecognised constructor shape",
        ),
        (
            {
                "ctor": "base(Cost(), CardType.Attack, CardRarity.Basic, TargetType.AnyEnemy)"
            },
            "constructor cost",
        ),
        (
            {"upgrade": "base.DynamicVars.Block.UpgradeValueBy(1m);"},
            "unknown DynamicVars accessor 'Block'",
        ),
        (
            {"upgrade": 'base.DynamicVars["Nope"].UpgradeValueBy(1m);'},
            "upgrades unknown var 'Nope'",
        ),
    ],
)
def test_unknown_shapes_fail_naming_card_and_construct(
    classes: VarClasses,
    accessors: dict[str, str],
    kwargs: dict[str, str],
    construct: str,
) -> None:
    with pytest.raises(ExtractError, match=r"^Foo: ") as err:
        extract_card(card(**kwargs), "Foo", classes, accessors)
    assert construct in str(err.value)


def test_unknown_shape_names_the_line(
    classes: VarClasses, accessors: dict[str, str]
) -> None:
    src = card(upgrade="Frobnicate();")
    with pytest.raises(ExtractError, match=r"\(line \d+\)"):
        extract_card(src, "Foo", classes, accessors)


def test_pool_lists_cards_in_order() -> None:
    src = """
public sealed class RedPool : CardPoolModel
{
    protected override CardModel[] GenerateAllCards()
    {
        return new CardModel[2] { ModelDb.Card<Bash>(), ModelDb.Card<Anger>() };
    }
}"""
    assert pool_cards(src, "RedPool") == ["Bash", "Anger"]


@pytest.mark.parametrize(
    ("name", "slug"),
    [
        ("ExpectAFight", "EXPECT_A_FIGHT"),
        ("StrikeIronclad", "STRIKE_IRONCLAD"),
        ("OneTwoPunch", "ONE_TWO_PUNCH"),
        ("Bash", "BASH"),
        ("ABCard", "A_B_CARD"),
    ],
)
def test_slugify_matches_the_game(name: str, slug: str) -> None:
    assert slugify(name) == slug
