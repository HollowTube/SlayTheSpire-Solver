"""Read card data out of decompiled STS2 C# (``CardModel`` subclasses).

Uses a real C# parser (tree-sitter) and recognises a closed set of shapes.
Anything else raises ``ExtractError`` naming the card, the construct and the
line, so a game patch that changes a card's shape fails loudly instead of
producing wrong numbers.
"""

from __future__ import annotations

import hashlib
import re
from collections.abc import Iterator, Mapping
from dataclasses import dataclass, field
from decimal import Decimal
from pathlib import Path
from typing import Any

import tree_sitter_c_sharp
from tree_sitter import Language, Node, Parser

_LANGUAGE = Language(tree_sitter_c_sharp.language())
_DYNAMIC_VAR = "DynamicVar"
_CALCULATED_VAR = "CalculatedVar"
_CALCULATION_BASE = "CalculationBase"
_WRAPPERS = ("ReadOnlyArray", "ReadOnlySingleElementList")


class ExtractError(Exception):
    pass


def _parse(source: str | bytes) -> Node:
    data = source.encode() if isinstance(source, str) else source
    return Parser(_LANGUAGE).parse(data).root_node


def _text(node: Node) -> str:
    assert node.text is not None
    return node.text.decode()


def _named(node: Node) -> list[Node]:
    return [c for c in node.children if c.is_named and c.type != "comment"]


def _field(node: Node, name: str) -> Node:
    child = node.child_by_field_name(name)
    if child is None:
        line = node.start_point[0] + 1
        raise ExtractError(f"line {line}: {node.type} has no {name}")
    return child


def _name_of(node: Node) -> str | None:
    child = node.child_by_field_name("name")
    return _text(child) if child is not None else None


def _walk(node: Node) -> Iterator[Node]:
    yield node
    for child in node.children:
        yield from _walk(child)


def slugify(name: str) -> str:
    """Port of the game's ``StringHelper.Slugify`` (``ExpectAFight`` -> ``EXPECT_A_FIGHT``)."""
    out = []
    text = name.strip()
    for i, ch in enumerate(text):
        if (
            i > 0
            and "A" <= ch <= "Z"
            and text[i - 1].isascii()
            and text[i - 1].isalnum()
        ):
            out.append("_")
        out.append(ch)
    upper = re.sub(r"\s+", "_", "".join(out).upper())
    return re.sub(r"[^A-Z0-9_]", "", upper)


def formula_sha256(lambda_source: str) -> str:
    return hashlib.sha256(" ".join(lambda_source.split()).encode()).hexdigest()


def _number(text: str) -> int:
    value = Decimal(text.rstrip("mMfFdD"))
    if value != value.to_integral_value():
        raise ValueError(f"non-integer value {text}")
    return int(value)


# --------------------------------------------------------------------------
# DynamicVar classes: default names, named overloads, inheritance
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class _Ctor:
    params: tuple[str, ...]
    name_arg: str  # "lit:<name>" | "param:<i>" | "generic"
    value_arg: str | None  # "lit:<n>" | "param:<i>" | None


@dataclass
class VarClasses:
    ctors: dict[str, list[_Ctor]] = field(default_factory=dict)
    parents: dict[str, str] = field(default_factory=dict)

    def is_calculated(self, cls: str) -> bool:
        while cls:
            if cls == _CALCULATED_VAR:
                return True
            cls = self.parents.get(cls, "")
        return False


def _initializer_arg(node: Node, params: tuple[str, ...]) -> str:
    src = _text(node)
    if node.type == "string_literal":
        return "lit:" + src.strip('"')
    if node.type in ("integer_literal", "real_literal"):
        return f"lit:{_number(src)}"
    if node.type == "identifier" and src in params:
        return f"param:{params.index(src)}"
    if (
        node.type == "member_access_expression"
        and src.replace(" ", "") == "typeof(T).Name"
    ):
        return "generic"
    # Only an error if a card actually calls this constructor.
    return f"bad:{src}"


def load_var_classes(dynamic_vars_dir: str | Path) -> VarClasses:
    """Constructors of every ``DynamicVar`` subclass in ``DynamicVars/*.cs``."""
    out = VarClasses()
    for path in sorted(Path(dynamic_vars_dir).glob("*.cs")):
        root = _parse(path.read_bytes())
        for cls in (n for n in _walk(root) if n.type == "class_declaration"):
            name = _text(_field(cls, "name"))
            bases = [c for c in cls.children if c.type == "base_list"]
            if bases:
                first = _named(bases[0])[0]
                parent = _text(first)
                out.parents[name] = parent.split("<")[0]
            ctors: list[_Ctor] = []
            for ctor in (n for n in _walk(cls) if n.type == "constructor_declaration"):
                params = tuple(
                    _text(_field(p, "name"))
                    for p in _named(_field(ctor, "parameters"))
                    if p.type == "parameter"
                )
                init = next(
                    (c for c in ctor.children if c.type == "constructor_initializer"),
                    None,
                )
                if init is None:
                    continue
                args = [
                    _named(a)[0]
                    for a in _named(_named(init)[-1])
                    if a.type == "argument"
                ]
                name_arg = _initializer_arg(args[0], params) if args else "bad:<none>"
                value_arg = _initializer_arg(args[1], params) if len(args) > 1 else None
                ctors.append(_Ctor(params, name_arg, value_arg))
            if ctors:
                out.ctors[name] = ctors
    return out


def load_typed_accessors(dynamic_var_set: str | Path) -> dict[str, str]:
    """``DynamicVarSet``'s typed accessors, e.g. ``Vulnerable`` -> ``VulnerablePower``."""
    root = _parse(Path(dynamic_var_set).read_bytes())
    out = {}
    for prop in (n for n in _walk(root) if n.type == "property_declaration"):
        keys = [n for n in _walk(prop) if n.type == "string_literal"]
        if len(keys) == 1:
            out[_text(_field(prop, "name"))] = _text(keys[0]).strip('"')
    return out


# --------------------------------------------------------------------------
# One card
# --------------------------------------------------------------------------


class _Card:
    def __init__(self, source: str | bytes, class_name: str):
        self.name = class_name
        root = _parse(source)
        found = [
            n
            for n in _walk(root)
            if n.type == "class_declaration" and _name_of(n) == class_name
        ]
        if len(found) != 1:
            raise ExtractError(f"{class_name}: class declaration not found")
        self.cls = found[0]
        self.members = _named(_field(self.cls, "body"))
        self.consts: dict[str, str] = {}
        for m in self.members:
            if m.type == "field_declaration" and any(
                _text(c) == "const" for c in m.children
            ):
                for decl in (n for n in _walk(m) if n.type == "variable_declarator"):
                    lits = [n for n in _walk(decl) if n.type == "string_literal"]
                    ident = decl.child_by_field_name("name") or _named(decl)[0]
                    if lits:
                        self.consts[_text(ident)] = _text(lits[0]).strip('"')

    def fail(self, node: Node | None, what: str) -> ExtractError:
        where = f" (line {node.start_point[0] + 1})" if node is not None else ""
        return ExtractError(f"{self.name}: {what}{where}")

    def member(self, kind: str, name: str) -> Node | None:
        hits = [m for m in self.members if m.type == kind and _name_of(m) == name]
        if len(hits) > 1:
            raise self.fail(hits[1], f"more than one {name}")
        return hits[0] if hits else None

    def arrow_value(self, prop: Node) -> Node:
        arrow = next(
            (c for c in prop.children if c.type == "arrow_expression_clause"), None
        )
        if arrow is None:
            raise self.fail(
                prop, f"property {_name_of(prop)} is not an expression body"
            )
        return _named(arrow)[0]

    def collection_items(self, expr: Node, what: str) -> list[Node]:
        """Elements of `new ReadOnlyArray<T>(new T[n] {...})` / `new ReadOnlySingleElementList<T>(x)`."""
        if expr.type != "object_creation_expression" or not any(
            w in _text(_field(expr, "type")) for w in _WRAPPERS
        ):
            raise self.fail(
                expr, f"unrecognised {what} collection {_text(expr)[:60]!r}"
            )
        items = [_named(a)[0] for a in _named(_field(expr, "arguments"))]
        if len(items) != 1:
            raise self.fail(expr, f"unrecognised {what} collection arity")
        item = items[0]
        if item.type == "array_creation_expression":
            init = next(
                (c for c in item.children if c.type == "initializer_expression"), None
            )
            if init is None:
                raise self.fail(item, f"{what} array has no initializer")
            return _named(init)
        return [item]

    # -- constructor ---------------------------------------------------------

    def base_ctor(self) -> dict[str, Any]:
        ctors = [m for m in self.members if m.type == "constructor_declaration"]
        if len(ctors) != 1:
            raise self.fail(None, f"expected 1 constructor, found {len(ctors)}")
        init = next(
            (c for c in ctors[0].children if c.type == "constructor_initializer"), None
        )
        if init is None or not _text(init).lstrip(": \t\n").startswith("base"):
            raise self.fail(ctors[0], "constructor has no base(...) initializer")
        args = [_named(a)[0] for a in _named(_named(init)[-1])]
        if len(args) != 4:
            raise self.fail(
                init,
                f"unrecognised constructor shape: base() with {len(args)} arguments",
            )
        cost, ctype, rarity, target = args
        try:
            cost_value = _number(_text(cost).replace(" ", ""))
        except (ValueError, ArithmeticError):
            raise self.fail(
                cost, f"unrecognised constructor cost {_text(cost)!r}"
            ) from None
        out: dict[str, Any] = {"cost": cost_value}
        for key, node, enum in (
            ("type", ctype, "CardType"),
            ("rarity", rarity, "CardRarity"),
            ("target", target, "TargetType"),
        ):
            m = re.fullmatch(rf"{enum}\.(\w+)", _text(node))
            if not m:
                raise self.fail(node, f"unrecognised constructor {key} {_text(node)!r}")
            out[key] = m.group(1)
        return out

    def energy_cost_x(self) -> bool:
        prop = self.member("property_declaration", "HasEnergyCostX")
        if prop is None:
            return False
        value = _text(self.arrow_value(prop))
        if value not in ("true", "false"):
            raise self.fail(prop, f"unrecognised HasEnergyCostX value {value!r}")
        return value == "true"

    # -- keywords ------------------------------------------------------------

    def keyword(self, node: Node) -> str:
        m = re.fullmatch(r"CardKeyword\.(\w+)", _text(node))
        if node.type != "member_access_expression" or not m:
            raise self.fail(node, f"unrecognised keyword {_text(node)!r}")
        return m.group(1)

    def canonical_keywords(self) -> list[str]:
        prop = self.member("property_declaration", "CanonicalKeywords")
        if prop is None:
            return []
        return [
            self.keyword(n)
            for n in self.collection_items(self.arrow_value(prop), "CanonicalKeywords")
        ]

    # -- vars ----------------------------------------------------------------

    def literal(self, node: Node) -> str | int:
        src = _text(node)
        if node.type == "string_literal":
            return src.strip('"')
        if node.type == "identifier" and src in self.consts:
            return self.consts[src]
        try:
            return _number(src.replace(" ", ""))
        except (ValueError, ArithmeticError):
            raise self.fail(node, f"unrecognised literal {src!r}") from None

    def var(self, item: Node, classes: VarClasses) -> dict[str, Any]:
        lambda_src = None
        creation: Node | None = item
        if item.type == "invocation_expression":
            fn = _field(item, "function")
            if (
                fn.type != "member_access_expression"
                or _name_of(fn) != "WithMultiplier"
            ):
                raise self.fail(
                    item, f"unrecognised var construct {_text(item)[:60]!r}"
                )
            args = _named(_field(item, "arguments"))
            if len(args) != 1 or _named(args[0])[0].type != "lambda_expression":
                raise self.fail(item, "WithMultiplier takes one lambda")
            lambda_src = _text(_named(args[0])[0])
            creation = fn.child_by_field_name("expression")
        if creation is None or creation.type != "object_creation_expression":
            raise self.fail(item, f"unrecognised var construct {_text(item)[:60]!r}")
        type_src = _text(_field(creation, "type"))
        cls, _, generic = type_src.partition("<")
        generic = generic.rstrip(">")
        arg_nodes = [_named(a)[0] for a in _named(_field(creation, "arguments"))]
        value: str | int | None
        if cls == _DYNAMIC_VAR:
            if len(arg_nodes) != 2:
                raise self.fail(creation, "DynamicVar takes (name, value)")
            name, value = self.literal(arg_nodes[0]), self.literal(arg_nodes[1])
        else:
            ctors = [
                c for c in classes.ctors.get(cls, []) if len(c.params) == len(arg_nodes)
            ]
            if len(ctors) != 1:
                raise self.fail(
                    creation,
                    f"unrecognised var constructor {type_src}({len(arg_nodes)} args)",
                )
            ctor = ctors[0]

            def resolve(spec: str) -> str | int:
                kind, _, rest = spec.partition(":")
                if kind == "lit":
                    return int(rest) if re.fullmatch(r"-?\d+", rest) else rest
                if kind == "param":
                    return self.literal(arg_nodes[int(rest)])
                if kind == "bad":
                    raise self.fail(
                        item, f"unrecognised {type_src} constructor argument {rest!r}"
                    )
                if not generic:
                    raise self.fail(item, f"{type_src} needs a generic argument")
                return generic

            name = resolve(ctor.name_arg)
            value = resolve(ctor.value_arg) if ctor.value_arg else None
        if not isinstance(name, str):
            raise self.fail(creation, f"var name is not a string: {name!r}")
        calculated = classes.is_calculated(cls)
        if calculated != (lambda_src is not None):
            raise self.fail(
                creation,
                f"{type_src} {'needs' if calculated else 'cannot take'} WithMultiplier",
            )
        out: dict[str, Any] = {"class": type_src}
        if calculated:
            out["formula_sha256"] = formula_sha256(lambda_src or "")
        elif not isinstance(value, int):
            raise self.fail(creation, f"var {name} has no numeric value")
        else:
            out["base"] = value
        return {"name": name, **out}

    def canonical_vars(self, classes: VarClasses) -> dict[str, dict[str, Any]]:
        prop = self.member("property_declaration", "CanonicalVars")
        if prop is None:
            return {}
        out: dict[str, dict[str, Any]] = {}
        for item in self.collection_items(self.arrow_value(prop), "CanonicalVars"):
            var = self.var(item, classes)
            name = var.pop("name")
            if name in out:
                raise self.fail(item, f"duplicate var {name}")
            out[name] = var
        return out

    # -- OnUpgrade -----------------------------------------------------------

    def upgrades(self, accessors: Mapping[str, str]) -> list[dict[str, Any]] | None:
        method = self.member("method_declaration", "OnUpgrade")
        if method is None:
            return None
        body = method.child_by_field_name("body")
        if body is None or body.type != "block":
            raise self.fail(method, "OnUpgrade has no block body")
        out = []
        for stmt in _named(body):
            inv = _named(stmt)[0] if stmt.type == "expression_statement" else None
            if inv is None or inv.type != "invocation_expression":
                raise self.fail(
                    stmt, f"unrecognised OnUpgrade statement {_text(stmt)!r}"
                )
            out.append(self.upgrade_call(inv, accessors))
        return out

    def upgrade_call(self, inv: Node, accessors: Mapping[str, str]) -> dict[str, Any]:
        src = re.sub(r"\s+", "", _text(inv))
        args = _named(_field(inv, "arguments"))
        arg = _named(args[0])[0] if len(args) == 1 else None
        m = re.fullmatch(r"(?:base\.)?(Add|Remove)Keyword\((.*)\)", src)
        if m and arg is not None:
            return {"op": m.group(1).lower() + "_keyword", "keyword": self.keyword(arg)}
        if re.fullmatch(r"base\.EnergyCost\.UpgradeBy\(.*\)", src) and arg is not None:
            return {"op": "cost", "by": self.literal(arg)}
        m = re.fullmatch(r"base\.DynamicVars\.(\w+)\.UpgradeValueBy\(.*\)", src)
        if m and arg is not None:
            if m.group(1) not in accessors:
                raise self.fail(inv, f"unknown DynamicVars accessor {m.group(1)!r}")
            return {"op": "var", "var": accessors[m.group(1)], "by": self.literal(arg)}
        m = re.fullmatch(r"base\.DynamicVars\[(.+)\]\.UpgradeValueBy\(.*\)", src)
        if m and arg is not None:
            index = [n for n in _walk(inv) if n.type == "element_access_expression"][0]
            # element_access -> bracketed_argument_list -> argument -> expression
            subscript = _named(index)[-1]
            key = self.literal(_named(_named(subscript)[0])[0])
            if not isinstance(key, str):
                raise self.fail(inv, f"DynamicVars index is not a string: {key!r}")
            return {"op": "var", "var": key, "by": self.literal(arg)}
        raise self.fail(inv, f"unrecognised OnUpgrade statement {_text(inv)!r}")

    def uses_is_upgraded(self) -> bool:
        return any(
            n.type == "identifier" and _text(n) == "IsUpgraded" for n in _walk(self.cls)
        )


def extract_card(
    source: str | bytes,
    class_name: str,
    classes: VarClasses,
    accessors: Mapping[str, str],
) -> dict[str, Any]:
    """The game data of one card class: metadata, vars (base/upgraded), keywords."""
    card = _Card(source, class_name)
    entry: dict[str, Any] = {"class": class_name, **card.base_ctor()}
    entry["x_cost"] = card.energy_cost_x()
    keywords = card.canonical_keywords()
    vars_ = card.canonical_vars(classes)
    upgrades = card.upgrades(accessors)
    cost_up = entry["cost"]
    kw_up = list(keywords)
    deltas: dict[str, int] = {}
    for op in upgrades or []:
        if op["op"] == "cost":
            cost_up += op["by"]
        elif op["op"] == "add_keyword":
            kw_up.append(op["keyword"])
        elif op["op"] == "remove_keyword":
            if op["keyword"] not in kw_up:
                raise card.fail(
                    None, f"RemoveKeyword({op['keyword']}) of a keyword the card lacks"
                )
            kw_up.remove(op["keyword"])
        else:
            if op["var"] not in vars_:
                raise card.fail(None, f"OnUpgrade upgrades unknown var {op['var']!r}")
            deltas[op["var"]] = deltas.get(op["var"], 0) + op["by"]
    for name, var in vars_.items():
        if "formula_sha256" in var:
            continue
        var["upgraded"] = var["base"] + deltas.get(name, 0)
    for name, var in vars_.items():
        if "formula_sha256" in var:
            base_var = vars_.get(_CALCULATION_BASE)
            if base_var is None:
                raise card.fail(
                    None, f"calculated var {name} without a {_CALCULATION_BASE} var"
                )
            if deltas.get(name):
                raise card.fail(None, f"OnUpgrade upgrades calculated var {name}")
            var["base"], var["upgraded"] = base_var["base"], base_var["upgraded"]
    entry["upgraded_cost"] = cost_up
    entry["keywords"] = sorted(keywords)
    entry["upgraded_keywords"] = sorted(kw_up)
    entry["vars"] = vars_
    entry["has_on_upgrade"] = upgrades is not None
    entry["uses_is_upgraded"] = card.uses_is_upgraded()
    return entry


# --------------------------------------------------------------------------
# The card pool
# --------------------------------------------------------------------------


def pool_cards(source: str | bytes, pool_class: str) -> list[str]:
    """Card class names from ``<pool_class>.GenerateAllCards``."""
    card = _Card(source, pool_class)
    method = card.member("method_declaration", "GenerateAllCards")
    if method is None:
        raise ExtractError(f"{pool_class}: no GenerateAllCards")
    names = []
    for n in _walk(method):
        if n.type == "generic_name" and _text(n).startswith("Card<"):
            names.append(_text(n)[len("Card<") : -1])
    if not names:
        raise card.fail(method, "GenerateAllCards lists no ModelDb.Card<T>() entries")
    return names
