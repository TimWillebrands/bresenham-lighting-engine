//! Pure rule step (ADR-0013): a cast or a rider event → outcomes. Board reads arrive as [`Probes`].

use super::rng::Rng;
use super::stats::*;
use super::tiles::{manhattan, until_turn};
use super::types::*;
use std::collections::BTreeMap;

/// Board reads a resolution takes; none = ungated (LoS) / denied (light).
pub trait Probes {
    /// Line of sight between two tiles; none = never blocked.
    fn cast_ray(&mut self, _from: Pt, _to: Pt) -> Option<bool> {
        None
    }
    /// Light 0–1 on a tile of the actor's layer; none = can't judge, so `light` gates deny.
    fn light_at(&mut self, _tile: Pt) -> Option<f64> {
        None
    }
    /// Combatants within `r` tiles of a tile, for `target="area"`.
    fn within(&mut self, _tile: Pt, _r: f64) -> Vec<Combatant> {
        vec![]
    }
}

/// No board: every ray passes, no light, area hits nobody.
pub struct NoProbes;
impl Probes for NoProbes {}

pub struct ResolveCtx<'a> {
    pub turn: i64,
    pub roles: &'a Roles,
    pub defence_chance: f64,
    pub rng: &'a mut dyn Rng,
    pub probes: &'a mut dyn Probes,
}

pub enum RuleEvent<'a> {
    /// `carrier` indexes `actor.carriers`.
    Cast {
        carrier: usize,
        spell: &'a Spell,
        target: Option<&'a Combatant>,
        tile: Option<Pt>,
    },
    Turn,
    /// `mover` stepped onto the actor's tile: the actor's `enter` riders, the mover as other party.
    Enter {
        mover: &'a Combatant,
    },
    /// The actor walked onto its `tile`: its `step` riders.
    Step,
}

pub struct DamageRoll {
    pub damage: f64,
    pub blocks: f64,
    pub rolls: Vec<String>,
}

/// JS `toFixed(0)` on non-negative values.
fn fixed0(x: f64) -> String {
    format!("{}", (x + 0.5).floor())
}

/// Per-point defence roll: each attack point is blocked when any defence roll beats the block chance.
pub fn do_damage(attack: f64, defence: f64, defence_chance: f64, rng: &mut dyn Rng) -> DamageRoll {
    let chance = (100.0 - defence_chance) / 100.0;
    let mut blocks = 0.0;
    let mut rolls = vec![];
    let mut a = 0.0;
    while a < attack {
        let mut block = false;
        let mut d = 0.0;
        while d < defence {
            let roll = rng.next_f64();
            let blocked = roll >= chance;
            block |= blocked;
            rolls.push(format!(
                "{}attack {} vs defence {}: {}% (block ≥ {}%)",
                if blocked { "[BLOCK] " } else { "" },
                a + 1.0,
                d + 1.0,
                fixed0(roll * 100.0),
                fixed0(chance * 100.0)
            ));
            d += 1.0;
        }
        if block {
            blocks += 1.0;
        }
        a += 1.0;
    }
    DamageRoll {
        damage: (attack - blocks).max(0.0),
        blocks,
        rolls,
    }
}

/// Defence left to roll after `pierce`; never below 0.
fn pierced_defence(defence: f64, pierce: Option<Pierce>) -> f64 {
    match pierce {
        Some(Pierce::All(_)) => 0.0,
        Some(Pierce::Points(p)) => (defence - p.max(0.0)).max(0.0),
        None => defence.max(0.0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reach {
    Ok,
    Range,
    Blocked,
    Unplaced,
    Lit,
    Dark,
}

/// Range a spell reaches with; `cast` spells add the caster's `cast` reach bonus.
pub fn range_of(spell: &Spell, bonus: f64) -> f64 {
    spell.range.unwrap_or(1.0)
        + if spell.trigger == Trigger::Cast {
            bonus
        } else {
            0.0
        }
}

/// Whether a `cast` spell is offered on `target` (none = bare ground).
pub fn aims(spell: &Spell, caster_yid: &str, target: Option<&Token>) -> bool {
    match target {
        None => matches!(spell.target, Target::Tile | Target::Area),
        Some(t) => match spell.target {
            Target::Self_ => t.yid == caster_yid,
            Target::Token => t.is_character(),
            _ => false,
        },
    }
}

/// Range + line-of-sight + `light`/`exposed` gates; `self` always reaches, rangeless riders reach their trigger.
pub fn reach(
    spell: &Spell,
    from: Option<Pt>,
    to: Option<Pt>,
    probes: &mut dyn Probes,
    bonus: f64,
) -> Reach {
    if (spell.light.is_some() || spell.exposed.is_some()) && from.is_none() {
        return Reach::Unplaced;
    }
    match in_range(spell, from, to, probes, bonus) {
        Reach::Ok => light_gate(spell, from, to, probes),
        r => r,
    }
}

fn in_range(
    spell: &Spell,
    from: Option<Pt>,
    to: Option<Pt>,
    probes: &mut dyn Probes,
    bonus: f64,
) -> Reach {
    if spell.target == Target::Self_ || (spell.range.is_none() && spell.trigger != Trigger::Cast) {
        return Reach::Ok;
    }
    let (Some(from), Some(to)) = (from, to) else {
        return Reach::Unplaced;
    };
    if manhattan(from, to) as f64 > range_of(spell, bonus) {
        return Reach::Range;
    }
    if from != to && probes.cast_ray(from, to) == Some(false) {
        return Reach::Blocked;
    }
    Reach::Ok
}

/// Caster's tile and (unless `self`) the spell's tile: each at most `light` and above `exposed`; unjudgeable = denied.
fn light_gate(spell: &Spell, from: Option<Pt>, to: Option<Pt>, probes: &mut dyn Probes) -> Reach {
    if spell.light.is_none() && spell.exposed.is_none() {
        return Reach::Ok;
    }
    let tiles = if spell.target == Target::Self_ {
        vec![from]
    } else {
        vec![from, to]
    };
    let levels: Vec<Option<f64>> = tiles
        .into_iter()
        .map(|t| t.and_then(|t| probes.light_at(t)))
        .collect();
    if let Some(max) = spell.light {
        if !levels.iter().all(|l| l.is_some_and(|l| l <= max)) {
            return Reach::Lit;
        }
    }
    if let Some(min) = spell.exposed {
        if !levels.iter().all(|l| l.is_some_and(|l| l > min)) {
            return Reach::Dark;
        }
    }
    Reach::Ok
}

fn spends_action(spell: &Spell, roles: &Roles) -> bool {
    spell.trigger == Trigger::Cast && spell.action && roles.actions.is_some()
}

#[derive(Default)]
struct Fired {
    dealt: f64,
    blocked: f64,
    damaged: bool,
}

/// Balance key: an item instance, or a token.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Holder {
    Item(String),
    Token(String),
}

/// One resolution: working balances so several spells can't overspend one pool; inputs stay untouched.
struct Work<'c, 'a> {
    ctx: &'c mut ResolveCtx<'a>,
    out: Vec<Outcome>,
    balances: BTreeMap<Holder, Pools>,
}

impl<'c, 'a> Work<'c, 'a> {
    fn new(ctx: &'c mut ResolveCtx<'a>) -> Self {
        Work {
            ctx,
            out: vec![],
            balances: BTreeMap::new(),
        }
    }

    fn roll(&mut self, chance: f64) -> bool {
        chance >= 1.0 || self.ctx.rng.next_f64() < chance
    }

    /// Working pools of a holder: an item carrier's instance, or the token itself.
    fn balance(&mut self, token: &Combatant, item: Option<&LiveCarrier>) -> &mut Pools {
        let (key, seed) = match item.and_then(|c| c.item_yid.as_ref().map(|y| (y, &c.pools))) {
            Some((yid, pools)) => (Holder::Item(yid.clone()), pools),
            None => (Holder::Token(token.yid.clone()), &token.pools),
        };
        self.balances.entry(key).or_insert_with(|| seed.clone())
    }

    /// A verb's amount; pool `per`s read the actor's working balance (after this spell's costs).
    fn amount(&mut self, e: &Effect, actor: &Combatant, tallies: Tallies) -> f64 {
        let Some((amount, per)) = e.amount() else {
            return 0.0;
        };
        let Some(per) = per else { return amount };
        let pools = self.balance(actor, None).clone();
        let g = Gauges {
            stats: &actor.stats,
            pools: &pools,
            full: actor.full.as_ref(),
            tallies,
        };
        scaled_amount(amount, per_value(per, &g))
    }

    fn health(&mut self, token: &Combatant) -> f64 {
        match self.ctx.roles.health.clone() {
            None => 1.0,
            Some(h) => self.balance(token, None).get(&h).copied().unwrap_or(0.0),
        }
    }

    /// Points of a hit the ward-role pool soaks up, taken off its working balance.
    fn ward(&mut self, token: &Combatant, landed: f64) -> f64 {
        let Some(w) = self.ctx.roles.ward.clone() else {
            return 0.0;
        };
        let b = self.balance(token, None);
        let has = b.get(&w).copied().unwrap_or(0.0);
        let soaked = landed.min(has.max(0.0));
        b.insert(w, has - soaked);
        soaked
    }

    fn take(
        &mut self,
        actor: &Combatant,
        holder: Option<&LiveCarrier>,
        pool: &str,
        amount: f64,
    ) -> bool {
        let b = self.balance(actor, holder);
        let has = b.get(pool).copied().unwrap_or(0.0);
        if has < amount {
            return false;
        }
        b.insert(pool.to_string(), has - amount);
        let item = holder.and_then(|h| h.item_yid.clone());
        self.out.push(Outcome::Spend {
            token: actor.yid.clone(),
            item,
            pool: pool.to_string(),
            amount,
        });
        true
    }

    /// Costs (and the action, for casts) all payable or nothing spent; holders = own instance → token → equipped items.
    fn pay(&mut self, spell: &Spell, carrier: usize, actor: &Combatant, action: bool) -> bool {
        let snapshot = self.balances.clone();
        let out_len = self.out.len();
        let own = &actor.carriers[carrier];
        let mut holders: Vec<Option<&LiveCarrier>> = vec![own.item_yid.as_ref().map(|_| own), None];
        holders.extend(
            actor
                .carriers
                .iter()
                .enumerate()
                .filter(|(i, c)| {
                    c.item_yid.is_some()
                        && *i != carrier
                        && matches!(c.status, While::Equipped | While::Wielded)
                })
                .map(|(_, c)| Some(c)),
        );
        let actions = self.ctx.roles.actions.clone();
        let mut ok = match actions {
            Some(pool) if action && spends_action(spell, self.ctx.roles) => {
                self.take(actor, None, &pool, 1.0)
            }
            _ => true,
        };
        for cost in &spell.costs {
            if !ok {
                break;
            }
            ok = holders
                .iter()
                .any(|h| self.take(actor, *h, &cost.pool, cost.amount));
        }
        if !ok {
            self.balances = snapshot;
            self.out.truncate(out_len);
        }
        ok
    }

    /// Item yid whose pool a restore refills; none = the token. Order as `pay`, own spells only.
    fn restore_holder(
        &mut self,
        target: &Combatant,
        actor: &Combatant,
        carrier: Option<usize>,
        pool: &str,
    ) -> Option<String> {
        if target.yid == actor.yid {
            if let Some(c) = carrier
                .map(|i| &actor.carriers[i])
                .filter(|c| c.item_yid.is_some())
            {
                if self.balance(actor, Some(c)).contains_key(pool) {
                    return c.item_yid.clone();
                }
            }
        }
        if self.balance(target, None).contains_key(pool) {
            return None;
        }
        target
            .carriers
            .iter()
            .filter(|c| {
                c.item_yid.is_some() && matches!(c.status, While::Equipped | While::Wielded)
            })
            .find(|c| self.balance(target, Some(c)).contains_key(pool))
            .and_then(|c| c.item_yid.clone())
    }

    /// Every verb of the spell that passes its own chance roll.
    #[allow(clippy::too_many_arguments)]
    fn fire(
        &mut self,
        spell: &Spell,
        carrier: Option<usize>,
        actor: &Combatant,
        to: Option<&Combatant>,
        tile: Option<Pt>,
        tallies: Tallies,
        swinging: bool,
        aura: bool,
    ) -> Fired {
        let name = spell.name.clone().unwrap_or_else(|| {
            carrier.map_or_else(String::new, |i| actor.carriers[i].name.clone())
        });
        let trigger = (spell.trigger != Trigger::Cast).then_some(spell.trigger);
        let mut fired = Fired::default();
        for e in &spell.effects {
            if !self.roll(e.chance()) {
                continue;
            }
            let amount = self.amount(e, actor, tallies);
            let targets: Vec<Combatant> = if spell.target == Target::Area {
                match tile {
                    Some(t) => self
                        .ctx
                        .probes
                        .within(t, e.r().unwrap_or(0.0))
                        .into_iter()
                        .filter(|c| !aura || c.yid != actor.yid)
                        .collect(),
                    None => vec![],
                }
            } else {
                to.into_iter().cloned().collect()
            };
            let turn = self.ctx.turn;
            match e {
                Effect::Damage { pierce, .. } => {
                    for t in &targets {
                        let defence = pierced_defence(t.stats.defence, *pierce);
                        let pierced = t.stats.defence.max(0.0) - defence;
                        let roll =
                            do_damage(amount, defence, self.ctx.defence_chance, self.ctx.rng);
                        let warded = self.ward(t, roll.damage);
                        if let Some(h) = self.ctx.roles.health.clone() {
                            let b = self.balance(t, None);
                            let has = b.get(&h).copied().unwrap_or(0.0);
                            b.insert(h, has - (roll.damage - warded));
                        }
                        self.out.push(Outcome::Damage {
                            carrier: name.clone(),
                            trigger,
                            target: t.yid.clone(),
                            amount: roll.damage,
                            blocks: roll.blocks,
                            rolls: roll.rolls,
                            warded: (warded > 0.0).then_some(warded),
                            pierced: (pierced > 0.0).then_some(pierced),
                        });
                        fired.dealt += roll.damage;
                        fired.blocked += roll.blocks;
                        fired.damaged = true;
                    }
                }
                Effect::Restore { pool, .. } => {
                    for t in &targets {
                        let item = self.restore_holder(t, actor, carrier, pool);
                        self.out.push(Outcome::Restore {
                            carrier: name.clone(),
                            trigger,
                            target: t.yid.clone(),
                            item,
                            pool: pool.clone(),
                            amount,
                        });
                    }
                }
                Effect::Modify {
                    stat,
                    pool,
                    reach,
                    turns,
                    ..
                } => {
                    for t in &targets {
                        self.out.push(Outcome::Modify {
                            carrier: name.clone(),
                            trigger,
                            target: t.yid.clone(),
                            stat: *stat,
                            pool: pool.clone(),
                            reach: *reach,
                            amount,
                            until: turns.map(|n| until_turn(turn, n)),
                        });
                    }
                }
                Effect::Spawn { def, turns, .. } => {
                    if let Some(at) = tile.or(to.and_then(|t| t.tile)) {
                        let expires_turn = turns.map(|n| until_turn(turn, n));
                        self.out.push(Outcome::Spawn {
                            carrier: name.clone(),
                            trigger,
                            tile: at,
                            def: def.clone(),
                            expires_turn,
                        });
                    }
                }
                Effect::Give {
                    def,
                    equipped,
                    turns,
                    ..
                } => {
                    if amount < 1.0 {
                        continue;
                    }
                    for t in &targets {
                        self.out.push(Outcome::Give {
                            carrier: name.clone(),
                            trigger,
                            target: t.yid.clone(),
                            def: def.clone(),
                            amount,
                            equipped: (*equipped == Some(true)).then_some(true),
                            expires_turn: turns.map(|n| until_turn(turn, n)),
                        });
                    }
                }
                Effect::Polymorph { def, turns, .. } => {
                    for t in &targets {
                        let until = def.as_ref().and(*turns).map(|n| until_turn(turn, n));
                        self.out.push(Outcome::Polymorph {
                            carrier: name.clone(),
                            trigger,
                            target: t.yid.clone(),
                            def: def.clone(),
                            until,
                        });
                    }
                }
                Effect::Teleport { .. } => {
                    if let Some(at) = tile.or(to.and_then(|t| t.tile)) {
                        if Some(at) != actor.tile {
                            self.out.push(Outcome::Teleport {
                                carrier: name.clone(),
                                trigger,
                                target: actor.yid.clone(),
                                tile: at,
                            });
                        }
                    }
                }
                Effect::Attack { .. } => {
                    let Some(to) = to.filter(|t| !swinging && t.yid != actor.yid) else {
                        continue;
                    };
                    for (w, ws) in weapons_of(actor) {
                        let bonus = actor.reach.cast.unwrap_or(0.0);
                        if reach(ws, actor.tile, to.tile, self.ctx.probes, bonus) != Reach::Ok
                            || !self.pay(ws, w, actor, false)
                        {
                            continue;
                        }
                        let hit = self.fire(
                            ws,
                            Some(w),
                            actor,
                            Some(to),
                            to.tile,
                            Tallies::default(),
                            true,
                            false,
                        );
                        fired.dealt += hit.dealt;
                        fired.blocked += hit.blocked;
                        fired.damaged |= hit.damaged;
                    }
                }
            }
        }
        fired
    }

    /// Riders on `trigger` of `owner`; `other` = the event's other party. Auto-fire while payable.
    fn riders(
        &mut self,
        owner: &Combatant,
        trigger: Trigger,
        other: Option<&Combatant>,
        tallies: Tallies,
    ) {
        for (ci, c) in owner.carriers.iter().enumerate() {
            for s in &c.spells {
                if s.trigger != trigger || !is_live(s.while_, c.status) {
                    continue;
                }
                if matches!(trigger, Trigger::Turn | Trigger::Step)
                    && matches!(s.target, Target::Area | Target::Tile)
                {
                    // area = aura (owner excluded); tile = owner's own tile
                    let Some(at) = owner.tile else { continue };
                    if light_gate(s, Some(at), Some(at), self.ctx.probes) != Reach::Ok
                        || !self.roll(s.chance)
                        || !self.pay(s, ci, owner, true)
                    {
                        continue;
                    }
                    self.fire(
                        s,
                        Some(ci),
                        owner,
                        None,
                        Some(at),
                        tallies,
                        false,
                        s.target == Target::Area,
                    );
                    continue;
                }
                let (to, tile) = if s.target == Target::Self_ {
                    (Some(owner), owner.tile)
                } else {
                    (other, other.and_then(|o| o.tile))
                };
                if s.target != Target::Self_ && to.is_none() {
                    continue;
                }
                if reach(s, owner.tile, tile, self.ctx.probes, 0.0) != Reach::Ok {
                    continue;
                }
                if !self.roll(s.chance) || !self.pay(s, ci, owner, true) {
                    continue;
                }
                self.fire(s, Some(ci), owner, to, tile, tallies, false, false);
            }
        }
    }
}

/// Each wielded item's first live damage `cast`: what an `<attack/>` verb swings.
fn weapons_of(actor: &Combatant) -> Vec<(usize, &Spell)> {
    actor
        .carriers
        .iter()
        .enumerate()
        .filter(|(_, c)| c.item_yid.is_some() && c.status == While::Wielded)
        .filter_map(|(i, c)| {
            c.spells
                .iter()
                .find(|s| {
                    s.trigger == Trigger::Cast
                        && is_live(s.while_, c.status)
                        && s.effects.iter().any(|e| matches!(e, Effect::Damage { .. }))
                })
                .map(|s| (i, s))
        })
        .collect()
}

struct NoRng;
impl Rng for NoRng {
    fn next_u32(&mut self) -> u32 {
        0
    }
}

/// Dry run of the cost check (UI gating); same holder order as a real resolution.
pub fn payable(spell: &Spell, carrier: usize, actor: &Combatant, roles: &Roles) -> bool {
    let mut rng = NoRng;
    let mut probes = NoProbes;
    let mut ctx = ResolveCtx {
        turn: 0,
        roles,
        defence_chance: 0.0,
        rng: &mut rng,
        probes: &mut probes,
    };
    Work::new(&mut ctx).pay(spell, carrier, actor, true)
}

/// A cast (costs + action, chance, verbs, then attack/hurt/kill riders), or turn/enter/step riders.
/// None = could not fire (unreachable or unpayable).
pub fn resolve(event: RuleEvent, actor: &Combatant, ctx: &mut ResolveCtx) -> Option<Vec<Outcome>> {
    let mut w = Work::new(ctx);
    let (carrier, s, target, tile) = match event {
        RuleEvent::Turn => {
            w.riders(actor, Trigger::Turn, None, Tallies::default());
            return Some(w.out);
        }
        RuleEvent::Enter { mover } => {
            w.riders(actor, Trigger::Enter, Some(mover), Tallies::default());
            return Some(w.out);
        }
        RuleEvent::Step => {
            w.riders(actor, Trigger::Step, None, Tallies::default());
            return Some(w.out);
        }
        RuleEvent::Cast {
            carrier,
            spell,
            target,
            tile,
        } => (carrier, spell, target, tile),
    };
    let to = if s.target == Target::Self_ {
        Some(actor)
    } else {
        target
    };
    let tile = if s.target == Target::Self_ {
        actor.tile
    } else {
        tile.or(target.and_then(|t| t.tile))
    };
    if reach(
        s,
        actor.tile,
        tile,
        w.ctx.probes,
        actor.reach.cast.unwrap_or(0.0),
    ) != Reach::Ok
    {
        return None;
    }
    if !w.pay(s, carrier, actor, true) {
        return None;
    }
    if w.roll(s.chance) {
        let alive_before = to.is_some_and(|t| w.health(t) > 0.0);
        let hit = w.fire(
            s,
            Some(carrier),
            actor,
            to,
            tile,
            Tallies::default(),
            false,
            false,
        );
        if let Some(to) = to.filter(|t| hit.damaged && t.yid != actor.yid) {
            let tallies = Tallies {
                damage: hit.dealt,
                blocked: hit.blocked,
            };
            w.riders(actor, Trigger::Attack, Some(to), tallies);
            if hit.dealt > 0.0 {
                w.riders(to, Trigger::Hurt, Some(actor), tallies);
            }
            if alive_before && w.health(to) <= 0.0 {
                w.riders(actor, Trigger::Kill, Some(to), tallies);
            }
        }
    }
    Some(w.out)
}
