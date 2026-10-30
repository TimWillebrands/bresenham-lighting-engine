//! Character derivation over a [`Model`]: class, carriers, effective stats, pool views, combatants.

use super::model::Model;
use super::stats::*;
use super::tiles::tile_of;
use super::types::*;
use std::collections::BTreeMap;

pub const STAT_MIN: f64 = 1.0;
pub const STAT_MAX: f64 = 10.0;

/// The doc-reading seam (ADR-0012) as data: a model plus the roles it declares.
pub struct Ctx<'a> {
    pub model: &'a Model,
    pub roles: Roles,
}

impl<'a> Ctx<'a> {
    pub fn new(model: &'a Model) -> Self {
        Ctx {
            model,
            roles: roles_of(&model.game.pools),
        }
    }

    pub fn game(&self) -> &'a Game {
        &self.model.game
    }

    pub fn turn(&self) -> i64 {
        self.model.turn
    }

    pub fn item(&self, yid: &str) -> Option<&'a Item> {
        self.model.items.get(yid)
    }

    pub fn item_def(&self, id: Option<&str>) -> Option<&'a ItemDef> {
        id.and_then(|id| self.model.defs.items.get(id))
    }

    pub fn token_def(&self, id: Option<&str>) -> Option<&'a TokenDef> {
        id.and_then(|id| self.model.defs.tokens.get(id))
    }

    /// `<pool>` overrides declared on a token def.
    pub fn token_pools(&self, id: Option<&str>) -> &'a [PoolDecl] {
        self.token_def(id).map_or(&[], |d| &d.pools)
    }

    pub fn token_stats(&self, id: Option<&str>) -> Option<&'a Stats> {
        self.token_def(id).and_then(|d| d.stats.as_ref())
    }

    pub fn token_spells(&self, id: Option<&str>) -> Option<&'a Vec<String>> {
        self.token_def(id).and_then(|d| d.spells.as_ref())
    }

    pub fn inventory(&self, yid: Option<&str>) -> Option<&'a Inventory> {
        yid.and_then(|y| self.model.inventories.get(y))
    }
}

pub fn stat_total(s: &Stats) -> f64 {
    STATS.iter().map(|k| s.get(*k)).sum()
}

/// Why `stats` can't make a Character under `budget`; none = they can.
pub fn stats_problem(stats: &Stats, budget: f64) -> Option<String> {
    if STATS.iter().any(|k| {
        let v = stats.get(*k);
        v.fract() != 0.0 || !(STAT_MIN..=STAT_MAX).contains(&v)
    }) {
        return Some(format!(
            "each stat is a whole number from {STAT_MIN} to {STAT_MAX}"
        ));
    }
    let total = stat_total(stats);
    (total > budget).then(|| format!("{total} points spent, the budget is {budget}"))
}

/// Point-buy spread: `lead` ahead of an even rest by `gap` (none = even); sums to ≤ budget.
pub fn preset_stats(budget: f64, lead: Option<Stat>, gap: f64) -> Stats {
    let clamp = |v: f64| v.clamp(STAT_MIN, STAT_MAX);
    let n = STATS.len() as f64;
    let rest = clamp(((budget - if lead.is_none() { 0.0 } else { gap }) / n).floor());
    let mut out = Stats {
        physique: rest,
        defence: rest,
        intelligence: rest,
        speed: rest,
    };
    match lead {
        None => {
            let mut spare = budget - rest * n;
            for s in STATS {
                let more = spare > 0.0;
                spare -= 1.0;
                if more && out.get(s) < STAT_MAX {
                    *out.get_mut(s) += 1.0;
                }
            }
        }
        Some(l) => *out.get_mut(l) = clamp(budget - rest * (n - 1.0)),
    }
    out
}

pub fn item_status(token: &Token, item_yid: &str) -> CarrierStatus {
    let Some(eq) = &token.equiped else {
        return While::Carried;
    };
    let at = |s: EquipSpot| eq.get(s.key()).map(String::as_str) == Some(item_yid);
    if at(EquipSpot::HandL) || at(EquipSpot::HandR) {
        While::Wielded
    } else if eq.values().any(|y| y == item_yid) {
        While::Equipped
    } else {
        While::Carried
    }
}

/// Toggle-equip: `hand` = first free hand (both full → handR); re-equipping unequips.
pub fn toggle_equip(equiped: Option<&Equiped>, item_yid: &str, spot: ItemSpot) -> Equiped {
    let mut current = equiped.cloned().unwrap_or_default();
    let worn_at = current
        .iter()
        .find(|(_, y)| *y == item_yid)
        .map(|(s, _)| s.clone());
    let target = worn_at.clone().unwrap_or_else(|| {
        match spot {
            ItemSpot::Chest => EquipSpot::Chest,
            ItemSpot::Back => EquipSpot::Back,
            ItemSpot::Shoulders => EquipSpot::Shoulders,
            ItemSpot::Head => EquipSpot::Head,
            ItemSpot::Hand if !current.contains_key(EquipSpot::HandL.key()) => EquipSpot::HandL,
            ItemSpot::Hand => EquipSpot::HandR,
        }
        .key()
        .to_string()
    });
    current.remove(&target);
    if worn_at.is_none() {
        current.insert(target, item_yid.to_string());
    }
    current
}

/// Equip map with every spot holding `item_yid` cleared.
pub fn equiped_without(equiped: Option<&Equiped>, item_yid: &str) -> Equiped {
    equiped.map_or_else(Equiped::new, |e| {
        e.iter()
            .filter(|(_, y)| *y != item_yid)
            .map(|(s, y)| (s.clone(), y.clone()))
            .collect()
    })
}

/// Timed spawn or given item whose `turns` ran out.
pub fn expired(expires_turn: Option<i64>, turn: i64) -> bool {
    expires_turn.is_some_and(|e| e <= turn)
}

/// Current value of the pool in `role`; none = no such pool.
pub fn role_value(token: &Token, ctx: &Ctx, role: Role) -> Option<f64> {
    let name = ctx.roles.get(role)?;
    pools_of(token, ctx, None).get(name).copied()
}

/// Dead = a character whose health-role pool ran out; no such pool = never dead.
pub fn is_dead(token: &Token, ctx: &Ctx) -> bool {
    token.is_character() && role_value(token, ctx, Role::Health).is_some_and(|hp| hp <= 0.0)
}

/// Hidden = a living character whose hidden-role pool is above 0 (#145).
pub fn is_hidden(token: &Token, ctx: &Ctx) -> bool {
    token.is_character()
        && !is_dead(token, ctx)
        && role_value(token, ctx, Role::Hidden).unwrap_or(0.0) > 0.0
}

/// The token's form while it lasts; none = itself.
pub fn live_form(token: &Token, turn: i64) -> Option<&Form> {
    token
        .form
        .as_ref()
        .filter(|f| f.until.is_none_or(|u| u > turn))
}

/// Token def the rules read: the form's while polymorphed, else its own.
pub fn rule_def_of<'t>(token: &'t Token, ctx: &Ctx) -> Option<&'t str> {
    live_form(token, ctx.turn())
        .map(|f| f.def.as_str())
        .or(token.token_id.as_deref())
}

/// Base stats: the form def's while polymorphed (a stat-less def keeps the token's own).
pub fn base_stats_of(token: &Token, ctx: &Ctx) -> Stats {
    let own = token.stats.unwrap_or_default();
    live_form(token, ctx.turn())
        .and_then(|f| ctx.token_stats(Some(&f.def)))
        .copied()
        .unwrap_or(own)
}

/// A form keeps its own class name but none of the class's spells or pools.
pub fn class_of_token(token: &Token, ctx: &Ctx) -> Class {
    let cls = class_of(&token.stats.unwrap_or_default(), &ctx.game().classes);
    if live_form(token, ctx.turn()).is_none() {
        cls
    } else {
        Class {
            name: cls.name,
            ..Class::default()
        }
    }
}

pub fn spell_slots(stats: &Stats, list: &SpellList) -> usize {
    (stats.intelligence - list.base).floor().max(0.0) as usize
}

pub fn meets_requires(spell: &Spell, stats: &Stats) -> bool {
    spell.requires.as_ref().is_none_or(|r| {
        STATS
            .iter()
            .all(|s| stats.get(*s) >= r.get(*s).unwrap_or(0.0))
    })
}

/// Live list spells: picks whose requirements hold, first `slots` of them.
pub fn learned_spells(picks: &[String], list: &SpellList, stats: &Stats) -> Vec<Spell> {
    picks
        .iter()
        .filter_map(|id| list.spells.iter().find(|s| s.id.as_deref() == Some(id)))
        .filter(|s| meets_requires(s, stats))
        .take(spell_slots(stats, list))
        .cloned()
        .collect()
}

/// A token def's `<spells>` refs as innate spells — no slots, no `<requires>`; deduped, unknown skipped.
pub fn innate_spells(refs: Option<&Vec<String>>, list: &SpellList) -> Vec<Spell> {
    let mut seen: Vec<&String> = vec![];
    let mut out = vec![];
    for id in refs.into_iter().flatten() {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        if let Some(s) = list.spells.iter().find(|s| s.id.as_ref() == Some(id)) {
            out.push(s.clone());
        }
    }
    out
}

/// Pool decls on a character: game ⊕ token-def overrides ⊕ class pools; roles only from the game.
pub fn pool_decls_of(token: &Token, ctx: &Ctx, cls: &Class) -> Vec<PoolDecl> {
    let def = without_roles(ctx.token_pools(rule_def_of(token, ctx)));
    let class = without_roles(&cls.pools);
    merge_pools(&[&ctx.game().pools, &def, &class])
}

/// A pool hosts its spells while above 0; a missing key reads as `start`, else full.
fn hosting(d: &PoolDecl, current: &Pools) -> bool {
    current
        .get(&d.name)
        .copied()
        .or(d.start)
        .or(d.capacity)
        .unwrap_or(1.0)
        > 0.0
}

fn carrier(
    name: &str,
    item_yid: Option<&str>,
    status: CarrierStatus,
    weight: f64,
    pools: &Pools,
    spells: &[Spell],
) -> LiveCarrier {
    LiveCarrier {
        name: name.to_string(),
        item_yid: item_yid.map(str::to_string),
        status,
        weight,
        pools: pools.clone(),
        spells: spells.to_vec(),
        learned: false,
    }
}

/// Class, hosting pools (by `reads`), equipped items in spot order, the rest of the inventory, then spells — cost-resolution order.
pub fn carriers_of(
    token: &Token,
    ctx: &Ctx,
    cls: &Class,
    reads: Option<&Pools>,
) -> Vec<LiveCarrier> {
    let empty = Pools::new();
    let pools = token.pools.as_ref().unwrap_or(&empty);
    let reads = reads.unwrap_or(pools);
    let mut out = vec![carrier(
        &cls.name,
        None,
        While::Always,
        0.0,
        pools,
        &cls.spells,
    )];
    for d in pool_decls_of(token, ctx, cls) {
        if let Some(spells) = d.spells.as_ref().filter(|_| hosting(&d, reads)) {
            out.push(carrier(&d.name, None, While::Always, 0.0, pools, spells));
        }
    }
    // A form has only its own body: no items, and its def's spell refs instead of the token's picks.
    let form = live_form(token, ctx.turn());
    let mut yids: Vec<&str> = vec![];
    if form.is_none() {
        if let Some(eq) = &token.equiped {
            yids.extend(
                EQUIP_SPOTS
                    .iter()
                    .filter_map(|s| eq.get(s.key()).map(String::as_str)),
            );
        }
        let equipped = yids.clone();
        if let Some(inv) = ctx.inventory(token.inventory_yid.as_deref()) {
            yids.extend(
                inv.stuff
                    .iter()
                    .map(String::as_str)
                    .filter(|y| !y.is_empty() && !equipped.contains(y)),
            );
        }
    }
    for yid in yids {
        let Some(inst) = ctx.item(yid) else { continue };
        let Some(def) = ctx.item_def(Some(&inst.item_definition_id)) else {
            continue;
        };
        let status = item_status(token, yid);
        let own = inst.pools.as_ref().unwrap_or(&empty);
        out.push(carrier(
            &def.name,
            Some(yid),
            status,
            def.weight,
            own,
            &def.spells,
        ));
        // An item's pool hosts its spells like a token's does, per instance, live by the item's status.
        for d in &def.pools {
            if let Some(spells) = d.spells.as_ref().filter(|_| hosting(d, own)) {
                out.push(carrier(&d.name, Some(yid), status, 0.0, own, spells));
            }
        }
    }
    // Def refs are innate, read live off the def; forms get no picks, a pick also innate is skipped.
    let list = &ctx.game().spells;
    let innate = innate_spells(ctx.token_spells(rule_def_of(token, ctx)), list);
    let picks: Vec<String> = match (form, &token.spells) {
        (None, Some(picks)) => picks
            .iter()
            .filter(|id| !innate.iter().any(|s| s.id.as_ref() == Some(*id)))
            .cloned()
            .collect(),
        _ => vec![],
    };
    // Slots/requirements read stats without learned spells' own passives or scaled ones (no cycle).
    let flat = effective_stats(
        &base_stats_of(token, ctx),
        &live_deltas(&out, token.modifiers.as_ref(), ctx.turn()),
    );
    let learned = learned_spells(&picks, list, &flat);
    if !innate.is_empty() || !learned.is_empty() {
        let mut c = carrier(
            "Spells",
            None,
            While::Always,
            0.0,
            pools,
            &[innate, learned].concat(),
        );
        c.learned = true;
        out.push(c);
    }
    out
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PoolView {
    pub name: String,
    pub value: f64,
    pub capacity: f64,
    pub blocked: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refill: Option<Refill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    pub start: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shows: Option<Vec<PoolShow>>,
}

/// Everything a character derives, from one carrier walk.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterView {
    pub cls: Class,
    pub carriers: Vec<LiveCarrier>,
    pub stats: Stats,
    pub pools: Vec<PoolView>,
    pub reach: Reach,
}

fn reads_of(views: &[PoolView]) -> Pools {
    views.iter().map(|v| (v.name.clone(), v.value)).collect()
}

/// Refill target per pool: capacity minus the carry block.
fn full_of(views: &[PoolView]) -> Pools {
    views
        .iter()
        .map(|v| (v.name.clone(), v.capacity - v.blocked))
        .collect()
}

/// Capacities from effective stats; moves role reports carry penalty as `blocked`. Scaled passives read a flat-only first pass.
pub fn character_of(token: &Token, ctx: &Ctx) -> CharacterView {
    let cls = class_of_token(token, ctx);
    let base = base_stats_of(token, ctx);
    let decls = pool_decls_of(token, ctx, &cls);
    let empty = Pools::new();
    let current = token.pools.as_ref().unwrap_or(&empty);
    let view_of = |carriers: Vec<LiveCarrier>| -> CharacterView {
        let weight: f64 = carriers.iter().map(|c| c.weight).sum();
        let views_of = |stats: &Stats, deltas: &[StatDelta]| -> Vec<PoolView> {
            let penalty = carry_penalty(weight, stats.physique, ctx.game().physique_carry_mult);
            decls
                .iter()
                .map(|d| {
                    let capacity = capacity_of(d, Some(stats), deltas);
                    let blocked = if d.role == Some(Role::Moves) {
                        capacity.min(penalty)
                    } else {
                        0.0
                    };
                    let start = d.start.unwrap_or(f64::INFINITY).min(capacity - blocked);
                    // No pool reads above its capacity: a lapsed max takes the points over it along.
                    let value = current.get(&d.name).copied().unwrap_or(start).min(capacity);
                    PoolView {
                        name: d.name.clone(),
                        value,
                        capacity,
                        blocked,
                        start,
                        refill: d.refill,
                        role: d.role,
                        shows: d.shows.clone(),
                    }
                })
                .collect()
        };
        let flat = live_deltas(&carriers, token.modifiers.as_ref(), ctx.turn());
        let flat_stats = effective_stats(&base, &flat);
        let flat_views = views_of(&flat_stats, &flat);
        let (reads, full) = (reads_of(&flat_views), full_of(&flat_views));
        let g = Gauges {
            stats: &flat_stats,
            pools: &reads,
            full: Some(&full),
            tallies: Tallies::default(),
        };
        let scaled = scaled_deltas(&carriers, &g);
        if scaled.is_empty() {
            let reach = reach_of(&flat);
            return CharacterView {
                cls: cls.clone(),
                carriers,
                stats: flat_stats,
                pools: flat_views,
                reach,
            };
        }
        let deltas = [flat, scaled].concat();
        let stats = effective_stats(&base, &deltas);
        let pools = views_of(&stats, &deltas);
        CharacterView {
            cls: cls.clone(),
            carriers,
            stats,
            pools,
            reach: reach_of(&deltas),
        }
    };
    let view = view_of(carriers_of(token, ctx, &cls, None));
    // Hosting re-checked at capped reads: a pool whose max lapsed drops its spells (one pass, no fixpoint).
    let reads = reads_of(&view.pools);
    if decls
        .iter()
        .any(|d| d.spells.is_some() && hosting(d, current) && !hosting(d, &reads))
    {
        view_of(carriers_of(token, ctx, &cls, Some(&reads)))
    } else {
        view
    }
}

/// Stat-less token (trap, fountain): its def's pools only; no game pools, roles or class (#153).
fn prop_decls_of(token: &Token, ctx: &Ctx) -> Vec<PoolDecl> {
    without_roles(ctx.token_pools(rule_def_of(token, ctx)))
}

fn prop_pool_views_of(token: &Token, ctx: &Ctx) -> Vec<PoolView> {
    let empty = Pools::new();
    let current = token.pools.as_ref().unwrap_or(&empty);
    prop_decls_of(token, ctx)
        .iter()
        .map(|d| {
            let capacity = capacity_of(d, None, &[]);
            let start = d.start.unwrap_or(f64::INFINITY).min(capacity);
            let value = current.get(&d.name).copied().unwrap_or(start).min(capacity);
            PoolView {
                name: d.name.clone(),
                value,
                capacity,
                blocked: 0.0,
                start,
                refill: d.refill,
                role: None,
                shows: None,
            }
        })
        .collect()
}

/// Pool views of any token: a character's full derivation, else its def's own pools.
pub fn any_pool_views_of(token: &Token, ctx: &Ctx) -> Vec<PoolView> {
    if token.is_character() {
        character_of(token, ctx).pools
    } else {
        prop_pool_views_of(token, ctx)
    }
}

/// Token pools as the rules read them: undeclared-yet keys at their start, none above capacity.
pub fn pools_of(token: &Token, ctx: &Ctx, views: Option<&[PoolView]>) -> Pools {
    let mut out = token.pools.clone().unwrap_or_default();
    let owned;
    let views = match views {
        Some(v) => v,
        None => {
            owned = any_pool_views_of(token, ctx);
            &owned
        }
    };
    for v in views {
        out.insert(v.name.clone(), v.value);
    }
    out
}

/// Pools after a `turn`/`game` refill: matching decls back to capacity, the rest as read.
pub fn refilled_pools(token: &Token, ctx: &Ctx, cadence: Refill) -> Pools {
    let mut out = token.pools.clone().unwrap_or_default();
    for v in character_of(token, ctx).pools {
        let value = if v.refill == Some(cadence) {
            v.capacity - v.blocked
        } else {
            v.value
        };
        out.insert(v.name, value);
    }
    out
}

/// Token after turn upkeep: `turn` pools refilled, expired modifiers and form dropped.
pub fn upkept(token: &Token, ctx: &Ctx) -> Token {
    let modifiers = live_modifiers(token.modifiers.as_ref(), ctx.turn());
    Token {
        pools: Some(refilled_pools(token, ctx, Refill::Turn)),
        modifiers: (!modifiers.is_empty()).then_some(modifiers),
        form: live_form(token, ctx.turn()).cloned(),
        ..token.clone()
    }
}

/// Stat-less tokens fight with stats 0 and carry only their def's pool-hosted spells (#153).
pub fn combatant_of(token: &Token, ctx: &Ctx) -> Combatant {
    let tile = token.position.map(tile_of);
    if !token.is_character() {
        let views = prop_pool_views_of(token, ctx);
        let pools = pools_of(token, ctx, Some(&views));
        let own = token.pools.clone().unwrap_or_default();
        let carriers = prop_decls_of(token, ctx)
            .iter()
            .filter(|d| d.spells.is_some() && hosting(d, &pools))
            .map(|d| {
                carrier(
                    &d.name,
                    None,
                    While::Always,
                    0.0,
                    &own,
                    d.spells.as_deref().unwrap_or(&[]),
                )
            })
            .collect();
        return Combatant {
            yid: token.yid.clone(),
            tile,
            stats: Stats::ZERO,
            pools,
            full: Some(full_of(&views)),
            carriers,
            reach: Reach::default(),
        };
    }
    let view = character_of(token, ctx);
    Combatant {
        yid: token.yid.clone(),
        tile,
        stats: view.stats,
        pools: pools_of(token, ctx, Some(&view.pools)),
        full: Some(full_of(&view.pools)),
        carriers: view.carriers,
        reach: view.reach,
    }
}

/// `cast` spells live on the token right now: (carrier index, spell index).
pub fn casts_of(carriers: &[LiveCarrier]) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for (ci, c) in carriers.iter().enumerate() {
        for (si, s) in c.spells.iter().enumerate() {
            if s.trigger == Trigger::Cast && is_live(s.while_, c.status) {
                out.push((ci, si));
            }
        }
    }
    out
}

/// Fresh pools for a new instance: every decl at its start value.
pub fn initial_pools(stats: Stats, token_id: Option<&str>, ctx: &Ctx) -> Pools {
    let t = Token {
        stats: Some(stats),
        token_id: token_id.map(str::to_string),
        ..Token::default()
    };
    pools_of(&t, ctx, None)
}

/// Pools seeded onto a new item instance from its def's decls.
pub fn item_pools(decls: &[PoolDecl]) -> Option<Pools> {
    (!decls.is_empty()).then(|| {
        decls
            .iter()
            .map(|d| (d.name.clone(), capacity_of(d, None, &[])))
            .collect::<BTreeMap<_, _>>()
    })
}
