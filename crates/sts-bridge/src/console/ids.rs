//! Ids the console takes, checked against the catalog's id domains so a name
//! the game does not know (an STS1 name, a typo, a removed model) is an error
//! before anything is sent.

use std::fmt;

use super::catalog;

/// An id that is not in the game's catalog.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{id}` is not a {domain} id in game version {}",
    catalog::GAME_VERSION
)]
pub struct UnknownId {
    pub domain: &'static str,
    pub id: String,
}

pub(crate) const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

pub(crate) const fn contains(domain: &[&str], id: &str) -> bool {
    let mut i = 0;
    while i < domain.len() {
        if str_eq(domain[i], id) {
            return true;
        }
        i += 1;
    }
    false
}

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident, $domain:ident, $label:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(&'static str);

        impl $name {
            /// The id as the console takes it (screaming snake case).
            pub fn new(id: &str) -> Result<Self, UnknownId> {
                catalog::$domain
                    .binary_search(&id)
                    .map(|i| Self(catalog::$domain[i]))
                    .map_err(|_| UnknownId { domain: $label, id: id.to_string() })
            }

            pub fn as_str(&self) -> &'static str {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.0)
            }
        }
    };
}

id_newtype!(
    /// An encounter id, e.g. [`NIBBITS_WEAK`].
    Encounter, ENCOUNTERS, "encounter"
);
id_newtype!(
    /// A power id, e.g. `VULNERABLE_POWER`.
    PowerName, POWERS, "power"
);
id_newtype!(
    /// A relic id, e.g. `BURNING_BLOOD`.
    Relic, RELICS, "relic"
);

/// One Nibbit alone: the encounter the live tests fight.
pub const NIBBITS_WEAK: Encounter = Encounter("NIBBITS_WEAK");
const _: () = assert!(
    contains(catalog::ENCOUNTERS, NIBBITS_WEAK.0),
    "NIBBITS_WEAK is no longer an encounter in the catalog"
);

/// Port of the game's `StringHelper.Slugify` (`ExpectAFight` -> `EXPECT_A_FIGHT`),
/// which names every model: a bridge class name maps to its console id.
pub fn slugify(name: &str) -> String {
    let chars: Vec<char> = name.trim().chars().collect();
    let mut out = String::new();
    let mut in_space = false;
    for (i, &ch) in chars.iter().enumerate() {
        let split = i > 0 && ch.is_ascii_uppercase() && chars[i - 1].is_ascii_alphanumeric();
        if ch.is_whitespace() {
            if !in_space {
                out.push('_');
            }
            in_space = true;
            continue;
        }
        in_space = false;
        if split {
            out.push('_');
        }
        let up = ch.to_ascii_uppercase();
        if up.is_ascii_uppercase() || up.is_ascii_digit() || up == '_' {
            out.push(up);
        }
    }
    out
}
