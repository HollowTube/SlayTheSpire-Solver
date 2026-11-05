//! Compiles the committed console command catalog into `console_catalog.rs`.
//!
//! `scripts/console_catalog.py extract` writes the catalog from the game
//! files; this turns it into one `const` per command, so typed constructors
//! that name a command, or pin its usage string, stop compiling when a game
//! patch renames or reshapes it.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::{env, fs};

use serde_json::Value;

/// The game version the crate is built and tested against. The catalog for
/// another version is added next to this one, then this is bumped.
const GAME_VERSION: &str = "0.107.1";

/// Names the generated file defines besides the command constants.
const RESERVED: &[&str] = &[
    "ALL",
    "CARDS",
    "ENCOUNTERS",
    "POWERS",
    "RELICS",
    "PILES",
    "GAME_VERSION",
    "GAME_COMMIT",
];

fn string_list(catalog: &Value, domain: &str) -> Vec<String> {
    let list = catalog["ids"][domain]
        .as_array()
        .unwrap_or_else(|| panic!("catalog has no ids.{domain}"));
    let mut ids: Vec<String> = list
        .iter()
        .map(|v| v.as_str().expect("id is a string").to_string())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets this"));
    let path = manifest
        .join("catalog")
        .join(GAME_VERSION)
        .join("console_commands.json");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", path.display());

    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nRun scripts/console_catalog.py extract --game-version {GAME_VERSION}",
            path.display()
        )
    });
    let catalog: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
    assert_eq!(
        catalog["game_version"].as_str(),
        Some(GAME_VERSION),
        "{} is for another game version",
        path.display()
    );

    let mut out = String::new();
    writeln!(out, "pub const GAME_VERSION: &str = {GAME_VERSION:?};").unwrap();
    writeln!(
        out,
        "pub const GAME_COMMIT: &str = {:?};",
        catalog["game_commit"].as_str().unwrap_or_default()
    )
    .unwrap();

    let commands = catalog["commands"].as_object().expect("catalog.commands");
    let mut consts = Vec::new();
    for (name, c) in commands {
        assert!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'),
            "command name {name:?} is not a valid constant name"
        );
        let konst = name.to_ascii_uppercase();
        assert!(
            !RESERVED.contains(&konst.as_str()),
            "command {name:?} would generate the constant {konst}, which the catalog already uses: namespace the command constants"
        );
        let flag = |key: &str| {
            c[key]
                .as_bool()
                .unwrap_or_else(|| panic!("command {name}: {key} is not a bool"))
        };
        let text = |key: &str| {
            c[key]
                .as_str()
                .unwrap_or_else(|| panic!("command {name}: {key} is not a string"))
        };
        writeln!(
            out,
            "pub const {konst}: CommandSpec = CommandSpec {{ name: {name:?}, args: {:?}, \
             description: {:?}, networked: {}, debug_only: {}, enters_room: {} }};",
            text("args"),
            text("description"),
            flag("networked"),
            flag("debug_only"),
            flag("enters_room"),
        )
        .unwrap();
        consts.push(konst);
    }
    writeln!(
        out,
        "pub const ALL: &[CommandSpec] = &[{}];",
        consts.join(", ")
    )
    .unwrap();

    for (domain, konst) in [
        ("cards", "CARDS"),
        ("encounters", "ENCOUNTERS"),
        ("powers", "POWERS"),
        ("relics", "RELICS"),
        ("piles", "PILES"),
    ] {
        let ids = string_list(&catalog, domain);
        let items: Vec<String> = ids.iter().map(|i| format!("{i:?}")).collect();
        writeln!(out, "pub const {konst}: &[&str] = &[{}];", items.join(", ")).unwrap();
    }

    let dest =
        PathBuf::from(env::var("OUT_DIR").expect("cargo sets this")).join("console_catalog.rs");
    fs::write(dest, out).expect("write console_catalog.rs");
}
