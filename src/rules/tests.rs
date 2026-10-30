use super::board::Boards;
use super::character::*;
use super::game::*;
use super::ledger::GameAction;
use super::model::Model;
use super::resolve::*;
use super::rng::Script;
use super::stats::*;
use super::tiles::*;
use super::types::*;
use serde_json::{json, Value};

fn de<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

/// One 6×3 room of type 1 at the world's corner.
fn one_room() -> Vec<i32> {
    let mut tiles = vec![0; (LAYER_SIZE * LAYER_SIZE) as usize];
    for y in 0..3 {
        for x in 0..6 {
            tiles[(x + y * LAYER_SIZE) as usize] = 1;
        }
    }
    tiles
}

fn at(x: i32, y: i32) -> Value {
    json!({ "x": x as f64 * TILE_SIZE, "y": y as f64 * TILE_SIZE })
}

fn hero(yid: &str, x: i32, y: i32) -> Value {
    json!({ "yid": yid, "name": yid, "ownerPlayerYid": "p1", "position": at(x, y),
            "stats": { "physique": 3, "defence": 0, "intelligence": 1, "speed": 3 } })
}

fn slash() -> Value {
    json!({ "name": "Slash", "trigger": "cast", "target": "token", "chance": 1, "while": "wielded", "action": true, "fold": true,
            "costs": [], "effects": [{ "verb": "damage", "amount": 2, "chance": 1 }] })
}

/// Hero `h` (p1's, wielding a sword) at (1,1) next to an unowned ogre at (2,1); a door on the edge (2,1)|(3,1).
fn model() -> Model {
    let mut h = hero("h", 1, 1);
    h["equiped"] = json!({ "handR": "sw" });
    h["inventoryYid"] = json!("inv");
    let mut ogre = hero("ogre", 2, 1);
    ogre.as_object_mut().unwrap().remove("ownerPlayerYid");
    de(json!({
        "game": { "defenceChance": 0, "physiqueCarryMult": 5, "pools": [
            { "name": "health", "capacity": 3, "role": "health" },
            { "name": "moves", "capacity": 4, "role": "moves", "refill": "turn" },
            { "name": "actions", "capacity": 1, "role": "actions", "refill": "turn" }
        ] },
        "turn": 1,
        "defs": {
            "tokens": {
                "door": { "id": "door", "tokenType": "wall", "isOpenable": true },
                "wall": { "id": "wall", "tokenType": "wall" },
                "rock": { "id": "rock", "solid": { "collides": true, "transparency": 0 } },
                "spikes": { "id": "spikes", "pools": [{ "name": "spikes", "capacity": 1, "spells": [
                    { "trigger": "enter", "target": "token", "chance": 1, "while": "always", "action": true, "fold": true, "costs": [],
                      "effects": [{ "verb": "damage", "amount": 1, "chance": 1 }] }] }] },
                "imp": { "id": "imp", "name": "Imp", "spawnable": true, "stats": { "physique": 1, "defence": 0, "intelligence": 1, "speed": 1 } }
            },
            "items": { "sword": { "id": "sword", "name": "Sword", "weight": 1, "spot": "hand", "spells": [slash()], "pools": [] } }
        },
        "layers": [{ "yid": "L", "tiles": one_room(), "tokens": [
            h, ogre,
            { "yid": "d", "tokenId": "door", "position": { "x": 3.0 * TILE_SIZE - HALF_TILE, "y": TILE_SIZE }, "angle": 90 }
        ] }],
        "items": { "sw": { "yid": "sw", "itemDefinitionId": "sword" } },
        "inventories": { "inv": { "yid": "inv", "stuff": ["sw", ""], "capacity": 2, "ownerTokenYid": "h" } },
        "players": { "p1": { "role": "Player" }, "gm": { "role": "GameMaster" } }
    }))
}

fn run(m: &Model, intent: Value) -> Result<Vec<Write>, Rejected> {
    update(
        m,
        &mut Boards::default(),
        &de(intent),
        &mut Script::always(0.0),
    )
}

fn step(m: Model, intent: Value) -> Model {
    let writes = run(&m, intent).unwrap_or_else(|r| panic!("rejected: {}", r.reason));
    apply(m, &writes)
}

fn token<'m>(m: &'m Model, yid: &str) -> &'m Token {
    m.token(yid).unwrap().1
}

fn pool(m: &Model, yid: &str, name: &str) -> f64 {
    let ctx = Ctx::new(m);
    pools_of(token(m, yid), &ctx, None)[name]
}

fn code(r: Result<Vec<Write>, Rejected>) -> Refusal {
    r.expect_err("expected a refusal").code
}

#[test]
fn door_opens_from_beside_it_spends_an_action_and_logs() {
    let m = model();
    let writes = run(
        &m,
        json!({ "kind": "door", "by": "p1", "door": "d", "actor": "ogre" }),
    );
    assert_eq!(code(writes), Refusal::NotYours, "the ogre is the GM's");
    let moved = apply(
        m.clone(),
        &[Write::SetToken {
            token: Token {
                position: Some(px_of(pt(2, 1))),
                ..token(&m, "h").clone()
            },
        }],
    );
    let after = step(
        moved.clone(),
        json!({ "kind": "door", "by": "p1", "door": "d", "actor": "h" }),
    );
    assert_eq!(token(&after, "d").open, Some(true));
    assert_eq!(pool(&after, "h", "actions"), 0.0);
    assert_eq!(
        after.ledger,
        vec![GameAction::Door {
            actor_yid: "h".into(),
            door_yid: "d".into(),
            open: true
        }]
    );
    assert_eq!(
        code(run(
            &after,
            json!({ "kind": "door", "by": "p1", "door": "d", "actor": "h" })
        )),
        Refusal::OutOfActions
    );
    assert_eq!(
        code(run(
            &moved,
            json!({ "kind": "door", "by": "p1", "door": "d", "actor": "h", "open": false })
        )),
        Refusal::Already
    );
}

#[test]
fn door_gates_distance_turn_and_editor_flips() {
    let m = model();
    assert_eq!(
        code(run(
            &m,
            json!({ "kind": "door", "by": "p1", "door": "d", "actor": "h" })
        )),
        Refusal::TooFar
    );
    let gm_turn = Model {
        turn: 2,
        ..m.clone()
    };
    let near = apply(
        gm_turn,
        &[Write::SetToken {
            token: Token {
                position: Some(px_of(pt(2, 1))),
                ..token(&m, "h").clone()
            },
        }],
    );
    assert_eq!(
        code(run(
            &near,
            json!({ "kind": "door", "by": "p1", "door": "d", "actor": "h" })
        )),
        Refusal::NotYourTurn
    );
    // No actor: the GM's free editor flip, nothing logged.
    assert_eq!(
        code(run(&m, json!({ "kind": "door", "by": "p1", "door": "d" }))),
        Refusal::NotGm
    );
    let writes = run(&m, json!({ "kind": "door", "by": "gm", "door": "d" })).unwrap();
    assert!(matches!(&writes[..], [Write::SetToken { token }] if token.open == Some(true)));
}

#[test]
fn a_closed_door_seals_the_route_an_open_one_carries_it() {
    let m = model();
    let to = json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 3, "y": 1 } });
    // Closed door: the long way round is 4 steps (via row 0 or 2), still within 4 moves — so wall the rows off.
    let mut sealed = m.clone();
    for y in [0, 2] {
        sealed.layers[0].tiles[(3 + y * LAYER_SIZE) as usize] = 0;
    }
    assert_eq!(code(run(&sealed, to.clone())), Refusal::Unreachable);
    let open = step(sealed, json!({ "kind": "door", "by": "gm", "door": "d" }));
    let walked = step(open, to);
    assert_eq!(token(&walked, "h").position, Some(px_of(pt(3, 1))));
}

#[test]
fn walking_spends_moves_logs_the_route_and_lands_with_a_heading() {
    let m = model();
    let after = step(
        m.clone(),
        json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 1, "y": 2 } }),
    );
    assert_eq!(pool(&after, "h", "moves"), 3.0);
    let h = token(&after, "h");
    assert_eq!(h.position, Some(px_of(pt(1, 2))));
    assert_eq!(h.angle, Some(180.0), "south");
    assert_eq!(
        after.ledger,
        vec![GameAction::Move {
            actor_yid: "h".into(),
            path: vec![144.0, 144.0, 144.0, 240.0]
        }]
    );
    assert_eq!(
        code(run(
            &m,
            json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 5, "y": 2 } })
        )),
        Refusal::TooFar
    );
    assert_eq!(
        code(run(
            &m,
            json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 9, "y": 9 } })
        )),
        Refusal::Unreachable
    );
}

#[test]
fn a_trap_fires_on_the_mover_and_logs_after_the_move() {
    let mut m = model();
    m.layers[0].tokens.push(de(
        json!({ "yid": "trap", "tokenId": "spikes", "position": at(1, 2) }),
    ));
    let after = step(
        m,
        json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 1, "y": 2 } }),
    );
    assert_eq!(pool(&after, "h", "health"), 2.0);
    assert!(
        matches!(&after.ledger[..], [GameAction::Move { .. }, GameAction::Attack { actor_yid, damage, tile: Some(t), .. }]
        if actor_yid == "trap" && *damage == 1.0 && *t == pt(1, 2))
    );
}

#[test]
fn casting_a_weapon_spell_hits_turns_the_caster_and_logs_an_attack() {
    let m = model();
    let cast = json!({ "kind": "cast", "by": "p1", "actor": "h", "carrier": "Sword", "item": "sw", "spell": "Slash", "target": "ogre" });
    let after = step(m.clone(), cast.clone());
    assert_eq!(pool(&after, "ogre", "health"), 1.0);
    assert_eq!(pool(&after, "h", "actions"), 0.0);
    assert_eq!(token(&after, "h").angle, Some(90.0), "east");
    assert!(
        matches!(&after.ledger[..], [GameAction::Attack { target_yid, damage, .. }] if target_yid == "ogre" && *damage == 2.0)
    );
    assert_eq!(
        code(run(&after, cast.clone())),
        Refusal::OutOfActions,
        "out of actions"
    );
    let far = apply(
        m.clone(),
        &[Write::SetToken {
            token: Token {
                position: Some(px_of(pt(5, 1))),
                ..token(&m, "ogre").clone()
            },
        }],
    );
    assert_eq!(code(run(&far, cast)), Refusal::TooFar);
    let unknown = json!({ "kind": "cast", "by": "p1", "actor": "h", "carrier": "Sword", "spell": "Fireball", "target": "ogre" });
    assert_eq!(code(run(&m, unknown)), Refusal::NotFound);
}

#[test]
fn a_spawn_lands_owned_like_its_caster_and_never_onto_a_solid() {
    let mut m = model();
    let summon = json!({ "name": "Summon", "trigger": "cast", "target": "tile", "range": 3, "chance": 1, "while": "equipped", "action": false, "fold": true,
                         "costs": [], "effects": [{ "verb": "spawn", "def": "imp", "chance": 1 }] });
    m.defs
        .items
        .get_mut("sword")
        .unwrap()
        .spells
        .push(de(summon));
    let cast = json!({ "kind": "cast", "by": "p1", "actor": "h", "carrier": "Sword", "item": "sw", "spell": "Summon", "tile": { "x": 1, "y": 0 } });
    let writes = run(&m, cast.clone()).unwrap();
    let imp = writes
        .iter()
        .find_map(|w| {
            if let Write::AddToken { token, .. } = w {
                Some(token)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        (
            imp.yid.as_str(),
            imp.owner_player_yid.as_deref(),
            imp.name.as_deref()
        ),
        ("1", Some("p1"), Some("Imp"))
    );
    assert_eq!(apply(m.clone(), &writes).layers[0].tokens.len(), 4);
    m.layers[0].tokens.push(de(
        json!({ "yid": "boulder", "tokenId": "rock", "position": at(1, 0) }),
    ));
    assert_eq!(code(run(&m, cast)), Refusal::Blocked);
}

#[test]
fn passing_the_turn_bumps_clears_expires_and_upkeeps_the_players_side() {
    let mut m = model();
    m.ledger.push(GameAction::Say {
        actor_yid: "h".into(),
        text: "hi".into(),
        tile: pt(1, 1),
        volume: 6.0,
    });
    m.layers[0].tokens.push(de(
        json!({ "yid": "glow", "position": at(0, 0), "expiresTurn": 2 }),
    ));
    m.items.insert(
        "gift".into(),
        de(json!({ "yid": "gift", "itemDefinitionId": "sword", "expiresTurn": 2 })),
    );
    m.inventories.get_mut("inv").unwrap().stuff[1] = "gift".into();
    let walked = step(
        m,
        json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 0, "y": 1 } }),
    );
    assert_eq!(
        code(run(&walked, json!({ "kind": "passTurn", "by": "nobody" }))),
        Refusal::NotYourTurn
    );
    let gm_turn = step(walked, json!({ "kind": "passTurn", "by": "p1" }));
    assert_eq!(gm_turn.turn, 2);
    assert!(gm_turn.ledger.is_empty());
    assert!(gm_turn.token("glow").is_none() && !gm_turn.items.contains_key("gift"));
    assert_eq!(
        gm_turn.inventories["inv"].stuff,
        vec!["sw".to_string(), String::new()]
    );
    assert_eq!(
        pool(&gm_turn, "h", "moves"),
        3.0,
        "no upkeep on the GM's turn"
    );
    assert_eq!(
        code(run(&gm_turn, json!({ "kind": "passTurn", "by": "p1" }))),
        Refusal::NotYourTurn
    );
    let players = step(gm_turn, json!({ "kind": "passTurn", "by": "gm" }));
    assert_eq!(pool(&players, "h", "moves"), 4.0);
}

#[test]
fn rotate_and_remove_are_the_gms_and_walls_never_turn() {
    let mut m = model();
    m.layers[0].tokens.push(de(
        json!({ "yid": "w", "tokenId": "wall", "position": at(0, 0) }),
    ));
    assert_eq!(
        code(run(
            &m,
            json!({ "kind": "rotate", "by": "p1", "yid": "ogre" })
        )),
        Refusal::NotGm
    );
    assert_eq!(
        code(run(&m, json!({ "kind": "rotate", "by": "gm", "yid": "w" }))),
        Refusal::Wall
    );
    assert_eq!(
        token(
            &step(
                m.clone(),
                json!({ "kind": "rotate", "by": "gm", "yid": "ogre" })
            ),
            "ogre"
        )
        .angle,
        Some(90.0)
    );
    let gone = step(m, json!({ "kind": "remove", "by": "gm", "yid": "h" }));
    assert!(gone.token("h").is_none() && gone.items.is_empty() && gone.inventories.is_empty());
}

#[test]
fn say_is_trimmed_capped_and_frozen_at_the_speakers_tile() {
    let m = model();
    let long = "x".repeat(400);
    let after = step(
        m.clone(),
        json!({ "kind": "say", "by": "p1", "actor": "h", "text": format!("  {long} "), "volume": 6 }),
    );
    assert!(
        matches!(&after.ledger[..], [GameAction::Say { text, tile, .. }] if text.len() == SAY_MAX_LEN && *tile == pt(1, 1))
    );
    assert_eq!(
        code(run(
            &m,
            json!({ "kind": "say", "by": "p1", "actor": "h", "text": "  ", "volume": 6 })
        )),
        Refusal::Empty
    );
}

#[test]
fn writes_replay_to_the_state_update_chained_on() {
    // A trap that kills: later writes read the earlier ones (ledger order, health), and apply() lands the same.
    let mut m = model();
    m.layers[0].tokens.push(de(
        json!({ "yid": "t1", "tokenId": "spikes", "position": at(1, 2) }),
    ));
    let writes = run(
        &m,
        json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 1, "y": 2 } }),
    )
    .unwrap();
    let json = serde_json::to_value(&writes).unwrap();
    let back: Vec<Write> = de(json);
    assert_eq!(apply(m.clone(), &back), apply(m, &writes));
}

#[test]
fn class_matrix_picks_dominant_then_pair_then_fallback() {
    let classes: ClassesDef = de(json!({ "gap": 2, "fallback": "Adventurer", "cells": [
        { "name": "Barbarian", "stats": ["physique"], "spells": [], "pools": [] },
        { "name": "Knight", "stats": ["defence", "physique"], "spells": [], "pools": [] }
    ] }));
    let s = |p, d, i, sp| Stats {
        physique: p,
        defence: d,
        intelligence: i,
        speed: sp,
    };
    assert_eq!(class_of(&s(6.0, 3.0, 3.0, 3.0), &classes).name, "Barbarian");
    assert_eq!(class_of(&s(4.0, 5.0, 3.0, 3.0), &classes).name, "Knight");
    assert_eq!(
        class_of(&s(5.0, 4.0, 3.0, 3.0), &classes).name,
        "Knight",
        "reversed pair"
    );
    assert_eq!(
        class_of(&s(4.0, 3.0, 3.0, 3.0), &classes).name,
        "Adventurer",
        "ambiguous pair"
    );
}

#[test]
fn carry_weight_blocks_moves_and_refill_respects_it() {
    let mut m = model();
    m.game.physique_carry_mult = 0.1;
    let ctx = Ctx::new(&m);
    let view = character_of(token(&m, "h"), &ctx);
    let moves = view.pools.iter().find(|p| p.name == "moves").unwrap();
    assert_eq!(
        (moves.capacity, moves.blocked, moves.value),
        (4.0, 4.0, 0.0),
        "weight 1 vs physique 3 × 0.1"
    );
    assert_eq!(carry_penalty(1.0, 3.0, 0.1), 7.0);
}

#[test]
fn damage_rolls_per_point_ward_soaks_first_and_kill_riders_fire() {
    let mut m = model();
    m.game.defence_chance = 50.0;
    m.game
        .pools
        .push(de(json!({ "name": "ward", "capacity": 1, "role": "ward" })));
    let ctx = Ctx::new(&m);
    let me = combatant_of(token(&m, "h"), &ctx);
    let mut ogre = combatant_of(token(&m, "ogre"), &ctx);
    ogre.stats.defence = 1.0;
    let (ci, _) = casts_of(&me.carriers)[0];
    let mut spell = me.carriers[ci].spells[0].clone();
    if let Effect::Damage { amount, .. } = &mut spell.effects[0] {
        *amount = 3.0;
    }
    let mut rolls = Script::of(&[0.9, 0.1, 0.1]);
    let mut probes = NoProbes;
    let mut rctx = ResolveCtx {
        turn: 1,
        roles: &ctx.roles,
        defence_chance: 50.0,
        rng: &mut rolls,
        probes: &mut probes,
    };
    let out = resolve(
        RuleEvent::Cast {
            carrier: ci,
            spell: &spell,
            target: Some(&ogre),
            tile: None,
        },
        &me,
        &mut rctx,
    )
    .unwrap();
    let Outcome::Damage {
        amount,
        blocks,
        warded,
        rolls,
        ..
    } = &out[1]
    else {
        panic!("{out:?}")
    };
    assert_eq!((*amount, *blocks, *warded), (2.0, 1.0, Some(1.0)));
    assert_eq!(rolls[0], "[BLOCK] attack 1 vs defence 1: 90% (block ≥ 50%)");
}

#[test]
fn per_scales_floor_and_missing_reads_full_minus_current() {
    let pools: Pools = [("hp".to_string(), 1.0)].into();
    let full: Pools = [("hp".to_string(), 4.0)].into();
    let stats = Stats {
        physique: 3.0,
        ..Stats::ZERO
    };
    let g = Gauges {
        stats: &stats,
        pools: &pools,
        full: Some(&full),
        tallies: Tallies::default(),
    };
    assert_eq!(
        per_value(
            &Per::Gauge {
                pool: "hp".into(),
                missing: true
            },
            &g
        ),
        3.0
    );
    assert_eq!(
        scaled_amount(0.5, per_value(&Per::Stat(Stat::Physique), &g)),
        1.0
    );
}

#[test]
fn heading_matches_the_client() {
    assert_eq!(heading_deg(pt(0, 0), pt(0, -1)), 0.0);
    assert_eq!(heading_deg(pt(0, 0), pt(1, 0)), 90.0);
    assert_eq!(heading_deg(pt(0, 0), pt(-1, 0)), 270.0);
}

#[test]
fn a_walk_ends_where_a_trap_fells_the_mover_and_logs_only_what_was_walked() {
    let mut m = model();
    let pit = json!({ "id": "pit", "pools": [{ "name": "pit", "capacity": 1, "spells": [
        { "trigger": "enter", "target": "token", "chance": 1, "while": "always", "action": true, "fold": true, "costs": [],
          "effects": [{ "verb": "damage", "amount": 9, "chance": 1 }] }] }] });
    m.defs.tokens.insert("pit".into(), de(pit));
    m.layers[0].tokens.push(de(
        json!({ "yid": "p", "tokenId": "pit", "position": at(1, 1) }),
    ));
    m = apply(
        m.clone(),
        &[Write::SetToken {
            token: Token {
                position: Some(px_of(pt(0, 1))),
                ..token(&m, "h").clone()
            },
        }],
    );
    let after = step(
        m,
        json!({ "kind": "move", "by": "p1", "actor": "h", "to": { "x": 2, "y": 1 } }),
    );
    assert!(is_dead(token(&after, "h"), &Ctx::new(&after)));
    assert_eq!(token(&after, "h").position, Some(px_of(pt(1, 1))));
    assert!(
        matches!(&after.ledger[..], [GameAction::Move { path, .. }, GameAction::Attack { .. }] if *path == vec![48.0, 144.0, 144.0, 144.0])
    );
}
