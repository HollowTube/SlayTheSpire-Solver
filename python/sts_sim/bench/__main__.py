"""CLI entry point: python -m sts_sim.bench

Examples:
    # Quick random-policy sweep of all presets vs Jaw Worm
    python -m sts_sim.bench --policy random --seeds 100

    # MCTS comparison vs Gremlin Nob, 4 parallel workers
    python -m sts_sim.bench --monster gremlin-nob --seeds 50 --workers 4

    # Run a single named preset
    python -m sts_sim.bench --decks starter,add-iron-wave

    # Run a custom deck
    python -m sts_sim.bench --custom "Strike,Strike,Strike,Strike,Strike,Defend,Defend,Defend,Defend,Bash"
"""

import click

from . import _SCENARIOS, PRESETS, compare, run_deck


@click.command()
@click.option(
    "--monster",
    type=click.Choice(list(_SCENARIOS)),
    default="jaw-worm",
    show_default=True,
    help="Enemy to fight.",
)
@click.option(
    "--seeds",
    type=int,
    default=50,
    show_default=True,
    help="Number of independent fights (seeds) per deck.",
)
@click.option(
    "--iterations",
    type=int,
    default=200,
    show_default=True,
    help="MCTS iterations per decision (ignored with --policy random).",
)
@click.option(
    "--workers",
    type=int,
    default=4,
    show_default=True,
    help="Parallel worker processes.",
)
@click.option(
    "--policy",
    type=click.Choice(["mcts", "random"]),
    default="mcts",
    show_default=True,
    help="'mcts' for MCTS-guided play, 'random' for random baseline.",
)
@click.option(
    "--decks",
    default=None,
    help=(
        "Comma-separated preset names to run (default: all presets). "
        f"Available: {', '.join(PRESETS)}."
    ),
)
@click.option(
    "--custom",
    default=None,
    help=(
        "Comma-separated card names for a single custom deck run, "
        "e.g. 'Strike,Strike,Strike,Strike,Strike,Defend,Defend,Defend,Defend,Bash'."
    ),
)
@click.option(
    "--ceiling",
    is_flag=True,
    default=False,
    help=(
        "Also compute the per-seed optimal_value ceiling (exact "
        "branch-and-bound, no rollouts) and report it plus regret "
        "(avg_lost - ceiling)."
    ),
)
def main(monster, seeds, iterations, workers, policy, decks, custom, ceiling):
    """Benchmark deck configurations against sts_sim monsters."""
    if custom:
        card_list = [c.strip() for c in custom.split(",") if c.strip()]
        result = run_deck(
            card_list,
            monster=monster,
            seeds=seeds,
            iterations=iterations,
            workers=workers,
            label="custom",
            policy=policy,
            ceiling=ceiling,
        )
        print(result)
        return

    if decks:
        names = [d.strip() for d in decks.split(",") if d.strip()]
        unknown = [n for n in names if n not in PRESETS]
        if unknown:
            raise click.UsageError(
                f"Unknown preset(s): {', '.join(unknown)}. "
                f"Available: {', '.join(PRESETS)}."
            )
        configs = {n: PRESETS[n] for n in names}
    else:
        configs = PRESETS

    compare(
        configs,
        monster=monster,
        seeds=seeds,
        iterations=iterations,
        workers=workers,
        policy=policy,
        ceiling=ceiling,
    )


if __name__ == "__main__":
    main()
