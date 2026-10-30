//! Rule data as hosts send it (serde, camelCase = the TS/Yjs wire shape).
//! Quantities are `f64` like the JS numbers they mirror; a fixed-point pass is a later step.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Pools = BTreeMap<String, f64>;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Pt {
    pub x: i32,
    pub y: i32,
}

pub const fn pt(x: i32, y: i32) -> Pt {
    Pt { x, y }
}

/// World-px position; tokens sit on tile corners.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Px {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stat {
    Physique,
    Defence,
    Intelligence,
    Speed,
}

pub const STATS: [Stat; 4] = [
    Stat::Physique,
    Stat::Defence,
    Stat::Intelligence,
    Stat::Speed,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub physique: f64,
    pub defence: f64,
    pub intelligence: f64,
    pub speed: f64,
}

impl Stats {
    pub const ZERO: Stats = Stats {
        physique: 0.0,
        defence: 0.0,
        intelligence: 0.0,
        speed: 0.0,
    };

    pub fn get(&self, s: Stat) -> f64 {
        match s {
            Stat::Physique => self.physique,
            Stat::Defence => self.defence,
            Stat::Intelligence => self.intelligence,
            Stat::Speed => self.speed,
        }
    }

    pub fn get_mut(&mut self, s: Stat) -> &mut f64 {
        match s {
            Stat::Physique => &mut self.physique,
            Stat::Defence => &mut self.defence,
            Stat::Intelligence => &mut self.intelligence,
            Stat::Speed => &mut self.speed,
        }
    }
}

/// `<requires>`: only the stats present.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Requires {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physique: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intelligence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
}

impl Requires {
    pub fn get(&self, s: Stat) -> Option<f64> {
        match s {
            Stat::Physique => self.physique,
            Stat::Defence => self.defence,
            Stat::Intelligence => self.intelligence,
            Stat::Speed => self.speed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tally {
    Damage,
    Blocked,
}

/// Rider tallies of the triggering hit; zero outside riders.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tallies {
    pub damage: f64,
    pub blocked: f64,
}

/// `per=` scaler: a stat, a hit tally, or a pool gauge (`missing <pool>` = full minus current).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Per {
    Stat(Stat),
    Tally(Tally),
    Gauge { pool: String, missing: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verb {
    Damage,
    Restore,
    Modify,
    Spawn,
    Give,
    Attack,
    Teleport,
    Polymorph,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trigger {
    Cast,
    Attack,
    Hurt,
    Kill,
    Turn,
    Enter,
    Step,
    Passive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[serde(rename = "self")]
    Self_,
    Token,
    Tile,
    Area,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum While {
    Wielded,
    Equipped,
    Carried,
    Always,
}

/// Where a carrier sits on its actor; same words as `While`.
pub type CarrierStatus = While;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Refill {
    Never,
    Turn,
    Game,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReachKind {
    Cast,
    Interact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Health,
    Moves,
    Actions,
    Phase,
    Hidden,
    Ward,
}

/// Role → pool name; absent = the game has no pool in that role.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Roles {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ward: Option<String>,
}

impl Roles {
    pub fn get(&self, r: Role) -> Option<&str> {
        match r {
            Role::Health => self.health.as_deref(),
            Role::Moves => self.moves.as_deref(),
            Role::Actions => self.actions.as_deref(),
            Role::Phase => self.phase.as_deref(),
            Role::Hidden => self.hidden.as_deref(),
            Role::Ward => self.ward.as_deref(),
        }
    }

    pub fn slot(&mut self, r: Role) -> &mut Option<String> {
        match r {
            Role::Health => &mut self.health,
            Role::Moves => &mut self.moves,
            Role::Actions => &mut self.actions,
            Role::Phase => &mut self.phase,
            Role::Hidden => &mut self.hidden,
            Role::Ward => &mut self.ward,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EquipSpot {
    Chest,
    Back,
    Shoulders,
    HandL,
    HandR,
    Head,
}

/// Draw order; later = on top.
pub const EQUIP_SPOTS: [EquipSpot; 6] = [
    EquipSpot::Chest,
    EquipSpot::Back,
    EquipSpot::Shoulders,
    EquipSpot::HandL,
    EquipSpot::HandR,
    EquipSpot::Head,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemSpot {
    Chest,
    Back,
    Shoulders,
    Hand,
    Head,
}

impl EquipSpot {
    pub fn key(self) -> &'static str {
        match self {
            EquipSpot::Chest => "chest",
            EquipSpot::Back => "back",
            EquipSpot::Shoulders => "shoulders",
            EquipSpot::HandL => "handL",
            EquipSpot::HandR => "handR",
            EquipSpot::Head => "head",
        }
    }
}

/// Equip map: spot key → item yid; string keys so a stray key never fails a whole model.
pub type Equiped = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub pool: String,
    pub amount: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Pierce {
    Points(f64),
    All(AllWord),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AllWord {
    All,
}

/// A verb inside a `<spell>` (ADR-0017).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "lowercase")]
pub enum Effect {
    Damage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        per: Option<Per>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pierce: Option<Pierce>,
    },
    Restore {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
        pool: String,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        per: Option<Per>,
    },
    Modify {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stat: Option<Stat>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pool: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reach: Option<ReachKind>,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        per: Option<Per>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turns: Option<f64>,
    },
    Spawn {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
        def: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turns: Option<f64>,
    },
    Give {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
        def: String,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        per: Option<Per>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        equipped: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turns: Option<f64>,
    },
    Attack {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
    },
    Teleport {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
    },
    Polymorph {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<f64>,
        chance: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        def: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turns: Option<f64>,
    },
}

impl Effect {
    pub fn verb(&self) -> Verb {
        match self {
            Effect::Damage { .. } => Verb::Damage,
            Effect::Restore { .. } => Verb::Restore,
            Effect::Modify { .. } => Verb::Modify,
            Effect::Spawn { .. } => Verb::Spawn,
            Effect::Give { .. } => Verb::Give,
            Effect::Attack { .. } => Verb::Attack,
            Effect::Teleport { .. } => Verb::Teleport,
            Effect::Polymorph { .. } => Verb::Polymorph,
        }
    }

    pub fn chance(&self) -> f64 {
        match self {
            Effect::Damage { chance, .. }
            | Effect::Restore { chance, .. }
            | Effect::Modify { chance, .. }
            | Effect::Spawn { chance, .. }
            | Effect::Give { chance, .. }
            | Effect::Attack { chance, .. }
            | Effect::Teleport { chance, .. }
            | Effect::Polymorph { chance, .. } => *chance,
        }
    }

    pub fn r(&self) -> Option<f64> {
        match self {
            Effect::Damage { r, .. }
            | Effect::Restore { r, .. }
            | Effect::Modify { r, .. }
            | Effect::Spawn { r, .. }
            | Effect::Give { r, .. }
            | Effect::Attack { r, .. }
            | Effect::Teleport { r, .. }
            | Effect::Polymorph { r, .. } => *r,
        }
    }

    /// `amount` and `per`, for verbs that have an amount.
    pub fn amount(&self) -> Option<(f64, Option<&Per>)> {
        match self {
            Effect::Damage { amount, per, .. }
            | Effect::Restore { amount, per, .. }
            | Effect::Modify { amount, per, .. }
            | Effect::Give { amount, per, .. } => Some((*amount, per.as_ref())),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub x: f64,
    pub y: f64,
}

/// `<spell>` bundle: when, where, what it costs, then its verbs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spell {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub trigger: Trigger,
    pub target: Target,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposed: Option<f64>,
    pub chance: f64,
    #[serde(rename = "while")]
    pub while_: While,
    pub action: bool,
    pub fold: bool,
    pub costs: Vec<Cost>,
    pub effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pickable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requires>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<Cell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Empty {
    Keep,
    Destroy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShowAs {
    Bar,
    Pips,
    Number,
    Ring,
    Tint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShowTo {
    All,
    Owner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolShow {
    #[serde(rename = "as")]
    pub as_: ShowAs,
    pub to: ShowTo,
}

/// Only keys present in the XML are set, so a token-def override merges field-wise.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PoolDecl {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per: Option<Stat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refill: Option<Refill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<Empty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shows: Option<Vec<PoolShow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spells: Option<Vec<Spell>>,
}

/// Game `<spells base>`: slots = effective intelligence − base.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpellList {
    pub base: f64,
    pub spells: Vec<Spell>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemGrant {
    pub def: String,
    pub equipped: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InventoryDecl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<f64>,
    pub items: Vec<ItemGrant>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassCell {
    pub name: String,
    pub stats: Vec<Stat>,
    pub spells: Vec<Spell>,
    pub pools: Vec<PoolDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<InventoryDecl>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassesDef {
    pub gap: f64,
    pub fallback: String,
    pub cells: Vec<ClassCell>,
}

impl Default for ClassesDef {
    fn default() -> Self {
        ClassesDef {
            gap: 2.0,
            fallback: "Adventurer".into(),
            cells: vec![],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Class {
    pub name: String,
    pub spells: Vec<Spell>,
    pub pools: Vec<PoolDecl>,
    pub inventory: Option<InventoryDecl>,
}

/// Stat/pool-capacity/reach delta; passive `modify` verbs and timed modifiers share it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat: Option<Stat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<ReachKind>,
    pub amount: f64,
    /// Turn it lapses on; absent = permanent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
}

pub type StatDelta = Modifier;

/// A carrier as it sits on an actor: its spells + the instance holding its pools (`item_yid` none = the token).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCarrier {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_yid: Option<String>,
    pub status: CarrierStatus,
    pub weight: f64,
    pub pools: Pools,
    pub spells: Vec<Spell>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub learned: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reach {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cast: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interact: Option<f64>,
}

impl Reach {
    pub fn get(&self, k: ReachKind) -> Option<f64> {
        match k {
            ReachKind::Cast => self.cast,
            ReachKind::Interact => self.interact,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cast.is_none() && self.interact.is_none()
    }
}

/// Token as the resolver sees it: effective stats, live pools, carriers in cost-resolution order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Combatant {
    pub yid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<Pt>,
    pub stats: Stats,
    pub pools: Pools,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full: Option<Pools>,
    pub carriers: Vec<LiveCarrier>,
    #[serde(default, skip_serializing_if = "Reach::is_empty")]
    pub reach: Reach,
}

/// Polymorph (#152): rules read token def `def` until `until`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Form {
    pub def: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
}

/// Token fields the rules read and write; host-only fields (sprite, …) never cross.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub yid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_player_yid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Px>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<Stats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pools: Option<Pools>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<Vec<Modifier>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_turn: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equiped: Option<Equiped>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory_yid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spells: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<Form>,
}

impl Token {
    pub fn is_character(&self) -> bool {
        self.stats.is_some()
    }
}

/// Item instance: the def's `<pool>` decls live here as current values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub yid: String,
    pub item_definition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pools: Option<Pools>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_turn: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub weight: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot: Option<ItemSpot>,
    pub spells: Vec<Spell>,
    pub pools: Vec<PoolDecl>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub yid: String,
    pub stuff: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_token_yid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `<solid collides>` + `<transparency>` (ADR-0016).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Solid {
    pub collides: bool,
    pub transparency: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightDecl {
    pub radius: f64,
    pub intensity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmbientDecl {
    pub intensity: f64,
}

/// Token-def facts the rules read; host parsers (DOM, fast-xml) both produce it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(default)]
    pub is_window: bool,
    #[serde(default)]
    pub is_openable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solid: Option<Solid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// `<stats>`; none = not a character def.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<Stats>,
    #[serde(default)]
    pub pools: Vec<PoolDecl>,
    /// `<spells><spell def/>` refs: innate spells of its instances and of forms taking it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spells: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<LightDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<AmbientDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<InventoryDecl>,
    /// Has art: only such defs can spawn.
    #[serde(default)]
    pub spawnable: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Falloff {
    #[default]
    Linear,
    Sqrt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub defence_chance: f64,
    pub physique_carry_mult: f64,
    #[serde(default = "default_stat_points")]
    pub stat_points: f64,
    pub pools: Vec<PoolDecl>,
    #[serde(default)]
    pub classes: ClassesDef,
    #[serde(default)]
    pub spells: SpellList,
    #[serde(default)]
    pub lighting_falloff: Falloff,
}

fn default_stat_points() -> f64 {
    15.0
}

impl Default for Game {
    fn default() -> Self {
        Game {
            defence_chance: 50.0,
            physique_carry_mult: 0.0,
            stat_points: 15.0,
            pools: vec![],
            classes: ClassesDef::default(),
            spells: SpellList::default(),
            lighting_falloff: Falloff::Linear,
        }
    }
}

/// `trigger` absent = the cast itself; `carrier` = spell name, else its carrier's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum Outcome {
    Damage {
        carrier: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<Trigger>,
        target: String,
        amount: f64,
        blocks: f64,
        rolls: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        warded: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pierced: Option<f64>,
    },
    Restore {
        carrier: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<Trigger>,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<String>,
        pool: String,
        amount: f64,
    },
    Modify {
        carrier: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<Trigger>,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stat: Option<Stat>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pool: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reach: Option<ReachKind>,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        until: Option<i64>,
    },
    Spawn {
        carrier: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<Trigger>,
        tile: Pt,
        def: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expires_turn: Option<i64>,
    },
    Give {
        carrier: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<Trigger>,
        target: String,
        def: String,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        equipped: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expires_turn: Option<i64>,
    },
    Teleport {
        carrier: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<Trigger>,
        target: String,
        tile: Pt,
    },
    Polymorph {
        carrier: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger: Option<Trigger>,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        def: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        until: Option<i64>,
    },
    Spend {
        token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<String>,
        pool: String,
        amount: f64,
    },
}

impl Outcome {
    pub fn is_spend(&self) -> bool {
        matches!(self, Outcome::Spend { .. })
    }

    /// Verb name as the ledger writes it.
    pub fn verb(&self) -> Option<Verb> {
        Some(match self {
            Outcome::Damage { .. } => Verb::Damage,
            Outcome::Restore { .. } => Verb::Restore,
            Outcome::Modify { .. } => Verb::Modify,
            Outcome::Spawn { .. } => Verb::Spawn,
            Outcome::Give { .. } => Verb::Give,
            Outcome::Teleport { .. } => Verb::Teleport,
            Outcome::Polymorph { .. } => Verb::Polymorph,
            Outcome::Spend { .. } => return None,
        })
    }

    pub fn carrier(&self) -> Option<&str> {
        match self {
            Outcome::Damage { carrier, .. }
            | Outcome::Restore { carrier, .. }
            | Outcome::Modify { carrier, .. }
            | Outcome::Spawn { carrier, .. }
            | Outcome::Give { carrier, .. }
            | Outcome::Teleport { carrier, .. }
            | Outcome::Polymorph { carrier, .. } => Some(carrier),
            Outcome::Spend { .. } => None,
        }
    }

    pub fn trigger(&self) -> Option<Trigger> {
        match self {
            Outcome::Damage { trigger, .. }
            | Outcome::Restore { trigger, .. }
            | Outcome::Modify { trigger, .. }
            | Outcome::Spawn { trigger, .. }
            | Outcome::Give { trigger, .. }
            | Outcome::Teleport { trigger, .. }
            | Outcome::Polymorph { trigger, .. } => *trigger,
            Outcome::Spend { .. } => None,
        }
    }

    /// Token yid the outcome lands on; none for spawn and spend.
    pub fn target(&self) -> Option<&str> {
        match self {
            Outcome::Damage { target, .. }
            | Outcome::Restore { target, .. }
            | Outcome::Modify { target, .. }
            | Outcome::Give { target, .. }
            | Outcome::Teleport { target, .. }
            | Outcome::Polymorph { target, .. } => Some(target),
            Outcome::Spawn { .. } | Outcome::Spend { .. } => None,
        }
    }

    /// The ledger's `amount`: 0 for verbs without one.
    pub fn amount(&self) -> f64 {
        match self {
            Outcome::Damage { amount, .. }
            | Outcome::Restore { amount, .. }
            | Outcome::Modify { amount, .. }
            | Outcome::Give { amount, .. }
            | Outcome::Spend { amount, .. } => *amount,
            _ => 0.0,
        }
    }
}
