//! Canals: an alien transport system of huge half-pipes, each running dead
//! straight from one station to another (see `network.rs`: they cross
//! nowhere but at a station). They were endless parallel loops round the
//! torus; ending at junctions, places in the world like stations, the user.
//!
//! The canal floor follows a gentle height field (given by the caller)
//! evaluated at the centreline, so canals cut trenches through high ground
//! and ride embankments over low ground, easing at each end to its
//! station's. The surface is slick (see the player): no friction, so the
//! pipe rides like a skate or snowboard half-pipe.
//!
//! A station, for now, is a placeholder: a flat round court at the level of
//! the pipes' floor, so a pipe opens out onto it.

use glam::{DVec2, Vec3};

use crate::mesh::ColumnMesh;
use crate::network::Network;
use crate::noise::hash01;

/// Radius of the half-pipe, in metres.
pub const PIPE_RADIUS: f64 = 7.0;
/// Half the width of the corridor a canal occupies: the pipe's own (a flat
/// dark lip along each side, 3 m wide, read as slabs laid beside it: gone,
/// the user; the pipe's own outer facets dark instead, see `DARK_FACETS`).
pub const HALF_WIDTH: f64 = PIPE_RADIUS;
/// How far the canal's outer walls reach below its rim.
const OUTER_DEPTH: f64 = 70.0;
/// Facets across the half-pipe.
const FACETS: usize = 24;
/// Target length of a mesh segment at the finest level of detail.
const SEGMENT: f64 = 8.0;
/// A station's court: its radius (to its corners), and its sides.
pub const STATION_RADIUS: f64 = 40.0;
const STATION_SIDES: usize = 32;
const STATION_ALBEDO: f32 = 0.2;
/// How far along a canal its floor eases to its station's (m).
const RAMP: f64 = 200.0;

const PIPE_ALBEDO: f32 = 0.27;
/// The pipe's outermost facets each side, up to its rim, dark.
const DARK_FACETS: usize = 3;
const DARK_ALBEDO: f32 = 0.06;
const WALL_ALBEDO: f32 = 0.12;
/// The grooves along a pipe (the canal's light runs in them when it is on),
/// as angles from its bottom (radians): one under the rider, one up each
/// wall. Each is `GROOVE_HALF` m to either side of its line and
/// `GROOVE_DEPTH` m deep.
pub const GROOVES: [f64; 3] = [-0.87, 0.0, 0.87];
pub const GROOVE_HALF: f64 = 0.25;
pub const GROOVE_DEPTH: f64 = 0.12;
const GROOVE_ALBEDO: f32 = 0.05;
/// A joint between the pipe's sections: every `JOINT_EVERY` finest
/// segments (some 24 m), a ring `JOINT_WIDTH` m long sunk `JOINT_DEPTH` m.
const JOINT_EVERY: i64 = 3;
const JOINT_WIDTH: f64 = 0.3;
const JOINT_DEPTH: f64 = 0.06;
const JOINT_ALBEDO: f32 = 0.12;

/// A point across a canal's profile, and the facet that starts at it.
#[derive(Clone, Copy, Debug)]
struct Facet {
    /// Offset across, height above the floor.
    o: f64,
    h: f64,
    albedo: f32,
    /// A groove's side: flat, facing this way (across, up); else curved.
    flat: Option<DVec2>,
    /// Lined with stones (near you): the pipe's own surface, not a
    /// groove's or a joint's.
    stone: bool,
}

/// The stones lining a pipe near you (at the finest detail only: smaller
/// than a pixel they would shimmer): each `length` m along it, its rows
/// staggered by half that, its tone varying by up to `tone` (a share of its
/// albedo), and lit as if tilted up to `tilt` degrees, each its own way.
/// The same whether a canal is on or off: they are the stone itself. (The
/// user: in the pipe it was hard to judge how fast you go, nothing in it
/// passing; grain, but not a texture.)
#[derive(Clone, Copy, Debug)]
pub struct Stones {
    pub length: f64,
    pub tone: f32,
    pub tilt: f32,
}

/// Rises and dips along a canal, over the land's broad shape: up to
/// `height` m either way, about `wave` m from one crest to the next (two
/// waves of different lengths, their phases each canal's own). Downhill
/// gives speed, uphill takes it; risen, the pipe stands high on its outer
/// walls like an aqueduct, dipped, it runs in a deep cutting; and a crest
/// hides what lies beyond it. (The user: more vertical variation, to make
/// the journey more interesting.) Near the stations it eases out with the
/// floor. Off for now (`--set canal_rise=30`, `canal_wave=1800` to see it):
/// how canals rise and fall waits on the world's own verticality, still to
/// decide, the user; a canal's waves should perhaps come from the land it
/// crosses rather than their own.
#[derive(Clone, Copy, Debug)]
pub struct Rise {
    pub height: f64,
    pub wave: f64,
}

impl Default for Rise {
    fn default() -> Self {
        Self { height: 0.0, wave: 1800.0 }
    }
}

impl Default for Stones {
    fn default() -> Self {
        Self { length: 2.0, tone: 0.08, tilt: 2.0 }
    }
}

/// A canal: from station `a` to station `b`.
#[derive(Clone, Copy, Debug)]
pub struct Canal {
    pub a: usize,
    pub b: usize,
    /// Station `a`, and the direction to `b`.
    start: DVec2,
    dir: DVec2,
    perp: DVec2,
    length: f64,
    /// +1 or -1: which way the flow runs along `dir`.
    pub flow: f64,
}

/// A point's relation to a canal.
#[derive(Clone, Copy, Debug)]
pub struct CanalHit {
    pub canal: usize,
    /// Signed distance from the centreline, across it.
    pub offset: f64,
    /// The nearest point on the centreline.
    pub center: DVec2,
    /// Height of the floor there.
    pub floor: f64,
    /// Direction of the flow.
    pub flow_dir: DVec2,
    /// Unit direction in which `offset` grows.
    pub across: DVec2,
}

/// A point in a canal's frame: across and along it, and where its start
/// lies in the point's own copy of the world.
#[derive(Clone, Copy, Debug)]
struct Local {
    offset: f64,
    along: f64,
    start: DVec2,
}

pub struct Canals {
    size: f64,
    pub lines: Vec<Canal>,
    pub stations: Vec<DVec2>,
    pub stones: Stones,
    pub rise: Rise,
    /// How much of a district's relief a canal's floor follows, at most
    /// (`--set canal_relief`): over it the hills rise round the canal, in a
    /// cutting. 1.5 (the user: working the right way; at 0.8 the highlands,
    /// at 3, put canals in gorges 14 to 86 m deep, 48 on average, which the
    /// user found dramatic; at 1.5, 10 to 65, 36).
    pub relief: f64,
}

impl Canals {
    pub fn new(size: f64, seed: u32) -> Self {
        let network = Network::new(size, seed);
        let r = |k: i32| hash01(0, k, 0, seed ^ 0xca7a1) as f64;
        let lines = network
            .links
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let dir = l.offset / l.length();
                Canal {
                    a: l.a,
                    b: l.b,
                    start: network.stations[l.a],
                    dir,
                    perp: DVec2::new(-dir.y, dir.x),
                    length: l.length(),
                    flow: if r(3 + i as i32) < 0.5 { 1.0 } else { -1.0 },
                }
            })
            .collect();
        Self { size, lines, stations: network.stations, stones: Stones::default(), rise: Rise::default(), relief: 1.5 }
    }

    /// `d` round the wrap the short way.
    fn wrap(&self, d: DVec2) -> DVec2 {
        d - (d / self.size).round() * self.size
    }

    /// `p` in canal `i`'s frame, from the copy of the canal nearest it.
    fn local(&self, i: usize, p: DVec2) -> Local {
        let c = &self.lines[i];
        let mid = c.start + c.dir * (c.length * 0.5);
        let rel = self.wrap(p - mid);
        Local { offset: rel.dot(c.perp), along: rel.dot(c.dir) + c.length * 0.5, start: p - rel - c.dir * (c.length * 0.5) }
    }

    /// Where a pipe meets the court at either end: how far along the canal
    /// from each station (its corners on the court's rim).
    fn end(&self) -> f64 {
        let apothem = STATION_RADIUS * (std::f64::consts::PI / STATION_SIDES as f64).cos();
        (apothem * apothem - HALF_WIDTH * HALF_WIDTH).sqrt()
    }

    /// The floor of canal `i` at `along` (its centreline point `at`): the
    /// field's, easing to each station's near its ends.
    fn floor(&self, i: usize, along: f64, at: DVec2, field: &impl Fn(DVec2) -> f64) -> f64 {
        let c = &self.lines[i];
        // The land's broad shape, and the canal's own rises and dips.
        let phase = |k: i32| hash01(i as i32, k, 0, 0x51e5) as f64 * std::f64::consts::TAU;
        let wave = |length: f64, k: i32| (along / length * std::f64::consts::TAU + phase(k)).sin();
        let f = field(at) + self.rise.height * (0.65 * wave(self.rise.wave, 1) + 0.35 * wave(self.rise.wave * 0.43, 2));
        let ease = |s: f64| {
            let t = ((s - self.end()) / RAMP).clamp(0.0, 1.0);
            1.0 - t * t * (3.0 - 2.0 * t)
        };
        let (wa, wb) = (ease(along), ease(c.length - along));
        f + (field(self.stations[c.a]) - f) * wa + (field(self.stations[c.b]) - f) * wb
    }

    /// Canal `i`'s stations: where `a` stands, and `b`'s copy nearest it.
    pub fn ends(&self, i: usize) -> (DVec2, DVec2) {
        let c = &self.lines[i];
        (c.start, c.start + c.dir * c.length)
    }

    /// Station `s`'s floor (the level of its canals' floors).
    pub fn station_floor(&self, s: usize, field: &impl Fn(DVec2) -> f64) -> f64 {
        field(self.stations[s])
    }

    /// The lowest floor of any station court the segment `a`-`b` touches
    /// (within half a metre), if any: a plate edge there must reach it.
    pub fn court_floor_near(&self, a: DVec2, b: DVec2, field: &impl Fn(DVec2) -> f64) -> Option<f64> {
        let mut lowest: Option<f64> = None;
        for s in 0..self.stations.len() {
            // (The station's copy nearest the segment.)
            let at = a - self.wrap(a - self.stations[s]);
            let edge = b - a;
            let t = ((at - a).dot(edge) / edge.length_squared().max(1e-12)).clamp(0.0, 1.0);
            if (a + edge * t).distance(at) <= STATION_RADIUS + 0.5 {
                let f = self.station_floor(s, field);
                lowest = Some(lowest.map_or(f, |l| l.min(f)));
            }
        }
        lowest
    }

    /// The station whose court contains `p`, if any.
    pub fn station_at(&self, p: DVec2) -> Option<usize> {
        // (Its edges tested only near it: built for every station at every
        // point asked, they made the ground's meshing several times slower.)
        let apothem = STATION_RADIUS * (std::f64::consts::PI / STATION_SIDES as f64).cos();
        (0..self.stations.len()).find(|&s| {
            let rel = self.wrap(p - self.stations[s]);
            let r = rel.length();
            if r > STATION_RADIUS {
                return false;
            }
            if r <= apothem {
                return true;
            }
            // Between the edges' and the corners' reach: against the
            // nearest edge, whose outward normal is at a multiple of the
            // sector from the x axis.
            let sector = std::f64::consts::TAU / STATION_SIDES as f64;
            let k = (rel.y.atan2(rel.x) / sector).round();
            rel.dot(DVec2::new((k * sector).cos(), (k * sector).sin())) <= apothem
        })
    }

    /// The edges of station `s`'s court in `near`'s copy of the world: each
    /// how far outside it a point is (positive outside).
    fn station_planes(&self, s: usize, near: DVec2) -> Vec<Box<dyn Fn(DVec2) -> f64>> {
        let center = near - self.wrap(near - self.stations[s]);
        let apothem = STATION_RADIUS * (std::f64::consts::PI / STATION_SIDES as f64).cos();
        (0..STATION_SIDES)
            .map(|k| {
                let a = k as f64 / STATION_SIDES as f64 * std::f64::consts::TAU;
                let n = DVec2::new(a.cos(), a.sin());
                Box::new(move |v: DVec2| (v - center).dot(n) - apothem) as Box<dyn Fn(DVec2) -> f64>
            })
            .collect()
    }

    /// The canal whose pipe corridor (plus `margin` across it) contains
    /// `p`, if any.
    pub fn hit(&self, p: DVec2, margin: f64, field: &impl Fn(DVec2) -> f64) -> Option<CanalHit> {
        let end = self.end();
        (0..self.lines.len())
            .filter_map(|i| {
                let l = self.local(i, p);
                (l.offset.abs() < HALF_WIDTH + margin && l.along >= end && l.along <= self.lines[i].length - end).then(|| self.hit_at(i, l, l.along, field))
            })
            .min_by(|a, b| a.offset.abs().total_cmp(&b.offset.abs()))
    }

    fn hit_at(&self, i: usize, l: Local, along: f64, field: &impl Fn(DVec2) -> f64) -> CanalHit {
        let c = &self.lines[i];
        let center = l.start + c.dir * along;
        CanalHit { canal: i, offset: l.offset, center, floor: self.floor(i, along, center, field), flow_dir: c.dir * c.flow, across: c.perp }
    }

    /// The nearest point on any canal's centreline (a canal runs on to its
    /// stations' centres), however far away.
    pub fn nearest(&self, p: DVec2, field: &impl Fn(DVec2) -> f64) -> Option<CanalHit> {
        (0..self.lines.len())
            .map(|i| {
                let l = self.local(i, p);
                let along = l.along.clamp(0.0, self.lines[i].length);
                let hit = self.hit_at(i, l, along, field);
                (hit, hit.center.distance(p))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(h, _)| h)
    }

    /// Height of the canal's surface across its profile, given the floor
    /// height at the centreline.
    pub fn profile_height(offset: f64, floor: f64) -> f64 {
        let o = offset.abs();
        if o >= PIPE_RADIUS {
            floor + PIPE_RADIUS
        } else {
            floor + PIPE_RADIUS - (PIPE_RADIUS * PIPE_RADIUS - o * o).sqrt()
        }
    }

    /// Surface height in a station's court or a canal's corridor at `p`.
    pub fn surface(&self, p: DVec2, field: &impl Fn(DVec2) -> f64) -> Option<f64> {
        if let Some(s) = self.station_at(p) {
            return Some(self.station_floor(s, field));
        }
        self.hit(p, 0.0, field).map(|h| Self::profile_height(h.offset, h.floor))
    }

    /// The parts of a convex polygon outside every canal corridor and
    /// station court.
    pub fn cut(&self, poly: Vec<DVec2>) -> Vec<Vec<DVec2>> {
        let mut pieces = vec![poly];
        let end = self.end();
        for s in 0..self.stations.len() {
            pieces = pieces
                .into_iter()
                .flat_map(|piece| {
                    let near = centroid(&piece);
                    // (Only pieces that reach the court: the rest whole.)
                    let reach = piece.iter().map(|v| v.distance(near)).fold(0.0, f64::max);
                    if self.wrap(near - self.stations[s]).length() > STATION_RADIUS + reach {
                        return vec![piece];
                    }
                    subtract(piece, &self.station_planes(s, near))
                })
                .collect();
        }
        for i in 0..self.lines.len() {
            let c = self.lines[i];
            pieces = pieces
                .into_iter()
                .flat_map(|piece| {
                    // Each vertex in the frame of the copy nearest the
                    // piece (pieces are far smaller than the world).
                    let near = centroid(&piece);
                    let l = self.local(i, near);
                    let offset = move |v: DVec2| l.offset + (v - near).dot(c.perp);
                    let along = move |v: DVec2| l.along + (v - near).dot(c.dir);
                    let planes: [Box<dyn Fn(DVec2) -> f64>; 4] = [
                        Box::new(move |v| offset(v) - HALF_WIDTH),
                        Box::new(move |v| -offset(v) - HALF_WIDTH),
                        Box::new(move |v| along(v) - (c.length - end)),
                        Box::new(move |v| end - along(v)),
                    ];
                    subtract(piece, &planes)
                })
                .collect();
        }
        pieces
    }

    /// Meshes the canal segments and station courts whose midpoint lies in
    /// the square `[x0, x0 + size) x [z0, z0 + size)`, with segments `scale`
    /// times the finest length. Positions are relative to `(x0, z0)`.
    pub fn mesh_square(&self, mesh: &mut ColumnMesh, (x0, z0): (f64, f64), size: f64, scale: f64, field: &impl Fn(DVec2) -> f64) {
        let inside = |p: DVec2| p.x >= x0 && p.x < x0 + size && p.y >= z0 && p.y < z0 + size;
        let center = DVec2::new(x0 + size * 0.5, z0 + size * 0.5);
        let reach = size * 0.75 + HALF_WIDTH;
        let end = self.end();
        for i in 0..self.lines.len() {
            let c = self.lines[i];
            let l = self.local(i, center);
            if l.offset.abs() > reach || l.along < -reach || l.along > c.length + reach {
                continue;
            }
            let run = c.length - 2.0 * end;
            let n = (run / (SEGMENT * scale)).round().max(1.0) as i64;
            let seg = run / n as f64;
            let first = (((l.along - reach - end) / seg).floor() as i64).max(0);
            let last = (((l.along + reach - end) / seg).ceil() as i64).min(n);
            let (surface, sunk) = (Self::profile(0.0), Self::profile(JOINT_DEPTH));
            for k in first..last {
                let (s0, s1) = (end + k as f64 * seg, end + (k + 1) as f64 * seg);
                if !inside(l.start + c.dir * ((s0 + s1) * 0.5)) {
                    continue;
                }
                let (a, b) = (l.start + c.dir * s0, l.start + c.dir * s1);
                let (fa, fb) = (self.floor(i, s0, a, field), self.floor(i, s1, b, field));
                // A joint at its start (near enough to be seen): a ring
                // sunk a little, stepping down into it and up out of it.
                let (from, floor_from) = if scale <= 1.0 && k > 0 && k % JOINT_EVERY == 0 {
                    let j = l.start + c.dir * (s0 + JOINT_WIDTH);
                    let fj = self.floor(i, s0 + JOINT_WIDTH, j, field);
                    Self::mesh_segment(mesh, c.perp, a, b, fa, fj, &sunk, (x0, z0), Some(j), None);
                    Self::mesh_step(mesh, c.perp, a, fa, &surface, &sunk, c.dir, (x0, z0));
                    Self::mesh_step(mesh, c.perp, j, fj, &surface, &sunk, -c.dir, (x0, z0));
                    (j, fj)
                } else {
                    (a, fa)
                };
                let start = s0 + (from - a).length();
                let stones = (scale <= 1.0).then_some((self.stones, i, start, s1));
                Self::mesh_segment(mesh, c.perp, from, b, floor_from, fb, &surface, (x0, z0), None, stones);
                // Its ends, where it opens onto a court: capped.
                if k == 0 {
                    Self::mesh_cap(mesh, c.perp, a, fa, -c.dir, (x0, z0));
                }
                if k == n - 1 {
                    Self::mesh_cap(mesh, c.perp, b, fb, c.dir, (x0, z0));
                }
            }
        }
        for s in 0..self.stations.len() {
            let at = center - self.wrap(center - self.stations[s]);
            if inside(at) {
                Self::mesh_station(mesh, at, self.station_floor(s, field), (x0, z0));
            }
        }
    }

    /// A station's court: its floor, and a skirt down past the ground.
    fn mesh_station(mesh: &mut ColumnMesh, at: DVec2, floor: f64, origin: (f64, f64)) {
        let local = |p: DVec2, y: f64| Vec3::new((p.x - origin.0) as f32, y as f32, (p.y - origin.1) as f32);
        // (Corners between the edges `station_planes` gives.)
        let corner = |k: usize| {
            let a = (k as f64 + 0.5) / STATION_SIDES as f64 * std::f64::consts::TAU;
            at + DVec2::new(a.cos(), a.sin()) * STATION_RADIUS
        };
        let middle = local(at, floor);
        for k in 0..STATION_SIDES {
            let (p, q) = (local(corner(k), floor), local(corner(k + 1), floor));
            let base = mesh.positions.len() as u32;
            for v in [middle, p, q] {
                mesh.positions.push(v.to_array());
                mesh.normals.push([0.0, 1.0, 0.0]);
                mesh.albedo.push(STATION_ALBEDO);
                mesh.ao.push(1.0);
            }
            let face = (p - middle).cross(q - middle);
            let up = if face.y >= 0.0 { [base, base + 1, base + 2] } else { [base, base + 2, base + 1] };
            mesh.indices.extend_from_slice(&up);
            let out = (corner(k) + corner(k + 1)) * 0.5 - at;
            let normal = Vec3::new(out.x as f32, 0.0, out.y as f32).normalize();
            let down = Vec3::Y * OUTER_DEPTH as f32;
            push_quad(mesh, [p, q, q - down, p - down], normal, WALL_ALBEDO, [1.0, 1.0, 0.0, 0.0]);
        }
    }

    /// The end of a pipe where it opens onto a court: closed from its
    /// surface down to the court's floor, facing `out`.
    fn mesh_cap(mesh: &mut ColumnMesh, perp: DVec2, at: DVec2, floor: f64, out: DVec2, origin: (f64, f64)) {
        let local = |o: f64, h: f64| {
            let p = at + perp * o;
            Vec3::new((p.x - origin.0) as f32, (floor + h) as f32, (p.y - origin.1) as f32)
        };
        let normal = Vec3::new(out.x as f32, 0.0, out.y as f32);
        for w in Self::profile(0.0).windows(2) {
            let (p0, p1) = (w[0], w[1]);
            if p0.h + p1.h < 1e-3 {
                continue;
            }
            push_quad(mesh, [local(p0.o, p0.h), local(p1.o, p1.h), local(p1.o, 0.0), local(p0.o, 0.0)], normal, WALL_ALBEDO, [1.0; 4]);
        }
    }

    /// Where the surface steps between two profiles (`from` and `to`, of
    /// the same points), facing `out`.
    #[allow(clippy::too_many_arguments)]
    fn mesh_step(mesh: &mut ColumnMesh, perp: DVec2, at: DVec2, floor: f64, from: &[Facet], to: &[Facet], out: DVec2, origin: (f64, f64)) {
        let local = |p: &Facet| {
            let q = at + perp * p.o;
            Vec3::new((q.x - origin.0) as f32, (floor + p.h) as f32, (q.y - origin.1) as f32)
        };
        let normal = Vec3::new(out.x as f32, 0.0, out.y as f32);
        for k in 0..from.len() - 1 {
            push_quad(mesh, [local(&from[k]), local(&from[k + 1]), local(&to[k + 1]), local(&to[k])], normal, JOINT_ALBEDO, [1.0; 4]);
        }
    }

    /// The profile across a canal, `sink` m outside its surface (a joint's
    /// ring): the half-pipe in facets, the outermost each side dark, with
    /// its grooves cut in.
    fn profile(sink: f64) -> Vec<Facet> {
        use std::f64::consts::{FRAC_PI_2, PI};
        let r = PIPE_RADIUS + sink;
        let half = GROOVE_HALF / PIPE_RADIUS;
        // By the angle from the bottom (`phi`): where, and which way along
        // the surface (towards greater `phi`).
        let at = |phi: f64, radius: f64| (radius * phi.sin(), PIPE_RADIUS - radius * phi.cos());
        let tangent = |phi: f64| DVec2::new(phi.cos(), phi.sin());
        let dark = |phi: f64| phi.abs() > FRAC_PI_2 - DARK_FACETS as f64 * PI / FACETS as f64;
        let surface = |phi: f64| {
            if sink > 0.0 {
                JOINT_ALBEDO
            } else if dark(phi) {
                DARK_ALBEDO
            } else {
                PIPE_ALBEDO
            }
        };
        let facet = |phi: f64, radius: f64, albedo: f32, flat: Option<DVec2>| {
            let (o, h) = at(phi, radius);
            Facet { o, h, albedo, flat, stone: false }
        };
        let stone = |f: Facet| Facet { stone: sink == 0.0, ..f };
        // (Sorted by angle, then by order at one angle.)
        let mut points: Vec<(f64, u8, Facet)> = (0..=FACETS)
            .map(|f| -FRAC_PI_2 + PI * f as f64 / FACETS as f64)
            .filter(|phi| GROOVES.iter().all(|g| (phi - g).abs() > half))
            .map(|phi| (phi, 0, stone(facet(phi, r, surface(phi + PI / FACETS as f64 * 0.5), None))))
            .collect();
        for g in GROOVES {
            let (a, b) = (g - half, g + half);
            points.push((a, 0, facet(a, r, GROOVE_ALBEDO, Some(tangent(a)))));
            points.push((a, 1, facet(a, r + GROOVE_DEPTH, GROOVE_ALBEDO, None)));
            points.push((b, 0, facet(b, r + GROOVE_DEPTH, GROOVE_ALBEDO, Some(-tangent(b)))));
            points.push((b, 1, stone(facet(b, r, surface(b + 1e-6), None))));
        }
        points.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        points.into_iter().map(|(_, _, f)| f).collect()
    }

    /// A length of pipe from `a` to `b` (or only to `to`, if given) with
    /// the given profile, and its outer walls; lined with stones if given
    /// them (and which canal it is, and how far along it runs, from and to).
    #[allow(clippy::too_many_arguments)]
    fn mesh_segment(
        mesh: &mut ColumnMesh,
        perp: DVec2,
        a: DVec2,
        b: DVec2,
        floor_a: f64,
        floor_b: f64,
        profile: &[Facet],
        origin: (f64, f64),
        to: Option<DVec2>,
        stones: Option<(Stones, usize, f64, f64)>,
    ) {
        let b = to.unwrap_or(b);
        let world = |center: DVec2, f: f64, o: f64, h: f64| {
            let p = center + perp * o;
            Vec3::new((p.x - origin.0) as f32, (f + h) as f32, (p.y - origin.1) as f32)
        };
        let across = |n: DVec2| Vec3::new((perp.x * n.x) as f32, n.y as f32, (perp.y * n.x) as f32).normalize_or(Vec3::Y);
        // Smooth shading on the curve: each point gets the curved surface's
        // normal there (towards the pipe's axis); a groove's sides are flat.
        let radial = |p: &Facet| across(DVec2::new(-p.o, PIPE_RADIUS - p.h).normalize_or_zero());
        let along = (world(b, floor_b, 0.0, 0.0) - world(a, floor_a, 0.0, 0.0)).normalize_or(Vec3::X);
        for (f, w) in profile.windows(2).enumerate() {
            let (p0, p1) = (&w[0], &w[1]);
            let (n0, n1) = match p0.flat {
                Some(n) => (across(n), across(n)),
                None => (radial(p0), radial(p1)),
            };
            let bottom_ness = 1.0 - (p0.h + p1.h) as f32 * 0.5 / PIPE_RADIUS as f32;
            let ao = 1.0 - 0.35 * bottom_ness.max(0.0);
            let facing = (n0 + n1).normalize_or(Vec3::Y);
            let Some((st, canal, from, upto)) = stones.filter(|_| p0.stone) else {
                let quad = [world(a, floor_a, p0.o, p0.h), world(a, floor_a, p1.o, p1.h), world(b, floor_b, p1.o, p1.h), world(b, floor_b, p0.o, p0.h)];
                push_quad_smooth(mesh, quad, [n0, n1, n1, n0], facing, p0.albedo, [ao; 4]);
                continue;
            };
            // The row's stones: cut every `length` m along the canal, every
            // other row staggered by half that.
            let stagger = if f % 2 == 1 { st.length * 0.5 } else { 0.0 };
            let mut cuts = vec![from];
            let mut k = ((from - stagger) / st.length).floor() as i64 + 1;
            while stagger + k as f64 * st.length < upto - 1e-6 {
                cuts.push(stagger + k as f64 * st.length);
                k += 1;
            }
            cuts.push(upto);
            let lean = st.tilt.to_radians().tan();
            for c in cuts.windows(2) {
                let (t0, t1) = ((c[0] - from) / (upto - from), (c[1] - from) / (upto - from));
                let id = (((c[0] + c[1]) * 0.5 - stagger) / st.length).floor() as i32;
                let r = |k: i32| hash01(id, f as i32 * 4 + k, canal as i32, 0x570e5);
                let tone = 1.0 + st.tone * (2.0 * r(0) - 1.0);
                let (x, y) = (2.0 * r(1) - 1.0, 2.0 * r(2) - 1.0);
                let tilt = |n: Vec3| (n + (n.cross(along) * x + along * y) * lean).normalize();
                let (q0, q1) = (a.lerp(b, t0), a.lerp(b, t1));
                let (f0, f1) = (floor_a + (floor_b - floor_a) * t0, floor_a + (floor_b - floor_a) * t1);
                let quad = [world(q0, f0, p0.o, p0.h), world(q0, f0, p1.o, p1.h), world(q1, f1, p1.o, p1.h), world(q1, f1, p0.o, p0.h)];
                push_quad_smooth(mesh, quad, [tilt(n0), tilt(n1), tilt(n1), tilt(n0)], facing, p0.albedo * tone, [ao; 4]);
            }
        }

        // Outer walls, down past the ground on both sides.
        for side in [-1.0, 1.0] {
            let top_a = world(a, floor_a, side * HALF_WIDTH, PIPE_RADIUS);
            let top_b = world(b, floor_b, side * HALF_WIDTH, PIPE_RADIUS);
            let down = Vec3::Y * OUTER_DEPTH as f32;
            let normal = Vec3::new((perp.x * side) as f32, 0.0, (perp.y * side) as f32);
            push_quad(mesh, [top_a, top_b, top_b - down, top_a - down], normal, WALL_ALBEDO, [1.0, 1.0, 0.0, 0.0]);
        }
    }

    /// Canal `i`'s pipe as points along its centreline about `step` m
    /// apart, from end to end: each where it is, how far along, and its
    /// floor there.
    pub fn track(&self, i: usize, step: f64, field: &impl Fn(DVec2) -> f64) -> Vec<(DVec2, f64, f64)> {
        let c = &self.lines[i];
        let end = self.end();
        let n = ((c.length - 2.0 * end) / step).round().max(1.0) as usize;
        (0..=n)
            .map(|k| {
                let along = end + (c.length - 2.0 * end) * k as f64 / n as f64;
                let at = c.start + c.dir * along;
                (at, along, self.floor(i, along, at, field))
            })
            .collect()
    }

    /// The unit across canal `i`.
    pub fn across(&self, i: usize) -> DVec2 {
        self.lines[i].perp
    }
}

fn centroid(poly: &[DVec2]) -> DVec2 {
    poly.iter().copied().sum::<DVec2>() / poly.len() as f64
}

/// The parts of a convex polygon outside a convex region, the region given
/// by its edges (each how far outside it a point is: positive outside).
fn subtract(poly: Vec<DVec2>, planes: &[Box<dyn Fn(DVec2) -> f64>]) -> Vec<Vec<DVec2>> {
    if poly.len() < 3 {
        return Vec::new();
    }
    // Wholly outside one edge: untouched.
    if planes.iter().any(|f| poly.iter().all(|&v| f(v) >= 0.0)) {
        return vec![poly];
    }
    let mut out = Vec::new();
    let mut rest = poly;
    for f in planes {
        let outside = clip_half_plane(&rest, f);
        if outside.len() >= 3 {
            out.push(outside);
        }
        rest = clip_half_plane(&rest, |v| -f(v));
        if rest.len() < 3 {
            break;
        }
    }
    out
}

/// Keeps the part of a convex polygon where `side(v) >= 0`.
pub fn clip_half_plane(poly: &[DVec2], side: impl Fn(DVec2) -> f64) -> Vec<DVec2> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for k in 0..poly.len() {
        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    out
}

/// Appends a quad as two triangles facing `facing`, with its own normal at
/// each corner for smooth shading.
fn push_quad_smooth(
    mesh: &mut ColumnMesh,
    quad: [Vec3; 4],
    normals: [Vec3; 4],
    facing: Vec3,
    albedo: f32,
    ao: [f32; 4],
) {
    let base = mesh.positions.len() as u32;
    for ((p, n), a) in quad.iter().zip(normals).zip(ao) {
        mesh.positions.push(p.to_array());
        mesh.normals.push(n.to_array());
        mesh.albedo.push(albedo);
        mesh.ao.push(a);
    }
    let face = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
    if face.dot(facing) >= 0.0 {
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        mesh.indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

/// Appends a quad as two triangles facing `normal`.
fn push_quad(mesh: &mut ColumnMesh, quad: [Vec3; 4], normal: Vec3, albedo: f32, ao: [f32; 4]) {
    let base = mesh.positions.len() as u32;
    for (p, a) in quad.iter().zip(ao) {
        mesh.positions.push(p.to_array());
        mesh.normals.push(normal.to_array());
        mesh.albedo.push(albedo);
        mesh.ao.push(a);
    }
    let face = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
    if face.dot(normal) >= 0.0 {
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        mesh.indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: f64 = 16384.0;

    fn field(p: DVec2) -> f64 {
        (p.x * 0.001).sin() * 20.0 + (p.y * 0.0013).cos() * 15.0
    }

    /// A canal's floor meets its stations' at its ends: no step out of the
    /// pipe onto the court.
    #[test]
    fn floors_meet_the_stations() {
        let canals = Canals::new(SIZE, 1);
        let end = canals.end();
        for (i, c) in canals.lines.iter().enumerate() {
            for (along, s) in [(end, c.a), (c.length - end, c.b)] {
                let at = c.start + c.dir * along;
                let f = canals.floor(i, along, at, &field);
                assert!((f - canals.station_floor(s, &field)).abs() < 1e-6, "canal {i} at {along}");
            }
        }
    }

    /// The cut removes exactly the corridors and the courts: no piece left
    /// inside one, and the pieces cover the rest.
    #[test]
    fn cut_removes_corridors_and_courts() {
        let canals = Canals::new(SIZE, 1);
        let c = canals.lines[0];
        let area = |p: &[DVec2]| (0..p.len()).map(|k| p[k].perp_dot(p[(k + 1) % p.len()])).sum::<f64>().abs() * 0.5;
        for along in [c.length * 0.5, 10.0, c.length - 25.0] {
            let at = c.start + c.dir * along;
            let square = vec![at + DVec2::new(-30.0, -30.0), at + DVec2::new(30.0, -30.0), at + DVec2::new(30.0, 30.0), at + DVec2::new(-30.0, 30.0)];
            let pieces = canals.cut(square.clone());
            let kept: f64 = pieces.iter().map(|p| area(p)).sum();
            // The square's area outside, sampled a metre apart.
            let mut outside = 0;
            for x in 0..60 {
                for z in 0..60 {
                    let p = square[0] + DVec2::new(x as f64 + 0.5, z as f64 + 0.5);
                    if canals.station_at(p).is_none() && canals.hit(p, 0.0, &field).is_none() {
                        outside += 1;
                    }
                }
            }
            assert!((kept - outside as f64).abs() < 60.0, "along {along}: kept {kept:.0} m2, sampled {outside}");
            for piece in &pieces {
                let m = centroid(piece);
                assert!(canals.station_at(m).is_none() && canals.hit(m, 0.0, &field).is_none(), "a piece inside, along {along}");
            }
        }
    }

    /// Round the wrap, a canal is the same canal.
    #[test]
    fn canals_wrap() {
        let canals = Canals::new(SIZE, 5);
        for i in 0..canals.lines.len() {
            for k in 0..20 {
                let p = DVec2::new(k as f64 * 173.0, k as f64 * 61.0);
                let (l1, l2) = (canals.local(i, p), canals.local(i, p + DVec2::new(SIZE, -SIZE)));
                assert!((l1.offset - l2.offset).abs() < 1e-6 && (l1.along - l2.along).abs() < 1e-6);
            }
        }
    }
}
