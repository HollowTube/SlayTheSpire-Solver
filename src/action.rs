use pyo3::prelude::*;
use pyo3::types::PyString;

/// Hash using Python's own string hash so that `hash(action) == hash(str(action))`
/// and Action objects work as drop-in keys in string-keyed dicts.
fn py_str_hash(py: Python<'_>, s: &str) -> PyResult<isize> {
    PyString::new(py, s).hash()
}

#[pyclass]
#[derive(Clone)]
pub struct EndTurnAction {}

#[pymethods]
impl EndTurnAction {
    #[new]
    pub fn new() -> Self {
        EndTurnAction {}
    }

    fn __str__(&self) -> &'static str {
        "EndTurn"
    }

    fn __repr__(&self) -> &'static str {
        "EndTurnAction()"
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if other.is_instance_of::<EndTurnAction>() {
            return true;
        }
        other.extract::<String>().map(|s| s == "EndTurn").unwrap_or(false)
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        py_str_hash(py, "EndTurn")
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other_s = other.str()?.to_string();
        Ok("EndTurn" < other_s.as_str())
    }

    /// Substring containment shim — `"EndTurn" in EndTurnAction()` → True.
    fn __contains__(&self, item: &str) -> bool {
        "EndTurn".contains(item)
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PlayCardAction {
    #[pyo3(get)]
    pub card: String,
}

#[pymethods]
impl PlayCardAction {
    #[new]
    pub fn new(card: String) -> Self {
        PlayCardAction { card }
    }

    fn __str__(&self) -> String {
        format!("PlayCard:{}", self.card)
    }

    fn __repr__(&self) -> String {
        format!("PlayCardAction({:?})", self.card)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<PlayCardAction>>() {
            return self.card == other.card;
        }
        other.extract::<String>().map(|s| s == self.__str__()).unwrap_or(false)
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        py_str_hash(py, &self.__str__())
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let self_s = self.__str__();
        let other_s = other.str()?.to_string();
        Ok(self_s < other_s)
    }

    /// Substring containment shim — `"PlayCard:" in PlayCardAction("Strike")` → True.
    fn __contains__(&self, item: &str) -> bool {
        self.__str__().contains(item)
    }

    /// Base card name without the upgrade suffix (e.g. ``"Strike"`` from ``"Strike+"``).
    #[getter]
    pub fn card_name(&self) -> &str {
        self.card.strip_suffix('+').unwrap_or(&self.card)
    }

    /// Whether the card is upgraded (``card`` ends with ``"+"``).
    #[getter]
    pub fn upgraded(&self) -> bool {
        self.card.ends_with('+')
    }
}

#[pyclass]
#[derive(Clone)]
pub struct SelectTargetAction {
    #[pyo3(get)]
    pub monster_index: usize,
}

#[pymethods]
impl SelectTargetAction {
    #[new]
    pub fn new(monster_index: usize) -> Self {
        SelectTargetAction { monster_index }
    }

    fn __str__(&self) -> String {
        format!("SelectTarget:Monster:{}", self.monster_index)
    }

    fn __repr__(&self) -> String {
        format!("SelectTargetAction({})", self.monster_index)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<SelectTargetAction>>() {
            return self.monster_index == other.monster_index;
        }
        other.extract::<String>().map(|s| s == self.__str__()).unwrap_or(false)
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        py_str_hash(py, &self.__str__())
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let self_s = self.__str__();
        let other_s = other.str()?.to_string();
        Ok(self_s < other_s)
    }

    /// Substring containment shim — `"SelectTarget:" in SelectTargetAction(0)` → True.
    fn __contains__(&self, item: &str) -> bool {
        self.__str__().contains(item)
    }
}

#[pyclass]
#[derive(Clone)]
pub struct UpgradeCardFromHandAction {
    #[pyo3(get)]
    pub card: String,
}

#[pymethods]
impl UpgradeCardFromHandAction {
    #[new]
    pub fn new(card: String) -> Self {
        UpgradeCardFromHandAction { card }
    }

    fn __str__(&self) -> String {
        format!("UpgradeCardFromHand:{}", self.card)
    }

    fn __repr__(&self) -> String {
        format!("UpgradeCardFromHandAction({:?})", self.card)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<UpgradeCardFromHandAction>>() {
            return self.card == other.card;
        }
        other.extract::<String>().map(|s| s == self.__str__()).unwrap_or(false)
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        py_str_hash(py, &self.__str__())
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let self_s = self.__str__();
        let other_s = other.str()?.to_string();
        Ok(self_s < other_s)
    }

    fn __contains__(&self, item: &str) -> bool {
        self.__str__().contains(item)
    }
}

#[pyclass]
#[derive(Clone)]
pub struct ExhaustCardFromHandAction {
    #[pyo3(get)]
    pub card: String,
}

#[pymethods]
impl ExhaustCardFromHandAction {
    #[new]
    pub fn new(card: String) -> Self {
        ExhaustCardFromHandAction { card }
    }

    fn __str__(&self) -> String {
        format!("ExhaustCardFromHand:{}", self.card)
    }

    fn __repr__(&self) -> String {
        format!("ExhaustCardFromHandAction({:?})", self.card)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        if let Ok(other) = other.extract::<PyRef<ExhaustCardFromHandAction>>() {
            return self.card == other.card;
        }
        other.extract::<String>().map(|s| s == self.__str__()).unwrap_or(false)
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        py_str_hash(py, &self.__str__())
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let self_s = self.__str__();
        let other_s = other.str()?.to_string();
        Ok(self_s < other_s)
    }

    fn __contains__(&self, item: &str) -> bool {
        self.__str__().contains(item)
    }
}
