//! Stat and pool arithmetic: `per` scalers, pool capacities and merges, class matrix, effective stats.

use super::types::*;

/// What an actor's `per` reads; `full` none = missing reads 0.
pub struct Gauges<'a> {
    pub stats: &'a Stats,
    pub pools: &'a Pools,
    pub full: Option<&'a Pools>,
    pub tallies: Tallies,
}

pub fn per_value(per: &Per, g: &Gauges) -> f64 {
    match per {
        Per::Gauge { pool, missing } => {
            let now = g.pools.get(pool).copied().unwrap_or(0.0);
            if !*missing {
                return now;
            }
            let full = g.full.and_then(|f| f.get(pool)).copied().unwrap_or(now);
            (full - now).max(0.0)
        }
        Per::Tally(Tally::Damage) => g.tallies.damage,
        Per::Tally(Tally::Blocked) => g.tallies.blocked,
        Per::Stat(s) => g.stats.get(*s),
    }
}

/// `amount` × scaler, floored: the one rounding every `per` uses.
pub fn scaled_amount(amount: f64, by: f64) -> f64 {
    (amount * by).floor()
}

pub fn per_text(per: &Per) -> String {
    match per {
        Per::Stat(s) => stat_name(*s).into(),
        Per::Tally(Tally::Damage) => "damage".into(),
        Per::Tally(Tally::Blocked) => "blocked".into(),
        Per::Gauge { pool, missing } => {
            format!("{}{}", if *missing { "missing " } else { "" }, pool)
        }
    }
}

pub fn stat_name(s: Stat) -> &'static str {
    match s {
        Stat::Physique => "physique",
        Stat::Defence => "defence",
        Stat::Intelligence => "intelligence",
        Stat::Speed => "speed",
    }
}

/// Capacity from a decl: fixed, or stat × amount, plus pool deltas; never below 0.
pub fn capacity_of(decl: &PoolDecl, stats: Option<&Stats>, deltas: &[StatDelta]) -> f64 {
    let base = match (decl.capacity, decl.per, stats) {
        (Some(c), _, _) => c,
        (None, Some(per), Some(s)) => s.get(per) * decl.amount.unwrap_or(1.0),
        _ => 0.0,
    };
    let bonus: f64 = deltas
        .iter()
        .filter(|d| d.pool.as_deref() == Some(decl.name.as_str()))
        .map(|d| d.amount)
        .sum();
    (base + bonus).floor().max(0.0)
}

/// Later decls override earlier ones by name, field-wise; a new capacity/per replaces the old derivation.
pub fn merge_pools(layers: &[&[PoolDecl]]) -> Vec<PoolDecl> {
    let mut out: Vec<PoolDecl> = Vec::new();
    for layer in layers {
        for decl in layer.iter() {
            let Some(prev) = out.iter_mut().find(|p| p.name == decl.name) else {
                out.push(decl.clone());
                continue;
            };
            if decl.capacity.is_some() || decl.per.is_some() {
                prev.capacity = None;
                prev.per = None;
                prev.amount = None;
            }
            macro_rules! over {
                ($($f:ident),*) => { $(if decl.$f.is_some() { prev.$f = decl.$f.clone(); })* };
            }
            over!(capacity, per, amount, refill, empty, start, role, shows, spells);
        }
    }
    out
}

/// 1 × (weight − physique × mult) / mult, ceiled, never negative; taken off `moves` only.
pub fn carry_penalty(weight: f64, physique: f64, carry_mult: f64) -> f64 {
    if carry_mult <= 0.0 {
        return 0.0;
    }
    ((weight - physique * carry_mult) / carry_mult)
        .max(0.0)
        .ceil()
}

/// Role → pool name from game-level decls; the first decl claiming a role wins.
pub fn roles_of(pools: &[PoolDecl]) -> Roles {
    let mut out = Roles::default();
    for p in pools {
        if let Some(r) = p.role {
            let slot = out.slot(r);
            if slot.is_none() {
                *slot = Some(p.name.clone());
            }
        }
    }
    out
}

/// Non-game layers may override capacity but never add or drop a role.
pub fn without_roles(pools: &[PoolDecl]) -> Vec<PoolDecl> {
    pools
        .iter()
        .map(|p| PoolDecl {
            role: None,
            ..p.clone()
        })
        .collect()
}

/// Authored order key of a class cell: first stat = primary (ADR-0018).
fn ordered_key(stats: &[Stat]) -> Vec<Stat> {
    stats.to_vec()
}

/// Cell key in STATS order, so `"defence physique"` and `"physique defence"` meet.
fn cell_key(stats: &[Stat]) -> Vec<Stat> {
    STATS
        .iter()
        .copied()
        .filter(|s| stats.contains(s))
        .collect()
}

/// Matrix cell for base stats (ADR-0013/0018): dominant stat leading by `gap`, else the
/// authored pair, else that pair reversed; an ambiguous pair or missing cell = fallback.
pub fn class_of(stats: &Stats, classes: &ClassesDef) -> Class {
    let mut ranked = STATS;
    // Stable sort: ties rank in STATS order.
    ranked.sort_by(|a, b| {
        stats
            .get(*b)
            .partial_cmp(&stats.get(*a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let [s1, s2, s3, _] = ranked;
    let fallback = Class {
        name: classes.fallback.clone(),
        ..Class::default()
    };
    let by_key = |key: fn(&[Stat]) -> Vec<Stat>, wanted: Vec<Stat>| {
        classes.cells.iter().find(|c| key(&c.stats) == wanted)
    };
    let cell = if stats.get(s1) - stats.get(s2) >= classes.gap {
        by_key(ordered_key, vec![s1])
    } else if stats.get(s2) == stats.get(s3) {
        return fallback;
    } else {
        by_key(ordered_key, vec![s1, s2]).or_else(|| by_key(cell_key, cell_key(&[s1, s2])))
    };
    match cell {
        None => fallback,
        Some(c) => Class {
            name: c.name.clone(),
            spells: c.spells.clone(),
            pools: c.pools.clone(),
            inventory: c.inventory.clone(),
        },
    }
}

/// Whether an effect with this `while` is live on a carrier in `status`.
pub fn is_live(whilst: While, status: CarrierStatus) -> bool {
    match whilst {
        While::Always | While::Carried => true,
        While::Equipped => matches!(status, While::Equipped | While::Wielded | While::Always),
        While::Wielded => matches!(status, While::Wielded | While::Always),
    }
}

pub fn live_modifiers(modifiers: Option<&Vec<Modifier>>, turn: i64) -> Vec<Modifier> {
    modifiers.map_or(vec![], |ms| {
        ms.iter()
            .filter(|m| m.until.is_none_or(|u| u > turn))
            .cloned()
            .collect()
    })
}

/// `modify` verbs of live passive spells, with their `per`.
fn passive_modifies(carriers: &[LiveCarrier]) -> impl Iterator<Item = &Effect> {
    carriers.iter().flat_map(|c| {
        c.spells
            .iter()
            .filter(move |s| s.trigger == Trigger::Passive && is_live(s.while_, c.status))
            .flat_map(|s| {
                s.effects
                    .iter()
                    .filter(|e| matches!(e, Effect::Modify { .. }))
            })
    })
}

/// Flat passive `modify` verbs plus unexpired modifiers; `per` ones are `scaled_deltas`.
pub fn live_deltas(
    carriers: &[LiveCarrier],
    modifiers: Option<&Vec<Modifier>>,
    turn: i64,
) -> Vec<StatDelta> {
    let mut out: Vec<StatDelta> = passive_modifies(carriers)
        .filter_map(|e| match e {
            Effect::Modify {
                stat,
                pool,
                reach,
                amount,
                per: None,
                ..
            } => Some(StatDelta {
                stat: *stat,
                pool: pool.clone(),
                reach: *reach,
                amount: *amount,
                until: None,
            }),
            _ => None,
        })
        .collect();
    out.extend(live_modifiers(modifiers, turn));
    out
}

/// Live passive `modify … per` verbs scaled by `g`; callers pass gauges read without these (no cycle).
pub fn scaled_deltas(carriers: &[LiveCarrier], g: &Gauges) -> Vec<StatDelta> {
    passive_modifies(carriers)
        .filter_map(|e| match e {
            Effect::Modify {
                stat,
                pool,
                reach,
                amount,
                per: Some(per),
                ..
            } => Some(StatDelta {
                stat: *stat,
                pool: pool.clone(),
                reach: *reach,
                amount: scaled_amount(*amount, per_value(per, g)),
                until: None,
            }),
            _ => None,
        })
        .collect()
}

/// Base + every stat delta, floored at 0.
pub fn effective_stats(base: &Stats, deltas: &[StatDelta]) -> Stats {
    let mut out = *base;
    for d in deltas {
        if let Some(s) = d.stat {
            *out.get_mut(s) += d.amount;
        }
    }
    for s in STATS {
        *out.get_mut(s) = out.get(s).max(0.0);
    }
    out
}

/// Tiles each reach kind gains from live deltas, floored at 0.
pub fn reach_of(deltas: &[StatDelta]) -> Reach {
    let mut out = Reach::default();
    for d in deltas {
        match d.reach {
            Some(ReachKind::Cast) => out.cast = Some(out.cast.unwrap_or(0.0) + d.amount),
            Some(ReachKind::Interact) => {
                out.interact = Some(out.interact.unwrap_or(0.0) + d.amount)
            }
            None => {}
        }
    }
    out.cast = out.cast.map(|v| v.max(0.0));
    out.interact = out.interact.map(|v| v.max(0.0));
    out
}
