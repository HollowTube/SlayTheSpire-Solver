from . import CombatState, Monster

# Per the Slay the Spire wiki, the Ironclad's starting deck is 5 Strike,
# 4 Defend, and 1 Bash — 10 cards total. `CombatState`'s `deck` constructor
# param shuffles this into the draw pile and deals a real opening hand from
# it (see HOL-13), rather than dumping the whole deck into `hand` at once.
IRONCLAD_STARTING_DECK = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]

# Per the Slay the Spire wiki, Burning Blood heals 6 HP at the *end* of
# combat — it has no effect during a fight, so there's nothing for the combat
# sim to model. It's recorded here only so the scenario faithfully matches
# the wiki-documented starting loadout that HOL-10's AC names explicitly.
STARTING_RELICS = ["Burning Blood"]

PLAYER_STARTING_HP = 80


# Per the Slay the Spire wiki, Jaw Worm's HP ranges 40-44 in Act 1 — 44 pins
# the canonical scenario to its documented maximum.
JAW_WORM_STARTING_HP = 44


# Per the Slay the Spire wiki, Gremlin Nob is an Act 1 elite with 82-86 HP.
# 85 pins the canonical scenario to a challenging but beatable value.
GREMLIN_NOB_STARTING_HP = 85

# Nibbit HP: 42-46 normal, 44-48 on A9. 44 used as canonical value.
NIBBIT_STARTING_HP = 44

# Fuzzy Wurm Crawler HP: 55-57 normal. 56 used as canonical value.
FUZZY_WURM_CRAWLER_STARTING_HP = 56


def ironclad_starter_deck_vs_gremlin_nob(seed, deck=None):
    """Harder canonical scenario: Ironclad's starting loadout against Gremlin
    Nob (Act 1 elite). Nob opens with Bellow (+3 Strength), then alternates
    Rush (14 damage) / Skull Bash (6 damage + permanent Vulnerable) — making
    deck composition measurably more important than Jaw Worm.

    Pass `deck` to override the default IRONCLAD_STARTING_DECK (used by
    benchmarking to test alternative deck configurations).
    """
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[Monster(hp=GREMLIN_NOB_STARTING_HP, attack=0, name="Gremlin Nob")],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_nibbit(seed, deck=None):
    """Ironclad's starting loadout against a Nibbit (fixed cycle: Butt →
    Hesitant Slice → Hiss → repeat). Hiss accumulates Strength indefinitely,
    making the fight increasingly punishing the longer it runs."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[Monster(hp=NIBBIT_STARTING_HP, attack=0, name="Nibbit")],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_fuzzy_wurm_crawler(seed, deck=None):
    """Ironclad's starting loadout against a Fuzzy Wurm Crawler (alternating
    Acid Goop / Inhale). Each Inhale grants +7 Strength, making every
    subsequent Acid Goop hit harder than the last."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=FUZZY_WURM_CRAWLER_STARTING_HP, attack=0, name="Fuzzy Wurm Crawler"
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_jaw_worm(seed, deck=None):
    """The fixed canonical M1 scenario (HOL-5/HOL-10/HOL-11) named in the M1
    exit criteria: Ironclad's starting loadout against a single Jaw Worm,
    whose AI-driven move pool (HOL-11) replaces the placeholder monster's
    flat attack without changing the player's loadout.

    Pass `deck` to override the default IRONCLAD_STARTING_DECK (used by
    benchmarking to test alternative deck configurations).
    """
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        # monster_attack is meaningless for an AI-driven monster — its move
        # pool decides what it does each turn — but Monster's `attack`
        # defaults to 0, so it's simply omitted here.
        monsters=[Monster(hp=JAW_WORM_STARTING_HP, name="Jaw Worm")],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )
