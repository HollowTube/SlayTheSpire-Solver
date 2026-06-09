import math
import random

from . import apply, is_terminal, legal_actions, reward

# Standard UCB1 exploration constant (sqrt(2)) — balances exploiting the
# best-known action against trying under-visited ones.
EXPLORATION_CONSTANT = math.sqrt(2)

DEFAULT_ITERATIONS = 200


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
    """Play uniformly-random legal actions to a terminal state and score it.

    This is the "full-random-rollout simulation" leg of MCTS: a maximally
    dumb policy that nonetheless reaches a correctly-shaped terminal reward
    either way, giving the backprop step a real signal to learn from.
    """
    while not is_terminal(state):
        state = apply(state, rng.choice(legal_actions(state)))
    return reward(state)


def _backpropagate(node, value):
    while node is not None:
        node.visits += 1
        node.total_value += value
        node = node.parent


def search(state, iterations=DEFAULT_ITERATIONS, rng=None):
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
    """
    rng = rng if rng is not None else random.Random()
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

    return root.most_visited_action()
