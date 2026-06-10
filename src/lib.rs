mod cards;
mod engine;
mod monsters;
mod state;

use cards::{card_data, CardType};
use engine::{fire_event, run_effect_ops, tick_debuffs, Actor, GameEvent};
use monsters::{monster_move, select_next_intent};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::Rng;
use state::{draw_cards, CombatState, Monster, PendingDecision, HAND_SIZE};

#[pyfunction]
fn legal_actions(state: &CombatState) -> Vec<String> {
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
                .filter(|name| {
                    card_data(name)
                        .map(|data| data.cost <= state.player_energy)
                        .unwrap_or(false)
                })
                .map(|name| format!("PlayCard:{name}"))
                .collect();
            actions.push("EndTurn".to_string());
            actions
        }
    }
}

#[pyfunction]
fn apply(state: &CombatState, action: &str) -> PyResult<CombatState> {
    match action {
        "EndTurn" => {
            let mut next = state.clone();
            next.turn += 1;

            // The remaining hand is discarded before the monsters' turn
            // resolves — mirrors Slay the Spire's end-of-turn cleanup.
            next.discard_pile.append(&mut next.hand);

            // Each living monster takes its turn in order: block resets,
            // then either its telegraphed move resolves through the same
            // generic effect-pipeline engine cards use (with that monster as
            // the actor and the player as the sole target), or — for the
            // trivial flat-attacker (placeholder monsters, every pre-HOL-11
            // test) — it always swings for its fixed `attack`. Dead monsters
            // (hp <= 0) don't act.
            for i in 0..next.monsters.len() {
                if next.monsters[i].fighter.hp <= 0 {
                    continue;
                }
                next.monsters[i].fighter.block = 0;
                match next.monsters[i].name.clone() {
                    Some(name) => {
                        if let Some(intent) = next.monsters[i].intent.clone() {
                            if let Some(effects) = monster_move(&name, &intent) {
                                run_effect_ops(&mut next, &effects, Actor::Monster(i), &[Actor::Player]);
                            }
                            let streak = if next.monsters[i].last_move.as_deref() == Some(intent.as_str()) {
                                next.monsters[i].move_streak + 1
                            } else {
                                1
                            };
                            next.monsters[i].move_streak = streak;
                            next.monsters[i].last_move = Some(intent.clone());
                            next.monsters[i].intent = select_next_intent(
                                &name,
                                &next.monsters[i].last_move,
                                streak,
                                &mut next.rng,
                            );
                        }
                    }
                    None => {
                        let attack = next.monsters[i].attack;
                        let absorbed = attack.min(next.player.block);
                        next.player.hp -= attack - absorbed;
                    }
                }
                // This monster's debuffs (e.g. Vulnerable) tick down at the
                // end of its own turn — after it has acted.
                tick_debuffs(&mut next.monsters[i].fighter.statuses);
            }

            // Block does not carry over between turns.
            next.player.block = 0;
            // Energy refreshes to its per-turn maximum each turn — it does
            // not carry over either.
            next.player_energy = next.player_max_energy;
            // Draw the next turn's opening hand (reshuffling the discard
            // pile back in if the draw pile runs dry mid-draw).
            draw_cards(&mut next, HAND_SIZE);
            // Player's duration debuffs (e.g. Vulnerable) tick at the start
            // of the player's turn — after the monster has already attacked,
            // matching Slay the Spire's documented turn structure.
            tick_debuffs(&mut next.player.statuses);
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
                let data = card_data(card).expect("pending card is always known");
                run_effect_ops(&mut next, &data.effects, Actor::Player, &[Actor::Monster(idx)]);
                match data.card_type {
                    CardType::Skill => fire_event(&mut next, GameEvent::SkillPlayed),
                    CardType::Attack => fire_event(&mut next, GameEvent::AttackPlayed),
                    CardType::Power => {}
                }
                next.pending = None;
                Ok(next)
            }
            None => Err(PyValueError::new_err(format!("unknown action: {other}"))),
        },
        other => match other.strip_prefix("PlayCard:") {
            Some(card_name) => {
                let data = card_data(card_name)
                    .ok_or_else(|| PyValueError::new_err(format!("unknown card: {card_name}")))?;
                let mut next = state.clone();
                let position = next
                    .hand
                    .iter()
                    .position(|c| c == card_name)
                    .ok_or_else(|| PyValueError::new_err(format!("{card_name} is not in hand")))?;
                if data.cost > next.player_energy {
                    return Err(PyValueError::new_err(format!(
                        "cannot afford {card_name}: costs {} but only {} energy available",
                        data.cost, next.player_energy
                    )));
                }
                let played = next.hand.remove(position);
                if data.card_type == CardType::Power {
                    next.exhaust_pile.push(played);
                } else {
                    next.discard_pile.push(played);
                }
                next.player_energy -= data.cost;
                if data.targeted {
                    next.pending = Some(PendingDecision::SelectTarget {
                        card: card_name.to_string(),
                    });
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
                    run_effect_ops(&mut next, &data.effects, Actor::Player, &targets);
                    match data.card_type {
                        CardType::Skill => fire_event(&mut next, GameEvent::SkillPlayed),
                        CardType::Attack => fire_event(&mut next, GameEvent::AttackPlayed),
                        CardType::Power => {}
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

/// Run a complete random playout from `state` (re-seeded with `seed`) and
/// return the terminal reward. Doing the hot loop in Rust eliminates the
/// per-step Python/Rust FFI overhead that makes the Python rollout ~3x slower
/// than raw `apply()` throughput — the tree (UCB1, selection, expansion,
/// backprop) stays in Python so a future neural policy can plug in there.
#[pyfunction]
fn random_rollout(state: &CombatState, seed: u64) -> f64 {
    let mut s = state.reseeded(seed);
    while !is_terminal(&s) {
        let actions = legal_actions(&s);
        let idx = s.rng.gen_range(0..actions.len());
        s = apply(&s, &actions[idx]).expect("legal action is always valid");
    }
    reward(&s)
}

#[pymodule]
fn _sts_sim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CombatState>()?;
    m.add_class::<Monster>()?;
    m.add_function(wrap_pyfunction!(legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(apply, m)?)?;
    m.add_function(wrap_pyfunction!(is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    m.add_function(wrap_pyfunction!(reward, m)?)?;
    m.add_function(wrap_pyfunction!(random_rollout, m)?)?;
    Ok(())
}
