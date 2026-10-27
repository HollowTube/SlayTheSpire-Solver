"""Renderer tests. Templates here are synthetic, written in the game's syntax."""

from __future__ import annotations

import pytest

from sts_sim.parity.template import TemplateError, render, with_keywords

TITLES = {
    k: k
    for k in ("Ethereal", "Sly", "Retain", "Innate", "Unplayable", "Exhaust", "Eternal")
}


def test_diff_prints_the_value() -> None:
    assert (
        render("Deal {Damage:diff()} damage.", {"Damage": 8}, False) == "Deal 8 damage."
    )


def test_bare_placeholder_prints_the_value() -> None:
    assert render("Gain {Block} Block.", {"Block": 5}, False) == "Gain 5 Block."


def test_plural_picks_one_or_many() -> None:
    tpl = "{Repeat:diff()} {Repeat:plural:time|times}"
    assert render(tpl, {"Repeat": 1}, False) == "1 time"
    assert render(tpl, {"Repeat": 4}, False) == "4 times"


def test_energy_icons_repeat_for_one_to_three_else_count() -> None:
    tpl = "Gain {Energy:energyIcons()}."
    assert render(tpl, {"Energy": 1}, False) == "Gain [E]."
    assert render(tpl, {"Energy": 3}, False) == "Gain [E][E][E]."
    assert render(tpl, {"Energy": 4}, False) == "Gain 4[E]."
    assert render(tpl, {"Energy": 0}, False) == "Gain 0[E]."


def test_energy_prefix_placeholder_uses_the_option_count() -> None:
    assert render("Costs 0 {energyPrefix:energyIcons(1)}.", {}, False) == "Costs 0 [E]."


def test_if_upgraded_show() -> None:
    tpl = "Upgrade {IfUpgraded:show:ALL cards|a card}."
    assert render(tpl, {}, False) == "Upgrade a card."
    assert render(tpl, {}, True) == "Upgrade ALL cards."


def test_if_upgraded_show_without_base_branch_renders_nothing() -> None:
    tpl = "top X{IfUpgraded:show:+1} cards"
    assert render(tpl, {}, False) == "top X cards"
    assert render(tpl, {}, True) == "top X+1 cards"


def test_in_combat_renders_the_out_of_combat_branch() -> None:
    tpl = "Deal damage.{InCombat:\n(Deals {CalculatedDamage:diff()} damage)|}"
    assert render(tpl, {"CalculatedDamage": 0}, False) == "Deal damage."
    tpl = "{InCombat:in|out}"
    assert render(tpl, {}, False) == "out"


def test_cond_picks_the_matching_branch() -> None:
    tpl = "next {Attacks:cond:>1?{Attacks:diff()} Attacks are|Attack is} played"
    assert render(tpl, {"Attacks": 1}, False) == "next Attack is played"
    assert render(tpl, {"Attacks": 2}, False) == "next 2 Attacks are played"


def test_nested_placeholders_inside_branches() -> None:
    tpl = "{IfUpgraded:show:{Damage:diff()} {Damage:plural:hit|hits}|none}"
    assert render(tpl, {"Damage": 2}, True) == "2 hits"
    assert render(tpl, {"Damage": 2}, False) == "none"


def test_tags_are_stripped() -> None:
    tpl = "Apply {VulnerablePower:diff()} [gold]Vulnerable[/gold]."
    assert render(tpl, {"VulnerablePower": 2}, False) == "Apply 2 Vulnerable."


def test_unknown_formatter_fails_naming_card_and_formatter() -> None:
    with pytest.raises(TemplateError, match=r"Foo: unknown formatter 'starIcons'"):
        render("{Stars:starIcons()}", {"Stars": 1}, False, card="Foo")


def test_unknown_var_fails_naming_card_and_var() -> None:
    with pytest.raises(TemplateError, match=r"Foo: unknown var 'Damage'"):
        render("{Damage:diff()}", {}, False, card="Foo")


def test_keyword_lines_follow_the_game_order() -> None:
    kws = ["Exhaust", "Ethereal", "Innate", "Retain", "Unplayable", "Eternal", "Sly"]
    assert with_keywords("Body.", kws, TITLES).split("\n") == [
        "Unplayable.",
        "Innate.",
        "Retain.",
        "Sly.",
        "Ethereal.",
        "Body.",
        "Exhaust.",
        "Eternal.",
    ]


def test_keyword_lines_skip_an_empty_body() -> None:
    assert with_keywords("", ["Unplayable"], TITLES) == "Unplayable."
