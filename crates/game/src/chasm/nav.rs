//! Where creatures can go in a built place (a region of the plan): its
//! floors sampled half a metre apart (see `Grid::walkable`, and the decks
//! out on the chasm), each spot joined to those beside it no more than a
//! step higher or lower (on a diagonal only if both spots between are
//! joined too, so no corner is cut through), and the way between two
//! points found over them (A*).
//!
//! A creature's way is found in the background (`Route`, `Nav::steer`),
//! never in a frame: a way up a level of the arena was 20 ms to find, and
//! ten creatures finding theirs in the same frame paused the game (the
//! user). It follows the way it has till the new one comes.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    ops::Deref,
    sync::Arc,
};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};

/// How far apart the spots are, and how high a step between two may be.
pub(crate) const SPACING: f32 = 0.5;
const STEP: f32 = 0.6;
/// How often a way is found afresh (s), and how far on along it to head
/// for (m).
const AGAIN: f32 = 0.5;
/// How many spots on along a way are looked down (12 m).
const LOOK: usize = 24;

/// The spots and how they join (see `Nav`, which shares them with the
/// searches in the background).
pub(crate) struct Graph {
    pub nodes: Vec<Vec3>,
    columns: HashMap<(i32, i32), Vec<u32>>,
    links: Vec<Vec<u32>>,
    /// How far each spot is from a wall or an edge: 0 beside one (not
    /// joined all round), 1 next to such a spot, 2 further in.
    clearance: Vec<u8>,
}

/// What a step onto a spot costs (times its length), by its clearance: a
/// way keeps to the middle of a hall or a door, where a body fits, and goes
/// by a wall only where it must. (A way hugged the walls round a corner,
/// where no body fits: a hunter stuck there, pressed into the wall.)
const CROWDED: [f32; 3] = [4.0, 1.6, 1.0];

fn column(p: Vec3) -> (i32, i32) {
    ((p.x / SPACING).floor() as i32, (p.z / SPACING).floor() as i32)
}

/// Where creatures can go in a built place: its `Graph`, shared.
#[derive(Resource, Clone)]
pub(crate) struct Nav(Arc<Graph>);

impl Deref for Nav {
    type Target = Graph;

    fn deref(&self) -> &Graph {
        &self.0
    }
}

/// A creature's way through a built place (see `Nav::steer`): the way it
/// follows, how long till it is found afresh, the search for it under way,
/// and whether the last found none.
#[derive(Default)]
pub(crate) struct Route {
    way: Vec<Vec3>,
    age: f32,
    asked: Option<Task<Option<Vec<Vec3>>>>,
    none: bool,
}

impl Route {
    /// Where the way it follows ends (none yet, none).
    pub fn end(&self) -> Option<Vec3> {
        self.way.last().copied()
    }

    /// Found afresh at once (a new goal).
    pub fn renew(&mut self) {
        self.age = 0.0;
    }

    /// Whether it has no way (none found, or none yet).
    pub fn is_empty(&self) -> bool {
        self.way.is_empty()
    }
}

/// Where to head for (see `Nav::steer`).
pub(crate) enum Steer {
    /// The furthest spot on along the way in plain sight, and how far the
    /// way has left to go.
    Ahead(Vec3, f32),
    /// No way yet: the first is being found.
    Waiting,
    /// No way there.
    NoWay,
}

impl Nav {
    pub fn new(points: &[Vec3]) -> Nav {
        Nav(Arc::new(Graph::new(points)))
    }

    /// Where to head for on the way from `from` to `to`: the furthest spot
    /// on along it in plain sight (`clear`), and how far is left to go
    /// along it. The way is kept in `route`, found afresh every `AGAIN`
    /// seconds in the background, the spots passed dropped; while a new
    /// one is found, the old is followed.
    pub fn steer(&self, from: Vec3, to: Vec3, route: &mut Route, dt: f32, clear: &dyn Fn(Vec3, Vec3) -> bool) -> Steer {
        let flat = |a: Vec3, b: Vec3| Vec2::new(a.x - b.x, a.z - b.z).length();
        // A search done: its way taken up.
        if let Some(task) = route.asked.as_mut()
            && task.is_finished()
        {
            let found = block_on(poll_once(task)).flatten();
            route.asked = None;
            route.none = found.is_none();
            route.way = found.unwrap_or_default();
        }
        // Another asked for, when it is time (or there is none yet, and
        // none asked for). (Where there is no way, not every frame: that
        // searched everything there was, each frame.)
        route.age -= dt;
        if route.asked.is_none() && (route.age <= 0.0 || (route.way.is_empty() && !route.none)) {
            route.age = AGAIN;
            let graph = self.0.clone();
            route.asked = Some(AsyncComputeTaskPool::get().spawn(async move { graph.path(from, to) }));
        }
        if route.way.is_empty() {
            return if route.none { Steer::NoWay } else { Steer::Waiting };
        }
        let way = &mut route.way;
        while way.len() > 1 && flat(way[0], from) < 1.2 {
            way.remove(0);
        }
        // (The furthest spot on, up to `LOOK` along it, in plain sight: so
        // a corner is gone round, not cut through; failing that, the next.)
        let up = Vec3::Y * 0.5;
        let ahead = (0..way.len().min(LOOK))
            .rev()
            .step_by(3)
            .map(|i| way[i])
            .find(|&q| flat(q, from) >= 1.0 && clear(from + up, q + up))
            .unwrap_or(way[0]);
        let left = flat(from, way[0]) + way.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f32>();
        Steer::Ahead(ahead, left)
    }
}

impl Graph {
    fn new(points: &[Vec3]) -> Graph {
        let mut columns: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            columns.entry(column(*p)).or_default().push(i as u32);
        }
        // The spot in a column nearest in height to `y`, within a step.
        let at = |c: (i32, i32), y: f32| -> Option<u32> {
            columns.get(&c)?.iter().copied().filter(|&j| (points[j as usize].y - y).abs() <= STEP).min_by(|&a, &b| (points[a as usize].y - y).abs().total_cmp(&(points[b as usize].y - y).abs()))
        };
        let links: Vec<Vec<u32>> = points
            .iter()
            .map(|p| {
                let c = column(*p);
                let mut out = Vec::new();
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if let Some(j) = at((c.0 + dx, c.1 + dz), p.y) {
                        out.push(j);
                    }
                }
                for (dx, dz) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                    if at((c.0 + dx, c.1), p.y).is_some() && at((c.0, c.1 + dz), p.y).is_some()
                        && let Some(j) = at((c.0 + dx, c.1 + dz), p.y)
                    {
                        out.push(j);
                    }
                }
                out
            })
            .collect();
        let open: Vec<bool> = links.iter().map(|l| l.len() == 8).collect();
        let clearance = links.iter().enumerate().map(|(i, l)| if !open[i] { 0 } else if l.iter().any(|&j| !open[j as usize]) { 1 } else { 2 }).collect();
        Graph { nodes: points.to_vec(), columns, links, clearance }
    }

    /// The spot nearest `p`: in its column or those round it (up to `reach`
    /// spots away), not far above it, nor more than a storey below.
    pub fn nearest(&self, p: Vec3, reach: i32) -> Option<u32> {
        let c = column(p);
        let mut best: Option<(f32, u32)> = None;
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                for &j in self.columns.get(&(c.0 + dx, c.1 + dz)).into_iter().flatten() {
                    let q = self.nodes[j as usize];
                    let dy = q.y - p.y;
                    if !(-6.0..=1.0).contains(&dy) {
                        continue;
                    }
                    let d = (q - p).length_squared() + dy.abs() * 4.0;
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, j));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The way from `from` to `to` over the spots (A*), as points; `None` if
    /// there is none (or either end is off the floors). (Its costs kept in
    /// arrays, not maps, and a spot reached again more cheaply since it was
    /// queued passed over when its old entry comes up: a way up a level of
    /// the arena, 235 000 spots, took 20 ms.)
    pub fn path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let (a, b) = (self.nearest(from, 3)?, self.nearest(to, 3)?);
        let goal = self.nodes[b as usize];
        let h = |j: u32| (self.nodes[j as usize] - goal).length();
        let key = |f: f32| (f * 1000.0) as u64;
        let mut cost = vec![f32::INFINITY; self.nodes.len()];
        let mut came = vec![u32::MAX; self.nodes.len()];
        cost[a as usize] = 0.0;
        let mut open = BinaryHeap::from([(Reverse(key(h(a))), a)]);
        while let Some((Reverse(f), j)) = open.pop() {
            let here = cost[j as usize];
            if f > key(here + h(j)) {
                continue;
            }
            if j == b {
                let mut way = vec![self.nodes[j as usize]];
                let mut k = j;
                while came[k as usize] != u32::MAX {
                    k = came[k as usize];
                    way.push(self.nodes[k as usize]);
                }
                way.reverse();
                return Some(way);
            }
            for &n in &self.links[j as usize] {
                let c = here + (self.nodes[n as usize] - self.nodes[j as usize]).length() * CROWDED[self.clearance[n as usize] as usize];
                if c < cost[n as usize] {
                    cost[n as usize] = c;
                    came[n as usize] = j;
                    open.push((Reverse(key(c + h(n))), n));
                }
            }
        }
        None
    }

    /// Of the spots reachable from `from`, the nearest to `to` (where the
    /// way gives out, when it does).
    pub fn closest_reach(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        let a = self.nearest(from, 3)?;
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![a];
        seen[a as usize] = true;
        let mut best = (f32::MAX, self.nodes[a as usize]);
        while let Some(j) = stack.pop() {
            let p = self.nodes[j as usize];
            let d = (p - to).length();
            if d < best.0 {
                best = (d, p);
            }
            for &n in &self.links[j as usize] {
                if !seen[n as usize] {
                    seen[n as usize] = true;
                    stack.push(n);
                }
            }
        }
        Some(best.1)
    }

    /// How many separate pieces the spots fall into, and the largest's size.
    pub fn pieces(&self) -> (usize, usize) {
        let mut seen = vec![false; self.nodes.len()];
        let (mut count, mut largest) = (0, 0);
        for start in 0..self.nodes.len() {
            if seen[start] {
                continue;
            }
            count += 1;
            let (mut stack, mut size) = (vec![start as u32], 0);
            seen[start] = true;
            while let Some(j) = stack.pop() {
                size += 1;
                for &n in &self.links[j as usize] {
                    if !seen[n as usize] {
                        seen[n as usize] = true;
                        stack.push(n);
                    }
                }
            }
            largest = largest.max(size);
        }
        (count, largest)
    }
}
