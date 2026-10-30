//! The turn ledger (`GameAction`): a projection of what happened, never the source of truth.

use super::types::*;
use serde::{Deserialize, Serialize};

/// A rider or further verb that fired alongside the primary one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rider {
    pub carrier: String,
    pub verb: Verb,
    pub amount: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum GameAction {
    /// `path` = flat world-px tile centres `[x0, y0, x1, y1, …]`.
    Move { actor_yid: String, path: Vec<f64> },
    /// A cast that hit a token; `carrier` = the spell's name; `tile` = where a trap hit landed.
    Attack {
        actor_yid: String,
        target_yid: String,
        carrier: String,
        description: String,
        damage: f64,
        riders: Vec<Rider>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tile: Option<Pt>,
    },
    /// Any other cast: restore/modify/give on a token, spawn on a tile.
    Ability {
        actor_yid: String,
        carrier: String,
        verb: Verb,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_yid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tile: Option<Pt>,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pool: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stat: Option<Stat>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        def: Option<String>,
        riders: Vec<Rider>,
    },
    /// Speaker tile frozen at speak time; hearers resolve live.
    Say {
        actor_yid: String,
        text: String,
        tile: Pt,
        volume: f64,
    },
    /// Door worked from an adjacent tile.
    Door {
        actor_yid: String,
        door_yid: String,
        open: bool,
    },
}

/// Who a ledger entry hit, as its roll log names them.
pub struct LedgerTarget<'a> {
    pub yid: &'a str,
    pub name: Option<&'a str>,
    pub defence: Option<f64>,
}

/// The cast's own first outcome (trigger-less, non-spend); none when every chance roll failed.
pub fn primary_of(outcomes: &[Outcome]) -> Option<&Outcome> {
    outcomes
        .iter()
        .find(|o| !o.is_spend() && o.trigger().is_none())
}

/// Everything that fired besides `primary`, as the ledger stores it.
pub fn riders_of(outcomes: &[Outcome], primary: Option<&Outcome>) -> Vec<Rider> {
    outcomes
        .iter()
        .filter(|o| !o.is_spend() && !primary.is_some_and(|p| std::ptr::eq(p, *o)))
        .map(|o| Rider {
            carrier: o.carrier().unwrap_or_default().to_string(),
            verb: o.verb().unwrap_or(Verb::Damage),
            amount: o.amount(),
        })
        .collect()
}

fn num(n: f64) -> String {
    format!("{n}")
}

/// Roll log of a hit: the summary line, then every die.
fn hit_description(target: &LedgerTarget, hit: &Outcome) -> String {
    let Outcome::Damage {
        amount,
        blocks,
        rolls,
        warded,
        pierced,
        ..
    } = hit
    else {
        return String::new();
    };
    let mut lines = vec![format!(
        "Hit '{}' (defence {}{}): {} blocked, {} damage{}",
        target.name.unwrap_or(target.yid),
        target.defence.map_or("?".into(), num),
        pierced.map_or(String::new(), |p| format!(", {} pierced", num(p))),
        num(*blocks),
        num(*amount),
        warded.map_or(String::new(), |w| format!(" ({} warded)", num(w))),
    )];
    lines.extend(rolls.iter().cloned());
    lines.join("\n")
}

/// Ledger entry for a resolved cast: damage on a token = attack (with the roll log), anything else = ability.
pub fn ledger_entry_of(
    actor_yid: &str,
    carrier: &str,
    spell: &Spell,
    target: Option<&LedgerTarget>,
    tile: Option<Pt>,
    outcomes: &[Outcome],
) -> GameAction {
    let primary = primary_of(outcomes);
    let riders = riders_of(outcomes, primary);
    if let (Some(p @ Outcome::Damage { amount, .. }), Some(t)) = (primary, target) {
        return GameAction::Attack {
            actor_yid: actor_yid.into(),
            target_yid: t.yid.into(),
            carrier: carrier.into(),
            description: hit_description(t, p),
            damage: *amount,
            riders,
            tile: None,
        };
    }
    let effect = spell
        .effects
        .iter()
        .find(|e| Some(e.verb()) == primary.and_then(Outcome::verb))
        .or(spell.effects.first());
    let (pool, stat, def) = match effect {
        Some(Effect::Restore { pool, .. }) => (Some(pool.clone()), None, None),
        Some(Effect::Modify { pool, stat, .. }) => (pool.clone(), *stat, None),
        Some(Effect::Spawn { def, .. }) | Some(Effect::Give { def, .. }) => {
            (None, None, Some(def.clone()))
        }
        Some(Effect::Polymorph { def, .. }) => (None, None, def.clone()),
        _ => (None, None, None),
    };
    GameAction::Ability {
        actor_yid: actor_yid.into(),
        carrier: carrier.into(),
        verb: effect.map_or(Verb::Damage, Effect::verb),
        target_yid: target.map(|t| t.yid.to_string()),
        tile,
        amount: primary.map_or(0.0, Outcome::amount),
        pool,
        stat,
        def,
        riders,
    }
}

/// Ledger entry for riders fired outside a cast (a trap's `enter`): the first outcome on `target`; none = nothing landed on it.
pub fn rider_entry_of(
    actor_yid: &str,
    target: &LedgerTarget,
    tile: Pt,
    outcomes: &[Outcome],
) -> Option<GameAction> {
    let primary = outcomes.iter().find(|o| {
        !matches!(o, Outcome::Spend { .. } | Outcome::Spawn { .. })
            && o.target() == Some(target.yid)
    })?;
    let riders = riders_of(outcomes, Some(primary));
    let carrier = primary.carrier().unwrap_or_default().to_string();
    if let Outcome::Damage { amount, .. } = primary {
        return Some(GameAction::Attack {
            actor_yid: actor_yid.into(),
            target_yid: target.yid.into(),
            carrier,
            description: hit_description(target, primary),
            damage: *amount,
            riders,
            tile: Some(tile),
        });
    }
    let (pool, stat, def) = match primary {
        Outcome::Restore { pool, .. } => (Some(pool.clone()), None, None),
        Outcome::Modify { pool, stat, .. } => (pool.clone(), *stat, None),
        Outcome::Give { def, .. } => (None, None, Some(def.clone())),
        Outcome::Polymorph { def, .. } => (None, None, def.clone()),
        _ => (None, None, None),
    };
    Some(GameAction::Ability {
        actor_yid: actor_yid.into(),
        carrier,
        verb: primary.verb().unwrap_or(Verb::Damage),
        target_yid: Some(target.yid.into()),
        tile: Some(tile),
        amount: primary.amount(),
        pool,
        stat,
        def,
        riders,
    })
}
