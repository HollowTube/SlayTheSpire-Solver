mod cards;
mod engine;
mod monsters;
mod state;

use cards::card_data;
use engine::{run_effect_ops, Actor};
use monsters::{monster_move, select_next_intent};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::Rng;
use state::{draw_cards, CombatState, PendingDecision, HAND_SIZE};

#[pyfunction]
fn legal_actions(state: &CombatState) -> Vec<String> {
    match state.pending {
        Some(PendingDecision::SelectTarget { .. }) => vec!["SelectTarget:Monster".to_string()],
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

            // The remaining hand is discarded before the monster's turn
            // resolves — mirrors Slay the Spire's end-of-turn cleanup.
            next.discard_pile.append(&mut next.hand);

            match next.monster_name.clone() {
                // Intent-driven monsters (e.g. Jaw Worm): their own turn
                // starts here — block resets, then the telegraphed move
                // resolves through the same generic effect-pipeline engine
                // cards use (with the monster as the actor).
                Some(name) => {
                    next.monster.block = 0;
                    if let Some(intent) = next.monster_intent.clone() {
                        if let Some(effects) = monster_move(&name, &intent) {
                            run_effect_ops(&mut next, &effects, Actor::Monster);
                        }
                        next.monster_move_streak = if next.monster_last_move.as_deref() == Some(intent.as_str()) {
                            next.monster_move_streak + 1
                        } else {
                            1
                        };
                        next.monster_last_move = Some(intent.clone());
                        next.monster_intent = select_next_intent(
                            &name,
                            &next.monster_last_move,
                            next.monster_move_streak,
                            &mut next.rng,
                        );
                    }
                }
                // The trivial flat-attacker (placeholder monster, every
                // pre-HOL-11 test): always swings for `monster_attack`.
                None => {
                    let absorbed = next.monster_attack.min(next.player.block);
                    next.player.hp -= next.monster_attack - absorbed;
                }
            }

            // Block does not carry over between turns.
            next.player.block = 0;
            // Energy refreshes to its per-turn maximum each turn — it does
            // not carry over either.
            next.player_energy = next.player_max_energy;
            // Draw the next turn's opening hand (reshuffling the discard
            // pile back in if the draw pile runs dry mid-draw).
            draw_cards(&mut next, HAND_SIZE);
            Ok(next)
        }
        "SelectTarget:Monster" => match &state.pending {
            Some(PendingDecision::SelectTarget { card }) => {
                let mut next = state.clone();
                let data = card_data(card).expect("pending card is always known");
                run_effect_ops(&mut next, &data.effects, Actor::Player);
                next.pending = None;
                Ok(next)
            }
            None => Err(PyValueError::new_err(format!("unknown action: {action}"))),
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
                next.discard_pile.push(played);
                next.player_energy -= data.cost;
                if data.targeted {
                    next.pending = Some(PendingDecision::SelectTarget {
                        card: card_name.to_string(),
                    });
                } else {
                    run_effect_ops(&mut next, &data.effects, Actor::Player);
                }
                Ok(next)
            }
            None => Err(PyValueError::new_err(format!("unknown action: {other}"))),
        },
    }
}

#[pyfunction]
fn is_terminal(state: &CombatState) -> bool {
    state.player.hp <= 0 || state.monster.hp <= 0
}

/// Heuristic value of a state from the player's perspective: each side's
/// remaining HP as a fraction of its max (clamped at zero — overkill damage
/// doesn't make a win "more lost"), player's fraction minus monster's.
///
/// This is deliberately the terminal `reward` formula generalized to every
/// state, not just terminal ones (`evaluate` must be "computable from any
/// reachable state" per HOL-9's AC, since MCTS needs to score states
/// mid-search). The two coincide exactly at the boundary: in a win the
/// monster's clamped fraction is 0, leaving `+player_fraction`; in a loss the
/// player's clamped fraction is 0, leaving `-monster_fraction` — i.e.
/// `(win ? +1 : -1) * hp_fraction` of the side that's still standing, which is
/// what an actual STS run optimizes for (player HP carries between fights) and
/// what an eventual RL value head learns to predict.
#[pyfunction]
fn evaluate(state: &CombatState) -> f64 {
    let player_fraction = state.player.hp.max(0) as f64 / state.player.max_hp as f64;
    let monster_fraction = state.monster.hp.max(0) as f64 / state.monster.max_hp as f64;
    player_fraction - monster_fraction
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
    m.add_function(wrap_pyfunction!(legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(apply, m)?)?;
    m.add_function(wrap_pyfunction!(is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    m.add_function(wrap_pyfunction!(reward, m)?)?;
    m.add_function(wrap_pyfunction!(random_rollout, m)?)?;
    Ok(())
}
