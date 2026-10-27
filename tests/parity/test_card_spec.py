from __future__ import annotations

import pytest

from sts_sim import card_spec
from sts_sim.sim._registry import CARD_ROWS


def test_base_and_upgraded_card() -> None:
    bash = card_spec("Bash")
    assert bash["cost"] == 2
    assert bash["targeted"] is True
    assert bash["card_type"] == "Attack"
    assert bash["rarity"] == "Starter"
    assert bash["keywords"] == []
    assert bash["description"] == "Deal 8 damage.\nApply 2 Vulnerable."
    assert bash["effects"] == [
        {"op": "DealDamage", "amount": 8},
        {"op": "ApplyStatusToTarget", "status": "Vulnerable", "amount": None},
        {"op": "ApplyStatusToTarget", "status": "Vulnerable", "amount": None},
    ]
    upgraded = card_spec("Bash+")
    assert upgraded["name"] == "Bash+"
    assert upgraded["effects"][0] == {"op": "DealDamage", "amount": 10}
    assert len(upgraded["effects"]) == 4


def test_amount_statuses_carry_their_number() -> None:
    assert card_spec("Inflame")["effects"] == [
        {"op": "ApplyStatusToSelf", "status": "Strength", "amount": 2}
    ]


def test_scaled_ops_name_their_source() -> None:
    (op,) = card_spec("BodySlam")["effects"]
    assert op == {
        "op": "DealDamageScaled",
        "base": 0,
        "per_unit": 1,
        "source": "CurrentBlock",
    }


def test_keywords_are_sorted() -> None:
    assert card_spec("Dazed")["keywords"] == ["Ethereal", "Unplayable"]


def test_accepts_sts2_ids() -> None:
    assert card_spec("STRIKE_IRONCLAD")["name"] == "Strike"


def test_unknown_card_fails() -> None:
    with pytest.raises(ValueError, match="unknown card: Nope"):
        card_spec("Nope")


@pytest.mark.parametrize("display", [row["display"] for row in CARD_ROWS])
def test_every_registry_card_has_a_spec(display: str) -> None:
    for name in (display, display + "+"):
        spec = card_spec(name)
        assert {
            "cost",
            "targeted",
            "card_type",
            "rarity",
            "keywords",
            "description",
            "effects",
        } <= set(spec)
