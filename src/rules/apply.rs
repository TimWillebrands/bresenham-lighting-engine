//! Outcome → next value of the token/item it lands on. Pure; the game layer turns these into writes.

use super::character::*;
use super::stats::live_modifiers;
use super::tiles::px_of;
use super::types::*;

/// Pools with `delta` on `pool`; undeclared keys materialise first, `clamp` caps at capacity (restore).
fn adjusted(token: &Token, ctx: &Ctx, pool: &str, delta: f64, clamp: bool) -> Pools {
    let views = any_pool_views_of(token, ctx);
    let mut out = pools_of(token, ctx, Some(&views));
    let next = out.get(pool).copied().unwrap_or(0.0) + delta;
    let cap = views
        .iter()
        .find(|v| v.name == pool)
        .map_or(f64::INFINITY, |v| v.capacity - v.blocked);
    out.insert(
        pool.to_string(),
        if clamp { next.min(cap).max(0.0) } else { next },
    );
    out
}

pub fn spent(token: &Token, ctx: &Ctx, pool: &str, amount: f64) -> Token {
    Token {
        pools: Some(adjusted(token, ctx, pool, -amount, false)),
        ..token.clone()
    }
}

/// Yid of the token an outcome writes; none = an item spend/restore, a spawn or a give.
pub fn outcome_token_yid(o: &Outcome) -> Option<&str> {
    match o {
        Outcome::Spawn { .. } | Outcome::Give { .. } => None,
        Outcome::Spend { item, token, .. } => item.is_none().then_some(token.as_str()),
        Outcome::Restore { item: Some(_), .. } => None,
        _ => o.target(),
    }
}

/// Token after an outcome lands on it; item spends, spawns and gives pass it through.
pub fn token_after(token: &Token, o: &Outcome, ctx: &Ctx) -> Token {
    match o {
        Outcome::Spend {
            item: None,
            pool,
            amount,
            ..
        } => spent(token, ctx, pool, *amount),
        Outcome::Spend { .. } | Outcome::Spawn { .. } | Outcome::Give { .. } => token.clone(),
        Outcome::Damage { amount, warded, .. } => {
            let soaked = if ctx.roles.ward.is_some() {
                warded.unwrap_or(0.0)
            } else {
                0.0
            };
            let after = match &ctx.roles.ward {
                Some(w) if soaked > 0.0 => spent(token, ctx, w, soaked),
                _ => token.clone(),
            };
            match &ctx.roles.health {
                None => after,
                Some(h) => spent(&after, ctx, h, amount - soaked),
            }
        }
        Outcome::Restore { item: Some(_), .. } => token.clone(),
        Outcome::Restore { pool, amount, .. } => Token {
            pools: Some(adjusted(token, ctx, pool, *amount, true)),
            ..token.clone()
        },
        Outcome::Modify {
            stat,
            pool,
            reach,
            amount,
            until,
            ..
        } => {
            let mut modifiers = live_modifiers(token.modifiers.as_ref(), ctx.turn());
            modifiers.push(Modifier {
                stat: *stat,
                pool: pool.clone(),
                reach: *reach,
                amount: *amount,
                until: *until,
            });
            Token {
                modifiers: Some(modifiers),
                ..token.clone()
            }
        }
        Outcome::Teleport { tile, .. } => Token {
            position: Some(px_of(*tile)),
            ..token.clone()
        },
        Outcome::Polymorph { def, until, .. } => Token {
            form: def.as_ref().map(|d| Form {
                def: d.clone(),
                until: *until,
            }),
            ..token.clone()
        },
    }
}

/// Item after a pool spend, and whether its `<pool empty="destroy">` says the instance is gone.
pub fn item_after_spend(
    item: &Item,
    def: Option<&ItemDef>,
    pool: &str,
    amount: f64,
) -> (Item, bool) {
    let mut pools = item.pools.clone().unwrap_or_default();
    let left = pools.get(pool).copied().unwrap_or(0.0) - amount;
    pools.insert(pool.to_string(), left);
    let destroy = left <= 0.0
        && def
            .and_then(|d| d.pools.iter().find(|p| p.name == pool))
            .is_some_and(|p| p.empty == Some(Empty::Destroy));
    (
        Item {
            pools: Some(pools),
            ..item.clone()
        },
        destroy,
    )
}

/// Item after a pool restore, clamped to 0..the def's capacity (unclamped above when undeclared).
pub fn item_after_restore(item: &Item, def: Option<&ItemDef>, pool: &str, amount: f64) -> Item {
    let cap = def
        .and_then(|d| d.pools.iter().find(|p| p.name == pool))
        .and_then(|p| p.capacity)
        .unwrap_or(f64::INFINITY);
    let mut pools = item.pools.clone().unwrap_or_default();
    let next = pools.get(pool).copied().unwrap_or(0.0) + amount;
    pools.insert(pool.to_string(), next.min(cap).max(0.0));
    Item {
        pools: Some(pools),
        ..item.clone()
    }
}
