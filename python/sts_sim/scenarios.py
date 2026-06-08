from . import CombatState

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


def ironclad_starter_deck_vs_jaw_worm(seed):
    """The fixed canonical M1 scenario (HOL-5/HOL-10/HOL-11) named in the M1
    exit criteria: Ironclad's starting loadout against a single Jaw Worm,
    whose AI-driven move pool (HOL-11) replaces the placeholder monster's
    flat attack without changing the player's loadout."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monster_hp=JAW_WORM_STARTING_HP,
        # monster_attack is meaningless for an AI-driven monster — its move
        # pool decides what it does each turn — but the constructor still
        # requires a value, so this is just a placeholder.
        monster_attack=0,
        seed=seed,
        deck=list(IRONCLAD_STARTING_DECK),
        monster_name="Jaw Worm",
    )
