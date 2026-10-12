//! Where creatures can go in a built place (a region of the plan): its
//! floors sampled half a metre apart (see `Grid::walkable`, and the decks
//! out on the chasm), each spot joined to those beside it no more than a
//! step higher or lower (on a diagonal only if both spots between are
//! joined too, so no corner is cut through), and the way between two
//! points found over them (A*).

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

use bevy::prelude::*;

/// How far apart the spots are, and how high a step between two may be.
pub(crate) const SPACING: f32 = 0.5;
const STEP: f32 = 0.6;

#[derive(Resource)]
pub(crate) struct Nav {
    pub nodes: Vec<Vec3>,
    columns: HashMap<(i32, i32), Vec<u32>>,
    links: Vec<Vec<u32>>,
}

fn column(p: Vec3) -> (i32, i32) {
    ((p.x / SPACING).floor() as i32, (p.z / SPACING).floor() as i32)
}

impl Nav {
    pub fn new(points: &[Vec3]) -> Nav {
        let mut columns: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            columns.entry(column(*p)).or_default().push(i as u32);
        }
        // The spot in a column nearest in height to `y`, within a step.
        let at = |c: (i32, i32), y: f32| -> Option<u32> {
            columns.get(&c)?.iter().copied().filter(|&j| (points[j as usize].y - y).abs() <= STEP).min_by(|&a, &b| (points[a as usize].y - y).abs().total_cmp(&(points[b as usize].y - y).abs()))
        };
        let links = points
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
        Nav { nodes: points.to_vec(), columns, links }
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
    /// there is none (or either end is off the floors).
    pub fn path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let (a, b) = (self.nearest(from, 3)?, self.nearest(to, 3)?);
        let goal = self.nodes[b as usize];
        let h = |j: u32| (self.nodes[j as usize] - goal).length();
        let mut came: HashMap<u32, u32> = HashMap::new();
        let mut cost: HashMap<u32, f32> = HashMap::from([(a, 0.0)]);
        let mut open = BinaryHeap::from([(Reverse((h(a) * 1000.0) as u64), a)]);
        while let Some((_, j)) = open.pop() {
            if j == b {
                let mut way = vec![self.nodes[j as usize]];
                let mut k = j;
                while let Some(&p) = came.get(&k) {
                    way.push(self.nodes[p as usize]);
                    k = p;
                }
                way.reverse();
                return Some(way);
            }
            let here = cost[&j];
            for &n in &self.links[j as usize] {
                let c = here + (self.nodes[n as usize] - self.nodes[j as usize]).length();
                if cost.get(&n).is_none_or(|&old| c < old) {
                    cost.insert(n, c);
                    came.insert(n, j);
                    open.push((Reverse(((c + h(n)) * 1000.0) as u64), n));
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
