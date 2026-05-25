use bresenham_lighting_engine::map_grid::{Edge, Point, UnionFind};
use wasm_bindgen_test::*;

fn pt(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn edge(a: (i32, i32), b: (i32, i32)) -> Edge {
    Edge(pt(a.0, a.1), pt(b.0, b.1))
}

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

    // Room with singleton tile (0,0) = type 8
    let room_root = mapgrid.find(0);
    let room = rooms.get(&room_root).unwrap();
    assert_eq!(room.edge_loops.len(), 1);
    let edges = &room.edge_loops[0];
    assert_eq!(edges.len(), 4);
    assert!(edges.contains(&edge((0, 0), (1, 0))));
    assert!(edges.contains(&edge((1, 0), (1, 1))));
    assert!(edges.contains(&edge((1, 1), (0, 1))));
    assert!(edges.contains(&edge((0, 1), (0, 0))));

    // Western singleton room (1,0) = type 1
    let west_root = mapgrid.find(1);
    let west = rooms.get(&west_root).unwrap();
    let west_edges = &west.edge_loops[0];
    assert_eq!(west_edges.len(), 4);
    assert!(west_edges.contains(&edge((1, 0), (2, 0))));
    assert!(west_edges.contains(&edge((2, 0), (2, 1))));
    assert!(west_edges.contains(&edge((2, 1), (1, 1))));
    assert!(west_edges.contains(&edge((1, 1), (1, 0))));

    // Southern singleton room (0,1) = type 1
    let south_root = mapgrid.find(4);
    let south = rooms.get(&south_root).unwrap();
    let south_edges = &south.edge_loops[0];
    assert!(south_edges.contains(&edge((0, 1), (1, 1))));
    assert!(south_edges.contains(&edge((1, 1), (1, 2))));
    assert!(south_edges.contains(&edge((1, 2), (0, 2))));
    assert!(south_edges.contains(&edge((0, 2), (0, 1))));
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
    assert_eq!(room.points.len(), 3);
    assert!(room.points.contains(&pt(0, 0)));
    assert!(room.points.contains(&pt(1, 0)));
    assert!(room.points.contains(&pt(0, 1)));

    assert_eq!(room.edge_loops.len(), 1);
    let edges = &room.edge_loops[0];
    assert_eq!(edges.len(), 6);
    assert!(edges.contains(&edge((0, 0), (2, 0))));
    assert!(edges.contains(&edge((2, 0), (2, 1))));
    assert!(edges.contains(&edge((2, 1), (1, 1))));
    assert!(edges.contains(&edge((1, 1), (1, 2))));
    assert!(edges.contains(&edge((1, 2), (0, 2))));
    assert!(edges.contains(&edge((0, 2), (0, 0))));

    // The L-shaped room should NOT contain the interior cut edge
    assert!(!edges.contains(&edge((1, 1), (0, 1))));

    // Loop must close: every edge's tail meets the next edge's head
    for i in 0..edges.len() {
        let next = &edges[(i + 1) % edges.len()];
        assert_eq!(edges[i].1, next.0);
    }
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
