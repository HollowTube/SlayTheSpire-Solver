mod act;
mod action;
mod cards;
mod engine;
mod mcts;
mod monsters;
mod run;
mod state;

use act::{draw_overgrowth_elite, draw_overgrowth_monster_sequence};
use action::{EndTurnAction, PlayCardAction, SelectTargetAction};
use cards::{card_data, CardData, CardKeyword, CardType};
use engine::{fire_event, run_effect_ops, tick_debuffs, Actor, GameEvent, Status};
use mcts::{
    fight_outcomes_per_fight, hp_lost_per_fight, mcts_action_values, mcts_search,
    simulate_hp_lost,
};
use monsters::{monster_move, opening_intent, select_next_intent, is_one_time_move};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::{Rng, RngCore};
use run::{
    run_apply, run_is_terminal, run_legal_actions, simulate_run_outcome, simulate_run_outcomes,
    RunState,
};
use std::collections::HashMap;
use state::{draw_cards, CardInstance, CombatState, Fighter, Monster, PendingDecision, HAND_SIZE};

/// The Energy cost to play `data`, accounting for Corruption (Skills cost 0
/// while the player holds it) and Stomp (costs 1 less per Attack played).
fn effective_cost(state: &CombatState, data: &CardData) -> i32 {
    let base = if matches!(data.card_type, CardType::Skill) && state.player.statuses.contains(&Status::Corruption) {
        0
    } else if matches!(data.card_type, CardType::Attack) && state.player.statuses.contains(&Status::FreeAttack) {
        0
    } else {
        let cost = data.cost;
        // Stomp costs 1 less for each Attack played this turn (min 0).
        if data.effects.iter().any(|op| matches!(op, engine::EffectOp::DealDamageToAllEnemies(_))) {
            (cost - state.attacks_played_this_turn).max(0)
        } else {
            cost
        }
    };
    // Tangled: while active on the player, all Attack cards cost +n energy.
    let tangled_bonus = if matches!(data.card_type, CardType::Attack) {
        state
            .player
            .statuses
            .iter()
            .filter_map(|s| {
                if let Status::Tangled(n) = s {
                    Some(*n)
                } else {
                    None
                }
            })
            .sum::<i32>()
    } else {
        0
    };
    base + tangled_bonus
}

/// Internal hot-path version returning strings — used by mcts.rs, random_rollout,
/// and optimal_value_rec which run entirely in Rust and have no Python token.
pub(crate) fn legal_actions_str(state: &CombatState) -> Vec<String> {
    match state.pending {
        Some(PendingDecision::SelectTarget { .. }) => state
            .living_monster_indices()
            .into_iter()
            .map(|i| format!("SelectTarget:Monster:{i}"))
            .collect(),
        None => {
            let mut actions: Vec<String> = state
                .hand
                .iter()
                // Mirrors Slay the Spire greying out cards you can't afford —
                // unaffordable cards are never legal plays, so the engine
                // never has to model (or reject) going into negative energy.
                .filter(|card| {
                    card_data(&card.name, card.upgrade_level)
                        .map(|data| {
                            !data.keywords.contains(&CardKeyword::Unplayable)
                                && effective_cost(state, &data) <= state.player_energy
                        })
                        .unwrap_or(false)
                })
                .map(|card| format!("PlayCard:{}", card.as_str()))
                .collect();
            actions.push("EndTurn".to_string());
            actions
        }
    }
}

/// Python-facing wrapper: converts each string action into a typed Action object.
#[pyfunction]
fn legal_actions(py: Python<'_>, state: &CombatState) -> Vec<PyObject> {
    legal_actions_str(state)
        .into_iter()
        .map(|s| str_to_action(py, &s))
        .collect()
}

fn str_to_action(py: Python<'_>, s: &str) -> PyObject {
    if s == "EndTurn" {
        return EndTurnAction::new().into_pyobject(py).unwrap().into_any().unbind();
    }
    if let Some(card) = s.strip_prefix("PlayCard:") {
        return PlayCardAction::new(card.to_string()).into_pyobject(py).unwrap().into_any().unbind();
    }
    if let Some(idx_str) = s.strip_prefix("SelectTarget:Monster:") {
        if let Ok(idx) = idx_str.parse::<usize>() {
            return SelectTargetAction::new(idx).into_pyobject(py).unwrap().into_any().unbind();
        }
    }
    unreachable!("legal_actions_str only produces well-formed action strings")
}

#[pyfunction]
fn apply(state: &CombatState, action: &str) -> PyResult<CombatState> {
    match action {
        "EndTurn" => {
            let mut next = state.clone();
            next.turn += 1;

            // "HP lost this turn" tracks damage taken since the last
            // EndTurn — reset here so the monsters' attacks below populate
            // it fresh for the upcoming player turn (e.g. Spite).
            next.player_hp_lost_this_turn = false;
            // "Exhausted a card this turn" tracks Exhausts since the last
            // EndTurn — reset here for the upcoming player turn (e.g. Evil
            // Eye, Forgotten Ritual).
            next.player_exhausted_card_this_turn = false;

            // Tangled is removed at the end of the player's turn (before the
            // monster resolves its next action), not at the start — so it
            // persists through the player's full card-play window.
            next.player.statuses.retain(|s| !matches!(s, Status::Tangled(_)));

            // The remaining hand is discarded before the monsters' turn
            // resolves — mirrors Slay the Spire's end-of-turn cleanup.
            // Ethereal cards exhaust instead of being discarded.
            let (ethereal, rest): (Vec<CardInstance>, Vec<CardInstance>) =
                next.hand.drain(..).partition(|card| {
                    card_data(&card.name, card.upgrade_level)
                        .map(|data| data.keywords.contains(&CardKeyword::Ethereal))
                        .unwrap_or(false)
                });
            next.exhaust_pile.extend(ethereal);
            next.discard_pile.extend(rest);

            // Each living monster takes its turn in order: block resets,
            // then either its telegraphed move resolves through the same
            // generic effect-pipeline engine cards use (with that monster as
            // the actor and the player as the sole target), or — for the
            // trivial flat-attacker (placeholder monsters, every pre-HOL-11
            // test) — it always swings for its fixed `attack`. Dead monsters
            // (hp <= 0) don't act.
            for i in 0..next.monsters.len() {
                // Infested spawn-on-death: if the monster has hp <= 0 and
                // carries Status::Infested, spawn its minions before skipping
                // its turn. Spawned monsters go to the end of the list so
                // existing loop indices remain valid; they'll act on subsequent
                // EndTurn cycles.
                let infested = next.monsters[i]
                    .fighter
                    .statuses
                    .iter()
                    .find_map(|s| {
                        if let Status::Infested { minion_name, minion_hp, count } = s {
                            Some((minion_name.clone(), *minion_hp, *count))
                        } else {
                            None
                        }
                    });
                if next.monsters[i].fighter.hp <= 0 {
                    if let Some((mname, mhp, mcount)) = infested {
                        for _ in 0..mcount {
                            let monster = Monster {
                                fighter: Fighter {
                                    hp: mhp,
                                    max_hp: mhp,
                                    block: 0,
                                    statuses: vec![Status::Stun],
                                },
                                attack: 0,
                                name: Some(mname.clone()),
                                intent: opening_intent(&mname),
                                last_move: None,
                                move_streak: 0,
                                moves_used: Vec::new(),
                            };
                            next.monsters.push(monster);
                        }
                        next.monsters[i].fighter.statuses.retain(|s| {
                            !matches!(s, Status::Infested { .. })
                        });
                    }
                    continue;
                }

                // Minion flee check: if this monster has Status::Minion and
                // its named leader is dead, skip its turn entirely.
                let is_fled_minion = next.monsters[i].fighter.statuses.iter().any(|s| {
                    if let Status::Minion { leader } = s {
                        next.monsters.iter().any(|m| {
                            m.name.as_deref() == Some(leader.as_str()) && m.fighter.hp <= 0
                        })
                    } else {
                        false
                    }
                });
                if is_fled_minion {
                    next.monsters[i].intent = None;
                    continue;
                }

                // Stun check: the monster skips its turn entirely. The status
                // is consumed after the skip.
                let is_stunned = next.monsters[i]
                    .fighter
                    .statuses
                    .iter()
                    .any(|s| matches!(s, Status::Stun));
                if is_stunned {
                    next.monsters[i].fighter.statuses.retain(|s| !matches!(s, Status::Stun));
                    next.monsters[i].intent = None;
                    continue;
                }

                next.monsters[i].fighter.block = 0;
                match next.monsters[i].name.clone() {
                    Some(name) => {
                        if let Some(intent) = next.monsters[i].intent.clone() {
                            if let Some(effects) = monster_move(&name, &intent) {
                                run_effect_ops(&mut next, &effects, Actor::Monster(i), &[Actor::Player], false);
                            }
                            let streak = if next.monsters[i].last_move.as_deref() == Some(intent.as_str()) {
                                next.monsters[i].move_streak + 1
                            } else {
                                1
                            };
                            next.monsters[i].move_streak = streak;
                            next.monsters[i].last_move = Some(intent.clone());
                            if is_one_time_move(&name, &intent) {
                                next.monsters[i].moves_used.push(intent.clone());
                            }
                            next.monsters[i].intent = select_next_intent(
                                &name,
                                &next.monsters[i].last_move,
                                streak,
                                &next.monsters[i].moves_used,
                                &mut next.rng,
                            );
                        }
                    }
                    None => {
                        let attack = next.monsters[i].attack;
                        let absorbed = attack.min(next.player.block);
                        next.player.block -= absorbed;
                        let hp_loss = attack - absorbed;
                        next.player.hp -= hp_loss;
                        if hp_loss > 0 {
                            next.player_times_damaged_this_combat += 1;
                            next.player_hp_lost_this_turn = true;
                        }
                    }
                }
                // This monster's debuffs (e.g. Vulnerable) tick down at the
                // end of its own turn — after it has acted.
                tick_debuffs(&mut next.monsters[i].fighter.statuses);
            }

            // Constrict: the player takes `n` unblockable damage at end of
            // their turn, summed across all Constrict stacks. Damage bypasses
            // block and the damage-modifier pipeline entirely.
            let constrict_damage: i32 = next
                .player
                .statuses
                .iter()
                .filter_map(|s| {
                    if let Status::Constrict(n) = s {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .sum();
            if constrict_damage > 0 {
                next.player.hp -= constrict_damage;
                next.player_hp_lost_this_turn = true;
            }

            // Block does not carry over between turns, unless Barricade is
            // held.
            if !next.player.statuses.contains(&Status::Barricade) {
                next.player.block = 0;
            }
            // Energy refreshes to its per-turn maximum each turn — it does
            // not carry over either.
            next.player_energy = next.player_max_energy;
            // Attack-played count resets at the start of each player turn
            // (e.g. Conflagration).
            next.attacks_played_this_turn = 0;
            // Cards-played count resets at the start of each player turn
            // (e.g. Bygone Effigy's Status::Slow).
            next.cards_played_this_turn = 0;
            // Draw the next turn's opening hand (reshuffling the discard
            // pile back in if the draw pile runs dry mid-draw).
            draw_cards(&mut next, HAND_SIZE);
            // Player's duration debuffs (e.g. Vulnerable) tick at the start
            // of the player's turn — after the monster has already attacked,
            // matching Slay the Spire's documented turn structure.
            tick_debuffs(&mut next.player.statuses);
            // Persistent powers that react to the new turn beginning (e.g.
            // Demon Form's +2 Strength) fire after the above resets.
            fire_event(&mut next, GameEvent::TurnStart);
            Ok(next)
        }
        other if other.starts_with("SelectTarget:Monster:") => match &state.pending {
            Some(PendingDecision::SelectTarget { card }) => {
                let idx: usize = other
                    .strip_prefix("SelectTarget:Monster:")
                    .unwrap()
                    .parse()
                    .map_err(|_| PyValueError::new_err(format!("unknown action: {other}")))?;
                if idx >= state.monsters.len() {
                    return Err(PyValueError::new_err(format!("unknown action: {other}")));
                }
                let mut next = state.clone();
                let data = card_data(&card.name, card.upgrade_level).expect("pending card is always known");
                let is_attack = matches!(data.card_type, CardType::Attack);
                run_effect_ops(&mut next, &data.effects, Actor::Player, &[Actor::Monster(idx)], is_attack);
                match data.card_type {
                    CardType::Skill => fire_event(&mut next, GameEvent::SkillPlayed),
                    CardType::Attack => {
                        next.attacks_played_this_turn += 1;
                        fire_event(&mut next, GameEvent::AttackPlayed);
                        // One Two Punch: the next Attack played this turn
                        // resolves a second time, then the power is consumed.
                        if let Some(pos) = next.player.statuses.iter().position(|s| *s == Status::OneTwoPunch) {
                            next.player.statuses.remove(pos);
                            run_effect_ops(&mut next, &data.effects, Actor::Player, &[Actor::Monster(idx)], is_attack);
                            next.attacks_played_this_turn += 1;
                            fire_event(&mut next, GameEvent::AttackPlayed);
                        }
                    }
                    CardType::Power | CardType::Status => {}
                }
                next.pending = None;
                Ok(next)
            }
            None => Err(PyValueError::new_err(format!("unknown action: {other}"))),
        },
        other => match other.strip_prefix("PlayCard:") {
            Some(card_name) => {
                let instance = CardInstance::parse(card_name);
                let data = card_data(&instance.name, instance.upgrade_level)
                    .ok_or_else(|| PyValueError::new_err(format!("unknown card: {}", instance.name)))?;
                let mut next = state.clone();
                let position = next
                    .hand
                    .iter()
                    .position(|c| c.as_str() == card_name)
                    .ok_or_else(|| PyValueError::new_err(format!("{card_name} is not in hand")))?;
                let cost = effective_cost(&next, &data);
                if cost > next.player_energy {
                    return Err(PyValueError::new_err(format!(
                        "cannot afford {card_name}: costs {} but only {} energy available",
                        cost, next.player_energy
                    )));
                }
                let played = next.hand.remove(position);
                // Corruption sends played Skills to the exhaust pile instead
                // of discard, just like the Exhaust keyword.
                let corruption_exhausts_skill =
                    matches!(data.card_type, CardType::Skill) && next.player.statuses.contains(&Status::Corruption);
                let data_exhausts = data.keywords.contains(&CardKeyword::Exhaust);
                if matches!(data.card_type, CardType::Power | CardType::Status) || data_exhausts || corruption_exhausts_skill {
                    next.exhaust_pile.push(played);
                    // Power/Status cards leave play permanently but aren't
                    // "Exhausted" in the keyword sense (e.g. playing
                    // DarkEmbrace itself doesn't trigger DarkEmbrace) — only
                    // cards with the Exhaust keyword (or Corruption's
                    // Skill-exhaust) fire CardExhausted.
                    if data_exhausts || corruption_exhausts_skill {
                        fire_event(&mut next, GameEvent::CardExhausted);
                    }
                } else {
                    next.discard_pile.push(played);
                }
                next.player_energy -= cost;
                next.cards_played_this_turn += 1;
                // Unrelenting's FreeAttack: consumed by the next Attack
                // played, regardless of whether it was actually free
                // (mirrors OneTwoPunch's one-shot consumption).
                if matches!(data.card_type, CardType::Attack) {
                    if let Some(pos) = next.player.statuses.iter().position(|s| *s == Status::FreeAttack) {
                        next.player.statuses.remove(pos);
                    }
                }
                let is_attack = matches!(data.card_type, CardType::Attack);
                if data.targeted {
                    next.pending = Some(PendingDecision::SelectTarget { card: instance });
                } else {
                    // Non-targeted attacks (e.g. Thunderclap) hit every
                    // living enemy; non-targeted skills (Defend, Inflame,
                    // Rage) have no `DealDamage`/`ApplyStatusToTarget` ops,
                    // so the target list is simply unused for them.
                    let targets: Vec<Actor> = next
                        .living_monster_indices()
                        .into_iter()
                        .map(Actor::Monster)
                        .collect();
                                run_effect_ops(&mut next, &data.effects, Actor::Player, &targets, is_attack);
                    match data.card_type {
                        CardType::Skill => fire_event(&mut next, GameEvent::SkillPlayed),
                        CardType::Attack => {
                            next.attacks_played_this_turn += 1;
                            fire_event(&mut next, GameEvent::AttackPlayed);
                            // One Two Punch: the next Attack played this turn
                            // resolves a second time, then the power is
                            // consumed.
                            if let Some(pos) = next.player.statuses.iter().position(|s| *s == Status::OneTwoPunch) {
                                next.player.statuses.remove(pos);
                    run_effect_ops(&mut next, &data.effects, Actor::Player, &targets, is_attack);
                                next.attacks_played_this_turn += 1;
                                fire_event(&mut next, GameEvent::AttackPlayed);
                            }
                        }
                        CardType::Power | CardType::Status => {}
                    }
                }
                Ok(next)
            }
            None => Err(PyValueError::new_err(format!("unknown action: {other}"))),
        },
    }
}

#[pyfunction]
fn is_terminal(state: &CombatState) -> bool {
    state.player.hp <= 0 || state.monsters.iter().all(|m| m.fighter.hp <= 0)
}

/// Heuristic value of a state from the player's perspective: each side's
/// remaining HP as a fraction of its max (clamped at zero — overkill damage
/// doesn't make a win "more lost"), player's fraction minus the monsters'.
///
/// This is deliberately the terminal `reward` formula generalized to every
/// state, not just terminal ones (`evaluate` must be "computable from any
/// reachable state" per HOL-9's AC, since MCTS needs to score states
/// mid-search). The two coincide exactly at the boundary: in a win every
/// monster's clamped fraction is 0, leaving `+player_fraction`; in a loss the
/// player's clamped fraction is 0, leaving `-monsters_fraction` — i.e.
/// `(win ? +1 : -1) * hp_fraction` of the side that's still standing, which is
/// what an actual STS run optimizes for (player HP carries between fights) and
/// what an eventual RL value head learns to predict.
///
/// With multiple monsters, "the monsters' fraction" is pooled HP — total
/// remaining HP over total max HP across all enemies — so it stays in 0..=1
/// and comparable to the player's fraction regardless of enemy count.
#[pyfunction]
fn evaluate(state: &CombatState) -> f64 {
    let player_fraction = state.player.hp.max(0) as f64 / state.player.max_hp as f64;
    let monsters_hp: i32 = state.monsters.iter().map(|m| m.fighter.hp.max(0)).sum();
    let monsters_max_hp: i32 = state.monsters.iter().map(|m| m.fighter.max_hp).sum();
    let monsters_fraction = monsters_hp as f64 / monsters_max_hp as f64;
    player_fraction - monsters_fraction
}

/// Terminal reward signal: zero while combat is ongoing, otherwise the HP
/// margin (positive for a win, negative for a loss; magnitude reflects how
/// decisive the outcome was).
#[pyfunction]
fn reward(state: &CombatState) -> f64 {
    if is_terminal(state) {
        evaluate(state)
    } else {
        0.0
    }
}

/// Return a copy of `state` with its draw pile shuffled into a fresh random
/// order and its RNG reseeded from `seed` — see
/// `CombatState::redeterminized`. Used by MCTS to sample possible futures a
/// real player can't actually foresee, rather than solving the single
/// perfect-information tree implied by `state`'s embedded shuffle/RNG.
#[pyfunction]
fn redeterminized(state: &CombatState, seed: u64) -> CombatState {
    state.redeterminized(seed)
}

/// Run a complete random playout from `state` (re-seeded with `seed`) and
/// return the terminal reward. Doing the hot loop in Rust eliminates the
/// per-step Python/Rust FFI overhead that makes the Python rollout ~3x slower
/// than raw `apply()` throughput — the tree (UCB1, selection, expansion,
/// backprop) stays in Python so a future neural policy can plug in there.
#[pyfunction]
fn random_rollout(state: &CombatState, seed: u64) -> f64 {
    let mut s = state.reseeded(seed);
    while !is_terminal(&s) {
        let actions = legal_actions_str(&s);
        let idx = s.rng.gen_range(0..actions.len());
        s = apply(&s, &actions[idx]).expect("legal action is always valid");
    }
    reward(&s)
}

/// Upper bound on the reward achievable from `state` onward, assuming no
/// further player healing: the monsters' pooled fraction can only fall to 0,
/// so `evaluate(state) + monsters_fraction(state)` (= the player's current HP
/// fraction) is the best `reward` any future path from here could reach.
fn player_fraction(state: &CombatState) -> f64 {
    let monsters_hp: i32 = state.monsters.iter().map(|m| m.fighter.hp.max(0)).sum();
    let monsters_max_hp: i32 = state.monsters.iter().map(|m| m.fighter.max_hp).sum();
    evaluate(state) + monsters_hp as f64 / monsters_max_hp as f64
}

/// Transposition key: every part of `CombatState` that affects its future
/// (everything `optimal_value_rec` recurses on), used to detect when
/// different action sequences reach an equivalent state — e.g. the several
/// orderings of playing the same hand of non-interacting cards in one turn.
/// `Pcg32` (the `rng` field) isn't `Hash`, so it's represented by a
/// fingerprint: a few `next_u32()` draws from a *clone* of the RNG (which
/// doesn't disturb the real one) — two RNGs in the same internal state
/// produce the same draws, and different states differ with overwhelming
/// probability.
#[derive(PartialEq, Eq, Hash)]
struct StateKey {
    player: Fighter,
    player_energy: i32,
    monsters: Vec<Monster>,
    turn: u32,
    hand: Vec<CardInstance>,
    draw_pile: Vec<CardInstance>,
    discard_pile: Vec<CardInstance>,
    exhaust_pile: Vec<CardInstance>,
    attacks_played_this_turn: i32,
    pending: Option<PendingDecision>,
    rng_fingerprint: [u32; 4],
}

fn state_key(state: &CombatState) -> StateKey {
    let mut rng_clone = state.rng.clone();
    StateKey {
        player: state.player.clone(),
        player_energy: state.player_energy,
        monsters: state.monsters.clone(),
        turn: state.turn,
        hand: state.hand.clone(),
        draw_pile: state.draw_pile.clone(),
        discard_pile: state.discard_pile.clone(),
        exhaust_pile: state.exhaust_pile.clone(),
        attacks_played_this_turn: state.attacks_played_this_turn,
        pending: state.pending.clone(),
        rng_fingerprint: [
            rng_clone.next_u32(),
            rng_clone.next_u32(),
            rng_clone.next_u32(),
            rng_clone.next_u32(),
        ],
    }
}

/// Branch-and-bound search for the true optimal `reward` reachable from
/// `state` under perfect foresight of the (deterministic, seed-driven) RNG —
/// i.e. the best any sequence of legal actions from here can do. `best` is
/// the best terminal reward found anywhere in the search so far; a node is
/// pruned once `player_fraction(state) <= *best`, since no descendant can
/// then beat `best` (see `player_fraction`'s doc comment for why that bound
/// is valid). Exploring children in descending `evaluate` order finds strong
/// solutions early, so this pruning kicks in as soon as possible.
///
/// `memo` caches the return value for every non-pruned, non-terminal state by
/// `StateKey` (transposition table) — for any such state, the value computed
/// here is its *exact* value regardless of `*best`'s value at the time (a
/// pruned child's true value is always `<= *best`, which is itself always
/// `<= best_val` once an unpruned ancestor's loop completes, so pruning a
/// child never changes its parent's max). That makes the cached value valid
/// to reuse — and to fold into `*best` — from any later context, which is
/// what collapses transpositions like the several orderings of playing the
/// same non-interacting cards in one turn.
///
/// This is an analysis/benchmarking tool (used to measure how far MCTS falls
/// short of optimal play on a given seed), not part of the play loop — it has
/// no caller within `apply`/`legal_actions` and can be exponential without
/// the no-healing assumption that makes the `player_fraction` bound tight.
fn optimal_value_rec(state: &CombatState, best: &mut f64, memo: &mut HashMap<StateKey, f64>) -> f64 {
    if is_terminal(state) {
        let r = reward(state);
        if r > *best {
            *best = r;
        }
        return r;
    }
    if player_fraction(state) <= *best {
        return f64::NEG_INFINITY;
    }
    let key = state_key(state);
    if let Some(&cached) = memo.get(&key) {
        if cached > *best {
            *best = cached;
        }
        return cached;
    }
    let mut children: Vec<(f64, CombatState)> = legal_actions_str(state)
        .into_iter()
        .map(|action| {
            let next = apply(state, &action).expect("legal action is always valid");
            (evaluate(&next), next)
        })
        .collect();
    children.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let mut best_val = f64::NEG_INFINITY;
    for (_, next) in children {
        let val = optimal_value_rec(&next, best, memo);
        if val > best_val {
            best_val = val;
        }
    }
    memo.insert(key, best_val);
    best_val
}

/// Python entry point for `optimal_value_rec`: the true optimal `reward`
/// reachable from `state` (see that function's doc comment).
#[pyfunction]
fn optimal_value(state: &CombatState) -> f64 {
    let mut best = f64::NEG_INFINITY;
    let mut memo = HashMap::new();
    optimal_value_rec(state, &mut best, &mut memo)
}

#[pymodule]
fn _sts_sim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EndTurnAction>()?;
    m.add_class::<PlayCardAction>()?;
    m.add_class::<SelectTargetAction>()?;
    m.add_class::<CombatState>()?;
    m.add_class::<Monster>()?;
    m.add_class::<RunState>()?;
    m.add_function(wrap_pyfunction!(run_legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(run_apply, m)?)?;
    m.add_function(wrap_pyfunction!(run_is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_run_outcome, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_run_outcomes, m)?)?;
    m.add_function(wrap_pyfunction!(draw_overgrowth_monster_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(draw_overgrowth_elite, m)?)?;
    m.add_function(wrap_pyfunction!(legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(apply, m)?)?;
    m.add_function(wrap_pyfunction!(is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    m.add_function(wrap_pyfunction!(reward, m)?)?;
    m.add_function(wrap_pyfunction!(random_rollout, m)?)?;
    m.add_function(wrap_pyfunction!(redeterminized, m)?)?;
    m.add_function(wrap_pyfunction!(optimal_value, m)?)?;
    m.add_function(wrap_pyfunction!(mcts_action_values, m)?)?;
    m.add_function(wrap_pyfunction!(mcts_search, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_hp_lost, m)?)?;
    m.add_function(wrap_pyfunction!(hp_lost_per_fight, m)?)?;
    m.add_function(wrap_pyfunction!(fight_outcomes_per_fight, m)?)?;
    Ok(())
}
