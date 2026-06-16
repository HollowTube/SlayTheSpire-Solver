from enum import Enum
from typing import cast

from . import CombatState, Monster


class CardName(str, Enum):
    """Canonical card names — one per card implemented in `cards.rs`'s
    `card_data()`. A `str` subclass, so these compare equal to (and can be
    used wherever code expects) the plain string names — e.g.
    `CardName.STRIKE == "Strike"` is true and hashes the same. Exists so
    deck-building code (e.g. IRONCLAD_STARTING_DECK, `sts_sim.bench.PRESETS`)
    gets autocomplete/typo-checking instead of repeating string literals."""

    AGGRESSION = "Aggression"
    ASHEN_STRIKE = "AshenStrike"
    BARRICADE = "Barricade"
    BASH = "Bash"
    BLOODLETTING = "Bloodletting"
    BLOOD_WALL = "BloodWall"
    BLUDGEON = "Bludgeon"
    BODY_SLAM = "BodySlam"
    BREAK = "Break"
    BULLY = "Bully"
    BURNING_PACT = "BurningPact"
    CINDER = "Cinder"
    COLOSSUS = "Colossus"
    CONFLAGRATION = "Conflagration"
    CORRUPTION = "Corruption"
    CRIMSON_MANTLE = "CrimsonMantle"
    CRUELTY = "Cruelty"
    DARK_EMBRACE = "DarkEmbrace"
    DAZED = "Dazed"
    DEFEND = "Defend"
    DEMON_FORM = "DemonForm"
    DISMANTLE = "Dismantle"
    DOMINATE = "Dominate"
    FEEL_NO_PAIN = "FeelNoPain"
    FIEND_FIRE = "FiendFire"
    FLAME_BARRIER = "FlameBarrier"
    HEADBUTT = "Headbutt"
    HEMOKINESIS = "Hemokinesis"
    IMPERVIOUS = "Impervious"
    INFERNAL_BLADE = "InfernalBlade"
    INFERNO = "Inferno"
    INFLAME = "Inflame"
    IRON_WAVE = "Iron Wave"
    JUGGERNAUT = "Juggernaut"
    MANGLE = "Mangle"
    MOLTEN_FIST = "MoltenFist"
    NOT_YET = "NotYet"
    OFFERING = "Offering"
    ONE_TWO_PUNCH = "OneTwoPunch"
    PERFECTED_STRIKE = "PerfectedStrike"
    POMMEL_STRIKE = "Pommel Strike"
    RAGE = "Rage"
    SECOND_WIND = "SecondWind"
    SHRUG_IT_OFF = "ShrugItOff"
    SLIMED = "Slimed"
    SPITE = "Spite"
    STRIKE = "Strike"
    SWORD_BOOMERANG = "Sword Boomerang"
    TAUNT = "Taunt"
    TEAR_ASUNDER = "TearAsunder"
    WOUND = "Wound"
    THRASH = "Thrash"
    THUNDERCLAP = "Thunderclap"
    TREMBLE = "Tremble"
    TRUE_GRIT = "TrueGrit"
    TWIN_STRIKE = "TwinStrike"
    UPPERCUT = "Uppercut"
    FIGHT_ME = "FightMe!"
    PYRE = "Pyre"
    ANGER = "Anger"
    DRUM_OF_BATTLE = "DrumOfBattle"
    STOMP = "Stomp"


class MonsterName(str, Enum):
    """Canonical monster names used as `Monster(name=...)` below. A `str`
    subclass, so these compare equal to (and can be used wherever code
    expects) the plain string names — e.g. `state.monsters[0].name ==
    MonsterName.JAW_WORM` and `MonsterName.JAW_WORM == "Jaw Worm"` are both
    true. Exists so scenario authors get autocomplete/typo-checking instead
    of repeating string literals."""

    JAW_WORM = "Jaw Worm"
    GREMLIN_NOB = "Gremlin Nob"
    NIBBIT = "Nibbit"
    FUZZY_WURM_CRAWLER = "Fuzzy Wurm Crawler"
    TWIG_SLIME_S = "Twig Slime (S)"
    SHRINKER_BEETLE = "Shrinker Beetle"
    LEAF_SLIME_S = "Leaf Slime (S)"
    LEAF_SLIME_M = "Leaf Slime (M)"
    TWIG_SLIME_M = "Twig Slime (M)"
    BYRDONIS = "Byrdonis"
    INKLET = "Inklet"
    VANTOM = "Vantom"
    SNAPPING_JAXFRUIT = "Snapping Jaxfruit"
    AXE_RUBY_RAIDER = "Axe Ruby Raider"
    ASSASSIN_RUBY_RAIDER = "Assassin Ruby Raider"
    BRUTE_RUBY_RAIDER = "Brute Ruby Raider"
    CROSSBOW_RUBY_RAIDER = "Crossbow Ruby Raider"
    SLITHERING_STRANGLER = "Slithering Strangler"
    CUBEX_CONSTRUCT = "Cubex Construct"
    KIN_PRIEST = "Kin Priest"
    KIN_FOLLOWER = "Kin Follower"


# Per the Slay the Spire wiki, the Ironclad's starting deck is 5 Strike,
# 4 Defend, and 1 Bash — 10 cards total. `CombatState`'s `deck` constructor
# param shuffles this into the draw pile and deals a real opening hand from
# it (see HOL-13), rather than dumping the whole deck into `hand` at once.
IRONCLAD_STARTING_DECK = cast(
    "list[str]", [CardName.STRIKE] * 5 + [CardName.DEFEND] * 4 + [CardName.BASH]
)

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

# Twig Slime (S) HP: 7-11 normal. 11 used as canonical value.
TWIG_SLIME_S_STARTING_HP = 11

# Shrinker Beetle HP: 38-40 normal. 38 used as canonical value.
SHRINKER_BEETLE_STARTING_HP = 38

# Leaf Slime (S) HP: 11-15 normal. 13 used as canonical value.
LEAF_SLIME_S_STARTING_HP = 13

# Leaf Slime (M) HP: 32-35 normal. 33 used as canonical value.
LEAF_SLIME_M_STARTING_HP = 33

# Twig Slime (M) HP: 26-28 normal. 27 used as canonical value.
TWIG_SLIME_M_STARTING_HP = 27

# Byrdonis (Act 1 elite) HP: 81-84 normal. 84 pins the canonical scenario to
# its documented maximum, matching the Gremlin Nob convention.
BYRDONIS_STARTING_HP = 84

# Inklet HP: 11-17 normal. 15 used as canonical value.
INKLET_STARTING_HP = 15

# Vantom (Overgrowth boss) HP: 173 normal, 183 at Ascension 8. 173 used as the
# canonical value, per the existing "pick one documented number" convention.
VANTOM_STARTING_HP = 173

# Snapping Jaxfruit HP range: 31-33 normal. 31 used as canonical (minimum).
SNAPPING_JAXFRUIT_STARTING_HP = 31

# Axe Ruby Raider HP range: 20-22 normal. 20 used as canonical (minimum).
AXE_RUBY_RAIDER_STARTING_HP = 20

# Assassin Ruby Raider HP range: 18-23 normal. 18 used as canonical (minimum).
ASSASSIN_RUBY_RAIDER_STARTING_HP = 18

# Brute Ruby Raider HP range: 30-33 normal. 30 used as canonical (minimum).
BRUTE_RUBY_RAIDER_STARTING_HP = 30

# Crossbow Ruby Raider HP range: 18-21 normal. 18 used as canonical (minimum).
CROSSBOW_RUBY_RAIDER_STARTING_HP = 18

# Slithering Strangler (elite) HP range: 53-55 normal. 54 used as canonical.
SLITHERING_STRANGLER_STARTING_HP = 54
# Cubex Construct (elite) HP: 65 normal. 65 used as canonical.
CUBEX_CONSTRUCT_STARTING_HP = 65

# Kin Priest HP: 190 normal, 199 at Ascension 8. 190 used as canonical.
KIN_PRIEST_STARTING_HP = 190

# Kin Follower HP: 58-59 normal, 62-63 at Ascension 8. 59 used as canonical.
KIN_FOLLOWER_STARTING_HP = 59

# Canonical starting HP keyed by MonsterName value. Used by the server's
# deck_baseline handler to construct a "fresh start" scenario from a named
# monster list, so the benchmark uses canonical HP regardless of what the
# game rolled for this specific encounter.
MONSTER_STARTING_HP: dict[str, int] = {
    MonsterName.JAW_WORM: JAW_WORM_STARTING_HP,
    MonsterName.GREMLIN_NOB: GREMLIN_NOB_STARTING_HP,
    MonsterName.NIBBIT: NIBBIT_STARTING_HP,
    MonsterName.FUZZY_WURM_CRAWLER: FUZZY_WURM_CRAWLER_STARTING_HP,
    MonsterName.TWIG_SLIME_S: TWIG_SLIME_S_STARTING_HP,
    MonsterName.SHRINKER_BEETLE: SHRINKER_BEETLE_STARTING_HP,
    MonsterName.LEAF_SLIME_S: LEAF_SLIME_S_STARTING_HP,
    MonsterName.LEAF_SLIME_M: LEAF_SLIME_M_STARTING_HP,
    MonsterName.TWIG_SLIME_M: TWIG_SLIME_M_STARTING_HP,
    MonsterName.BYRDONIS: BYRDONIS_STARTING_HP,
    MonsterName.INKLET: INKLET_STARTING_HP,
    MonsterName.VANTOM: VANTOM_STARTING_HP,
    MonsterName.SNAPPING_JAXFRUIT: SNAPPING_JAXFRUIT_STARTING_HP,
    MonsterName.AXE_RUBY_RAIDER: AXE_RUBY_RAIDER_STARTING_HP,
    MonsterName.ASSASSIN_RUBY_RAIDER: ASSASSIN_RUBY_RAIDER_STARTING_HP,
    MonsterName.BRUTE_RUBY_RAIDER: BRUTE_RUBY_RAIDER_STARTING_HP,
    MonsterName.CROSSBOW_RUBY_RAIDER: CROSSBOW_RUBY_RAIDER_STARTING_HP,
    MonsterName.SLITHERING_STRANGLER: SLITHERING_STRANGLER_STARTING_HP,
    MonsterName.CUBEX_CONSTRUCT: CUBEX_CONSTRUCT_STARTING_HP,
    MonsterName.KIN_PRIEST: KIN_PRIEST_STARTING_HP,
    MonsterName.KIN_FOLLOWER: KIN_FOLLOWER_STARTING_HP,
}


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
        monsters=[
            Monster(hp=GREMLIN_NOB_STARTING_HP, attack=0, name=MonsterName.GREMLIN_NOB)
        ],
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
        monsters=[Monster(hp=NIBBIT_STARTING_HP, attack=0, name=MonsterName.NIBBIT)],
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
                hp=FUZZY_WURM_CRAWLER_STARTING_HP,
                attack=0,
                name=MonsterName.FUZZY_WURM_CRAWLER,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_twig_slime_s(seed, deck=None):
    """Ironclad's starting loadout against a Twig Slime (S) — the simplest of
    the Act 1 "easy pool" slimes: a single repeating Tackle for 4 damage."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=TWIG_SLIME_S_STARTING_HP, attack=0, name=MonsterName.TWIG_SLIME_S
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_shrinker_beetle(seed, deck=None):
    """Ironclad's starting loadout against a Shrinker Beetle. Its opening
    Shrink permanently reduces the player's outgoing damage by 30%, then it
    alternates Chomp (7 damage) / Stomp (13 damage) forever."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=SHRINKER_BEETLE_STARTING_HP,
                attack=0,
                name=MonsterName.SHRINKER_BEETLE,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_leaf_slime_s(seed, deck=None):
    """Ironclad's starting loadout against a Leaf Slime (S), which alternates
    Tackle (3 damage) / Goop (sticks a "Slimed" card into the player's deck)
    forever."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=LEAF_SLIME_S_STARTING_HP, attack=0, name=MonsterName.LEAF_SLIME_S
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_leaf_slime_m(seed, deck=None):
    """Ironclad's starting loadout against a Leaf Slime (M), which strictly
    alternates StickyShot (two "Slimed" cards) / ClumpShot (8 damage)
    forever."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=LEAF_SLIME_M_STARTING_HP, attack=0, name=MonsterName.LEAF_SLIME_M
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_twig_slime_m(seed, deck=None):
    """Ironclad's starting loadout against a Twig Slime (M). Opens with
    StickyShot (one "Slimed" card), then rolls ClumpShot (11 damage, 67%) /
    StickyShot (33%) forever, never repeating StickyShot consecutively."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=TWIG_SLIME_M_STARTING_HP, attack=0, name=MonsterName.TWIG_SLIME_M
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_slimes_weak(seed, deck=None):
    """Ironclad's starting loadout against the Act 1 "SlimesWeak" easy-pool
    encounter: three slimes, one medium and two small. The real encounter
    randomly picks the medium slime from {Leaf Slime (M), Twig Slime (M)} and
    the two smalls from {Leaf Slime (S), Twig Slime (S)} (one of each); this
    fixes one representative composition (Leaf Slime (M) + Leaf Slime (S) +
    Twig Slime (S)) as the canonical multi-enemy scenario. See
    `ironclad_starter_deck_vs_slimes_weak_twig` for the other possible medium
    slime roll."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=LEAF_SLIME_M_STARTING_HP, attack=0, name=MonsterName.LEAF_SLIME_M
            ),
            Monster(
                hp=LEAF_SLIME_S_STARTING_HP, attack=0, name=MonsterName.LEAF_SLIME_S
            ),
            Monster(
                hp=TWIG_SLIME_S_STARTING_HP, attack=0, name=MonsterName.TWIG_SLIME_S
            ),
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_slimes_weak_twig(seed, deck=None):
    """Ironclad's starting loadout against the other possible composition of
    the Act 1 "SlimesWeak" easy-pool encounter: Twig Slime (M) as the medium
    slime, alongside Leaf Slime (S) and Twig Slime (S). See
    `ironclad_starter_deck_vs_slimes_weak` for the Leaf Slime (M) variant."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=TWIG_SLIME_M_STARTING_HP, attack=0, name=MonsterName.TWIG_SLIME_M
            ),
            Monster(
                hp=LEAF_SLIME_S_STARTING_HP, attack=0, name=MonsterName.LEAF_SLIME_S
            ),
            Monster(
                hp=TWIG_SLIME_S_STARTING_HP, attack=0, name=MonsterName.TWIG_SLIME_S
            ),
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
        monsters=[Monster(hp=JAW_WORM_STARTING_HP, name=MonsterName.JAW_WORM)],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_byrdonis(seed, deck=None):
    """Ironclad's starting loadout against Byrdonis (Act 1 elite). Opens
    with Swoop (17 damage) and alternates with Peck (3 hits of 3 damage)
    forever; Territorial 1 grants it +1 Strength at the end of every one of
    its turns, so both moves get steadily harder the longer the fight runs."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(hp=BYRDONIS_STARTING_HP, attack=0, name=MonsterName.BYRDONIS)
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_vantom(seed, deck=None):
    """Ironclad's starting loadout against Vantom (Overgrowth boss): a fixed
    4-move cycle (Ink Blot -> Inky Lance -> Dismember -> Prepare -> repeat),
    starting with 9 stacks of Slippery."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=VANTOM_STARTING_HP,
                name=MonsterName.VANTOM,
                statuses=[("Slippery", 9)],
            ),
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_snapping_jaxfruit(seed, deck=None):
    """Ironclad's starting loadout against a Snapping Jaxfruit (Overgrowth
    normal). Single move "Energy Orb" forever: deal 3 damage, gain 2 Strength
    — gets progressively harder as Strength accumulates."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=SNAPPING_JAXFRUIT_STARTING_HP,
                attack=0,
                name=MonsterName.SNAPPING_JAXFRUIT,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_axe_ruby_raider(seed, deck=None):
    """Ironclad's starting loadout against an Axe Ruby Raider (Overgrowth
    normal). Fixed 3-move cycle: Swing 1 (5 damage + 5 block) → Swing 2
    (5 damage + 5 block) → Big Swing (12 damage) → repeat."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=AXE_RUBY_RAIDER_STARTING_HP,
                attack=0,
                name=MonsterName.AXE_RUBY_RAIDER,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_assassin_ruby_raider(seed, deck=None):
    """Ironclad's starting loadout against an Assassin Ruby Raider (Overgrowth
    normal). Single move "Killshot" forever: deal 11 damage."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=ASSASSIN_RUBY_RAIDER_STARTING_HP,
                attack=0,
                name=MonsterName.ASSASSIN_RUBY_RAIDER,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_brute_ruby_raider(seed, deck=None):
    """Ironclad's starting loadout against a Brute Ruby Raider (Overgrowth
    normal). Fixed 2-move cycle: Beat (7 damage) → Roar (gain 3 Strength)
    → repeat. Strength accumulates, making each Beat hit harder over time."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=BRUTE_RUBY_RAIDER_STARTING_HP,
                attack=0,
                name=MonsterName.BRUTE_RUBY_RAIDER,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_crossbow_ruby_raider(seed, deck=None):
    """Ironclad's starting loadout against a Crossbow Ruby Raider (Overgrowth
    normal). Fixed alternating cycle: Reload (gain 3 block) ↔ Fire (14
    damage), opens with Reload."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=CROSSBOW_RUBY_RAIDER_STARTING_HP,
                attack=0,
                name=MonsterName.CROSSBOW_RUBY_RAIDER,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_slithering_strangler(seed, deck=None):
    """Ironclad's starting loadout against Slithering Strangler (Overgrowth
    elite). Opens with Constrict (applies 3 stacks), then alternates
    Thwack/Lash (random 50/50) with Constrict reapplied every other turn.
    Constrict stacks accumulate (3, 6, 9, ...), dealing escalating end-of-turn
    unblockable damage."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=SLITHERING_STRANGLER_STARTING_HP,
                attack=0,
                name=MonsterName.SLITHERING_STRANGLER,
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_cubex_construct(seed, deck=None):
    """Ironclad's starting loadout against Cubex Construct (Overgrowth elite).
    Starts with 13 block and 1 Artifact. Fixed cycle: Charge Up (opening) →
    Repeater Blast → Repeater Blast → Expel Blast → repeat."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=CUBEX_CONSTRUCT_STARTING_HP,
                name=MonsterName.CUBEX_CONSTRUCT,
                block=13,
                statuses=[("Artifact", 1)],
            )
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_ruby_raiders(seed, deck=None):
    """Ironclad's starting loadout against the "RubyRaidersNormal" Overgrowth
    encounter: pick 3 of the 5 Ruby Raider types. This canonical scenario
    uses Axe + Assassin + Crossbow."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=AXE_RUBY_RAIDER_STARTING_HP,
                attack=0,
                name=MonsterName.AXE_RUBY_RAIDER,
            ),
            Monster(
                hp=ASSASSIN_RUBY_RAIDER_STARTING_HP,
                attack=0,
                name=MonsterName.ASSASSIN_RUBY_RAIDER,
            ),
            Monster(
                hp=CROSSBOW_RUBY_RAIDER_STARTING_HP,
                attack=0,
                name=MonsterName.CROSSBOW_RUBY_RAIDER,
            ),
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_inklet(seed, deck=None):
    """Ironclad's starting loadout against a single Inklet (an Overgrowth
    enemy that can also appear alone), starting with one stack of Slippery.
    Opens with Jab; alternates Piercing Gaze/Windup Punch (50/50) until
    returning to Jab."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=INKLET_STARTING_HP,
                name=MonsterName.INKLET,
                statuses=[("Slippery", 1)],
            ),
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_the_kin(seed, deck=None):
    """Ironclad's starting loadout against The Kin (Overgrowth boss): Kin Priest
    + 2x Kin Follower. Kin Priest follows a fixed 4-move cycle (Orb of Frailty
    -> Orb of Weakness -> Soul Beam -> Dark Ritual -> repeat). Each Follower
    follows an independent fixed 3-move cycle (Quick Slash -> Boomerang ->
    Power Dance -> repeat), with offset openers: Follower A opens on Quick
    Slash, Follower B on Power Dance. Both Followers start with
    Status::Minion { leader: "Kin Priest" } — they flee (stop acting) once
    the Priest is dead."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(hp=KIN_PRIEST_STARTING_HP, name=MonsterName.KIN_PRIEST),
            Monster(
                hp=KIN_FOLLOWER_STARTING_HP,
                name=MonsterName.KIN_FOLLOWER,
                intent="Quick Slash",
                statuses=[("Minion", 0)],
            ),
            Monster(
                hp=KIN_FOLLOWER_STARTING_HP,
                name=MonsterName.KIN_FOLLOWER,
                intent="Power Dance",
                statuses=[("Minion", 0)],
            ),
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )


def ironclad_starter_deck_vs_inklets(seed, deck=None):
    """Ironclad's starting loadout against the Overgrowth "Inklet" encounter:
    three Inklets, each starting with one stack of Slippery. Per the wiki,
    the middle Inklet always opens with Windup Punch (2 damage x3); the two
    outer Inklets open with Jab (3 damage, most likely) or Windup Punch -
    `opening_intent` has no RNG access (matching every other monster's fixed
    opener), so Jab is picked deterministically for the outer two, and the
    middle one's opening intent is overridden explicitly."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monsters=[
            Monster(
                hp=INKLET_STARTING_HP,
                name=MonsterName.INKLET,
                statuses=[("Slippery", 1)],
            ),
            Monster(
                hp=INKLET_STARTING_HP,
                name=MonsterName.INKLET,
                statuses=[("Slippery", 1)],
                intent="Windup Punch",
            ),
            Monster(
                hp=INKLET_STARTING_HP,
                name=MonsterName.INKLET,
                statuses=[("Slippery", 1)],
            ),
        ],
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
    )
