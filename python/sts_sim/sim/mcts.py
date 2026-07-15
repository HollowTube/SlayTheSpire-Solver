import math
import random

from .. import apply, is_terminal, legal_actions, random_rollout, redeterminized, reward

# Standard UCB1 exploration constant (sqrt(2)) — balances exploiting the
# best-known action against trying under-visited ones.
EXPLORATION_CONSTANT = math.sqrt(2)

DEFAULT_ITERATIONS = 200

# Number of independent redeterminized trees averaged by `determinize=True`
# (the default) — see `action_values`.
DEFAULT_DETERMINIZATIONS = 8


class _Node:
    """One decision point in the search tree, rooted at a `CombatState`.

    Lives entirely in Python (per HOL-12: a future neural policy/value net
    will need to be evaluated from here too) and drives the Rust core
    through its public `apply`/`legal_actions`/`is_terminal`/`reward`
    interface — exactly like any other consumer of the engine.
    """

    def __init__(self, state, parent=None, action=None):
        self.state = state
        self.parent = parent
        self.action = action
        self.children = []
        self.untried_actions = [] if is_terminal(state) else list(legal_actions(state))
        self.visits = 0
        self.total_value = 0.0

    def is_leaf(self):
        return not self.children and not self.untried_actions

    def ucb1(self):
        exploitation = self.total_value / self.visits
        exploration = EXPLORATION_CONSTANT * math.sqrt(
            math.log(self.parent.visits) / self.visits
        )
        return exploitation + exploration

    def select_child(self):
        return max(self.children, key=lambda child: child.ucb1())

    def expand(self, rng):
        action = self.untried_actions.pop(rng.randrange(len(self.untried_actions)))
        child = _Node(apply(self.state, action), parent=self, action=action)
        self.children.append(child)
        return child

    def most_visited_action(self):
        return max(self.children, key=lambda child: child.visits).action


def _rollout(state, rng):
    """Run a complete random playout via the Rust `random_rollout` function.

    Delegates the hot per-step loop to Rust, eliminating Python/Rust FFI
    overhead on each apply()/legal_actions() call. The seed passed in is
    derived from the Python MCTS rng so each rollout explores a different
    draw sequence (single-observer determinization).
    """
    return random_rollout(state, rng.randint(0, 2**64 - 1))


def _backpropagate(node, value):
    while node is not None:
        node.visits += 1
        node.total_value += value
        node = node.parent


def _build_tree(state, iterations, rng):
    """Run `iterations` of MCTS from `state` and return the root node."""
    root = _Node(state)
    for _ in range(iterations):
        node = root
        while not node.untried_actions and node.children:
            node = node.select_child()
        if node.untried_actions:
            node = node.expand(rng)
        value = (
            reward(node.state) if is_terminal(node.state) else _rollout(node.state, rng)
        )
        _backpropagate(node, value)
    return root


def _single_tree_action_values(state, iterations, rng):
    root = _build_tree(state, iterations, rng)
    return {child.action: child.total_value / child.visits for child in root.children}


def search(
    state,
    iterations=DEFAULT_ITERATIONS,
    rng=None,
    determinize=True,
    determinizations=DEFAULT_DETERMINIZATIONS,
):
    """Run MCTS from `state` and return the most-promising legal action.

    Each iteration is the textbook select → expand → simulate → backpropagate
    cycle: walk down the tree by UCB1 while every node is fully expanded,
    expand one untried action, roll the resulting state out to terminal with
    a random policy, and propagate the resulting reward back up the path.

    Works at *any* decision point — including mid-resolution
    `PendingDecision` states — because it only ever asks the core for
    `legal_actions`/`apply`/`is_terminal`/`reward`, the same uniform
    interface every decision (playing a card, selecting a target, ending a
    turn) already flows through.

    See `action_values` for what `determinize`/`determinizations` mean. When
    `determinize=True` (the default), the returned action is the argmax of
    the redeterminization-averaged `action_values`; when `False`, it's the
    most-visited child of a single tree built directly against `state`.
    """
    rng = rng if rng is not None else random.Random()
    if not determinize:
        return _build_tree(state, iterations, rng).most_visited_action()
    values = action_values(
        state, iterations, rng, determinize=True, determinizations=determinizations
    )
    return max(values, key=values.__getitem__)


def action_values(
    state,
    iterations=DEFAULT_ITERATIONS,
    rng=None,
    determinize=True,
    determinizations=DEFAULT_DETERMINIZATIONS,
):
    """Run MCTS from `state` and return the estimated value for every legal
    action as a dict mapping action string → float in [-1, 1].

    By default (`determinize=True`), this runs Perfect Information Monte
    Carlo: `determinizations` independent `iterations`-sized trees, each
    built against a `redeterminized` copy of `state` — same hand, energy,
    and HP, but with the draw pile reshuffled into a fresh random order and
    the RNG reseeded. This models the actual situation during play: the
    player doesn't know the order of their face-down draw pile or the
    seed's future monster rolls, so the value of the next move must be
    averaged over those possible futures rather than computed against one
    fixed (and effectively clairvoyant) future. The returned value for each
    action is the mean of `total_value / visits` across determinizations.

    Pass `determinize=False` for the original single-tree behavior: one
    `iterations`-sized tree built directly against `state` as given, with no
    redeterminization. This solves the deterministic, perfect-information
    tree implied by `state`'s embedded RNG/draw-pile order — useful for
    comparing against `optimal_value`'s clairvoyant ceiling for the same
    seed, but not representative of in-game decision-making.

    In both modes, the value for each action is `total_value / visits`
    across all rollouts that passed through that child — higher is better
    for the player. All legal actions are guaranteed to appear (the tree
    always expands every child at least once before revisiting, and every
    determinization shares the same root `legal_actions` since only the draw
    pile and RNG are redeterminized).
    """
    rng = rng if rng is not None else random.Random()
    if not determinize:
        return _single_tree_action_values(state, iterations, rng)

    totals: dict[str, float] = {}
    counts: dict[str, int] = {}
    for _ in range(determinizations):
        sample = redeterminized(state, rng.randint(0, 2**64 - 1))
        for action, value in _single_tree_action_values(
            sample, iterations, rng
        ).items():
            totals[action] = totals.get(action, 0.0) + value
            counts[action] = counts.get(action, 0) + 1
    return {action: totals[action] / counts[action] for action in totals}
