//! Recorded game traces: what the live game did for each case, replayed
//! against the sim without the game.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sts_bridge::GameState;

use crate::case::{classify, record_step, CaseError, ParityCase, ParityOutcome, Verdict};
use crate::expected::Expected;
use crate::map::Carry;
use crate::step::ParityStep;

pub const FORMAT_VERSION: u32 = 1;

/// Bump in any change to what the game side records: fixture setup or
/// projection, `Fixtures::ensure`/settle/`act` semantics, the `StepAction` to
/// `Command` translation, or the `Carry` shape. The audit fails until every
/// trace is re-recorded.
pub const HARNESS_VERSION: u32 = 1;

/// Cap on one serialized `GameState` (compact JSON) in a trace. The largest
/// recorded state is 2552 bytes.
pub const MAX_STATE_BYTES: usize = 4 * 1024;

#[derive(Debug)]
pub struct TraceError(pub String);

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TraceError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameBuild {
    pub version: String,
    pub commit: String,
}

impl GameBuild {
    /// From the game's `release_info.json`; the leading `v` of the version is
    /// dropped to match `catalog::GAME_VERSION`.
    pub fn from_release_info(json: &str) -> Result<Self, TraceError> {
        let v: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| TraceError(format!("release_info.json does not parse: {e}")))?;
        let field = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .map(str::to_string)
                .ok_or_else(|| TraceError(format!("release_info.json has no string `{k}`")))
        };
        let version = field("version")?;
        Ok(GameBuild {
            version: version.strip_prefix('v').unwrap_or(&version).to_string(),
            commit: field("commit")?,
        })
    }
}

/// The game build to record against, from the install directory
/// (`STS2_GAME_DIR`) or the PCK next to it (`STS2_PCK`). Refuses when neither
/// is given, when `release_info.json` cannot be read, or when its version is
/// not `expected`.
pub fn game_build(
    game_dir: Option<&Path>,
    pck: Option<&Path>,
    expected: &str,
) -> Result<GameBuild, TraceError> {
    let dir = match (game_dir, pck.and_then(Path::parent)) {
        (Some(d), _) => d.to_path_buf(),
        (None, Some(d)) => d.to_path_buf(),
        (None, None) => {
            return Err(TraceError(
                "set STS2_GAME_DIR (the game's install directory) or STS2_PCK to record traces"
                    .into(),
            ))
        }
    };
    let file = dir.join("release_info.json");
    let text = fs::read_to_string(&file)
        .map_err(|e| TraceError(format!("cannot read {}: {e}", file.display())))?;
    let build = GameBuild::from_release_info(&text)?;
    if build.version != expected {
        return Err(TraceError(format!(
            "the game is {} but this crate is built against catalog {expected}; \
             update GAME_VERSION and the catalog first (README: Card parity)",
            build.version
        )));
    }
    Ok(build)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendBuild {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceStep {
    pub step: ParityStep,
    /// The carry going into the step.
    pub carry: Carry,
    pub game_after: GameState,
}

/// One case as the game ran it. A step's `before` is the previous step's
/// `game_after` (the first one's is `initial`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub format_version: u32,
    pub game: GameBuild,
    pub backend: BackendBuild,
    pub harness_version: u32,
    pub case: ParityCase,
    pub initial: GameState,
    pub steps: Vec<TraceStep>,
}

impl Trace {
    /// `Err` with the reason when the outcome has nothing to replay.
    pub fn from_outcome(
        outcome: &ParityOutcome,
        game: &GameBuild,
        backend: &BackendBuild,
    ) -> Result<Self, String> {
        if outcome.case.blocked.is_some() {
            return Err("the case is blocked".into());
        }
        if let Some(e) = &outcome.error {
            return Err(format!("the run failed: {}: {}", e.kind, e.message));
        }
        let Some(initial) = &outcome.verified else {
            return Err("the fixture was never verified".into());
        };
        if outcome.steps.is_empty() {
            return Err("the case ran no steps".into());
        }
        Ok(Trace {
            format_version: FORMAT_VERSION,
            game: game.clone(),
            backend: backend.clone(),
            harness_version: HARNESS_VERSION,
            case: outcome.case.clone(),
            initial: initial.clone(),
            steps: outcome
                .steps
                .iter()
                .map(|s| TraceStep {
                    step: s.step.clone(),
                    carry: s.carry.clone(),
                    game_after: s.game_after.clone(),
                })
                .collect(),
        })
    }

    pub fn file_name(&self) -> String {
        format!("{}.json", self.case.name)
    }

    /// Byte-stable JSON: struct field order, a trailing newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("a trace serializes");
        s.push('\n');
        s
    }

    /// The compact size of each recorded state: `initial`, then each step.
    pub fn state_sizes(&self) -> Vec<usize> {
        let size = |g: &GameState| serde_json::to_vec(g).expect("a state serializes").len();
        std::iter::once(size(&self.initial))
            .chain(self.steps.iter().map(|s| size(&s.game_after)))
            .collect()
    }

    fn label(&self) -> String {
        let backend = match &self.backend.version {
            Some(v) => format!("{} {v}", self.backend.name),
            None => self.backend.name.clone(),
        };
        format!(
            "{} [game {}, {backend}, harness {}]",
            self.case.name, self.game.version, self.harness_version
        )
    }
}

/// Writes `<dir>/<case.name>.json` through a temp file and a rename, so an
/// interrupted run never leaves half a trace.
pub fn write(dir: &Path, trace: &Trace) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(trace.file_name());
    let tmp = dir.join(format!(".{}.tmp", trace.file_name()));
    let done = fs::write(&tmp, trace.to_json()).and_then(|()| fs::rename(&tmp, &path));
    if done.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    done?;
    Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The message names the serde path.
    Parse(String),
    Format(u64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceFile {
    pub file: String,
    pub trace: Result<Trace, LoadError>,
}

/// Parses one trace file's text.
pub fn parse(text: &str) -> Result<Trace, LoadError> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| LoadError::Parse(e.to_string()))?;
    match v.get("format_version").and_then(|f| f.as_u64()) {
        Some(n) if n == u64::from(FORMAT_VERSION) => {}
        Some(n) => return Err(LoadError::Format(n)),
        None => return Err(LoadError::Parse("no format_version".into())),
    }
    serde_path_to_error::deserialize(v).map_err(|e| LoadError::Parse(e.to_string()))
}

/// Every file in `dir` but hidden ones, by file name; one that is not a
/// `.json` trace comes back as a parse error so the audit names it. A missing
/// directory is empty.
pub fn load_dir(dir: &Path) -> io::Result<Vec<TraceFile>> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let p = entry?.path();
        let hidden = p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'));
        if p.is_file() && !hidden {
            paths.push(p);
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let file = p.file_name().unwrap().to_string_lossy().into_owned();
        let trace = if p.extension() == Some(OsStr::new("json")) {
            parse(&fs::read_to_string(&p)?)
        } else {
            Err(LoadError::Parse(
                "not a <case>.json trace; remove it".into(),
            ))
        };
        out.push(TraceFile { file, trace });
    }
    Ok(out)
}

/// Rebuilds the outcome from the recorded game states, running the sim on
/// each recorded `before` and carry.
pub fn replay(trace: &Trace) -> ParityOutcome {
    let mut case = trace.case.clone();
    case.blocked = None;
    let mut outcome = ParityOutcome {
        case,
        verified: Some(trace.initial.clone()),
        steps: Vec::new(),
        error: None,
        backend_version: trace.backend.version.clone(),
    };
    let mut before = &trace.initial;
    for s in &trace.steps {
        match record_step(before, &s.carry, &s.step, &s.game_after, &trace.case.masks) {
            Ok((so, _)) => outcome.steps.push(so),
            Err(f) => {
                outcome.error = Some(CaseError {
                    kind: "Mapping".into(),
                    message: format!("{}: {f}", s.step.card),
                });
                break;
            }
        }
        before = &s.game_after;
    }
    outcome
}

/// The live command that re-records a trace's case.
pub fn rerun_command(case: &ParityCase) -> String {
    let card = case.cards.first().map_or("", String::as_str);
    format!(
        "STS2_GAME_DIR=<game dir> STS2_RECORD_TRACES=1 cargo test -p sts-parity --test live_cards -- --live --card \"{card}\""
    )
}

/// Replays twice (the results must be equal) and classifies against
/// `expected`. One line per mismatch; empty when the case passes.
pub fn replay_failures(trace: &Trace, expected: &[Expected]) -> Vec<String> {
    let first = replay(trace);
    let mut lines = Vec::new();
    if first != replay(trace) {
        lines.push("replay is not deterministic".to_string());
    }
    match classify(&first, expected) {
        Verdict::Fail(l) | Verdict::Stale(l) => lines.extend(l),
        _ => {}
    }
    if lines.is_empty() {
        return lines;
    }
    let label = trace.label();
    let mut out: Vec<String> = lines.into_iter().map(|l| format!("{label} {l}")).collect();
    out.push(format!("    re-run live: {}", rerun_command(&trace.case)));
    out
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Audit {
    pub failures: Vec<String>,
    /// Traces recorded on another game version (reported, not failed).
    pub other_version: Vec<String>,
    pub traces: usize,
    pub exempt_blocked: usize,
}

impl Audit {
    pub fn report(&self, game_version: &str) -> String {
        let mut s = format!(
            "traces: {} loaded, {} blocked cases exempt, {} failures, {} on another game version (current {game_version})",
            self.traces,
            self.exempt_blocked,
            self.failures.len(),
            self.other_version.len()
        );
        if !self.other_version.is_empty() {
            s.push_str(&format!(
                "\n  re-record after the patch: {}",
                self.other_version.join(", ")
            ));
        }
        s
    }
}

fn unblocked(case: &ParityCase) -> ParityCase {
    let mut c = case.clone();
    c.blocked = None;
    c
}

/// Pure: whether the traces cover `declared` (`cases(&[])`) and are current.
pub fn audit(
    files: &[TraceFile],
    declared: &[ParityCase],
    harness_version: u32,
    game_version: &str,
) -> Audit {
    let mut a = Audit::default();
    let by_name: BTreeMap<&str, &ParityCase> =
        declared.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut present: BTreeSet<String> = BTreeSet::new();
    let mut old_harness = Vec::new();
    for f in files {
        present.insert(f.file.trim_end_matches(".json").to_string());
        let t = match &f.trace {
            Ok(t) => t,
            Err(LoadError::Parse(m)) => {
                a.failures.push(format!("{}: does not parse: {m}", f.file));
                continue;
            }
            Err(LoadError::Format(n)) => {
                a.failures.push(format!(
                    "{}: unknown format_version {n} (this build reads {FORMAT_VERSION})",
                    f.file
                ));
                continue;
            }
        };
        a.traces += 1;
        if f.file != t.file_name() {
            a.failures.push(format!(
                "{}: file name does not match its case {}; rename it",
                f.file, t.case.name
            ));
        }
        present.insert(t.case.name.clone());
        if t.harness_version != harness_version {
            old_harness.push(t.case.name.clone());
        }
        match by_name.get(t.case.name.as_str()) {
            None => a.failures.push(format!(
                "{}: orphan, case {} is not declared; delete it",
                f.file, t.case.name
            )),
            Some(c) if unblocked(c) != unblocked(&t.case) => a.failures.push(format!(
                "{}: stale: the declared case {} changed; re-record it ({})",
                f.file,
                t.case.name,
                rerun_command(c)
            )),
            Some(_) => {}
        }
        if t.game.version != game_version {
            a.other_version
                .push(format!("{} (game {})", t.case.name, t.game.version));
        }
    }
    if !old_harness.is_empty() {
        a.failures.push(format!(
            "harness_version differs from {harness_version} in {} traces ({}); bump: re-record all traces",
            old_harness.len(),
            old_harness.join(", ")
        ));
    }
    for c in declared {
        if c.blocked.is_some() {
            a.exempt_blocked += 1;
        } else if !present.contains(&c.name) {
            a.failures.push(format!(
                "missing trace for {}: record it ({})",
                c.name,
                rerun_command(c)
            ));
        }
    }
    a
}
