//! What a console command needs before it is sent, and what the state must
//! show after: the declared change plus a frame condition (the parts it
//! should not touch did not change). Both are pure functions of settled
//! states; `settle` calls them for `Command::Console`.

use std::collections::BTreeMap;

use super::ids::slugify;
use super::{catalog, CombatPile, ConsoleCommand, Target};
use crate::settle::{player_turn, Unmet};
use crate::state::{
    BridgeId, CardId, CardRef, CombatView, Entity, GameState, Piles, Power, StatusId,
};

/// The most cards the hand holds.
const MAX_HAND: usize = 10;

/// `Ok` when the game would take the command in `state`; otherwise why not.
pub(crate) fn precheck(command: &ConsoleCommand, state: &GameState) -> Result<(), String> {
    use ConsoleCommand as C;
    if let C::Probe(line) = command {
        return match command.spec() {
            Some(_) => Ok(()),
            None => Err(format!(
                "`{}` is not a console command in game version {}",
                line.split_whitespace().next().unwrap_or(""),
                catalog::GAME_VERSION
            )),
        };
    }
    if matches!(command, C::ToggleInstant) {
        return Err("instant mode is a state, not a toggle: use `set_instant`".to_string());
    }
    if state.run.is_none() {
        return Err(format!("no run in progress (screen {})", state.screen));
    }
    match command {
        C::Fight(_) | C::RelicAdd(_) => return Ok(()),
        C::RelicRemove(r) => {
            let owned = state
                .run
                .iter()
                .flat_map(|run| &run.relics)
                .any(|n| slugify(n) == r.as_str());
            return if owned {
                Ok(())
            } else {
                Err(format!("the player has no {r}"))
            };
        }
        _ => {}
    }
    let combat = player_turn(state)?;
    // A command whose effect is already true would pass its check on the
    // first read, before the game had acted.
    let no_op = match command {
        C::Damage { amount: 0, .. }
        | C::DamageAllEnemies { amount: 0 }
        | C::Block { amount: 0, .. }
        | C::Energy { amount: 0 }
        | C::HealPlayer { amount: 0 }
        | C::Draw { count: 0 } => Some("an amount of 0 changes nothing"),
        C::HealPlayer { .. } if combat.player.hp >= combat.player.max_hp => {
            Some("the player is at max hp")
        }
        C::Draw { .. }
            if combat.piles.hand.len() >= MAX_HAND
                || combat.piles.draw.len() + combat.piles.discard.len() == 0 =>
        {
            Some("there is nothing to draw or no room in the hand")
        }
        _ => None,
    };
    if let Some(why) = no_op {
        return Err(why.to_string());
    }
    match command {
        C::Damage { target, .. } | C::Block { target, .. } => living(combat, *target),
        C::Power { amount, target, .. } => {
            if *amount == 0 {
                return Err("the game applies positive power amounts only".to_string());
            }
            living(combat, *target)
        }
        C::DamageAllEnemies { .. } => {
            if combat.enemies.iter().any(|e| e.is_alive) {
                Ok(())
            } else {
                Err("no living enemy".to_string())
            }
        }
        C::AddCard { card, pile } => {
            if catalog::CARDS.binary_search(&card.sts2_id()).is_err() {
                return Err(format!(
                    "{} is not a card in game version {}",
                    card.sts2_id(),
                    catalog::GAME_VERSION
                ));
            }
            if *pile == CombatPile::Hand && combat.piles.hand.len() >= MAX_HAND {
                return Err(format!("the hand is full ({MAX_HAND})"));
            }
            Ok(())
        }
        C::RemoveCardFromHand { card } => {
            if combat.piles.hand.iter().any(|c| c.id == known(*card)) {
                Ok(())
            } else {
                Err(format!("{} is not in the hand", card.sts2_id()))
            }
        }
        C::Upgrade { hand_index } => match combat.piles.hand.get(*hand_index) {
            None => Err(format!(
                "hand index {hand_index} is out of range (hand size {})",
                combat.piles.hand.len()
            )),
            Some(c) if c.upgraded => Err(format!("{} is already upgraded", c.id.name())),
            Some(_) => Ok(()),
        },
        _ => Ok(()),
    }
}

fn living(combat: &CombatView, target: Target) -> Result<(), String> {
    match target {
        Target::Player => Ok(()),
        Target::Enemy(i) => match combat.enemies.get(i) {
            Some(e) if e.is_alive => Ok(()),
            Some(e) => Err(format!("enemy {i} ({}) is dead", e.entity_id)),
            None => Err(format!(
                "enemy {i} does not exist ({} enemies)",
                combat.enemies.len()
            )),
        },
    }
}

fn known(card: CardId) -> Entity<CardId> {
    Entity::Known(card)
}

fn unmet(field: &str, expected: impl std::fmt::Display, observed: impl std::fmt::Display) -> Unmet {
    Unmet::new(format!("{field} {expected}"), format!("{field} {observed}"))
}

fn require<T: PartialEq + std::fmt::Display>(
    field: &str,
    expected: T,
    observed: T,
) -> Result<(), Unmet> {
    if expected == observed {
        Ok(())
    } else {
        Err(unmet(
            field,
            format!("= {expected}"),
            format!("= {observed}"),
        ))
    }
}

type Counts = BTreeMap<(String, bool), usize>;

fn hand_counts(piles: &Piles) -> Counts {
    let mut out = Counts::new();
    for c in &piles.hand {
        *out.entry((c.id.name().to_string(), c.upgraded))
            .or_default() += 1;
    }
    out
}

fn pile_counts(cards: &[CardRef]) -> Counts {
    let mut out = Counts::new();
    for c in cards {
        *out.entry((c.id.name().to_string(), c.upgraded))
            .or_default() += 1;
    }
    out
}

fn show(counts: &Counts) -> String {
    if counts.is_empty() {
        return "empty".to_string();
    }
    let items: Vec<String> = counts
        .iter()
        .map(|((name, up), n)| format!("{name}{} x{n}", if *up { "+" } else { "" }))
        .collect();
    items.join(", ")
}

/// The four piles as multisets, in `[hand, draw, discard, exhaust]` order.
fn piles(p: &Piles) -> [(&'static str, Counts); 4] {
    [
        ("piles.hand", hand_counts(p)),
        ("piles.draw", pile_counts(&p.draw)),
        ("piles.discard", pile_counts(&p.discard)),
        ("piles.exhaust", pile_counts(&p.exhaust)),
    ]
}

fn pile_name(pile: CombatPile) -> &'static str {
    match pile {
        CombatPile::Hand => "piles.hand",
        CombatPile::Draw => "piles.draw",
        CombatPile::Discard => "piles.discard",
        CombatPile::Exhaust => "piles.exhaust",
    }
}

/// Every pile equals its `expected` multiset, where `expected` is the
/// before-state's with `change` applied to the named pile.
fn piles_are(
    before: &Piles,
    after: &Piles,
    change: impl Fn(&str, &mut Counts),
) -> Result<(), Unmet> {
    for ((name, was), (_, now)) in piles(before).into_iter().zip(piles(after)) {
        let mut expected = was;
        change(name, &mut expected);
        expected.retain(|_, n| *n > 0);
        if expected != now {
            return Err(unmet(name, show(&expected), show(&now)));
        }
    }
    Ok(())
}

fn bump(counts: &mut Counts, key: (String, bool), by: isize) {
    let n = counts.entry(key).or_default();
    *n = n.saturating_add_signed(by);
}

fn amount_i32(amount: u32) -> i32 {
    i32::try_from(amount).unwrap_or(i32::MAX)
}

/// Hp and block after `amount` unpowered damage.
fn after_damage(hp: i32, block: i32, amount: i32) -> (i32, i32) {
    (
        (hp - (amount - block).max(0)).max(0),
        (block - amount).max(0),
    )
}

/// Whether the power is the one the console calls `id`: any class name the
/// registry knows for it, or the raw class name for one it does not.
fn is_power(power: &Entity<StatusId>, id: &str) -> bool {
    match power {
        Entity::Known(status) => status.bridge_names().iter().any(|n| slugify(n) == id),
        Entity::Unknown(name) => slugify(name) == id,
    }
}

fn power_total(powers: &[Power], id: &str) -> i32 {
    powers
        .iter()
        .filter(|p| is_power(&p.id, id))
        .map(|p| p.amount)
        .sum()
}

fn relic_total(state: &GameState, id: &str) -> Result<usize, Unmet> {
    let run = state
        .run
        .as_ref()
        .ok_or_else(|| unmet("run", "present", "none"))?;
    Ok(run.relics.iter().filter(|r| slugify(r) == id).count())
}

fn combats<'a>(
    before: &'a GameState,
    after: &'a GameState,
) -> Result<(&'a CombatView, &'a CombatView), Unmet> {
    let b = before
        .combat
        .as_ref()
        .ok_or_else(|| unmet("combat", "present before", "none"))?;
    let a = after
        .combat
        .as_ref()
        .ok_or_else(|| unmet("combat", "still in progress", "ended"))?;
    Ok((b, a))
}

/// Everything but the targeted creature kept its hp and block.
fn others_unchanged(b: &CombatView, a: &CombatView, except: Option<Target>) -> Result<(), Unmet> {
    if except != Some(Target::Player) {
        require("player.hp", b.player.hp, a.player.hp)?;
        require("player.block", b.player.block, a.player.block)?;
    }
    for (i, (was, now)) in b.enemies.iter().zip(&a.enemies).enumerate() {
        if except == Some(Target::Enemy(i)) {
            continue;
        }
        require(&format!("enemies[{i}].hp"), was.hp, now.hp)?;
        require(&format!("enemies[{i}].block"), was.block, now.block)?;
    }
    Ok(())
}

/// `Ok` when `after` shows `command`'s effect over `before`.
pub(crate) fn postcondition(
    command: &ConsoleCommand,
    before: &GameState,
    after: &GameState,
) -> Result<(), Unmet> {
    use ConsoleCommand as C;
    match command {
        C::Fight(_) => {
            let c = after
                .combat
                .as_ref()
                .ok_or_else(|| unmet("combat", "started", "none"))?;
            let fresh = before.combat.as_ref().is_none_or(|was| was != c);
            if !fresh || c.enemies.is_empty() {
                return Err(unmet("combat", "a new combat", "the combat before"));
            }
            require("combat.round", 1, c.round)?;
            require("screen", "COMBAT_PLAYER_TURN", after.screen.as_str())
        }
        C::Win => {
            if after.combat.is_none() {
                Ok(())
            } else {
                Err(unmet("combat", "won", "still in progress"))
            }
        }
        C::RelicAdd(r) | C::RelicRemove(r) => {
            let was = relic_total(before, r.as_str())?;
            let now = relic_total(after, r.as_str())?;
            let expected = if matches!(command, C::RelicAdd(_)) {
                was + 1
            } else {
                was.saturating_sub(1)
            };
            require(&format!("run.relics[{r}] count"), expected, now)
        }
        C::Probe(_) => {
            if before == after {
                Err(unmet("state", "to change", "unchanged"))
            } else {
                Ok(())
            }
        }
        C::ToggleInstant => Ok(()),
        C::Damage { .. } | C::DamageAllEnemies { .. } if after.combat.is_none() => {
            lethal(command, before)
        }
        _ => in_combat(command, before, after),
    }
}

/// The combat ended: the damage must have been enough to end it.
fn lethal(command: &ConsoleCommand, before: &GameState) -> Result<(), Unmet> {
    let b = before
        .combat
        .as_ref()
        .ok_or_else(|| unmet("combat", "present before", "none"))?;
    let kills = |hp: i32, block: i32, amount: i32| after_damage(hp, block, amount).0 == 0;
    let ok = match command {
        ConsoleCommand::Damage { target, amount } => {
            let (hp, block) = vitals(b, *target)?;
            kills(hp, block, amount_i32(*amount))
        }
        ConsoleCommand::DamageAllEnemies { amount } => b
            .enemies
            .iter()
            .filter(|e| e.is_alive)
            .all(|e| kills(e.hp, e.block, amount_i32(*amount))),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(unmet(
            "combat",
            "to end only if the damage was lethal",
            "ended",
        ))
    }
}

fn in_combat(command: &ConsoleCommand, before: &GameState, after: &GameState) -> Result<(), Unmet> {
    use ConsoleCommand as C;
    let (b, a) = combats(before, after)?;
    let same_piles = |b: &CombatView, a: &CombatView| piles_are(&b.piles, &a.piles, |_, _| {});
    match command {
        C::AddCard { card, pile } => {
            let key = (card.bridge_name().to_string(), false);
            piles_are(&b.piles, &a.piles, |name, counts| {
                if name == pile_name(*pile) {
                    bump(counts, key.clone(), 1);
                }
            })
        }
        C::RemoveCardFromHand { card } => {
            let name = card.bridge_name();
            let first = b
                .piles
                .hand
                .iter()
                .find(|c| c.id == known(*card))
                .map(|c| (name.to_string(), c.upgraded));
            piles_are(&b.piles, &a.piles, |pile, counts| {
                if let (true, Some(key)) = (pile == "piles.hand", first.clone()) {
                    bump(counts, key, -1);
                }
            })
        }
        C::Damage { target, amount } => {
            let amount = amount_i32(*amount);
            let (hp, block) = vitals(b, *target)?;
            let (now_hp, now_block) = vitals(a, *target)?;
            let (want_hp, want_block) = after_damage(hp, block, amount);
            let who = who(*target);
            require(&format!("{who}.hp"), want_hp, now_hp)?;
            require(&format!("{who}.block"), want_block, now_block)?;
            same_entity(b, a, *target)?;
            others_unchanged(b, a, Some(*target))?;
            same_piles(b, a)
        }
        C::DamageAllEnemies { amount } => {
            let amount = amount_i32(*amount);
            for (i, (was, now)) in b.enemies.iter().zip(&a.enemies).enumerate() {
                let (hp, block) = if was.is_alive {
                    after_damage(was.hp, was.block, amount)
                } else {
                    (was.hp, was.block)
                };
                require(&format!("enemies[{i}].hp"), hp, now.hp)?;
                require(&format!("enemies[{i}].block"), block, now.block)?;
            }
            require("player.hp", b.player.hp, a.player.hp)?;
            same_piles(b, a)
        }
        C::HealPlayer { amount } => {
            let want = (b.player.hp.saturating_add(amount_i32(*amount))).min(b.player.max_hp);
            require("player.hp", want, a.player.hp)?;
            same_piles(b, a)
        }
        C::Block { target, amount } => {
            let (_, block) = vitals(b, *target)?;
            let (_, now) = vitals(a, *target)?;
            require(
                &format!("{}.block", who(*target)),
                block.saturating_add(amount_i32(*amount)),
                now,
            )?;
            same_entity(b, a, *target)?;
            same_piles(b, a)
        }
        C::Energy { amount } => {
            require(
                "player.energy",
                b.player.energy.saturating_add(amount_i32(*amount)),
                a.player.energy,
            )?;
            same_piles(b, a)
        }
        C::Power {
            power,
            amount,
            target,
        } => {
            let powers = |c: &CombatView| -> Result<Vec<Power>, Unmet> {
                match target {
                    Target::Player => Ok(c.player.powers.clone()),
                    Target::Enemy(i) => c
                        .enemies
                        .get(*i)
                        .map(|e| e.powers.clone())
                        .ok_or_else(|| unmet("enemies", format!("[{i}]"), "missing")),
                }
            };
            let was = power_total(&powers(b)?, power.as_str());
            let now = power_total(&powers(a)?, power.as_str());
            require(
                &format!("{}.powers[{power}]", who(*target)),
                was.saturating_add(amount_i32(*amount)),
                now,
            )?;
            same_entity(b, a, *target)?;
            same_piles(b, a)
        }
        C::Draw { count } => {
            let (hand, draw, discard) = (
                b.piles.hand.len(),
                b.piles.draw.len(),
                b.piles.discard.len(),
            );
            let drawn = usize::try_from(*count)
                .unwrap_or(usize::MAX)
                .min(MAX_HAND.saturating_sub(hand))
                .min(draw + discard);
            require("piles.hand size", hand + drawn, a.piles.hand.len())?;
            require(
                "piles.draw + piles.discard size",
                draw + discard - drawn,
                a.piles.draw.len() + a.piles.discard.len(),
            )?;
            require(
                "piles.exhaust size",
                b.piles.exhaust.len(),
                a.piles.exhaust.len(),
            )
        }
        C::Upgrade { hand_index } => {
            let card = b
                .piles
                .hand
                .get(*hand_index)
                .ok_or_else(|| unmet("piles.hand", format!("[{hand_index}]"), "missing"))?;
            let name = card.id.name().to_string();
            piles_are(&b.piles, &a.piles, |pile, counts| {
                if pile == "piles.hand" {
                    bump(counts, (name.clone(), false), -1);
                    bump(counts, (name.clone(), true), 1);
                }
            })
        }
        C::Fight(_)
        | C::Win
        | C::RelicAdd(_)
        | C::RelicRemove(_)
        | C::Probe(_)
        | C::ToggleInstant => Ok(()),
    }
}

fn who(target: Target) -> String {
    match target {
        Target::Player => "player".to_string(),
        Target::Enemy(i) => format!("enemies[{i}]"),
    }
}

fn vitals(c: &CombatView, target: Target) -> Result<(i32, i32), Unmet> {
    match target {
        Target::Player => Ok((c.player.hp, c.player.block)),
        Target::Enemy(i) => c
            .enemies
            .get(i)
            .map(|e| (e.hp, e.block))
            .ok_or_else(|| unmet("enemies", format!("[{i}]"), "missing")),
    }
}

/// The enemy at the target index is the same one as before.
fn same_entity(b: &CombatView, a: &CombatView, target: Target) -> Result<(), Unmet> {
    if let Target::Enemy(i) = target {
        let (was, now) = (b.enemies.get(i), a.enemies.get(i));
        if was.map(|e| &e.entity_id) != now.map(|e| &e.entity_id) {
            return Err(unmet(
                &format!("enemies[{i}]"),
                format!("still {}", was.map_or("?", |e| e.entity_id.as_str())),
                now.map_or("gone", |e| e.entity_id.as_str()),
            ));
        }
    }
    Ok(())
}
