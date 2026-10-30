//! JS face of the rules: plain-object model in, plain-object writes out (serde, json-compatible).

use super::board::{Boards, DEFAULT_CELLS_PER_TILE};
use super::character::{casts_of, character_of, combatant_of, Ctx, PoolView};
use super::game::{self, Intent, Rejected, Write};
use super::model::Model;
use super::resolve::aims;
use super::rng::SplitMix64;
use super::types::Pt;
use super::types::{LiveCarrier, Reach, Stats};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;

fn from_js<T: DeserializeOwned>(v: JsValue, what: &str) -> Result<T, JsError> {
    serde_wasm_bindgen::from_value(v).map_err(|e| JsError::new(&format!("rules: bad {what}: {e}")))
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, JsError> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsError::new(&e.to_string()))
}

#[derive(Serialize)]
struct Decided {
    #[serde(skip_serializing_if = "Option::is_none")]
    writes: Option<Vec<Write>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejected: Option<Rejected>,
}

impl From<Result<Vec<Write>, Rejected>> for Decided {
    fn from(r: Result<Vec<Write>, Rejected>) -> Self {
        match r {
            Ok(writes) => Decided {
                writes: Some(writes),
                rejected: None,
            },
            Err(rejected) => Decided {
                writes: None,
                rejected: Some(rejected),
            },
        }
    }
}

/// A cast the actor could aim at a token/tile, and why not if it can't.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CastOption {
    carrier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<String>,
    spell: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejected: Option<Rejected>,
}

/// A character as the rules derive it; what UI reads will come from once they move off the TS helpers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CharacterJs {
    class_name: String,
    stats: Stats,
    pools: Vec<PoolView>,
    #[serde(skip_serializing_if = "Reach::is_empty")]
    reach: Reach,
    carriers: Vec<LiveCarrier>,
}

/// Rules over one host doc: the per-layer boards are a memo of the models passed in.
#[wasm_bindgen]
pub struct Rules {
    boards: Boards,
}

#[wasm_bindgen]
impl Rules {
    #[wasm_bindgen(constructor)]
    pub fn new(cells_per_tile: Option<usize>) -> Rules {
        Rules {
            boards: Boards::new(cells_per_tile.unwrap_or(DEFAULT_CELLS_PER_TILE)),
        }
    }

    /// `{ writes }` to apply in one transaction, or `{ rejected: { code, reason } }`. `seed` drives every roll and new yid.
    pub fn update(
        &mut self,
        model: JsValue,
        intent: JsValue,
        seed: f64,
    ) -> Result<JsValue, JsError> {
        let model: Model = from_js(model, "model")?;
        let intent: Intent = from_js(intent, "intent")?;
        self.boards.retain(&model);
        let decided: Decided = game::update(
            &model,
            &mut self.boards,
            &intent,
            &mut SplitMix64(seed as u64),
        )
        .into();
        to_js(&decided)
    }

    /// Cast spokes for `actor` on `target` (token yid) or bare `tile`: each live cast that aims there, dry-run.
    pub fn casts(
        &mut self,
        model: JsValue,
        by: String,
        actor: String,
        target: Option<String>,
        tile: JsValue,
    ) -> Result<JsValue, JsError> {
        let model: Model = from_js(model, "model")?;
        let tile: Option<Pt> = from_js(tile, "tile")?;
        self.boards.retain(&model);
        let Some((layer, me)) = model.token(&actor) else {
            return to_js(&Vec::<CastOption>::new());
        };
        let goal = target
            .as_deref()
            .and_then(|y| layer.tokens.iter().find(|t| t.yid == y));
        let carriers = combatant_of(me, &Ctx::new(&model)).carriers;
        let mut out = vec![];
        for (ci, si) in casts_of(&carriers) {
            let (c, s) = (&carriers[ci], &carriers[ci].spells[si]);
            let Some(name) = s.name.clone().filter(|_| aims(s, &actor, goal)) else {
                continue;
            };
            let intent = Intent::Cast {
                by: by.clone(),
                actor: actor.clone(),
                carrier: c.name.clone(),
                item: c.item_yid.clone(),
                spell: name.clone(),
                target: target.clone(),
                tile,
            };
            let rejected =
                game::update(&model, &mut self.boards, &intent, &mut SplitMix64(0)).err();
            out.push(CastOption {
                carrier: c.name.clone(),
                item: c.item_yid.clone(),
                spell: name,
                rejected,
            });
        }
        to_js(&out)
    }

    /// Token `yid` as the resolver sees it; undefined = no such token.
    pub fn combatant(model: JsValue, yid: String) -> Result<JsValue, JsError> {
        let model: Model = from_js(model, "model")?;
        let c = model
            .token(&yid)
            .map(|(_, t)| combatant_of(t, &Ctx::new(&model)));
        to_js(&c)
    }

    /// Class, effective stats, pool views and carriers of a character; undefined = none or not a character.
    pub fn character(model: JsValue, yid: String) -> Result<JsValue, JsError> {
        let model: Model = from_js(model, "model")?;
        let ctx = Ctx::new(&model);
        let view = model
            .token(&yid)
            .filter(|(_, t)| t.is_character())
            .map(|(_, t)| {
                let v = character_of(t, &ctx);
                CharacterJs {
                    class_name: v.cls.name,
                    stats: v.stats,
                    pools: v.pools,
                    reach: v.reach,
                    carriers: v.carriers,
                }
            });
        to_js(&view)
    }

    /// In-memory writer: the model after `writes`.
    pub fn apply(model: JsValue, writes: JsValue) -> Result<JsValue, JsError> {
        let model: Model = from_js(model, "model")?;
        let writes: Vec<Write> = from_js(writes, "writes")?;
        to_js(&game::apply(model, &writes))
    }
}
