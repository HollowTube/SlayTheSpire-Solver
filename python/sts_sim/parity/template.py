"""Render STS2 card description templates (a SmartFormat subset) to plain text.

Renders the out-of-combat card-library view: ``InCombat`` takes its false
branch, calculated vars show their base value, energy icons become ``[E]``
and markup tags such as ``[gold]`` are stripped.
"""

from __future__ import annotations

import re
from collections.abc import Iterable, Mapping
from dataclasses import dataclass

ENERGY_ICON = "[E]"

# CardKeywordOrder: each "before" keyword is inserted at index 0 in this
# order, so the rendered order is the reverse.
KEYWORDS_BEFORE = ("Ethereal", "Sly", "Retain", "Innate", "Unplayable")
KEYWORDS_AFTER = ("Exhaust", "Eternal")

_TAG = re.compile(r"\[/?[a-z_]+(?:=[^\]]*)?\]")
_FORMATTER = re.compile(r"([A-Za-z]+)(?:\(([^)]*)\))?(?=[:}])")
_CONDITION = re.compile(r"^(>=|<=|!=|>|<|=)(-?\d+)\?")


class TemplateError(Exception):
    pass


@dataclass(frozen=True)
class _Placeholder:
    selector: str
    formatter: str | None
    options: str
    format: str | None


def _matching_brace(text: str, start: int) -> int:
    depth = 0
    for i in range(start, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    raise TemplateError(f"unbalanced braces in {text!r}")


def _split_top(text: str, sep: str) -> list[str]:
    parts: list[str] = []
    cur: list[str] = []
    depth = 0
    for ch in text:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == sep and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return parts


def _parse_placeholder(body: str) -> _Placeholder:
    selector, colon, rest = body.partition(":")
    if not colon:
        return _Placeholder(selector, None, "", None)
    # A formatter name is only a name when ':' or the end follows it; otherwise
    # the text is an implicit conditional's format (e.g. `{InCombat:Gain...|}`).
    m = _FORMATTER.match(rest + "}")
    if m:
        name, options = m.group(1), m.group(2) or ""
        after = rest[m.end() :]
        fmt = after[1:] if after.startswith(":") else None
        return _Placeholder(selector, name, options, fmt)
    return _Placeholder(selector, None, "", rest)


def _num(value: int | float) -> str:
    return str(int(value))


def _energy_icons(n: int) -> str:
    return ENERGY_ICON * n if 0 < n < 4 else f"{n}{ENERGY_ICON}"


def _cond(value: float, fmt: str, render_part: _RenderPart) -> str:
    parts = _split_top(fmt, "|")
    for part in parts[:-1] if len(parts) > 1 else parts:
        m = _CONDITION.match(part)
        if not m:
            raise TemplateError(f"cond option {part!r} has no comparison")
        op, rhs = m.group(1), int(m.group(2))
        hit = {
            ">": value > rhs,
            "<": value < rhs,
            ">=": value >= rhs,
            "<=": value <= rhs,
            "=": value == rhs,
            "!=": value != rhs,
        }[op]
        if hit:
            return render_part(part[m.end() :])
    return render_part(parts[-1]) if len(parts) > 1 else ""


class _RenderPart:
    def __init__(self, vars: Mapping[str, int | float], upgraded: bool, card: str):
        self.vars = vars
        self.upgraded = upgraded
        self.card = card

    def __call__(self, text: str) -> str:
        out: list[str] = []
        i = 0
        while i < len(text):
            ch = text[i]
            if ch == "{":
                end = _matching_brace(text, i)
                out.append(self.placeholder(_parse_placeholder(text[i + 1 : end])))
                i = end + 1
            elif ch == "}":
                raise TemplateError(f"{self.card}: stray '}}' in template")
            else:
                out.append(ch)
                i += 1
        return "".join(out)

    def fail(self, what: str) -> TemplateError:
        return TemplateError(f"{self.card}: {what}")

    def value(self, selector: str) -> int | float:
        if selector not in self.vars:
            raise self.fail(f"unknown var {selector!r}")
        return self.vars[selector]

    def placeholder(self, p: _Placeholder) -> str:
        if p.selector == "IfUpgraded":
            if p.formatter != "show" or p.format is None:
                raise self.fail(
                    f"IfUpgraded needs the 'show' formatter, got {p.formatter!r}"
                )
            options = _split_top(p.format, "|")
            if len(options) > 2:
                raise self.fail("IfUpgraded:show takes at most 2 options")
            chosen = (
                options[0]
                if self.upgraded
                else (options[1] if len(options) > 1 else "")
            )
            return self(chosen)
        if p.selector == "InCombat":
            if p.formatter is not None or p.format is None:
                raise self.fail(
                    f"unsupported InCombat form (formatter {p.formatter!r})"
                )
            options = _split_top(p.format, "|")
            return self(options[1]) if len(options) > 1 else ""
        if p.selector == "energyPrefix":
            if p.formatter != "energyIcons" or not p.options.strip().isdigit():
                raise self.fail("energyPrefix needs energyIcons(<n>)")
            return _energy_icons(int(p.options))
        if p.formatter is None:
            if p.format is not None:
                raise self.fail(
                    f"unsupported formatter for {p.selector!r}: {p.format!r}"
                )
            return _num(self.value(p.selector))
        value = self.value(p.selector)
        if p.formatter == "diff":
            return _num(value)
        if p.formatter == "energyIcons":
            return _energy_icons(int(value))
        if p.formatter == "plural":
            options = _split_top(p.format or "", "|")
            if len(options) != 2:
                raise self.fail(f"plural needs one|many, got {p.format!r}")
            return self(options[0] if value == 1 else options[1])
        if p.formatter == "cond":
            return _cond(value, p.format or "", self)
        raise self.fail(f"unknown formatter {p.formatter!r}")


def strip_tags(text: str) -> str:
    return _TAG.sub("", text)


def render(
    template: str,
    vars: Mapping[str, int | float],
    upgraded: bool,
    *,
    card: str = "<card>",
) -> str:
    """The template's text with ``vars`` filled in. Pure: no game state."""
    return strip_tags(_RenderPart(vars, upgraded, card)(template))


def with_keywords(
    body: str, keywords: Iterable[str], keyword_titles: Mapping[str, str]
) -> str:
    """Add the keyword lines the game shows around a card's description."""
    have = set(keywords)
    lines = [body]
    for kw in KEYWORDS_BEFORE:
        if kw in have:
            lines.insert(0, f"{keyword_titles[kw]}.")
    lines += [f"{keyword_titles[kw]}." for kw in KEYWORDS_AFTER if kw in have]
    return "\n".join(line for line in lines if line)
