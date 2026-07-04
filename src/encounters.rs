use crate::state::Monster;
use rand::Rng;
use rand_pcg::Pcg32;

/// How a named encounter resolves into one or more monsters.
#[derive(Clone)]
pub(crate) enum EncounterShape {
    /// One monster, HP sampled from [hp_min, hp_max].
    Single {
        name: String,
        hp_min: i32,
        hp_max: i32,
    },
    /// Fixed set of monsters, each with its own HP range.
    Fixed {
        monsters: Vec<(String, i32, i32)>, // (name, hp_min, hp_max)
    },
    /// Draw `count` names from a pool, each with HP sampled from
    /// [hp_min, hp_max]. Never repeats the immediately-previous name
    /// (reshuffle-bag strategy) unless pool size is 1.
    Pool {
        pool: Vec<String>,
        count: usize,
        hp_min: i32,
        hp_max: i32,
    },
}

/// Resolve an encounter shape into its monster list, using `rng` for HP
/// sampling and pool-order selection.
pub(crate) fn resolve_shape(shape: &EncounterShape, rng: &mut Pcg32) -> Vec<Monster> {
    match shape {
        EncounterShape::Single {
            name,
            hp_min,
            hp_max,
        } => {
            let hp = rng.gen_range(*hp_min..=*hp_max);
            vec![Monster::new(
                hp,
                0,
                None,
                Some(name.clone()),
                0,
                Vec::new(),
                None,
                None,
                0,
                Vec::new(),
            )]
        }
        EncounterShape::Fixed { monsters } => monsters
            .iter()
            .map(|(name, hp_min, hp_max)| {
                let hp = rng.gen_range(*hp_min..=*hp_max);
                Monster::new(
                    hp,
                    0,
                    None,
                    Some(name.clone()),
                    0,
                    Vec::new(),
                    None,
                    None,
                    0,
                    Vec::new(),
                )
            })
            .collect(),
        EncounterShape::Pool {
            pool,
            count,
            hp_min,
            hp_max,
        } => draw_from_pool(pool, *count, *hp_min, *hp_max, rng),
    }
}

/// Draw `slots` names from `pool` (in order), never repeating the
/// immediately-previous draw, and reshuffling (cycling) once the bag is
/// exhausted rather than erroring. Pool of size ≤ 1 is allowed to repeat.
pub(crate) fn draw_from_pool(
    pool: &[String],
    slots: usize,
    hp_min: i32,
    hp_max: i32,
    rng: &mut Pcg32,
) -> Vec<Monster> {
    if pool.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(slots);
    let mut bag: Vec<&String> = Vec::new();
    let mut last: Option<&String> = None;
    while result.len() < slots {
        if bag.is_empty() {
            let mut shuffled: Vec<&String> = pool.iter().collect();
            // Fisher-Yates shuffle
            let n = shuffled.len();
            for i in (1..n).rev() {
                let j = rng.gen_range(0..=i);
                shuffled.swap(i, j);
            }
            // Avoid repeating the last draw from the previous bag
            if pool.len() > 1 {
                while shuffled.first() == last.as_ref() {
                    let j = rng.gen_range(1..n);
                    shuffled.swap(0, j);
                }
            }
            bag = shuffled;
        }
        let picked = bag.remove(0);
        last = Some(picked);
        let hp = rng.gen_range(hp_min..=hp_max);
        result.push(Monster::new(
            hp,
            0,
            None,
            Some(picked.clone()),
            0,
            Vec::new(),
            None,
            None,
            0,
            Vec::new(),
        ));
    }
    result
}

/// Look up an encounter by name. Returns `None` when the name is unknown.
pub(crate) fn encounter_def(name: &str) -> Option<EncounterShape> {
    match name {
        // ── Overgrowth single-monster encounters ──
        "Inklet" => Some(EncounterShape::Single {
            name: "Inklet".into(),
            hp_min: 35,
            hp_max: 40,
        }),
        "Snapping Jaxfruit" => Some(EncounterShape::Single {
            name: "Snapping Jaxfruit".into(),
            hp_min: 48,
            hp_max: 54,
        }),
        "Slithering Strangler" => Some(EncounterShape::Single {
            name: "Slithering Strangler".into(),
            hp_min: 40,
            hp_max: 46,
        }),
        "Cubex Construct" => Some(EncounterShape::Single {
            name: "Cubex Construct".into(),
            hp_min: 55,
            hp_max: 62,
        }),
        "Mawler" => Some(EncounterShape::Single {
            name: "Mawler".into(),
            hp_min: 60,
            hp_max: 68,
        }),
        "Vine Shambler" => Some(EncounterShape::Single {
            name: "Vine Shambler".into(),
            hp_min: 45,
            hp_max: 52,
        }),
        "Flyconid" => Some(EncounterShape::Single {
            name: "Flyconid".into(),
            hp_min: 74,
            hp_max: 78,
        }),
        "Fogmog" => Some(EncounterShape::Single {
            name: "Fogmog".into(),
            hp_min: 48,
            hp_max: 54,
        }),
        "Shrinker Beetle" => Some(EncounterShape::Single {
            name: "Shrinker Beetle".into(),
            hp_min: 36,
            hp_max: 42,
        }),
        "Twig Slime" => Some(EncounterShape::Single {
            name: "Twig Slime".into(),
            hp_min: 18,
            hp_max: 24,
        }),
        "Twig Slime (S)" => Some(EncounterShape::Single {
            name: "Twig Slime (S)".into(),
            hp_min: 7,
            hp_max: 11,
        }),
        "Twig Slime (M)" => Some(EncounterShape::Single {
            name: "Twig Slime (M)".into(),
            hp_min: 26,
            hp_max: 28,
        }),
        "Leaf Slime (S)" => Some(EncounterShape::Single {
            name: "Leaf Slime (S)".into(),
            hp_min: 11,
            hp_max: 15,
        }),
        "Leaf Slime (M)" => Some(EncounterShape::Single {
            name: "Leaf Slime (M)".into(),
            hp_min: 32,
            hp_max: 35,
        }),

        // ── Overgrowth elite encounters ──
        "Byrdonis" => Some(EncounterShape::Single {
            name: "Byrdonis".into(),
            hp_min: 88,
            hp_max: 96,
        }),
        "Phrog Parasite" => Some(EncounterShape::Single {
            name: "Phrog Parasite".into(),
            hp_min: 72,
            hp_max: 80,
        }),
        "Bygone Effigy" => Some(EncounterShape::Single {
            name: "Bygone Effigy".into(),
            hp_min: 80,
            hp_max: 88,
        }),

        // ── Overgrowth boss encounters ──
        "Ceremonial Beast" => Some(EncounterShape::Single {
            name: "Ceremonial Beast".into(),
            hp_min: 220,
            hp_max: 240,
        }),
        "The Kin" => Some(EncounterShape::Single {
            name: "The Kin".into(),
            hp_min: 250,
            hp_max: 270,
        }),
        "Vantom" => Some(EncounterShape::Single {
            name: "Vantom".into(),
            hp_min: 200,
            hp_max: 220,
        }),

        // ── Individual Ruby Raiders (as single-monster encounters) ──
        "Axe Ruby Raider" => Some(EncounterShape::Single {
            name: "Axe Ruby Raider".into(),
            hp_min: 38,
            hp_max: 42,
        }),
        "Assassin Ruby Raider" => Some(EncounterShape::Single {
            name: "Assassin Ruby Raider".into(),
            hp_min: 38,
            hp_max: 42,
        }),
        "Brute Ruby Raider" => Some(EncounterShape::Single {
            name: "Brute Ruby Raider".into(),
            hp_min: 38,
            hp_max: 42,
        }),
        "Crossbow Ruby Raider" => Some(EncounterShape::Single {
            name: "Crossbow Ruby Raider".into(),
            hp_min: 38,
            hp_max: 42,
        }),
        "Tracker Ruby Raider" => Some(EncounterShape::Single {
            name: "Tracker Ruby Raider".into(),
            hp_min: 38,
            hp_max: 42,
        }),

        // ── Non-Overgrowth test monsters (for existing test fixtures) ──
        "Nibbit" => Some(EncounterShape::Single {
            name: "Nibbit".into(),
            hp_min: 20,
            hp_max: 28,
        }),
        "Fuzzy Wurm Crawler" => Some(EncounterShape::Single {
            name: "Fuzzy Wurm Crawler".into(),
            hp_min: 20,
            hp_max: 28,
        }),

        // ── Overgrowth fixed multi-monster encounters ──
        "OvergrowthCrawlers" => Some(EncounterShape::Fixed {
            monsters: vec![
                ("Shrinker Beetle".into(), 35, 40),
                ("Twig Slime".into(), 20, 24),
            ],
        }),

        // ── Overgrowth pool encounters ──
        "ruby_raiders" => Some(EncounterShape::Pool {
            pool: vec![
                "Axe Ruby Raider".into(),
                "Assassin Ruby Raider".into(),
                "Brute Ruby Raider".into(),
                "Crossbow Ruby Raider".into(),
                "Tracker Ruby Raider".into(),
            ],
            count: 2,
            hp_min: 38,
            hp_max: 42,
        }),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_draw_from_pool_no_repeat() {
        let pool: Vec<String> = ["A", "B", "C"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut rng = Pcg32::seed_from_u64(42);
        let monsters = draw_from_pool(&pool, 4, 30, 40, &mut rng);
        assert_eq!(monsters.len(), 4);
        for i in 1..monsters.len() {
            assert_ne!(
                monsters[i].name.as_deref(),
                monsters[i - 1].name.as_deref(),
                "consecutive draws must differ"
            );
        }
    }

    #[test]
    fn test_single_encounter() {
        let shape = encounter_def("Flyconid").unwrap();
        let mut rng = Pcg32::seed_from_u64(42);
        let monsters = resolve_shape(&shape, &mut rng);
        assert_eq!(monsters.len(), 1);
        assert_eq!(monsters[0].name.as_deref(), Some("Flyconid"));
        assert!(monsters[0].fighter.hp >= 74 && monsters[0].fighter.hp <= 78);
    }

    #[test]
    fn test_fixed_encounter() {
        let shape = encounter_def("OvergrowthCrawlers").unwrap();
        let mut rng = Pcg32::seed_from_u64(42);
        let monsters = resolve_shape(&shape, &mut rng);
        assert_eq!(monsters.len(), 2);
        assert_eq!(monsters[0].name.as_deref(), Some("Shrinker Beetle"));
        assert_eq!(monsters[1].name.as_deref(), Some("Twig Slime"));
    }

    #[test]
    fn test_pool_encounter() {
        let shape = encounter_def("ruby_raiders").unwrap();
        let mut rng = Pcg32::seed_from_u64(42);
        let monsters = resolve_shape(&shape, &mut rng);
        assert_eq!(monsters.len(), 2);
        // No consecutive repeats (pool size 5 > 1)
        for i in 1..monsters.len() {
            assert_ne!(monsters[i].name, monsters[i - 1].name);
        }
    }

    #[test]
    fn test_seeded_reproducibility() {
        let shape = encounter_def("Fogmog").unwrap();
        let mut rng_a = Pcg32::seed_from_u64(999);
        let mut rng_b = Pcg32::seed_from_u64(999);
        let a = resolve_shape(&shape, &mut rng_a);
        let b = resolve_shape(&shape, &mut rng_b);
        assert_eq!(a[0].fighter.hp, b[0].fighter.hp);
    }

    #[test]
    fn test_unknown_encounter_is_none() {
        assert!(encounter_def("NonExistentMonster").is_none());
    }
}
