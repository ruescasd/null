//! The canal network: stations, and straight canals between them that cross
//! nowhere but at a station (the user: canals ending at junctions, places
//! in the world, like stations, rather than endless parallel loops).
//!
//! Stations stand on a jittered grid over the torus, most cells holding one.
//! Two are linked when no third is nearer to both than they are to each
//! other (the relative neighbourhood graph): only true neighbours, every
//! station reached, no two links crossing, and the links leaving a station
//! at least 60 degrees apart (room for the station that joins them).
//! Distances are taken round the wrap: a link runs to the nearest copy of
//! the station at its far end.

use glam::DVec2;

use crate::noise::hash01;

/// Grid cells per side of the world (each holds at most one station), and
/// the chance that a cell holds one: about nine stations, canals of 4-7 km,
/// wide dark land between them. (Four a side, three in four filled, the
/// stations were never more than some 2.5 km away: lower density, the
/// user. Four a side, half filled, as few as four.)
const CELLS: i32 = 3;
const CHANCE: f32 = 0.95;
/// How far a station may sit from its cell's centre, as a share of the cell
/// (each way): neighbours stay at least `1 - 2 * JITTER` cells apart.
const JITTER: f64 = 0.3;

/// A canal: the stations at its ends, and where the second lies from the
/// first (its nearest copy round the wrap).
#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub a: usize,
    pub b: usize,
    pub offset: DVec2,
}

impl Link {
    pub fn length(&self) -> f64 {
        self.offset.length()
    }
}

pub struct Network {
    pub size: f64,
    pub stations: Vec<DVec2>,
    pub links: Vec<Link>,
}

impl Network {
    pub fn new(size: f64, seed: u32) -> Self {
        Self::with(size, seed, CELLS, CHANCE)
    }

    /// With `cells` grid cells per side, each holding a station by `chance`.
    pub fn with(size: f64, seed: u32, cells: i32, chance: f32) -> Self {
        let seed = seed ^ 0x57a7_1035;
        let cell = size / cells as f64;
        let stations: Vec<DVec2> = (0..cells)
            .flat_map(|z| (0..cells).map(move |x| (x, z)))
            .filter(|&(x, z)| hash01(x, 0, z, seed) < chance)
            .map(|(x, z)| {
                let j = |k| (hash01(x, k, z, seed) as f64 * 2.0 - 1.0) * JITTER;
                DVec2::new((x as f64 + 0.5 + j(1)) * cell, (z as f64 + 0.5 + j(2)) * cell)
            })
            .collect();
        let links = Self::neighbours(size, &stations);
        Self { size, stations, links }
    }

    /// Where `q` lies from `p`, round the wrap the short way.
    fn toward(size: f64, p: DVec2, q: DVec2) -> DVec2 {
        let d = q - p;
        d - (d / size).round() * size
    }

    /// The relative neighbourhood graph on the torus: `a` and `b` linked
    /// unless some third station (any copy of it) is nearer to both.
    fn neighbours(size: f64, stations: &[DVec2]) -> Vec<Link> {
        let copies = |p: DVec2| (-1..=1).flat_map(move |i| (-1..=1).map(move |j| p + DVec2::new(i as f64, j as f64) * size));
        let mut links = Vec::new();
        for a in 0..stations.len() {
            for b in a + 1..stations.len() {
                let offset = Self::toward(size, stations[a], stations[b]);
                let (pa, pb) = (stations[a], stations[a] + offset);
                let d = offset.length();
                let blocked = stations
                    .iter()
                    .enumerate()
                    .flat_map(|(k, &s)| copies(s).map(move |c| (k, c)))
                    .any(|(_, c)| c.distance(pa) < d - 1e-6 && c.distance(pb) < d - 1e-6);
                if !blocked {
                    links.push(Link { a, b, offset });
                }
            }
        }
        links
    }

    /// How many canals meet at each station.
    pub fn degrees(&self) -> Vec<usize> {
        let mut out = vec![0; self.stations.len()];
        for l in &self.links {
            out[l.a] += 1;
            out[l.b] += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: f64 = 16384.0;

    /// Whether segments p0-p1 and q0-q1 cross, other than at an end.
    fn cross(p0: DVec2, p1: DVec2, q0: DVec2, q1: DVec2) -> bool {
        let side = |a: DVec2, b: DVec2, c: DVec2| (b - a).perp_dot(c - a);
        let (d1, d2) = (side(q0, q1, p0), side(q0, q1, p1));
        let (d3, d4) = (side(p0, p1, q0), side(p0, p1, q1));
        let shared = [p0, p1].iter().any(|p| [q0, q1].iter().any(|q| p.distance(*q) < 1e-3));
        !shared && d1 * d2 < 0.0 && d3 * d4 < 0.0
    }

    /// No two canals cross, round the wrap too, for many worlds.
    #[test]
    fn links_never_cross() {
        for seed in 0..40 {
            let net = Network::new(SIZE, seed);
            for (i, l) in net.links.iter().enumerate() {
                let (p0, p1) = (net.stations[l.a], net.stations[l.a] + l.offset);
                for m in &net.links[i + 1..] {
                    for (x, z) in (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x, z))) {
                        let shift = DVec2::new(x as f64, z as f64) * SIZE;
                        let (q0, q1) = (net.stations[m.a] + shift, net.stations[m.a] + m.offset + shift);
                        assert!(!cross(p0, p1, q0, q1), "seed {seed}: links {:?} and {:?} cross", (l.a, l.b), (m.a, m.b));
                    }
                }
            }
        }
    }

    /// Every station can be reached from every other.
    #[test]
    fn every_station_reached() {
        for seed in 0..40 {
            let net = Network::new(SIZE, seed);
            let mut reached = vec![false; net.stations.len()];
            let mut todo = vec![0];
            while let Some(s) = todo.pop() {
                if std::mem::replace(&mut reached[s], true) {
                    continue;
                }
                todo.extend(net.links.iter().filter_map(|l| if l.a == s { Some(l.b) } else if l.b == s { Some(l.a) } else { None }));
            }
            assert!(reached.iter().all(|&r| r), "seed {seed}");
        }
    }
}
