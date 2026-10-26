//! The registry: every card, monster and status the solver knows, defined
//! once in [`registry.rs`](registry). Each row expands to an enum variant with
//! its display name, STS2 model id, bridge class names and per-kind fields.
//!
//! `sts-sim` re-exports these types, `_sts_sim` hands the rows to Python
//! (`sts_sim.sim._registry` builds `CardName`, `MonsterName`, `StatusName`
//! from them), and `sts-bridge` parses wire names into them.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Character suffixes the bridge appends to card class names (`StrikeIronclad`).
pub const CARD_CHARACTER_SUFFIXES: &[&str] =
    &["Ironclad", "Silent", "Defect", "Watcher", "Huntress"];

/// An id the live game reports by class name.
pub trait BridgeId: Copy + Sized + 'static {
    /// The one bridge name this id is written back out as.
    fn bridge_name(self) -> &'static str;
    /// Parse any bridge class name for this id.
    fn from_bridge(name: &str) -> Option<Self>;
}

macro_rules! some_or_none {
    () => {
        None
    };
    ($v:expr) => {
        Some($v)
    };
}

macro_rules! value_or {
    ($default:expr;) => {
        $default
    };
    ($default:expr; $v:expr) => {
        $v
    };
}

macro_rules! id_enum {
    ($name:ident { $($variant:ident => $display:literal),* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),* }

        impl $name {
            /// Canonical sim display name.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $display),* }
            }
            /// Parse from sim display name. Returns `None` for unknown names.
            #[allow(clippy::should_implement_trait)]
            pub fn from_str(s: &str) -> Option<Self> {
                match s { $($display => Some($name::$variant),)* _ => None }
            }
            /// The Rust variant name, e.g. `StrikeIronclad`.
            pub fn variant(self) -> &'static str {
                match self { $($name::$variant => stringify!($variant)),* }
            }
            /// Every row, in declaration order.
            pub fn all() -> &'static [Self] {
                &[$($name::$variant),*]
            }
        }
    };
}

macro_rules! sts2_ids {
    ($name:ident { $($variant:ident => $sts2:literal),* }) => {
        impl $name {
            /// STS2 model ID (screaming snake case).
            pub fn sts2_id(self) -> &'static str {
                match self { $($name::$variant => $sts2),* }
            }
            /// Parse from STS2 model ID. Returns `None` for unknown IDs.
            pub fn from_sts2(s: &str) -> Option<Self> {
                match s { $($sts2 => Some($name::$variant),)* _ => None }
            }
        }
    };
}

/// `Variant => "Display" / "STS2_ID" [, aliases = [..]] [, in_card_pool = false];`
///
/// Bridge names are derived: the variant, the variant plus each
/// [`CARD_CHARACTER_SUFFIXES`] entry it doesn't already end in, then `aliases`.
macro_rules! cards {
    ($(
        $variant:ident => $display:literal / $sts2:literal
        $(, aliases = [$($alias:literal),* $(,)?])?
        $(, in_card_pool = $pool:literal)?
    ;)*) => {
        id_enum!(CardId { $($variant => $display),* });
        sts2_ids!(CardId { $($variant => $sts2),* });

        impl CardId {
            /// Whether the card can be drawn as a card reward.
            pub fn in_card_pool(self) -> bool {
                match self { $(CardId::$variant => value_or!(true; $($pool)?)),* }
            }
            /// Bridge class names that the variant-plus-suffix rule doesn't produce.
            pub fn bridge_aliases(self) -> &'static [&'static str] {
                match self { $(CardId::$variant => &[$($($alias),*)?]),* }
            }
        }
    };
}

/// `Variant => "Display" / "STS2_ID", bridge = [..] [, hp = N] [, encounter = "key"];`
macro_rules! monsters {
    ($(
        $variant:ident => $display:literal / $sts2:literal,
        bridge = [$($bridge:literal),+ $(,)?]
        $(, hp = $hp:literal)?
        $(, encounter = $encounter:literal)?
    ;)*) => {
        id_enum!(MonsterId { $($variant => $display),* });
        sts2_ids!(MonsterId { $($variant => $sts2),* });

        impl MonsterId {
            /// Every bridge class name for this monster; the first is canonical.
            pub fn bridge_names(self) -> &'static [&'static str] {
                match self { $(MonsterId::$variant => &[$($bridge),+]),* }
            }
            /// Canonical starting HP; `None` for monsters with no fixed scenario HP.
            pub fn hp(self) -> Option<u32> {
                match self { $(MonsterId::$variant => some_or_none!($($hp)?)),* }
            }
            /// The `sts_sim.bench.Encounter` key of this monster's single-monster scenario.
            pub fn encounter(self) -> Option<&'static str> {
                match self { $(MonsterId::$variant => some_or_none!($($encounter)?)),* }
            }
        }

        impl BridgeId for MonsterId {
            fn bridge_name(self) -> &'static str {
                self.bridge_names()[0]
            }
            fn from_bridge(name: &str) -> Option<Self> {
                match name { $($($bridge)|+ => Some(MonsterId::$variant),)* _ => None }
            }
        }
    };
}

/// `Variant => "sim name", bridge = [..];` — the sim name is `Status::name()` in the engine.
macro_rules! statuses {
    ($(
        $variant:ident => $display:literal, bridge = [$($bridge:literal),+ $(,)?]
    ;)*) => {
        id_enum!(StatusId { $($variant => $display),* });

        impl StatusId {
            /// Every bridge class name for this status; the first is canonical.
            pub fn bridge_names(self) -> &'static [&'static str] {
                match self { $(StatusId::$variant => &[$($bridge),+]),* }
            }
        }

        impl BridgeId for StatusId {
            fn bridge_name(self) -> &'static str {
                self.bridge_names()[0]
            }
            fn from_bridge(name: &str) -> Option<Self> {
                match name { $($($bridge)|+ => Some(StatusId::$variant),)* _ => None }
            }
        }
    };
}

mod registry;

pub use registry::{CardId, MonsterId, StatusId};

impl CardId {
    /// Every bridge class name for this card; the first (the variant) is canonical.
    pub fn bridge_names(self) -> Vec<String> {
        let variant = self.variant();
        let mut names = vec![variant.to_string()];
        names.extend(
            CARD_CHARACTER_SUFFIXES
                .iter()
                .filter(|suffix| !variant.ends_with(*suffix))
                .map(|suffix| format!("{variant}{suffix}")),
        );
        names.extend(self.bridge_aliases().iter().map(|alias| alias.to_string()));
        names
    }
}

impl BridgeId for CardId {
    fn bridge_name(self) -> &'static str {
        self.variant()
    }

    fn from_bridge(name: &str) -> Option<Self> {
        static BY_BRIDGE_NAME: OnceLock<HashMap<String, CardId>> = OnceLock::new();
        BY_BRIDGE_NAME
            .get_or_init(|| {
                CardId::all()
                    .iter()
                    .flat_map(|&id| id.bridge_names().into_iter().map(move |n| (n, id)))
                    .collect()
            })
            .get(name)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn assert_unique<'a>(kind: &str, values: impl Iterator<Item = &'a str>) {
        let mut seen = HashSet::new();
        for v in values {
            assert!(seen.insert(v), "{kind}: duplicate {v:?}");
        }
    }

    #[test]
    fn card_ids_round_trip() {
        for &id in CardId::all() {
            assert_eq!(CardId::from_str(id.as_str()), Some(id), "{id:?}");
            assert_eq!(CardId::from_sts2(id.sts2_id()), Some(id), "{id:?}");
            for name in id.bridge_names() {
                assert_eq!(CardId::from_bridge(&name), Some(id), "{name}");
            }
            assert_eq!(CardId::from_bridge(id.bridge_name()), Some(id), "{id:?}");
        }
        assert_unique("card display", CardId::all().iter().map(|id| id.as_str()));
        assert_unique("card sts2_id", CardId::all().iter().map(|id| id.sts2_id()));
    }

    #[test]
    fn monster_ids_round_trip() {
        for &id in MonsterId::all() {
            assert_eq!(MonsterId::from_str(id.as_str()), Some(id), "{id:?}");
            assert_eq!(MonsterId::from_sts2(id.sts2_id()), Some(id), "{id:?}");
            for name in id.bridge_names() {
                assert_eq!(MonsterId::from_bridge(name), Some(id), "{name}");
            }
        }
        assert_unique(
            "monster display",
            MonsterId::all().iter().map(|id| id.as_str()),
        );
        assert_unique(
            "monster sts2_id",
            MonsterId::all().iter().map(|id| id.sts2_id()),
        );
        assert_unique(
            "monster bridge name",
            MonsterId::all()
                .iter()
                .flat_map(|id| id.bridge_names().iter().copied()),
        );
    }

    #[test]
    fn status_ids_round_trip() {
        for &id in StatusId::all() {
            assert_eq!(StatusId::from_str(id.as_str()), Some(id), "{id:?}");
            for name in id.bridge_names() {
                assert_eq!(StatusId::from_bridge(name), Some(id), "{name}");
            }
        }
        assert_unique(
            "status bridge name",
            StatusId::all()
                .iter()
                .flat_map(|id| id.bridge_names().iter().copied()),
        );
    }

    #[test]
    fn card_bridge_names_follow_the_suffix_rule() {
        assert_eq!(
            CardId::from_bridge("StrikeIronclad"),
            Some(CardId::StrikeIronclad)
        );
        assert_eq!(
            CardId::from_bridge("AggressionSilent"),
            Some(CardId::Aggression)
        );
        assert_eq!(
            CardId::from_bridge("Ascender'sBane"),
            Some(CardId::AscendersBane)
        );
        assert_eq!(CardId::from_bridge("StrikeIroncladIronclad"), None);
        assert_eq!(CardId::from_bridge("Strike"), None);
    }

    #[test]
    fn unknown_names_do_not_parse() {
        assert_eq!(MonsterId::from_bridge("AcidSlime_L"), None);
        assert_eq!(StatusId::from_bridge("NotAPower"), None);
        assert_eq!(CardId::from_sts2("NOT_A_CARD"), None);
    }

    #[test]
    fn per_kind_fields() {
        assert!(!CardId::AscendersBane.in_card_pool());
        assert!(!CardId::Burn.in_card_pool());
        assert!(CardId::Bash.in_card_pool());
        assert_eq!(MonsterId::JawWorm.hp(), Some(44));
        assert_eq!(MonsterId::JawWorm.encounter(), Some("jaw-worm"));
        assert_eq!(MonsterId::KinPriest.encounter(), None);
        assert_eq!(MonsterId::EyeWithTeeth.hp(), None);
        assert_eq!(StatusId::Vulnerable.bridge_name(), "Vulnerable");
    }
}
