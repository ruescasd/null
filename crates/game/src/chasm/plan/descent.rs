//! The way down through the plan, as a blockout (step 3a of the interior way
//! down): on the plan's grid, storey by storey, from the rim down. Legs, each
//! a stretch inside the rock (a cell back from the faces at least, stairs
//! down along it) and then an exposure where it meets a face: a ledge along
//! the face and back in, a bridge over a branch into the rock across it, a
//! glass stepwell against the void, a balcony out over it; never the same
//! kind twice running. Nothing of the way passes within a few storeys of
//! another part of it; where no leg fits, back a leg and try another.

use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::{CELL, N, Plan, tree};

/// A storey's height (m): the way's floors are whole storeys below the rim.
const STOREY: f32 = 6.0;

/// What comes out onto the chasm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Ledge,
    Bridge,
    Glass,
    Balcony,
}

const KINDS: [Kind; 4] = [Kind::Ledge, Kind::Bridge, Kind::Glass, Kind::Balcony];

/// What a step of the way is: inside the rock, on, or a stair down into the
/// storey below; or out on the chasm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum What {
    In,
    Stair,
    Out(Kind),
}

/// A step of the way: a cell, the storey (counted down from the rim), and
/// what it is.
#[derive(Clone, Copy, Debug)]
pub(super) struct Step {
    pub at: IVec2,
    pub storey: i32,
    pub what: What,
}

pub(super) struct Descent {
    pub steps: Vec<Step>,
    /// Whether it got as deep as it was to go.
    pub reached: bool,
    /// Where it makes for.
    pub goal: IVec2,
}

const DIRS: [IVec2; 4] = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];

/// What the plan's cells are to the way: the chasm; rock (not by the
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

/// What the way takes: cells, each at a storey.
#[derive(Default)]
struct Taken(HashSet<(i32, i32, i32)>);

impl Taken {
    fn any(&self, c: IVec2, s: (i32, i32)) -> bool {
        (s.0..=s.1).any(|s| self.0.contains(&(c.x, c.y, s)))
    }
}

/// A leg: its steps, what they take, and where it leaves the way (a cell
/// inside the rock, at a storey).
struct Leg {
    steps: Vec<Step>,
    takes: Vec<(IVec2, i32)>,
    exit: (IVec2, i32),
}

/// The exposure of `kind` at face `c` (the chasm `d` from it) at storey
/// `s`, if it fits: its steps, what it takes, and where it goes back into
/// the rock.
fn exposure(cells: &Cells, taken: &Taken, c: IVec2, d: IVec2, s: i32, kind: Kind, r: &dyn Fn(i32) -> f32) -> Option<Leg> {
    let free = |c: IVec2, s: i32| !taken.any(c, (s - 2, s + 3));
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
        // Straight over the chasm (2-16 cells of it) to the rock across, and
        // into it.
        Kind::Bridge => {
            let mut o = c + d;
            let mut span = Vec::new();
            while cells.void(o) && span.len() < 17 {
                span.push(o);
                o += d;
            }
            let exit = o + d;
            if !(2..=16).contains(&span.len()) || cells.face(o).is_none() || !cells.inside(exit) || !span.iter().chain([&c, &o, &exit]).all(|&a| free(a, s)) {
                return None;
            }
            Some(Leg {
                steps: [c].iter().chain(&span).chain([&o]).map(|&a| out(a, s)).collect(),
                takes: [c].iter().chain(&span).chain([&o, &exit]).flat_map(|&a| [(a, s - 1), (a, s), (a, s + 1)]).collect(),
                exit: (exit, s),
            })
        }
        // A stepwell two cells along the face, its front glass, down 3-5
        // storeys inside it, out at its foot by its far end.
        Kind::Glass => sides.into_iter().find_map(|t| {
            let c2 = c + t;
            let g = 3 + (r(3) * 3.0) as i32;
            let exit = c2 - d;
            let room = [c, c2];
            if !room.iter().all(|&a| cells.face(a).is_some() && cells.void(a + d) && (s..=s + g).all(|s| free(a, s))) || !cells.inside(exit) || !free(exit, s + g) {
                return None;
            }
            Some(Leg {
                steps: vec![out(c, s), out(c2, s + g)],
                takes: room.iter().flat_map(|&a| (s..=s + g + 1).map(move |s| (a, s))).chain([(exit, s + g), (exit, s + g + 1)]).collect(),
                exit: (exit, s + g),
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

/// A leg from `from` at storey `s`: inside the rock to a face 6-30 cells on
/// (the nearer `goal` the likelier), down `drop` storeys on the way in one
/// flight (or as many as there are cells for), and out there as `kind`.
fn leg(cells: &Cells, taken: &Taken, from: IVec2, s: i32, drop: i32, kind: Kind, goal: IVec2, r: &dyn Fn(i32) -> f32) -> Option<Leg> {
    let span = (s - 2, s + drop + 9);
    // Inside cells and the faces beyond them, by how far.
    let mut came = vec![None; (N * N) as usize];
    let mut dist = vec![i32::MAX; (N * N) as usize];
    let ix = |c: IVec2| (c.x + c.y * N) as usize;
    dist[ix(from)] = 0;
    let mut queue = VecDeque::from([from]);
    let mut faces = Vec::new();
    while let Some(c) = queue.pop_front() {
        if dist[ix(c)] >= 30 {
            continue;
        }
        for d in DIRS {
            let n = c + d;
            if !cells.rock(n) || dist[ix(n)] != i32::MAX || taken.any(n, span) {
                continue;
            }
            if cells.inside(n) {
                dist[ix(n)] = dist[ix(c)] + 1;
                came[ix(n)] = Some(c);
                queue.push_back(n);
            } else if cells.face(n).is_some() && dist[ix(c)] + 1 >= 6 {
                dist[ix(n)] = dist[ix(c)] + 1;
                came[ix(n)] = Some(c);
                faces.push(n);
            }
        }
    }
    // (Those nearer the goal first, give or take 25 cells.)
    let key = |c: &IVec2| (*c - goal).as_vec2().length() + 25.0 * r(c.x * 977 + c.y);
    faces.sort_by(|a, b| key(a).total_cmp(&key(b)));
    faces.into_iter().take(40).find_map(|c| {
        let mut path = vec![c];
        while let Some(p) = came[ix(*path.last().unwrap())] {
            path.push(p);
        }
        path.reverse();
        // (Level but for one flight down somewhere along it, a storey a
        // cell.)
        let k = path.len() as i32 - 1;
        let stairs = drop.min(k - 1);
        let at = 1 + (r(path.len() as i32 * 31 + c.x) * (k - 1 - stairs) as f32) as i32;
        let storey = |i: i32| s + (i - at + 1).clamp(0, stairs);
        let out = exposure(cells, taken, c, cells.face(c)?, storey(k), kind, r)?;
        let inside = (1..k).map(|i| Step { at: path[i as usize], storey: storey(i), what: if storey(i) > storey(i - 1) { What::Stair } else { What::In } });
        let takes = (1..k).flat_map(|i| [(path[i as usize], storey(i) - 1), (path[i as usize], storey(i)), (path[i as usize], storey(i) + 1)]);
        Some(Leg { steps: inside.chain(out.steps).collect(), takes: takes.chain(out.takes).collect(), exit: out.exit })
    })
}

/// The way down through `plan` to storey `depth`, from inside the rock on
/// the trunk's near side at its near end, making for the tip of a branch
/// far from there (more than a kilometre).
pub(super) fn descend(plan: &Plan, seed: u32, depth: i32) -> Descent {
    let cells = Cells(plan);
    let r = |a: i32, b: i32| hash01(seed as i32, a, b, 0x5d1);
    let trunk = plan.branches[0];
    let start = (16..N - 16)
        .find_map(|x| {
            let z = (0..trunk.a.y).rev().find(|&z| cells.rock(IVec2::new(x, z)))?;
            let c = IVec2::new(x, z - 2);
            cells.inside(c).then_some(c)
        })
        .unwrap_or(IVec2::splat(N / 2));
    // (On the trunk's near side, as the start: across it is too far to
    // bridge.)
    let far: Vec<IVec2> = plan.branches.iter().skip(1).map(|b| b.b).filter(|b| b.y < trunk.a.y && (*b - start).as_vec2().length() > 1000.0 / CELL).collect();
    let goal = if far.is_empty() { IVec2::new(N - 16, trunk.a.y) } else { far[(r(0, 77) * far.len() as f32) as usize % far.len()] };
    let mut steps = vec![Step { at: start, storey: 0, what: What::Stair }];
    let mut taken = Taken::default();
    taken.0.extend([(start.x, start.y, 0), (start.x, start.y, 1)]);
    let mut legs: Vec<(usize, Vec<(IVec2, i32)>, Kind)> = Vec::new();
    let (mut at, mut storey) = (start, 0);
    let (mut salt, mut stuck, mut tries) = (0, 0, 0);
    while storey < depth && tries < 600 {
        tries += 1;
        let k = legs.len() as i32 * 101 + salt * 7;
        let last = legs.last().map(|l| l.2);
        // (Of the kinds but the last, the one that comes out nearest the
        // goal, give or take 15 cells.)
        let drop = 3 + (r(k, 9) * 6.0) as i32;
        let found = KINDS
            .into_iter()
            .filter(|&x| Some(x) != last)
            .filter_map(|kind| leg(&cells, &taken, at, storey, drop, kind, goal, &|b| r(k * 13 + kind as i32, b)).map(|l| (l, kind)))
            .min_by(|a, b| {
                let key = |(l, kind): &(Leg, Kind)| (l.exit.0 - goal).as_vec2().length() + 15.0 * r(k, *kind as i32);
                key(a).total_cmp(&key(b))
            });
        match found {
            Some((l, kind)) => {
                legs.push((steps.len(), l.takes.clone(), kind));
                for &(c, s) in &l.takes {
                    taken.0.insert((c.x, c.y, s));
                }
                steps.extend(l.steps);
                steps.push(Step { at: l.exit.0, storey: l.exit.1, what: What::In });
                (at, storey) = l.exit;
                stuck = 0;
            }
            None => {
                salt += 1;
                stuck += 1;
                // Stuck: back a leg.
                if stuck > 3
                    && let Some((n, takes, _)) = legs.pop()
                {
                    steps.truncate(n);
                    for (c, s) in takes {
                        taken.0.remove(&(c.x, c.y, s));
                    }
                    let last = steps.last().unwrap();
                    (at, storey) = (last.at, last.storey);
                    stuck = 0;
                }
            }
        }
    }
    Descent { steps, reached: storey >= depth, goal }
}

impl Descent {
    /// How far it is to walk (m), all of it and inside the rock.
    fn lengths(&self) -> (f32, f32) {
        self.steps.windows(2).fold((0.0, 0.0), |(all, inside), p| {
            let h = (p[1].at - p[0].at).as_vec2().length() * CELL;
            let v = (p[1].storey - p[0].storey) as f32 * STOREY;
            let l = (h * h + v * v).sqrt();
            (all + l, inside + if matches!(p[1].what, What::Out(_)) { 0.0 } else { l })
        })
    }

    /// The exposures, in order, each with the storey it is at.
    fn exposures(&self) -> Vec<(Kind, i32)> {
        let mut out = Vec::new();
        for (i, s) in self.steps.iter().enumerate() {
            if let What::Out(k) = s.what
                && (i == 0 || self.steps[i - 1].what != s.what)
            {
                out.push((k, s.storey));
            }
        }
        out
    }
}

/// `--check descent`: the way down through the plan for seeds 1-50, to 60
/// storeys (360 m) below the rim: whether it gets there, how long it is,
/// how much of it inside, its exposures; seeds 1-8 written out as steps
/// (`screenshots/plan/descent_N.txt`: x, z, storey, what; the goal first)
/// to draw.
pub fn check() -> bool {
    let _ = std::fs::create_dir_all("screenshots/plan");
    let mut failed = Vec::new();
    for seed in 1..=50 {
        let plan = tree(seed);
        let d = descend(&plan, seed, 60);
        let (all, inside) = d.lengths();
        let ex = d.exposures();
        let count = |k: Kind| ex.iter().filter(|e| e.0 == k).count();
        eprintln!(
            "descent {seed}: {} to storey {}, ends {:.0} m from its goal, {:.0} m walked, {:.0}% inside, {} exposures (ledge {}, bridge {}, glass {}, balcony {}), one every {:.0} m inside",
            if d.reached { "reached" } else { "STUCK" },
            d.steps.last().map_or(0, |s| s.storey),
            d.steps.last().map_or(0.0, |s| (s.at - d.goal).as_vec2().length() * CELL),
            all,
            inside / all.max(1.0) * 100.0,
            ex.len(),
            count(Kind::Ledge),
            count(Kind::Bridge),
            count(Kind::Glass),
            count(Kind::Balcony),
            inside / ex.len().max(1) as f32,
        );
        if !d.reached {
            failed.push(seed);
        }
        if seed <= 8 {
            let steps: String = d.steps.iter().map(|s| format!("{} {} {} {:?}\n", s.at.x, s.at.y, s.storey, s.what)).collect();
            let text = format!("{} {} 0 Goal\n{steps}", d.goal.x, d.goal.y);
            let _ = std::fs::write(format!("screenshots/plan/descent_{seed}.txt"), text);
        }
    }
    eprintln!("descent: {} of 50 reached 360 m{}", 50 - failed.len(), if failed.is_empty() { String::new() } else { format!(" (stuck: {failed:?})") });
    failed.is_empty()
}
