use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::SeedableRng;
use rand_pcg::Pcg32;

#[pyclass(eq)]
#[derive(Clone, PartialEq)]
pub struct CombatState {
    #[pyo3(get)]
    player_hp: i32,
    player_energy: i32,
    #[pyo3(get)]
    monster_hp: i32,
    #[pyo3(get)]
    monster_attack: i32,
    #[pyo3(get)]
    turn: u32,
    rng: Pcg32,
}

#[pymethods]
impl CombatState {
    #[new]
    fn new(player_hp: i32, player_energy: i32, monster_hp: i32, monster_attack: i32, seed: u64) -> Self {
        CombatState {
            player_hp,
            player_energy,
            monster_hp,
            monster_attack,
            turn: 0,
            rng: Pcg32::seed_from_u64(seed),
        }
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

#[pyfunction]
fn legal_actions(_state: &CombatState) -> Vec<String> {
    vec!["EndTurn".to_string()]
}

#[pyfunction]
fn apply(state: &CombatState, action: &str) -> PyResult<CombatState> {
    match action {
        "EndTurn" => {
            let mut next = state.clone();
            next.turn += 1;
            next.player_hp -= next.monster_attack;
            Ok(next)
        }
        other => Err(PyValueError::new_err(format!("unknown action: {other}"))),
    }
}

#[pyfunction]
fn is_terminal(state: &CombatState) -> bool {
    state.player_hp <= 0 || state.monster_hp <= 0
}

/// Heuristic value of a state from the player's perspective: the HP margin.
/// Positive favours the player, negative favours the monster.
#[pyfunction]
fn evaluate(state: &CombatState) -> f64 {
    (state.player_hp - state.monster_hp) as f64
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

#[pymodule]
fn _sts_sim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CombatState>()?;
    m.add_function(wrap_pyfunction!(legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(apply, m)?)?;
    m.add_function(wrap_pyfunction!(is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    m.add_function(wrap_pyfunction!(reward, m)?)?;
    Ok(())
}
