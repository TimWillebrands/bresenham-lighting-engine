use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge(pub Point, pub Point);

#[derive(Debug)]
pub struct Room {
    pub points: Vec<Point>,
    pub edge_loops: Vec<Vec<Edge>>,
}

pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    map: Vec<i32>,
    layer_size: usize,
}

impl UnionFind {
    pub fn new(map: Vec<i32>, layer_size: usize) -> Self {
        let size = map.len();
        let mut parent = vec![0; size];
        for i in 0..size {
            parent[i] = i;
        }
        let rank = vec![0; size];

        let mut uf = UnionFind {
            parent,
            rank,
            map,
            layer_size,
        };

        uf.initialize();
        uf
    }

    fn initialize(&mut self) {
        for col in 0..self.layer_size {
            for row in 0..self.layer_size {
                let current = self.index(col as i32, row as i32);
                if row + 1 < self.layer_size {
                    let next = self.index(col as i32, (row + 1) as i32);
                    if self.map[current] == self.map[next] {
                        self.union(current, next);
                    }
                }
                if col + 1 < self.layer_size {
                    let next = self.index((col + 1) as i32, row as i32);
                    if self.map[current] == self.map[next] {
                        self.union(current, next);
                    }
                }
            }
        }
    }

    pub fn find(&mut self, i: usize) -> usize {
        if self.parent[i] == i {
            i
        } else {
            self.parent[i] = self.find(self.parent[i]);
            self.parent[i]
        }
    }

    pub fn union(&mut self, i: usize, j: usize) {
        let root_i = self.find(i);
        let root_j = self.find(j);

        if root_i != root_j {
            if self.rank[root_i] < self.rank[root_j] {
                self.parent[root_i] = root_j;
            } else if self.rank[root_i] > self.rank[root_j] {
                self.parent[root_j] = root_i;
            } else {
                self.parent[root_j] = root_i;
                self.rank[root_i] += 1;
            }
        }
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.layer_size as i32 + x) as usize
    }

    pub fn change_tile_type(&mut self, idx: usize, new_type: i32) -> (usize, usize) {
        let old_root = self.find(idx);
        self.map[idx] = new_type;

        self.parent[idx] = idx;
        self.rank[idx] = 0;

        let x = idx % self.layer_size;
        let y = idx / self.layer_size;

        let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];

        for (dx, dy) in directions.iter() {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;

            if nx >= 0 && nx < self.layer_size as i32 && ny >= 0 && ny < self.layer_size as i32 {
                let neighbor_idx = self.index(nx, ny);
                if self.map[neighbor_idx] == new_type {
                    self.union(idx, neighbor_idx);
                }
            }
        }

        (old_root, self.find(idx))
    }

    pub fn rooms(&mut self) -> HashMap<usize, Room> {
        let mut room_map: HashMap<usize, Vec<Point>> = HashMap::new();
        let mut edges: HashMap<usize, Vec<Edge>> = HashMap::new();

        for i in 0..self.map.len() {
            if self.map[i] <= 0 {
                continue;
            }
            let root = self.find(i);
            let x = (i % self.layer_size) as i32;
            let y = (i / self.layer_size) as i32;

            room_map.entry(root).or_insert_with(Vec::new).push(Point { x, y });
            edges.entry(root).or_insert_with(Vec::new);

            let direction_edges = [
                (Point { x: x, y: y }, Point { x: x + 1, y: y }),       // Top
                (Point { x: x + 1, y: y }, Point { x: x + 1, y: y + 1 }), // Right
                (Point { x: x + 1, y: y + 1 }, Point { x: x, y: y + 1 }), // Bottom
                (Point { x: x, y: y + 1 }, Point { x: x, y }),       // Left
            ];

            let neighbor_offsets = [
                (0, -1), // Top
                (1, 0),  // Right
                (0, 1),  // Bottom
                (-1, 0), // Left
            ];

            for (di, (dx, dy)) in neighbor_offsets.iter().enumerate() {
                let nx = x + dx;
                let ny = y + dy;

                let edge = Edge(direction_edges[di].0.clone(), direction_edges[di].1.clone());

                if nx < 0 || nx >= self.layer_size as i32 || ny < 0 || ny >= self.layer_size as i32 {
                    edges.get_mut(&root).unwrap().push(edge);
                } else {
                    let neighbor_idx = self.index(nx, ny);
                    let neighbor_root = self.find(neighbor_idx);
                    if neighbor_root != root {
                        edges.get_mut(&root).unwrap().push(edge);
                    }
                }
            }
        }

        let mut result: HashMap<usize, Room> = HashMap::new();

        for (root, points) in room_map.into_iter() {
            let edge_list = edges.remove(&root).unwrap_or_default();

            let mut stitched_edges: Vec<Edge> = Vec::new();

            let mut horizontals: HashMap<i32, Vec<Edge>> = HashMap::new();
            for edge in edge_list.iter().filter(|e| e.0.y == e.1.y) {
                horizontals.entry(edge.0.y).or_insert_with(Vec::new).push(edge.clone());
            }

            for (_, mut edges) in horizontals.into_iter() {
                edges.sort_by(|a, b| a.0.x.cmp(&b.0.x));
                while !edges.is_empty() {
                    let mut head = edges.remove(0);
                    let mut tail = head.clone();
                    loop {
                        let mut continue_head = None;
                        let mut continue_tail = None;

                        if let Some(idx) = edges.iter().position(|e| e.0.x == head.1.x) {
                            continue_head = Some(edges.remove(idx));
                        } else if let Some(idx) = edges.iter().position(|e| e.1.x == tail.0.x) {
                            continue_tail = Some(edges.remove(idx));
                        }

                        if let Some(h) = continue_head {
                            head = h;
                        } else if let Some(t) = continue_tail {
                            tail = t;
                        } else {
                            break;
                        }
                    }
                    stitched_edges.push(Edge(tail.0, head.1));
                }
            }

            let mut verticals: HashMap<i32, Vec<Edge>> = HashMap::new();
            for edge in edge_list.iter().filter(|e| e.0.x == e.1.x) {
                verticals.entry(edge.0.x).or_insert_with(Vec::new).push(edge.clone());
            }

            for (_, mut edges) in verticals.into_iter() {
                edges.sort_by(|a, b| a.0.y.cmp(&b.0.y));
                while !edges.is_empty() {
                    let mut head = edges.remove(0);
                    let mut tail = head.clone();
                    loop {
                        let mut continue_head = None;
                        let mut continue_tail = None;

                        if let Some(idx) = edges.iter().position(|e| e.0.y == head.1.y) {
                            continue_head = Some(edges.remove(idx));
                        } else if let Some(idx) = edges.iter().position(|e| e.1.y == tail.0.y) {
                            continue_tail = Some(edges.remove(idx));
                        }

                        if let Some(h) = continue_head {
                            head = h;
                        } else if let Some(t) = continue_tail {
                            tail = t;
                        } else {
                            break;
                        }
                    }
                    stitched_edges.push(Edge(tail.0, head.1));
                }
            }

            let mut edge_loops: Vec<Vec<Edge>> = Vec::new();
            let mut edge_map: HashMap<String, Edge> = HashMap::new();

            for edge in stitched_edges.into_iter() {
                edge_map.insert(format!("{},{}", edge.0.x, edge.0.y), edge);
            }

            while !edge_map.is_empty() {
                let start_key = edge_map.keys().next().unwrap().clone();
                if let Some(start_edge) = edge_map.remove(&start_key) {
                    let mut loop_edges: Vec<Edge> = Vec::new();
                    let mut current_edge = start_edge;
                    loop {
                        loop_edges.push(current_edge.clone());
                        let next_key = format!("{},{}", current_edge.1.x, current_edge.1.y);
                        if let Some(next_edge) = edge_map.remove(&next_key) {
                            current_edge = next_edge;
                            if current_edge.1 == loop_edges[0].0 {
                                loop_edges.push(current_edge.clone());
                                edge_map.remove(&format!("{},{}", current_edge.0.x, current_edge.0.y));
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    edge_loops.push(loop_edges);
                }
            }

            result.insert(root, Room { points, edge_loops });
        }

        result
    }

    pub fn cast_ray(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let nx = dx.abs();
        let ny = dy.abs();
        let sx = if dx > 0 { 1 } else { -1 };
        let sy = if dy > 0 { 1 } else { -1 };

        let mut p = Point { x: x1, y: y1 };
        let mut ix = 0;
        let mut iy = 0;

        while ix < nx || iy < ny {
            let current = self.index(p.x, p.y);
            let room = self.find(current);

            if (ix as f32 + 0.5) / (nx as f32)  < (iy as f32 + 0.5) / (ny as f32) {
                p.x += sx;
                ix += 1;
            } else {
                p.y += sy;
                iy += 1;
            }

            let next = self.index(p.x, p.y);
            if self.find(next) != room {
                return false;
            }
        }

        true
    }

    pub fn path(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<usize> {
        let mut points: Vec<usize> = Vec::new();
        let mut frontier: Vec<usize> = Vec::new();
        let mut came_from: HashMap<usize, Option<usize>> = HashMap::new();

        let start = self.index(x1, y1);
        let goal = self.index(x2, y2);

        if self.map[start] == 0 || self.map[goal] == 0 {
            return points;
        }

        frontier.push(start);
        came_from.insert(start, None);

        while !frontier.is_empty() {
            let current = frontier.remove(0);

            if current == goal {
                break;
            }

            let x = (current % self.layer_size) as i32;
            let y = (current / self.layer_size) as i32;

            let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];

            for (dx, dy) in directions.iter() {
                let nx = x + dx;
                let ny = y + dy;

                if nx >= 0 && nx < self.layer_size as i32 && ny >= 0 && ny < self.layer_size as i32 {
                    let next = self.index(nx, ny);
                    if !came_from.contains_key(&next) && self.map[next] > 0 {
                        frontier.push(next);
                        came_from.insert(next, Some(current));
                    }
                }
            }
        }

        let mut current = goal;
        while current != start {
            points.push(current);
            if let Some(Some(next)) = came_from.get(&current) {
                current = *next;
            } else {
                return Vec::new();
            }
        }
        points.push(start);
        points.reverse();

        points
    }
}
