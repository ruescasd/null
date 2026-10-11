//! The paths through the plan, as a blockout (step 3a′ of the interior way
//! down): places first, then paths between them. Places are rooms in the
//! rock, scattered through it storey by storey (a storey 6 m, counted down
//! from the rim), kept apart; one is the entrance, on the rim. Then paths
//! join them, the nearest pairs first, into a tree (every path leading
//! somewhere), and a few more where the tree's way round is long (loops,
//! shortcuts). A path is legs: each a stretch inside the rock (a cell back
//! from the faces at least), level but for one flight up or down along it,
//! then an exposure where it meets a face: a ledge along the face and back
//! in, a bridge over a branch into the rock across it, a glass stepwell
//! against the void, a balcony out over it; never the same kind twice
//! running. The last leg goes inside into the place it leads to, at a
//! storey it spans. Nothing of a path passes within a few storeys of
//! anything else. The spine is the shortest way from the entrance to the
//! deepest place.

use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::{CELL, N, Plan, tree};
use crate::chasm::rooms::Spec;

/// A storey's height (m).
const STOREY: f32 = 6.0;

/// How deep the places go (storeys below the rim).
const DEPTH: i32 = 60;

/// What comes out onto the chasm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Ledge,
    Bridge,
    Glass,
    Balcony,
}

const KINDS: [Kind; 4] = [Kind::Ledge, Kind::Bridge, Kind::Glass, Kind::Balcony];

/// What a step of a path is: inside the rock, on, or a stair onto another
/// storey; or out on the chasm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum What {
    In,
    Stair,
    Out(Kind),
}

/// A step of a path: a cell, the storey, and what it is.
#[derive(Clone, Copy, Debug)]
pub(super) struct Step {
    pub at: IVec2,
    pub storey: i32,
    pub what: What,
}

const DIRS: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

/// What the plan's cells are to the paths: the chasm; rock (not by the
/// plan's edge); rock with the chasm beside it (a face); rock with none
/// beside it, even across a corner (inside).
struct Cells<'a>(&'a Plan);

impl Cells<'_> {
    fn void(&self, c: IVec2) -> bool {
        (0..N).contains(&c.x) && (0..N).contains(&c.y) && self.0.get(c.x, c.y).is_some()
    }

    fn rock(&self, c: IVec2) -> bool {
        (2..N - 2).contains(&c.x) && (2..N - 2).contains(&c.y) && self.0.get(c.x, c.y).is_none()
    }

    /// Which way the chasm lies from a face (the first way, at a corner).
    fn face(&self, c: IVec2) -> Option<IVec2> {
        if !self.rock(c) {
            return None;
        }
        DIRS.into_iter().find(|&d| self.void(c + d))
    }

    fn inside(&self, c: IVec2) -> bool {
        self.rock(c) && (-1..=1).all(|x| (-1..=1).all(|z| !self.void(c + IVec2::new(x, z))))
    }
}

/// What is taken: cells, each at a storey.
#[derive(Default)]
struct Taken(HashSet<(i32, i32, i32)>);

impl Taken {
    fn any(&self, c: IVec2, s: (i32, i32)) -> bool {
        (s.0..=s.1).any(|s| self.0.contains(&(c.x, c.y, s)))
    }
}

/// A place: a room `size` cells across from `at`, spanning storeys
/// `storeys.0` (its top) to `storeys.1`.
#[derive(Clone, Copy)]
pub(super) struct Place {
    pub at: IVec2,
    pub size: IVec2,
    pub storeys: (i32, i32),
}

impl Place {
    fn cells(&self) -> impl Iterator<Item = IVec2> {
        let (a, s) = (self.at, self.size);
        (0..s.y).flat_map(move |z| (0..s.x).map(move |x| a + IVec2::new(x, z)))
    }

    fn contains(&self, c: IVec2) -> bool {
        c.cmpge(self.at).all() && c.cmplt(self.at + self.size).all()
    }

    fn centre(&self) -> Vec2 {
        self.at.as_vec2() + self.size.as_vec2() * 0.5
    }

    /// The storey of it nearest `s`.
    fn nearest(&self, s: i32) -> i32 {
        s.clamp(self.storeys.0, self.storeys.1)
    }

    /// How far apart two places are (m): their centres across, the storeys
    /// between them down.
    fn gap(&self, o: &Place) -> f32 {
        let across = (self.centre() - o.centre()).length() * CELL;
        let down = (o.storeys.0 - self.storeys.1).max(self.storeys.0 - o.storeys.1).max(0) as f32 * STOREY;
        (across * across + down * down).sqrt()
    }
}

/// A door of a place: the cell of it a path comes in by, the way out of
/// it from there (a unit step), and the storey.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Door {
    pub cell: IVec2,
    pub out: IVec2,
    pub storey: i32,
}

/// A leg: its steps (from where it leaves), what they take, and where it
/// ends (a cell, at a storey).
struct Leg {
    steps: Vec<Step>,
    takes: Vec<(IVec2, i32)>,
    exit: (IVec2, i32),
}

/// The exposure of `kind` at face `c` (the chasm `d` from it) at storey
/// `s`, if it fits: its steps, what it takes, and where it goes back into
/// the rock. A glass stepwell goes `toward` (1 down, -1 up).
fn exposure(cells: &Cells, taken: &Taken, c: IVec2, d: IVec2, s: i32, kind: Kind, toward: i32, r: &dyn Fn(i32) -> f32) -> Option<Leg> {
    let free = |c: IVec2, s: i32| !taken.any(c, (s - 2, s + 2));
    let flip = if r(1) < 0.5 { 1 } else { -1 };
    let sides = [IVec2::new(d.y, d.x) * flip, IVec2::new(d.y, d.x) * -flip];
    let out = |at: IVec2, s: i32| Step { at, storey: s, what: What::Out(kind) };
    match kind {
        // Along the face, 3-8 cells, outside it, then back in.
        Kind::Ledge => sides.into_iter().find_map(|t| {
            let len = 3 + (r(2) * 6.0) as i32;
            let along = (0..=len).map(|i| c + t * i);
            if !along.clone().all(|a| cells.face(a).is_some() && cells.void(a + d) && free(a, s) && free(a + d, s)) {
                return None;
            }
            let exit = c + t * len - d;
            (cells.inside(exit) && free(exit, s)).then(|| Leg {
                steps: along.clone().map(|a| out(a, s)).collect(),
                takes: along.flat_map(|a| [(a, s), (a, s + 1), (a + d, s), (a + d, s + 1)]).chain([(exit, s), (exit, s + 1)]).collect(),
                exit: (exit, s),
            })
        }),
        // Straight over the chasm (2-28 cells of it: across the trunk where
        // it narrows) to the rock across, and into it.
        Kind::Bridge => {
            let mut o = c + d;
            let mut span = Vec::new();
            while cells.void(o) && span.len() < 29 {
                span.push(o);
                o += d;
            }
            let exit = o + d;
            if !(2..=28).contains(&span.len()) || cells.face(o).is_none() || !cells.inside(exit) || !span.iter().chain([&c, &o, &exit]).all(|&a| free(a, s)) {
                return None;
            }
            Some(Leg {
                steps: [c].iter().chain(&span).chain([&o]).map(|&a| out(a, s)).collect(),
                takes: [c].iter().chain(&span).chain([&o, &exit]).flat_map(|&a| [(a, s - 1), (a, s), (a, s + 1)]).collect(),
                exit: (exit, s),
            })
        }
        // A stepwell two cells along the face, its front glass, 3-5 storeys
        // inside it, out by its far end.
        Kind::Glass => sides.into_iter().find_map(|t| {
            let c2 = c + t;
            let e = s + (3 + (r(3) * 3.0) as i32) * toward;
            let exit = c2 - d;
            let room = [c, c2];
            if e < 1 || !room.iter().all(|&a| cells.face(a).is_some() && cells.void(a + d) && (s.min(e)..=s.max(e)).all(|s| free(a, s))) || !cells.inside(exit) || !free(exit, e) {
                return None;
            }
            Some(Leg {
                steps: vec![out(c, s), out(c2, e)],
                takes: room.iter().flat_map(|&a| (s.min(e)..=s.max(e) + 1).map(move |s| (a, s))).chain([(exit, e), (exit, e + 1)]).collect(),
                exit: (exit, e),
            })
        }),
        // Out over the chasm a cell, back, a cell along, and in.
        Kind::Balcony => sides.into_iter().find_map(|t| {
            let (b, c2) = (c + d, c + t);
            let exit = c2 - d;
            if !(cells.void(b) && cells.face(c2).is_some() && cells.inside(exit) && [b, c2, exit].iter().all(|&a| free(a, s))) {
                return None;
            }
            Some(Leg { steps: vec![out(c, s), out(b, s), out(c2, s)], takes: [c, b, c2, exit].iter().flat_map(|&a| [(a, s), (a, s + 1)]).collect(), exit: (exit, s) })
        }),
    }
}

/// A stretch inside the rock from any of `from` at storey `s`, level but
/// for one flight of `climb` storeys (down if more than 0, a storey a cell)
/// somewhere along it, to a cell `end` says will do (searching no further
/// than `reach` cells): its steps (from where it leaves), what it takes,
/// and its last cell and storey. Where it may go: cells inside the rock,
/// nothing taken there near the storeys it passes; and the cells `end`
/// takes.
fn stretch(cells: &Cells, taken: &Taken, from: &[IVec2], s: i32, climb: i32, reach: i32, end: &dyn Fn(IVec2, i32) -> bool, pick: &dyn Fn(&[(IVec2, i32)]) -> Vec<IVec2>) -> Vec<(Vec<IVec2>, i32)> {
    let span = (s.min(s + climb) - 2, s.max(s + climb) + 2);
    let ix = |c: IVec2| (c.x + c.y * N) as usize;
    let mut came = vec![None; (N * N) as usize];
    let mut dist = vec![i32::MAX; (N * N) as usize];
    let mut queue = VecDeque::new();
    for &f in from {
        dist[ix(f)] = 0;
        queue.push_back(f);
    }
    let mut ends = Vec::new();
    while let Some(c) = queue.pop_front() {
        if dist[ix(c)] >= reach {
            continue;
        }
        for d in DIRS {
            let n = c + d;
            if !cells.rock(n) || dist[ix(n)] != i32::MAX {
                continue;
            }
            let k = dist[ix(c)] + 1;
            if end(n, k) {
                dist[ix(n)] = k;
                came[ix(n)] = Some(c);
                ends.push((n, k));
            } else if cells.inside(n) && !taken.any(n, span) {
                dist[ix(n)] = k;
                came[ix(n)] = Some(c);
                queue.push_back(n);
            }
        }
    }
    pick(&ends)
        .into_iter()
        .map(|e| {
            let mut path = vec![e];
            while let Some(p) = came[ix(*path.last().unwrap())] {
                path.push(p);
            }
            path.reverse();
            (path, s)
        })
        .collect()
}

/// The steps and what they take of a stretch along `path` from storey `s`,
/// `climb` storeys in one flight somewhere along it (`r` where), but its
/// last cell (left to what comes after).
fn flight(path: &[IVec2], s: i32, climb: i32, r: f32) -> Option<(Vec<Step>, Vec<(IVec2, i32)>, i32)> {
    let k = path.len() as i32 - 1;
    let n = climb.abs();
    if n > k - 1 {
        return None;
    }
    let at = 1 + (r * (k - 1 - n) as f32) as i32;
    let storey = |i: i32| s + climb.signum() * (i - at + 1).clamp(0, n);
    let steps = (0..k).map(|i| Step { at: path[i as usize], storey: storey(i), what: if i > 0 && storey(i) != storey(i - 1) { What::Stair } else { What::In } }).collect();
    let takes = (1..k).flat_map(|i| [(path[i as usize], storey(i) - 1), (path[i as usize], storey(i)), (path[i as usize], storey(i) + 1)]).collect();
    Some((steps, takes, storey(k)))
}

/// A leg from any of `from` at storey `s`: inside the rock to a face 6-30
/// cells on (the nearer `goal` the likelier), `climb` storeys on the way,
/// and out there as `kind`.
fn leg(cells: &Cells, taken: &Taken, from: &[IVec2], s: i32, climb: i32, kind: Kind, goal: Vec2, toward: i32, r: &dyn Fn(i32) -> f32) -> Option<Leg> {
    let key = |c: &IVec2| (c.as_vec2() - goal).length() + 25.0 * r(c.x * 977 + c.y);
    let pick = |ends: &[(IVec2, i32)]| {
        let mut e: Vec<IVec2> = ends.iter().map(|e| e.0).collect();
        e.sort_by(|a, b| key(a).total_cmp(&key(b)));
        e.truncate(40);
        e
    };
    let span = (s.min(s + climb) - 2, s.max(s + climb) + 2);
    let faces = stretch(cells, taken, from, s, climb, 30, &|n, k| k >= 6 && cells.face(n).is_some() && !taken.any(n, span), &pick);
    faces.into_iter().find_map(|(path, s)| {
        let c = *path.last().unwrap();
        let (mut steps, mut takes, e) = flight(&path, s, climb, r(path.len() as i32 * 31 + c.x))?;
        let out = exposure(cells, taken, c, cells.face(c)?, e, kind, toward, r)?;
        steps.extend(out.steps);
        takes.extend(out.takes);
        Some(Leg { steps, takes, exit: out.exit })
    })
}

/// The last leg, from any of `from` at storey `s`: inside the rock into
/// place `to`, at the storey of it nearest, within 40 cells.
fn arrive(cells: &Cells, taken: &Taken, from: &[IVec2], s: i32, to: &Place, r: f32) -> Option<Leg> {
    let t = to.nearest(s);
    let pick = |ends: &[(IVec2, i32)]| ends.iter().min_by_key(|e| e.1).map(|e| vec![e.0]).unwrap_or_default();
    let (path, s) = stretch(cells, taken, from, s, t - s, 40, &|n, _| to.contains(n), &pick).pop()?;
    let (mut steps, takes, e) = flight(&path, s, t - s, r)?;
    let last = *path.last().unwrap();
    steps.push(Step { at: last, storey: e, what: What::In });
    Some(Leg { steps, takes, exit: (last, e) })
}

/// A path from place `a` to place `b`: legs out onto the chasm (one at
/// least, unless they are near), then in to `b`; taken as it goes, given
/// back where it comes to nothing. Its steps, what it took, and the kind
/// of its last exposure.
fn path(cells: &Cells, taken: &mut Taken, a: &Place, b: &Place, last: Option<Kind>, r: &dyn Fn(i32) -> f32) -> Option<(Vec<Step>, Vec<(IVec2, i32)>, Option<Kind>)> {
    let goal = b.centre();
    let mut from: Vec<IVec2> = a.cells().collect();
    let mut s = a.nearest((b.storeys.0 + b.storeys.1) / 2);
    let (mut steps, mut took, mut last) = (Vec::new(), Vec::new(), last);
    let near = (a.centre() - b.centre()).length() < 14.0;
    let fail = |taken: &mut Taken, took: &[(IVec2, i32)]| {
        for &(c, s) in took {
            taken.0.remove(&(c.x, c.y, s));
        }
    };
    let take = |taken: &mut Taken, took: &mut Vec<(IVec2, i32)>, l: &[(IVec2, i32)]| {
        for &(c, s) in l {
            if taken.0.insert((c.x, c.y, s)) {
                took.push((c, s));
            }
        }
    };
    for n in 0..6 {
        if (near || n > 0)
            && let Some(l) = arrive(cells, taken, &from, s, b, r(n * 7 + 1))
        {
            take(taken, &mut took, &l.takes);
            steps.extend(l.steps);
            return Some((steps, took, last));
        }
        // Out, as the kind (but the last) that comes out nearest `b`, give
        // or take; half way to its storeys on the way.
        let rest = b.nearest(s) - s;
        let toward = if rest != 0 { rest.signum() } else if r(n * 7 + 2) < 0.5 { 1 } else { -1 };
        let climb = (rest / 2).clamp(-8, 8);
        let found = KINDS
            .into_iter()
            .filter(|&k| Some(k) != last)
            .filter_map(|k| leg(cells, taken, &from, s, climb, k, goal, toward, &|x| r(n * 1013 + k as i32 * 101 + x)).map(|l| (l, k)))
            .min_by(|x, y| {
                let key = |(l, k): &(Leg, Kind)| (l.exit.0.as_vec2() - goal).length() + 15.0 * r(n * 7 + *k as i32);
                key(x).total_cmp(&key(y))
            });
        let Some((l, k)) = found else { break };
        take(taken, &mut took, &l.takes);
        steps.extend(l.steps);
        (from, s, last) = (vec![l.exit.0], l.exit.1, Some(k));
    }
    fail(taken, &took);
    None
}

/// The paths through a plan: the places, the paths (from, to, steps;
/// whether part of the tree), the place the spine gets down to, and the
/// spine (paths, in order, from the entrance down to it).
pub(super) struct Network {
    pub places: Vec<Place>,
    pub paths: Vec<(usize, usize, Vec<Step>, bool)>,
    pub bottom: usize,
    pub spine: Vec<usize>,
}

impl Network {
    /// Each place's doors: where each path leaves it or comes into it.
    pub(super) fn doors(&self) -> Vec<Vec<Door>> {
        let mut doors = vec![Vec::new(); self.places.len()];
        for (a, b, steps, _) in &self.paths {
            let n = steps.len();
            for (place, (inside, outside)) in [(*a, (steps[0], steps[1])), (*b, (steps[n - 1], steps[n - 2]))] {
                doors[place].push(Door { cell: inside.at, out: outside.at - inside.at, storey: inside.storey });
            }
        }
        doors
    }
}

/// The paths through `plan`: the entrance on the rim beside the trunk's near
/// end (place 0), places scattered through the rock to `DEPTH` (16-32 m
/// across, 1-4 storeys, at least 120 m from each other, within 160 m of the
/// chasm), joined: first the
/// spine, a chain of places from the entrance down to a deep one most of a
/// kilometre off, each deeper than the last and no further from it; then
/// the rest, the nearest pairs not yet joined first.
pub(super) fn network(plan: &Plan, seed: u32) -> Network {
    // (Where the entrance can be: a few spots along the trunk's near end;
    // the one whose spine gets deepest, the first that gets near the
    // bottom.)
    let cells = Cells(plan);
    let trunk = plan.branches[0];
    let mut best: Option<Network> = None;
    for x in (16..80).step_by(12) {
        let Some(start) = (0..trunk.a.y).rev().find(|&z| cells.rock(IVec2::new(x, z))).map(|z| IVec2::new(x, z - 2)).filter(|&c| cells.inside(c)) else { continue };
        let net = joined(plan, seed, start);
        let deep = |n: &Network| n.places[n.bottom].storeys.1;
        if best.as_ref().is_none_or(|b| deep(&net) > deep(b)) {
            best = Some(net);
        }
        if best.as_ref().is_some_and(|b| deep(b) >= DEPTH * 4 / 5) {
            break;
        }
    }
    best.unwrap_or_else(|| joined(plan, seed, IVec2::splat(N / 2)))
}

/// The paths through `plan` from an entrance at `start`.
fn joined(plan: &Plan, seed: u32, start: IVec2) -> Network {
    let cells = Cells(plan);
    let r = |a: i32, b: i32| hash01(seed as i32, a, b, 0x5d2);
    let mut places = vec![Place { at: start, size: IVec2::ONE, storeys: (0, 0) }];
    // (How far each cell is from a face: places only within 20 cells, 160
    // m, of one; further in, the rock is not the chasm's.)
    let mut to_face = vec![i32::MAX; (N * N) as usize];
    let mut queue: VecDeque<IVec2> = (0..N).flat_map(|z| (0..N).map(move |x| IVec2::new(x, z))).filter(|&c| cells.face(c).is_some()).collect();
    for c in &queue {
        to_face[(c.x + c.y * N) as usize] = 0;
    }
    while let Some(c) = queue.pop_front() {
        for d in DIRS {
            let n = c + d;
            if cells.rock(n) && to_face[(n.x + n.y * N) as usize] == i32::MAX {
                to_face[(n.x + n.y * N) as usize] = to_face[(c.x + c.y * N) as usize] + 1;
                queue.push_back(n);
            }
        }
    }
    for t in 0..8000 {
        if places.len() >= 50 {
            break;
        }
        let size = IVec2::new(2 + (r(t, 1) * 3.0) as i32, 2 + (r(t, 2) * 3.0) as i32);
        let h = (r(t, 3) * 4.0) as i32;
        let top = 2 + (r(t, 4) * (DEPTH - 2 - h) as f32) as i32;
        let at = IVec2::new(4 + (r(t, 5) * (N - 12) as f32) as i32, 4 + (r(t, 6) * (N - 12) as f32) as i32);
        let p = Place { at, size, storeys: (top, top + h) };
        if p.cells().all(|c| cells.rock(c) && to_face[(c.x + c.y * N) as usize] <= 20) && places.iter().all(|o| o.gap(&p) >= 120.0) {
            places.push(p);
        }
    }
    let mut taken = Taken::default();
    for p in &places {
        for c in p.cells() {
            taken.0.extend((p.storeys.0 - 1..=p.storeys.1 + 1).map(|s| (c.x, c.y, s)));
        }
    }
    // Pairs worth a path: within 64 cells across (half a kilometre) and 30
    // storeys; the nearest first (the far ones only to join what the near
    // ones leave apart).
    let mut pairs: Vec<(usize, usize, f32)> = Vec::new();
    for i in 0..places.len() {
        for j in i + 1..places.len() {
            let (a, b) = (&places[i], &places[j]);
            let across = (a.centre() - b.centre()).length();
            let down = (b.storeys.0 - a.storeys.1).max(a.storeys.0 - b.storeys.1).max(0) as f32;
            if across <= 64.0 && down <= 30.0 {
                pairs.push((i, j, a.gap(b)));
            }
        }
    }
    pairs.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut group: Vec<usize> = (0..places.len()).collect();
    fn root(g: &mut [usize], i: usize) -> usize {
        if g[i] == i { i } else { let r = root(g, g[i]); g[i] = r; r }
    }
    let mut paths: Vec<(usize, usize, Vec<Step>, bool)> = Vec::new();
    let mut last = None;
    // The spine: to the deep place (in the last fifth of the depth) nearest
    // 110 cells across from the entrance, place by place.
    let across = |a: usize, b: usize| (places[a].centre() - places[b].centre()).length();
    let mut bottom = (1..places.len()).filter(|&i| places[i].storeys.1 >= DEPTH * 4 / 5).min_by(|&a, &b| (across(0, a) - 110.0).abs().total_cmp(&(across(0, b) - 110.0).abs())).unwrap_or(0);
    let mut spine = Vec::new();
    let mut at = 0;
    while at != bottom && spine.len() < 40 {
        // (Deeper and near; failing that, no shallower and further.)
        let found = [(2, 40.0, 8.0), (0, 64.0, 16.0)].into_iter().find_map(|(deeper, reach, back)| {
            let mut next: Vec<usize> = (1..places.len())
                .filter(|&q| root(&mut group, q) != root(&mut group, 0) && places[q].storeys.0 >= places[at].storeys.0 + deeper && across(at, q) <= reach && across(q, bottom) <= across(at, bottom) + back)
                .collect();
            next.sort_by(|&a, &b| (places[at].gap(&places[a]) + 0.7 * across(a, bottom) * CELL).total_cmp(&(places[at].gap(&places[b]) + 0.7 * across(b, bottom) * CELL)));
            next.into_iter().take(8).find_map(|q| path(&cells, &mut taken, &places[at], &places[q], last, &|x| r(q as i32 * 7919 + 1, x)).map(|p| (q, p)))
        });
        let Some((q, (steps, _, l))) = found else { break };
        let (ga, gq) = (root(&mut group, at), root(&mut group, q));
        group[gq] = ga;
        spine.push(paths.len());
        paths.push((at, q, steps, true));
        last = l.or(last);
        at = q;
    }
    bottom = at;
    // The tree: the nearest pairs not yet joined, if a path fits.
    for (n, &(i, j, _)) in pairs.iter().enumerate() {
        let (gi, gj) = (root(&mut group, i), root(&mut group, j));
        if gi == gj {
            continue;
        }
        if let Some((steps, _, l)) = path(&cells, &mut taken, &places[i], &places[j], last, &|x| r(n as i32 * 7919 + 3, x)) {
            group[gi] = gj;
            paths.push((i, j, steps, true));
            last = l.or(last);
        }
    }
    // Loops: pairs whose way round by the tree is more than three times as
    // long as they are apart; one for every six places at most.
    let lengths = |paths: &[(usize, usize, Vec<Step>, bool)]| paths.iter().map(|p| walked(&p.2).0).collect::<Vec<f32>>();
    let mut loops = 0;
    for (n, &(i, j, gap)) in pairs.iter().enumerate() {
        if loops >= places.len() / 6 {
            break;
        }
        let round = shortest(places.len(), &paths, &lengths(&paths), i).0[j];
        if round < gap * 3.0 || paths.iter().any(|p| (p.0, p.1) == (i, j)) {
            continue;
        }
        if let Some((steps, _, l)) = path(&cells, &mut taken, &places[i], &places[j], last, &|x| r(n as i32 * 7919 + 5, x)) {
            paths.push((i, j, steps, false));
            last = l.or(last);
            loops += 1;
        }
    }
    Network { places, paths, bottom, spine }
}

/// The shortest ways from place `from` along the paths (each `lengths`
/// long): how far to each place, and the place and path each is come to
/// by.
fn shortest(n: usize, paths: &[(usize, usize, Vec<Step>, bool)], lengths: &[f32], from: usize) -> (Vec<f32>, Vec<Option<(usize, usize)>>) {
    let mut dist = vec![f32::MAX; n];
    let mut came = vec![None; n];
    let mut done = vec![false; n];
    dist[from] = 0.0;
    while let Some(i) = (0..n).filter(|&i| !done[i] && dist[i] < f32::MAX).min_by(|&a, &b| dist[a].total_cmp(&dist[b])) {
        done[i] = true;
        for (k, p) in paths.iter().enumerate() {
            let o = if p.0 == i { p.1 } else if p.1 == i { p.0 } else { continue };
            if dist[i] + lengths[k] < dist[o] {
                dist[o] = dist[i] + lengths[k];
                came[o] = Some((i, k));
            }
        }
    }
    (dist, came)
}

/// The rooms the places of the seed's network must be (`--opt fit`): up to
/// `count` of them, those whose doors are furthest apart in storeys first,
/// then those with the most doors. A place's cells are 8 m across, its
/// storeys 6 m high, its floor slab a metre under its lowest storey's
/// floor; a door is in the middle of its cell's side.
pub(in crate::chasm) fn specs(seed: u32, count: usize) -> Vec<Spec> {
    let plan = tree(seed);
    let net = network(&plan, seed);
    let doors = net.doors();
    let mut out: Vec<(i32, usize, Spec)> = Vec::new();
    for (i, (p, ds)) in net.places.iter().zip(&doors).enumerate() {
        if ds.is_empty() || i == 0 {
            continue;
        }
        let size = p.size.as_vec2() * CELL;
        let level = |s: i32| 1.0 + (p.storeys.1 - s) as f32 * STOREY;
        let doors = ds
            .iter()
            .map(|d| {
                let along = |c: i32, a: i32| (c - a) as f32 * CELL + CELL * 0.5;
                let (wall, u) = match (d.out.x, d.out.y) {
                    (-1, _) => (0, along(d.cell.y, p.at.y)),
                    (1, _) => (1, along(d.cell.y, p.at.y)),
                    (_, -1) => (2, along(d.cell.x, p.at.x)),
                    _ => (3, along(d.cell.x, p.at.x)),
                };
                (wall, u, level(d.storey))
            })
            .collect();
        let spread = ds.iter().map(|d| d.storey).max().unwrap() - ds.iter().map(|d| d.storey).min().unwrap();
        let height = 1.0 + (p.storeys.1 - p.storeys.0 + 1) as f32 * STOREY;
        out.push((spread, ds.len(), Spec { size, height, doors, name: format!("place {i} (storeys {}-{})", p.storeys.0, p.storeys.1) }));
    }
    out.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    out.into_iter().take(count).map(|o| o.2).collect()
}

/// How far it is to walk along steps (m), all of it and inside the rock.
fn walked(steps: &[Step]) -> (f32, f32) {
    steps.windows(2).fold((0.0, 0.0), |(all, inside), p| {
        let h = (p[1].at - p[0].at).as_vec2().length() * CELL;
        let v = (p[1].storey - p[0].storey) as f32 * STOREY;
        let l = (h * h + v * v).sqrt();
        (all + l, inside + if matches!(p[1].what, What::Out(_)) { 0.0 } else { l })
    })
}

/// `--check network`: the paths through the plan for seeds 1-50: how many
/// places, how many reached from the entrance, paths (of the tree, loops),
/// how far to walk them all and how much of that inside, the exposures,
/// the spine (how long, how deep), how many ends each place has, and how
/// much of the chasm's faces lie near a path; seeds 1-8 written out to draw
/// (`screenshots/plan/network_N.txt`).
pub fn check() -> bool {
    let _ = std::fs::create_dir_all("screenshots/plan");
    let mut ok = true;
    let mut ends_all = [0usize; 6];
    let mut spread_all = [0usize; 5];
    let (mut touching, mut clashes, mut bad, mut all_places) = (0, 0, 0, 0);
    for seed in 1..=50 {
        let plan = tree(seed);
        let cells = Cells(&plan);
        let net = network(&plan, seed);
        let lengths: Vec<f32> = net.paths.iter().map(|p| walked(&p.2).0).collect();
        let (dist, _) = shortest(net.places.len(), &net.paths, &lengths, 0);
        let reached = dist.iter().filter(|d| **d < f32::MAX).count();
        let (all, inside) = net.paths.iter().map(|p| walked(&p.2)).fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
        let mut kinds = [0; 4];
        for p in &net.paths {
            for (i, s) in p.2.iter().enumerate() {
                if let What::Out(k) = s.what
                    && (i == 0 || p.2[i - 1].what != s.what)
                {
                    kinds[k as usize] += 1;
                }
            }
        }
        let spine: f32 = net.spine.iter().map(|&k| lengths[k]).sum();
        let mut ends = vec![0usize; net.places.len()];
        for p in &net.paths {
            ends[p.0] += 1;
            ends[p.1] += 1;
        }
        for &e in &ends {
            ends_all[e.min(5)] += 1;
        }
        // (What the places ask of rooms: how far apart in storeys their
        // doors are, whether they touch the chasm, doors that clash (one
        // cell and way at one storey), doors outside a place or its
        // storeys.)
        for (p, doors) in net.places.iter().zip(net.doors()) {
            if doors.is_empty() {
                continue;
            }
            let (lo, hi) = doors.iter().fold((i32::MAX, i32::MIN), |(lo, hi), d| (lo.min(d.storey), hi.max(d.storey)));
            spread_all[((hi - lo) as usize).min(4)] += 1;
            if p.cells().any(|c| cells.face(c).is_some()) {
                touching += 1;
            }
            clashes += doors.iter().enumerate().filter(|(i, d)| doors[..*i].contains(d)).count();
            bad += doors.iter().filter(|d| !p.contains(d.cell) || p.contains(d.cell + d.out) || d.out.abs().element_sum() != 1 || !(p.storeys.0..=p.storeys.1).contains(&d.storey)).count();
            all_places += 1;
        }
        // (Faces within 12 cells across of a step of a path.)
        let near: HashSet<(i32, i32)> = net.paths.iter().flat_map(|p| p.2.iter().map(|s| (s.at.x / 6, s.at.y / 6))).collect();
        let faces: Vec<IVec2> = (0..N).flat_map(|z| (0..N).map(move |x| IVec2::new(x, z))).filter(|&c| cells.face(c).is_some()).collect();
        let covered = faces.iter().filter(|c| (-2..=2).any(|i| (-2..=2).any(|j| near.contains(&(c.x / 6 + i, c.y / 6 + j))))).count();
        let tree_paths = net.paths.iter().filter(|p| p.3).count();
        eprintln!(
            "network {seed}: {} places, {} reached; {} paths ({} loops); {:.1} km walked, {:.0}% inside; exposures ledge {}, bridge {}, glass {}, balcony {}; spine {:.0} m to storey {}; faces near a path {:.0}%",
            net.places.len(),
            reached,
            net.paths.len(),
            net.paths.len() - tree_paths,
            all / 1000.0,
            inside / all.max(1.0) * 100.0,
            kinds[0],
            kinds[1],
            kinds[2],
            kinds[3],
            spine,
            net.places[net.bottom].storeys.1,
            covered as f32 / faces.len().max(1) as f32 * 100.0,
        );
        ok &= reached * 10 >= net.places.len() * 8;
        if seed <= 8 {
            let mut text = String::new();
            for p in &net.places {
                text += &format!("P {} {} {} {} {} {}\n", p.at.x, p.at.y, p.size.x, p.size.y, p.storeys.0, p.storeys.1);
            }
            for (k, p) in net.paths.iter().enumerate() {
                text += &format!("L {} {} {} {}\n", p.0, p.1, if p.3 { "tree" } else { "loop" }, if net.spine.contains(&k) { "spine" } else { "-" });
                for s in &p.2 {
                    text += &format!("S {} {} {} {:?}\n", s.at.x, s.at.y, s.storey, s.what);
                }
            }
            let _ = std::fs::write(format!("screenshots/plan/network_{seed}.txt"), text);
        }
    }
    eprintln!(
        "network: places by ends (0, 1, 2, 3, 4, 5+): {ends_all:?}; by storeys between their doors (0, 1, 2, 3, 4+): {spread_all:?}; touching the chasm {touching} of {all_places}; doors clashing {clashes}; doors misplaced {bad}{}",
        if ok { "" } else { "; FAILED: a seed reached fewer than 80% of its places" }
    );
    ok && bad == 0
}
