use bresenham_lighting_engine::map_grid::{UnionFind};
use wasm_bindgen_test::*;

#[test]
#[wasm_bindgen_test]
fn should_correctly_initialize_with_given_map() {
    let test_map = vec![
        1, 1, 2, 
        2, 1, 2, 
        1, 2, 2
    ];
    let mut mapgrid = UnionFind::new(test_map, 3);
    assert_eq!(mapgrid.find(0), 0);
}

#[test]
#[wasm_bindgen_test]
fn should_correctly_find_and_union_tiles() {
    let test_map = vec![
        1, 1, 2, 
        2, 1, 2, 
        1, 2, 2
    ];
    let mut mapgrid = UnionFind::new(test_map, 3);
    assert_eq!(mapgrid.find(0), 0);

    mapgrid.union(0, 1);
    assert_eq!(mapgrid.find(1), mapgrid.find(0));
}

#[test]
#[wasm_bindgen_test]
fn should_handle_changes_in_tile_type() {
    let test_map = vec![
        1, 1, 2, 
        2, 1, 2, 
        1, 2, 2
    ];
    let mut mapgrid = UnionFind::new(test_map, 3);
    let old_root = mapgrid.find(2);

    mapgrid.change_tile_type(2, 1);
    let new_root = mapgrid.find(2);

    assert_ne!(new_root, old_root);
}

#[test]
#[wasm_bindgen_test]
fn should_correctly_identify_room_edges() {
    let test_map = vec![
        8, 1, 0, 2,
        1, 0, 1, 2,
        0, 0, 0, 2,
        3, 3, 3, 3
    ];
    let mut mapgrid = UnionFind::new(test_map, 4);
    let rooms = mapgrid.rooms();

    let room_root = mapgrid.find(0);
    let room = rooms.get(&room_root).unwrap();
    assert_eq!(room.edge_loops.len(), 1);
    assert_eq!(room.edge_loops[0].len(), 4);
}

#[test]
#[wasm_bindgen_test]
fn should_correctly_identify_room_edges_advanced() {
    let test_map = vec![
        1, 1, 0, 2,
        1, 0, 1, 2,
        0, 0, 0, 2,
        3, 3, 3, 3
    ];
    let mut mapgrid = UnionFind::new(test_map, 4);
    let rooms = mapgrid.rooms();

    let room_root = mapgrid.find(0);
    let room = rooms.get(&room_root).unwrap();
    println!("Room points: {:?}", room.points);
    println!("Room edge_loops: {:?}", room.edge_loops);
    assert_eq!(room.points.len(), 3);
    assert_eq!(room.edge_loops.len(), 1);
    assert_eq!(room.edge_loops[0].len(), 6);
}

#[test]
#[wasm_bindgen_test]
fn all_edgeloops_should_fully_close() {
    let test_map = vec![
        1, 1, 0, 2,
        1, 0, 1, 2,
        0, 0, 0, 2,
        3, 3, 3, 3
    ];
    let mut mapgrid = UnionFind::new(test_map, 4);
    let rooms = mapgrid.rooms();

    for (_, room) in rooms.iter() {
        for edge_loop in room.edge_loops.iter() {
            for i in 0..edge_loop.len() {
                let edge = &edge_loop[i];
                let next = &edge_loop[(i + 1) % edge_loop.len()];
                assert_eq!(edge.1, next.0);
            }
        }
    }
}
