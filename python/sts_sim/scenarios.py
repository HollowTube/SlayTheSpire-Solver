from . import CombatState

# Per the Slay the Spire wiki, the Ironclad's starting deck is 5 Strike,
# 4 Defend, and 1 Bash — 10 cards total. There's no draw/discard model here:
# the "deck" below is just the opening hand, dealt once — `EndTurn` never
# draws or reshuffles, so all 10 cards sit in hand from turn one.
IRONCLAD_STARTING_DECK = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]

# Per the Slay the Spire wiki, Burning Blood heals 6 HP at the *end* of
# combat — it has no effect during a fight, so there's nothing for the combat
# sim to model. It's recorded here only so the scenario faithfully matches
# the wiki-documented starting loadout that HOL-10's AC names explicitly.
STARTING_RELICS = ["Burning Blood"]

PLAYER_STARTING_HP = 80


def ironclad_starter_deck_vs_placeholder_monster(seed):
    """The fixed canonical M1 scenario (HOL-5/HOL-10): Ironclad's starting
    loadout against a placeholder monster — HOL-11 swaps the monster for the
    real Jaw Worm once it's implemented, without changing this loadout."""
    return CombatState(
        player_hp=PLAYER_STARTING_HP,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=seed,
        hand=list(IRONCLAD_STARTING_DECK),
    )
