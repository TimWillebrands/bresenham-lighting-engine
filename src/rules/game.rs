//! Elm-style game loop over a host-owned doc: `update(model, intent) → writes | rejected`, `apply(model, writes) → model`.
//! Hosts apply `Write`s in one transaction; `update` chains on a scratch model, so a write list replays to the same state.

use super::apply::*;
use super::board::{Board, Boards, RouteGrid};
use super::character::*;
use super::edges::{collides_at, door_footprint, footprint, is_open, next_to_door};
use super::ledger::{ledger_entry_of, rider_entry_of, GameAction, LedgerTarget};
use super::light::{emitters_of, light_level, Emitter};
use super::model::{Model, PlayerRole};
use super::resolve::{aims, payable, reach, resolve, Probes, Reach, ResolveCtx, RuleEvent};
use super::rng::Rng;
use super::tiles::*;
use super::types::*;
use serde::{Deserialize, Serialize};

pub const SAY_MAX_LEN: usize = 280;

/// What a player asks for; `by` = the asking player's yid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Intent {
    /// Work a door as `actor`; no actor = the GM's editor flip (free, unlogged). `open` none = toggle.
    Door {
        by: String,
        door: String,
        #[serde(default)]
        actor: Option<String>,
        #[serde(default)]
        open: Option<bool>,
    },
    /// Cast `spell` (by name) off the carrier named `carrier` (`item` = its instance) at a token and/or tile.
    Cast {
        by: String,
        actor: String,
        carrier: String,
        #[serde(default)]
        item: Option<String>,
        spell: String,
        #[serde(default)]
        target: Option<String>,
        #[serde(default)]
        tile: Option<Pt>,
    },
    /// Walk to `to` within the moves budget, or one map-blocked phase step.
    Move {
        by: String,
        actor: String,
        to: Pt,
    },
    PassTurn {
        by: String,
    },
    Rotate {
        by: String,
        yid: String,
    },
    Remove {
        by: String,
        yid: String,
    },
    Say {
        by: String,
        actor: String,
        text: String,
        volume: f64,
    },
}

/// The patch union a writer applies. Token/item/inventory writes carry the whole rule view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Write {
    /// Rule fields of an existing token; host-only fields (sprite) stay, rule fields absent here are cleared.
    SetToken {
        token: Token,
    },
    /// A fresh token on `layer`; the host adds what it draws from `token.tokenId` (sprite).
    AddToken {
        layer: String,
        token: Token,
    },
    RemoveToken {
        yid: String,
    },
    SetItem {
        item: Item,
    },
    RemoveItem {
        yid: String,
    },
    SetInventory {
        inventory: Inventory,
    },
    RemoveInventory {
        yid: String,
    },
    SetTurn {
        turn: i64,
    },
    ClearLedger,
    PushAction {
        action: GameAction,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    NotFound,
    NotYourTurn,
    NotYours,
    NotGm,
    Dead,
    Unplaced,
    TooFar,
    OutOfActions,
    Unreachable,
    Blocked,
    Lit,
    Dark,
    CantPay,
    Wall,
    Already,
    Empty,
}

/// Why an intent was refused; `reason` is player-facing (UI tooltip, bot digest).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rejected {
    pub code: Refusal,
    pub reason: String,
}

fn no(code: Refusal, reason: impl Into<String>) -> Rejected {
    Rejected {
        code,
        reason: reason.into(),
    }
}

/// In-memory writer: the model after one write. Unknown targets are skipped, as a doc would.
pub fn apply_write(model: &mut Model, w: &Write) {
    match w {
        Write::SetToken { token } => {
            if let Some(t) = model.token_mut(&token.yid) {
                *t = token.clone();
            }
        }
        Write::AddToken { layer, token } => {
            if let Some(l) = model.layers.iter_mut().find(|l| &l.yid == layer) {
                l.tokens.push(token.clone());
            }
        }
        Write::RemoveToken { yid } => model
            .layers
            .iter_mut()
            .for_each(|l| l.tokens.retain(|t| &t.yid != yid)),
        Write::SetItem { item } => {
            model.items.insert(item.yid.clone(), item.clone());
        }
        Write::RemoveItem { yid } => {
            model.items.remove(yid);
        }
        Write::SetInventory { inventory } => {
            model
                .inventories
                .insert(inventory.yid.clone(), inventory.clone());
        }
        Write::RemoveInventory { yid } => {
            model.inventories.remove(yid);
        }
        Write::SetTurn { turn } => model.turn = *turn,
        Write::ClearLedger => model.ledger.clear(),
        Write::PushAction { action } => model.ledger.push(action.clone()),
    }
}

/// The model after a write list, in order.
pub fn apply(mut model: Model, writes: &[Write]) -> Model {
    for w in writes {
        apply_write(&mut model, w);
    }
    model
}

/// Who may act as a token: its owner, or the GM for unowned ones.
pub fn can_control(token: &Token, by: &str, model: &Model) -> bool {
    match &token.owner_player_yid {
        Some(owner) => owner == by,
        None => role_of(model, by) == Some(PlayerRole::GameMaster),
    }
}

fn role_of(model: &Model, by: &str) -> Option<PlayerRole> {
    model.players.get(by).map(|p| p.role)
}

/// Whose turn the counter shows: odd = Players, even = Gamemaster.
pub fn is_turn_of(model: &Model, by: &str) -> bool {
    matches!(
        (role_of(model, by), turn_type(model.turn)),
        (Some(PlayerRole::Player), Side::Players)
            | (Some(PlayerRole::GameMaster), Side::Gamemaster)
    )
}

/// Ending the turn: the side whose turn it is, or the GM any time.
pub fn can_pass_turn(model: &Model, by: &str) -> bool {
    is_turn_of(model, by) || role_of(model, by) == Some(PlayerRole::GameMaster)
}

/// Board reads of one layer for a resolution: LoS only for casts.
struct LayerProbes<'a> {
    model: &'a Model,
    layer: &'a str,
    grid: Option<&'a mut RouteGrid>,
    emitters: Vec<Emitter>,
    los: bool,
}

impl Probes for LayerProbes<'_> {
    fn cast_ray(&mut self, from: Pt, to: Pt) -> Option<bool> {
        if !self.los {
            return None;
        }
        // No grid = no map to block: casts see as before the board existed.
        Some(
            self.grid
                .as_deref_mut()
                .is_none_or(|g| g.cast_ray(from, to)),
        )
    }

    fn light_at(&mut self, tile: Pt) -> Option<f64> {
        let falloff = self.model.game.lighting_falloff;
        light_level(self.grid.as_deref_mut()?, &self.emitters, tile, falloff)
    }

    fn within(&mut self, tile: Pt, r: f64) -> Vec<Combatant> {
        let ctx = Ctx::new(self.model);
        let Some(layer) = self.model.layer(self.layer) else {
            return vec![];
        };
        layer
            .tokens
            .iter()
            .filter(|t| {
                t.is_character()
                    && t.position
                        .is_some_and(|p| manhattan(tile_of(p), tile) as f64 <= r)
            })
            .map(|t| combatant_of(t, &ctx))
            .collect()
    }
}

/// One intent's working state: a scratch model the writes land on as they're made.
struct Tx<'b> {
    model: Model,
    boards: &'b mut Boards,
    rng: &'b mut dyn Rng,
    writes: Vec<Write>,
}

impl<'b> Tx<'b> {
    fn write(&mut self, w: Write) {
        apply_write(&mut self.model, &w);
        self.writes.push(w);
    }

    fn token(&self, yid: &str) -> Option<Token> {
        self.model.token(yid).map(|(_, t)| t.clone())
    }

    fn layer_of(&self, yid: &str) -> Option<String> {
        self.model.token(yid).map(|(l, _)| l.yid.clone())
    }

    fn layer_tokens(&self, layer: &str) -> Vec<Token> {
        self.model.layer(layer).map_or(vec![], |l| l.tokens.clone())
    }

    /// Run `f` with a resolve context over `layer`'s board.
    fn resolving<R>(
        &mut self,
        layer: &str,
        los: bool,
        f: impl FnOnce(&mut ResolveCtx, &Ctx) -> R,
    ) -> R {
        let Tx {
            model, boards, rng, ..
        } = self;
        let model: &Model = model;
        let ctx = Ctx::new(model);
        let emitters = model
            .layer(layer)
            .map_or(vec![], |l| emitters_of(&l.tokens, &ctx));
        let grid = boards.grid(model, layer);
        let mut probes = LayerProbes {
            model,
            layer,
            grid,
            emitters,
            los,
        };
        let roles = ctx.roles.clone();
        let mut rctx = ResolveCtx {
            turn: model.turn,
            roles: &roles,
            defence_chance: model.game.defence_chance,
            rng: &mut **rng,
            probes: &mut probes,
        };
        f(&mut rctx, &Ctx::new(model))
    }

    fn set_token(&mut self, token: Token) {
        self.write(Write::SetToken { token });
    }

    /// Spend `amount` of the pool in `role`; a game without that role spends nothing.
    fn spend_role(&mut self, yid: &str, role: Role, amount: f64) {
        let Some(t) = self.token(yid) else { return };
        let ctx = Ctx::new(&self.model);
        let Some(pool) = ctx.roles.get(role).map(str::to_string) else {
            return;
        };
        let next = spent(&t, &ctx, &pool, amount);
        self.set_token(next);
    }

    /// An item and every reference to it (slots, equips).
    fn remove_item(&mut self, yid: &str) {
        let invs: Vec<Inventory> = self
            .model
            .inventories
            .values()
            .filter(|i| i.stuff.iter().any(|s| s == yid))
            .cloned()
            .collect();
        for mut inv in invs {
            inv.stuff
                .iter_mut()
                .filter(|s| *s == yid)
                .for_each(|s| s.clear());
            self.write(Write::SetInventory { inventory: inv });
        }
        let holders: Vec<Token> = self
            .model
            .tokens()
            .filter(|(_, t)| {
                t.equiped
                    .as_ref()
                    .is_some_and(|e| e.values().any(|v| v == yid))
            })
            .map(|(_, t)| t.clone())
            .collect();
        for t in holders {
            let equiped = equiped_without(t.equiped.as_ref(), yid);
            self.set_token(Token {
                equiped: (!equiped.is_empty()).then_some(equiped),
                ..t
            });
        }
        self.write(Write::RemoveItem {
            yid: yid.to_string(),
        });
    }

    /// A token with its inventory and the items in it or on it.
    fn remove_token(&mut self, yid: &str) {
        let Some(t) = self.token(yid) else { return };
        let inv = t
            .inventory_yid
            .as_deref()
            .and_then(|i| self.model.inventories.get(i))
            .cloned();
        let mut held: Vec<String> = vec![];
        for y in inv
            .iter()
            .flat_map(|i| i.stuff.iter())
            .chain(t.equiped.iter().flat_map(|e| e.values()))
        {
            if !y.is_empty() && !held.contains(y) {
                held.push(y.clone());
            }
        }
        for item in held {
            self.remove_item(&item);
        }
        if let Some(inv) = inv {
            self.write(Write::RemoveInventory { yid: inv.yid });
        }
        self.write(Write::RemoveToken {
            yid: yid.to_string(),
        });
    }

    /// A colliding solid on `tile` of `layer` denies spawn/teleport there.
    fn blocked(&self, layer: &str, tile: Pt, except: Option<&str>) -> bool {
        let ctx = Ctx::new(&self.model);
        self.model
            .layer(layer)
            .is_some_and(|l| collides_at(&l.tokens, &ctx, tile, except))
    }

    /// Grant items into a token's inventory (ADR-0019): created if none, grown if full; `equipped` ones onto their spot.
    fn outfit(
        &mut self,
        decl: &InventoryDecl,
        holder: &Token,
        expires_turn: Option<i64>,
    ) -> (String, Equiped) {
        let mut equiped = holder.equiped.clone().unwrap_or_default();
        let mut yids = vec![];
        for grant in &decl.items {
            let Some(def) = self.model.defs.items.get(&grant.def).cloned() else {
                continue;
            };
            let item = Item {
                yid: self.rng.next_yid(),
                item_definition_id: def.id.clone(),
                pools: item_pools(&def.pools),
                expires_turn,
            };
            yids.push(item.yid.clone());
            self.write(Write::SetItem { item });
            if let (true, Some(spot)) = (grant.equipped, def.spot) {
                equiped = toggle_equip(Some(&equiped), yids.last().unwrap(), spot);
            }
        }
        let existing = holder
            .inventory_yid
            .as_deref()
            .and_then(|y| self.model.inventories.get(y))
            .cloned();
        let inventory = match existing {
            None => {
                let capacity = decl.capacity.unwrap_or(3.0).max(yids.len() as f64);
                let mut stuff = yids.clone();
                stuff.resize(capacity as usize, String::new());
                Inventory {
                    yid: self.rng.next_yid(),
                    stuff,
                    capacity: Some(capacity),
                    owner_token_yid: Some(holder.yid.clone()),
                    name: Some(format!(
                        "{}'s Inventory",
                        holder.name.as_deref().unwrap_or("Token")
                    )),
                }
            }
            Some(mut inv) => {
                for y in yids {
                    match inv.stuff.iter_mut().find(|s| s.is_empty()) {
                        Some(free) => *free = y,
                        None => inv.stuff.push(y),
                    }
                }
                inv
            }
        };
        let yid = inventory.yid.clone();
        self.write(Write::SetInventory { inventory });
        (yid, equiped)
    }

    /// Instantiate token def `def` on `layer`, owned like `actor`; denied onto a colliding solid.
    fn spawn(
        &mut self,
        actor: &Token,
        layer: &str,
        def_id: &str,
        tile: Pt,
        expires_turn: Option<i64>,
    ) {
        let Some(def) = self
            .model
            .defs
            .tokens
            .get(def_id)
            .filter(|d| d.spawnable)
            .cloned()
        else {
            return;
        };
        // The spawner never blocks its own spawn (a lamp's glow on its own tile).
        if self.model.layer(layer).is_none() || self.blocked(layer, tile, Some(&actor.yid)) {
            return;
        }
        let yid = self.rng.next_yid();
        let ctx = Ctx::new(&self.model);
        let mut token = Token {
            yid: yid.clone(),
            token_id: Some(def.id.clone()),
            owner_player_yid: actor.owner_player_yid.clone(),
            position: Some(px_of(tile)),
            expires_turn,
            ..Token::default()
        };
        if let Some(stats) = def.stats {
            token.name = def.name.clone();
            token.stats = Some(stats);
            token.pools = Some(initial_pools(stats, Some(&def.id), &ctx));
        }
        if let Some(decl) = &def.inventory {
            let holder = Token {
                name: def.name.clone(),
                ..token.clone()
            };
            let (inv, equiped) = self.outfit(decl, &holder, None);
            token.inventory_yid = Some(inv);
            token.equiped = (!equiped.is_empty()).then_some(equiped);
        }
        self.write(Write::AddToken {
            layer: layer.to_string(),
            token,
        });
    }

    /// Writes for resolver outcomes, each on the state the last left.
    fn land(&mut self, actor: &Token, layer: &str, outcomes: &[Outcome]) {
        for o in outcomes {
            if let Outcome::Teleport { tile, target, .. } = o {
                if self.blocked(layer, *tile, Some(target)) {
                    continue;
                }
            }
            if let Some(yid) = outcome_token_yid(o) {
                let Some(t) = self.token(yid) else { continue };
                let next = token_after(&t, o, &Ctx::new(&self.model));
                self.set_token(next);
                continue;
            }
            match o {
                Outcome::Spend {
                    item: Some(item),
                    pool,
                    amount,
                    ..
                } => {
                    let Some(inst) = self.model.items.get(item).cloned() else {
                        continue;
                    };
                    let (next, destroy) = item_after_spend(
                        &inst,
                        self.model.defs.items.get(&inst.item_definition_id),
                        pool,
                        *amount,
                    );
                    self.write(Write::SetItem { item: next });
                    if destroy {
                        self.remove_item(item);
                    }
                }
                Outcome::Restore {
                    item: Some(item),
                    pool,
                    amount,
                    ..
                } => {
                    let Some(inst) = self.model.items.get(item).cloned() else {
                        continue;
                    };
                    let next = item_after_restore(
                        &inst,
                        self.model.defs.items.get(&inst.item_definition_id),
                        pool,
                        *amount,
                    );
                    self.write(Write::SetItem { item: next });
                }
                Outcome::Spawn {
                    def,
                    tile,
                    expires_turn,
                    ..
                } => self.spawn(actor, layer, def, *tile, *expires_turn),
                Outcome::Give {
                    target,
                    def,
                    amount,
                    equipped,
                    expires_turn,
                    ..
                } => {
                    let Some(t) = self.token(target) else {
                        continue;
                    };
                    let grant = ItemGrant {
                        def: def.clone(),
                        equipped: *equipped == Some(true),
                    };
                    let decl = InventoryDecl {
                        capacity: None,
                        items: vec![grant; *amount as usize],
                    };
                    let (inv, equiped) = self.outfit(&decl, &t, *expires_turn);
                    let t = self.token(target).unwrap_or(t);
                    self.set_token(Token {
                        inventory_yid: Some(inv),
                        equiped: (!equiped.is_empty())
                            .then_some(equiped)
                            .or(t.equiped.clone()),
                        ..t
                    });
                }
                _ => {}
            }
        }
    }

    /// `mover` walked onto `tile`: its `step` riders, then every token there fires its `enter` riders at it (#153).
    fn arrive(&mut self, mover_yid: &str, layer: &str, tile: Pt) {
        if let Some(mover) = self
            .token(mover_yid)
            .filter(|m| m.is_character() && !is_dead(m, &Ctx::new(&self.model)))
        {
            let outcomes = self.resolving(layer, false, |rctx, ctx| {
                let me = Combatant {
                    tile: Some(tile),
                    ..combatant_of(&mover, ctx)
                };
                resolve(RuleEvent::Step, &me, rctx)
            });
            self.land(&mover, layer, &outcomes.unwrap_or_default());
        }
        let Some(mover) = self.token(mover_yid).filter(|m| m.is_character()) else {
            return;
        };
        let ctx = Ctx::new(&self.model);
        if is_dead(&mover, &ctx) {
            return;
        }
        let them = Combatant {
            tile: Some(tile),
            ..combatant_of(&mover, &ctx)
        };
        // Wall tokens sit on edges: never entered.
        let carriers: Vec<Token> = self
            .layer_tokens(layer)
            .into_iter()
            .filter(|t| {
                let def = ctx.token_def(t.token_id.as_deref());
                t.yid != mover.yid
                    && def.and_then(|d| d.token_type.as_deref()) != Some("wall")
                    && t.position
                        .is_some_and(|p| footprint(p, def).contains(&tile))
                    && !is_dead(t, &ctx)
            })
            .collect();
        // Every carrier resolves on one state, then each lands in turn.
        let fired: Vec<(Token, Vec<Outcome>)> = self.resolving(layer, false, |rctx, ctx| {
            // Host positions may predate the move: the mover counts at `tile`.
            let mut at = MoverAt {
                inner: &mut *rctx.probes,
                mover: &them,
            };
            let mut rctx = ResolveCtx {
                turn: rctx.turn,
                roles: rctx.roles,
                defence_chance: rctx.defence_chance,
                rng: &mut *rctx.rng,
                probes: &mut at,
            };
            carriers
                .iter()
                .filter_map(|c| {
                    let outcomes = resolve(
                        RuleEvent::Enter { mover: &them },
                        &combatant_of(c, ctx),
                        &mut rctx,
                    )
                    .unwrap_or_default();
                    (!outcomes.is_empty()).then(|| (c.clone(), outcomes))
                })
                .collect()
        });
        let target = LedgerTarget {
            yid: &mover.yid,
            name: mover.name.as_deref(),
            defence: Some(them.stats.defence),
        };
        for (carrier, outcomes) in fired {
            self.land(&carrier, layer, &outcomes);
            if let Some(entry) = rider_entry_of(&carrier.yid, &target, tile, &outcomes) {
                self.write(Write::PushAction { action: entry });
            }
        }
    }
}

/// `within` with the mover counted at its new tile, whatever its stored position says.
struct MoverAt<'p, 'm> {
    inner: &'p mut dyn Probes,
    mover: &'m Combatant,
}

impl Probes for MoverAt<'_, '_> {
    fn cast_ray(&mut self, from: Pt, to: Pt) -> Option<bool> {
        self.inner.cast_ray(from, to)
    }
    fn light_at(&mut self, tile: Pt) -> Option<f64> {
        self.inner.light_at(tile)
    }
    fn within(&mut self, centre: Pt, r: f64) -> Vec<Combatant> {
        let mut out: Vec<Combatant> = self
            .inner
            .within(centre, r)
            .into_iter()
            .filter(|c| c.yid != self.mover.yid)
            .collect();
        if self
            .mover
            .tile
            .is_some_and(|t| manhattan(centre, t) as f64 <= r)
        {
            out.push(self.mover.clone());
        }
        out
    }
}

/// Gates every actor action shares: exists, controlled by `by`, alive, its side's turn.
fn actor_of(model: &Model, by: &str, actor: &str) -> Result<Token, Rejected> {
    let (_, t) = model
        .token(actor)
        .ok_or_else(|| no(Refusal::NotFound, "No such character"))?;
    if !can_control(t, by, model) {
        return Err(no(Refusal::NotYours, "You do not control that character"));
    }
    if is_dead(t, &Ctx::new(model)) {
        return Err(no(
            Refusal::Dead,
            format!(
                "{} is dead and cannot act",
                t.name.as_deref().unwrap_or(&t.yid)
            ),
        ));
    }
    if !is_turn_of(model, by) {
        return Err(no(
            Refusal::NotYourTurn,
            format!("It is not your turn (turn {})", model.turn),
        ));
    }
    Ok(t.clone())
}

fn gm(model: &Model, by: &str) -> Result<(), Rejected> {
    match role_of(model, by) {
        Some(PlayerRole::GameMaster) => Ok(()),
        _ => Err(no(Refusal::NotGm, "Only the game master can do that")),
    }
}

fn out_of_actions(token: &Token, ctx: &Ctx) -> bool {
    role_value(token, ctx, Role::Actions).unwrap_or(1.0) < 1.0
}

/// The game's one rule step: decide, then the writes that carry it out — or why not.
pub fn update(
    model: &Model,
    boards: &mut Boards,
    intent: &Intent,
    rng: &mut dyn Rng,
) -> Result<Vec<Write>, Rejected> {
    let mut tx = Tx {
        model: model.clone(),
        boards,
        rng,
        writes: vec![],
    };
    match intent {
        Intent::Door {
            by,
            door,
            actor,
            open,
        } => door_intent(&mut tx, by, door, actor.as_deref(), *open)?,
        Intent::Cast {
            by,
            actor,
            carrier,
            item,
            spell,
            target,
            tile,
        } => cast(
            &mut tx,
            by,
            actor,
            carrier,
            item.as_deref(),
            spell,
            target.as_deref(),
            *tile,
        )?,
        Intent::Move { by, actor, to } => walk(&mut tx, by, actor, *to)?,
        Intent::PassTurn { by } => pass_turn(&mut tx, by)?,
        Intent::Rotate { by, yid } => {
            gm(model, by)?;
            let t = tx
                .token(yid)
                .ok_or_else(|| no(Refusal::NotFound, "No such token"))?;
            if Ctx::new(model)
                .token_def(t.token_id.as_deref())
                .and_then(|d| d.token_type.as_deref())
                == Some("wall")
            {
                return Err(no(Refusal::Wall, "Walls don't rotate"));
            }
            let angle = t.angle.unwrap_or(0.0) + 90.0;
            tx.set_token(Token {
                angle: Some(angle),
                ..t
            });
        }
        Intent::Remove { by, yid } => {
            gm(model, by)?;
            tx.token(yid)
                .ok_or_else(|| no(Refusal::NotFound, "No such token"))?;
            tx.remove_token(yid);
        }
        Intent::Say {
            by,
            actor,
            text,
            volume,
        } => {
            let (_, t) = model
                .token(actor)
                .ok_or_else(|| no(Refusal::NotFound, "No such character"))?;
            if !can_control(t, by, model) {
                return Err(no(Refusal::NotYours, "You do not control that character"));
            }
            if is_dead(t, &Ctx::new(model)) {
                return Err(no(Refusal::Dead, "The dead don't talk"));
            }
            let tile = t
                .position
                .map(tile_of)
                .ok_or_else(|| no(Refusal::Unplaced, "Not on the board"))?;
            let text: String = text.trim().chars().take(SAY_MAX_LEN).collect();
            if text.is_empty() {
                return Err(no(Refusal::Empty, "Nothing to say"));
            }
            tx.write(Write::PushAction {
                action: GameAction::Say {
                    actor_yid: actor.clone(),
                    text,
                    tile,
                    volume: *volume,
                },
            });
        }
    }
    Ok(tx.writes)
}

fn door_intent(
    tx: &mut Tx,
    by: &str,
    door_yid: &str,
    actor: Option<&str>,
    want: Option<bool>,
) -> Result<(), Rejected> {
    let door = tx
        .token(door_yid)
        .ok_or_else(|| no(Refusal::NotFound, "No such door"))?;
    let open = want.unwrap_or(!is_open(&door));
    if open == is_open(&door) {
        return Err(no(
            Refusal::Already,
            if open {
                "Already open"
            } else {
                "Already closed"
            },
        ));
    }
    let Some(actor) = actor else {
        gm(&tx.model, by)?;
        tx.set_token(Token {
            open: Some(open),
            ..door
        });
        return Ok(());
    };
    let me = actor_of(&tx.model, by, actor)?;
    let ctx = Ctx::new(&tx.model);
    let tiles = door_footprint(&door, ctx.token_def(door.token_id.as_deref()))
        .ok_or_else(|| no(Refusal::NotFound, "That doesn't open"))?;
    if tx.layer_of(door_yid) != tx.layer_of(actor) {
        return Err(no(Refusal::TooFar, "Stand at the door first"));
    }
    let at = me
        .position
        .map(tile_of)
        .ok_or_else(|| no(Refusal::Unplaced, "Not on the board"))?;
    let bonus = if me.is_character() {
        character_of(&me, &ctx).reach.interact.unwrap_or(0.0)
    } else {
        0.0
    };
    if !next_to_door(at, &tiles, bonus) {
        return Err(no(Refusal::TooFar, "Stand at the door first"));
    }
    if out_of_actions(&me, &ctx) {
        return Err(no(Refusal::OutOfActions, "Out of actions"));
    }
    tx.set_token(Token {
        open: Some(open),
        ..door
    });
    tx.spend_role(actor, Role::Actions, 1.0);
    tx.write(Write::PushAction {
        action: GameAction::Door {
            actor_yid: actor.to_string(),
            door_yid: door_yid.to_string(),
            open,
        },
    });
    Ok(())
}

fn reach_refusal(r: Reach, range: f64) -> Rejected {
    match r {
        Reach::Range => no(Refusal::TooFar, format!("Out of range ({range})")),
        Reach::Blocked => no(Refusal::Blocked, "Blocked"),
        Reach::Lit => no(Refusal::Lit, "Too bright"),
        Reach::Dark => no(Refusal::Dark, "Too dark"),
        Reach::Unplaced | Reach::Ok => no(Refusal::Unplaced, "Not on the board"),
    }
}

#[allow(clippy::too_many_arguments)]
fn cast(
    tx: &mut Tx,
    by: &str,
    actor: &str,
    carrier: &str,
    item: Option<&str>,
    spell_name: &str,
    target: Option<&str>,
    tile: Option<Pt>,
) -> Result<(), Rejected> {
    let me_token = actor_of(&tx.model, by, actor)?;
    if !me_token.is_character() {
        return Err(no(Refusal::NotFound, "Only characters cast"));
    }
    let layer = tx
        .layer_of(actor)
        .ok_or_else(|| no(Refusal::NotFound, "No such character"))?;
    let ctx = Ctx::new(&tx.model);
    let me = combatant_of(&me_token, &ctx);
    let (ci, si) = casts_of(&me.carriers)
        .into_iter()
        .find(|(ci, si)| {
            let c = &me.carriers[*ci];
            c.name == carrier
                && c.item_yid.as_deref() == item
                && c.spells[*si].name.as_deref() == Some(spell_name)
        })
        .ok_or_else(|| no(Refusal::NotFound, format!("{spell_name} isn't available")))?;
    let spell = me.carriers[ci].spells[si].clone();
    let target_token = match target {
        None => None,
        Some(y) => Some(
            tx.model
                .layer(&layer)
                .and_then(|l| l.tokens.iter().find(|t| t.yid == y))
                .cloned()
                .ok_or_else(|| no(Refusal::NotFound, "No such target here"))?,
        ),
    };
    if !aims(&spell, actor, target_token.as_ref()) {
        return Err(no(
            Refusal::NotFound,
            format!("{spell_name} can't target that"),
        ));
    }
    if spell
        .effects
        .iter()
        .any(|e| matches!(e, Effect::Spawn { .. } | Effect::Teleport { .. }))
        && tile.is_some_and(|t| tx.blocked(&layer, t, Some(actor)))
    {
        return Err(no(Refusal::Blocked, "Something solid is in the way"));
    }
    let them = target_token.as_ref().map(|t| combatant_of(t, &ctx));
    let bonus = me.reach.cast.unwrap_or(0.0);
    let to = if spell.target == Target::Self_ {
        me.tile
    } else {
        tile.or(them.as_ref().and_then(|t| t.tile))
    };
    let outcomes = tx.resolving(&layer, true, |rctx, _| {
        let r = reach(&spell, me.tile, to, rctx.probes, bonus);
        if r != Reach::Ok {
            return Err(reach_refusal(r, super::resolve::range_of(&spell, bonus)));
        }
        if !payable(&spell, ci, &me, rctx.roles) {
            let actions = rctx.roles.actions.as_ref();
            if spell.action
                && actions.is_some_and(|a| me.pools.get(a).copied().unwrap_or(0.0) < 1.0)
            {
                return Err(no(Refusal::OutOfActions, "Out of actions"));
            }
            let pools: Vec<&str> = spell.costs.iter().map(|c| c.pool.as_str()).collect();
            return Err(no(Refusal::CantPay, format!("Needs {}", pools.join(", "))));
        }
        resolve(
            RuleEvent::Cast {
                carrier: ci,
                spell: &spell,
                target: them.as_ref(),
                tile,
            },
            &me,
            rctx,
        )
        .ok_or_else(|| no(Refusal::CantPay, "Can't pay for that"))
    })?;
    let at = tile.or(target_token.as_ref().and_then(|t| t.position).map(tile_of));
    let label = spell.name.clone().unwrap_or_else(|| carrier.to_string());
    let ledger_target = target_token.as_ref().map(|t| LedgerTarget {
        yid: &t.yid,
        name: t.name.as_deref(),
        defence: them.as_ref().map(|c| c.stats.defence),
    });
    let entry = ledger_entry_of(
        actor,
        &label,
        &spell,
        ledger_target.as_ref(),
        tile,
        &outcomes,
    );
    tx.land(&me_token, &layer, &outcomes);
    // Own tile (self, or a token on the caster) has no heading.
    if let (Some(from), Some(at)) = (me.tile, at) {
        if from != at {
            if let Some(t) = tx.token(actor) {
                tx.set_token(Token {
                    angle: Some(heading_deg(from, at)),
                    ..t
                });
            }
        }
    }
    tx.write(Write::PushAction { action: entry });
    Ok(())
}

/// Adjacent in-world step the map blocks (wall edge, closed door, rock) — ADR-0017 phase; solid tokens still block.
pub fn is_phase_step(
    board: &mut dyn Board,
    from: Pt,
    to: Pt,
    solid_at: impl Fn(Pt) -> bool,
) -> bool {
    if manhattan(from, to) != 1 || !board.in_world(to) || solid_at(to) {
        return false;
    }
    // Undrawn tiles are rock; the engine would route rock to rock as one room.
    if board.tile_at(from) == 0 || board.tile_at(to) == 0 {
        return true;
    }
    !board.neighbours(from).contains(&to)
}

/// A phase step spends 1 of the phase role and 1 of the moves role.
pub fn can_phase(token: &Token, ctx: &Ctx) -> bool {
    role_value(token, ctx, Role::Phase).unwrap_or(0.0) >= 1.0
        && role_value(token, ctx, Role::Moves).unwrap_or(0.0) >= 1.0
}

fn centres(route: &[Pt]) -> Vec<f64> {
    route
        .iter()
        .flat_map(|p| {
            [
                p.x as f64 * TILE_SIZE + HALF_TILE,
                p.y as f64 * TILE_SIZE + HALF_TILE,
            ]
        })
        .collect()
}

fn walk(tx: &mut Tx, by: &str, actor: &str, to: Pt) -> Result<(), Rejected> {
    let me = actor_of(&tx.model, by, actor)?;
    if !me.is_character() {
        return Err(no(Refusal::NotFound, "Only characters walk"));
    }
    let from = me
        .position
        .map(tile_of)
        .ok_or_else(|| no(Refusal::Unplaced, "Not on the board"))?;
    let layer = tx
        .layer_of(actor)
        .ok_or_else(|| no(Refusal::NotFound, "No such character"))?;
    let (route, phase) = {
        let Tx { model, boards, .. } = &mut *tx;
        let ctx = Ctx::new(model);
        let tokens = model.layer(&layer).map_or(&[][..], |l| &l.tokens[..]);
        let grid = boards
            .grid(model, &layer)
            .ok_or_else(|| no(Refusal::NotFound, "No such layer"))?;
        let route = if grid.in_world(to) {
            grid.path(from, to)
        } else {
            vec![]
        };
        let steps = route.len() as f64 - 1.0;
        let moves = role_value(&me, &ctx, Role::Moves).unwrap_or(0.0);
        if steps > 0.0 && steps <= moves {
            (route, false)
        } else if can_phase(&me, &ctx)
            && is_phase_step(grid, from, to, |p| {
                collides_at(tokens, &ctx, p, Some(actor))
            })
        {
            (vec![from, to], true)
        } else if steps > 0.0 {
            return Err(no(
                Refusal::TooFar,
                format!("Too far: {steps} steps, {moves} moves left"),
            ));
        } else {
            return Err(no(Refusal::Unreachable, "No route there"));
        }
    };
    if phase {
        tx.spend_role(actor, Role::Moves, 1.0);
        tx.spend_role(actor, Role::Phase, 1.0);
    } else {
        tx.spend_role(actor, Role::Moves, route.len() as f64 - 1.0);
    }
    // The move entry leads the riders it sets off; its path is what was actually walked.
    let (at_write, at_ledger) = (tx.writes.len(), tx.model.ledger.len());
    let start = me.position;
    let mut walked = 1;
    for tile in &route[1..] {
        tx.arrive(actor, &layer, *tile);
        walked += 1;
        // A rider that moved the mover (teleport) or felled it ends the walk there.
        let Some(live) = tx.token(actor) else { break };
        if live.position != start || is_dead(&live, &Ctx::new(&tx.model)) {
            break;
        }
    }
    let route = &route[..walked];
    let action = GameAction::Move {
        actor_yid: actor.to_string(),
        path: centres(route),
    };
    tx.model.ledger.insert(at_ledger, action.clone());
    tx.writes.insert(at_write, Write::PushAction { action });
    if let Some(t) = tx.token(actor).filter(|t| t.position == start) {
        let (last, end) = (route[route.len() - 2], route[route.len() - 1]);
        tx.set_token(Token {
            position: Some(px_of(end)),
            angle: Some(heading_deg(last, end)),
            ..t
        });
    }
    Ok(())
}

fn pass_turn(tx: &mut Tx, by: &str) -> Result<(), Rejected> {
    if !can_pass_turn(&tx.model, by) {
        return Err(no(
            Refusal::NotYourTurn,
            format!("It is not your turn (turn {})", tx.model.turn),
        ));
    }
    let turn = tx.model.turn + 1;
    tx.write(Write::SetTurn { turn });
    tx.write(Write::ClearLedger);
    let gone: Vec<String> = tx
        .model
        .items
        .values()
        .filter(|i| expired(i.expires_turn, turn))
        .map(|i| i.yid.clone())
        .collect();
    for yid in gone {
        tx.remove_item(&yid);
    }
    let all: Vec<(String, String)> = tx
        .model
        .tokens()
        .map(|(l, t)| (l.yid.clone(), t.yid.clone()))
        .collect();
    for (layer, yid) in all {
        // Earlier auras may have landed: read live.
        let Some(live) = tx.token(&yid) else { continue };
        if expired(live.expires_turn, turn) {
            tx.remove_token(&yid);
            continue;
        }
        if turn_type(turn) != Side::Players || is_dead(&live, &Ctx::new(&tx.model)) {
            continue;
        }
        let fresh = if live.is_character() {
            upkept(&live, &Ctx::new(&tx.model))
        } else {
            live.clone()
        };
        if live.is_character() {
            tx.set_token(fresh.clone());
        }
        let outcomes = tx.resolving(&layer, false, |rctx, ctx| {
            resolve(RuleEvent::Turn, &combatant_of(&fresh, ctx), rctx)
        });
        tx.land(&fresh, &layer, &outcomes.unwrap_or_default());
    }
    Ok(())
}
