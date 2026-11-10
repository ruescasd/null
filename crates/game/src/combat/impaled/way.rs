//! Where a brute lands, and its way there through the air (step 4; the
//! user, 2026-11-10: flight is its only way to move, but it can land, and
//! it attacks landed; like the blinkers, flying where they teleport).
//!
//! A spot: near you (`NEAR`), on its side of you before the far side, flat
//! under all its feet, in sight of you, apart from where others land. Its
//! way: straight there through the air if nothing is in the way, else up
//! over what is (higher each try), across and down; every leg swept with
//! its body (a ball) against what is built, and the ground under it felt
//! all along (out of the colliders' reach, the terrain is only heights).
//! Dense places (under decks, in the chasm's rooms) it stays out of, as it
//! finds no way in: by design for now, a 3D search if a place needs one.

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::{HEIGHT, feet};
use crate::ground::Ground;

/// How far from you it lands (m), and the distance it likes best: within
/// its lash's reach, its crash's edge on you if you stand still.
const NEAR: (f32, f32, f32) = (3.5, 8.0, 4.0);
/// Its feet: how far out the ground under them is felt (m), and how much
/// it may rise or fall within that (m).
const FOOT: f32 = 1.0;
const FLAT: f32 = 0.35;
/// How far apart brutes land (m).
const APART: f32 = 4.0;
/// Its body, swept along its way: a ball this wide about its middle (m;
/// its feet just inside it, its shoulders' cables outside).
pub(super) const BODY: f32 = 1.3;
/// How much higher than the higher end each way over tries (m); how far
/// apart the ground under a leg is felt (m), and how far its feet keep above
/// it (m).
const OVER: [f32; 6] = [0.0, 3.0, 6.0, 12.0, 24.0, 48.0];
const FELT: f32 = 2.0;
const CLEAR: f32 = 0.3;
/// How many spots it tries a way to, best first.
const TRIES: usize = 12;
/// No way from where it is (under a roof, in a pen): how far out, level with
/// it, it looks for ways out to start from (m), and how many it tries. (A
/// height that counts as shut in failed: the arena's pens are 21 m high.)
const OUT: [f32; 3] = [6.0, 12.0, 24.0];
const EXITS: usize = 4;

/// A way to land: the spot (on the ground), and the points its middle flies
/// through (the last but one above the spot at its hover, the last where it
/// stands on it).
pub(super) struct Way {
    pub spot: Vec3,
    pub points: Vec<Vec3>,
}

/// Why spots were turned down, counted: no ground, not flat, taken, out of
/// your sight, no way there (said when none is found).
#[derive(Default, Debug)]
pub(super) struct Refused {
    pub void: u32,
    pub rough: u32,
    pub taken: u32,
    pub unseen: u32,
    pub no_way: u32,
    /// The ways out tried, none from where it is.
    pub exits: usize,
}

/// A spot to land near `you` (your eye), and its way there from `from` (its
/// middle), its feet `hover` above the ground on the way down; `taken`,
/// where others land. Else why none was found (it tries again a moment
/// later).
pub(super) fn choose(ground: &Ground, from: Vec3, you: Vec3, hover: f32, taken: &[Vec3], seed: u32) -> Result<Way, Refused> {
    let mut refused = Refused::default();
    let mut spots = Vec::new();
    let toward = Vec2::new(from.x - you.x, from.z - you.z).normalize_or(Vec2::X);
    for i in 0..24 {
        let a = i as f32 / 24.0 * std::f32::consts::TAU + hash01(seed as i32, i, 0, 0x3a7) * 0.2;
        let dir = Vec2::from_angle(a);
        for r in [NEAR.0, NEAR.2, NEAR.1] {
            let q = Vec2::new(you.x, you.z) + dir * r;
            // (Best: short flights, its side of you, at the distance it
            // likes.)
            let score = Vec2::new(from.x, from.z).distance(q) + 4.0 * (1.0 - dir.dot(toward)) + (r - NEAR.2).abs();
            spots.push((score, q));
        }
    }
    spots.sort_by(|a, b| a.0.total_cmp(&b.0));
    // (The spots it could land on, best first.)
    let mut fit = Vec::new();
    for (_, q) in spots {
        let spot = match standing(ground, q, you.y) {
            Ok(spot) => spot,
            Err(void) => {
                if void {
                    refused.void += 1;
                } else {
                    refused.rough += 1;
                }
                continue;
            }
        };
        if taken.iter().any(|t| t.xz().distance(spot.xz()) < APART) {
            refused.taken += 1;
            continue;
        }
        if !ground.clear(spot + Vec3::Y * (HEIGHT - 0.3), you) {
            refused.unseen += 1;
            continue;
        }
        fit.push(spot);
        if fit.len() >= TRIES {
            break;
        }
    }
    // (From where it is; else from a way out, level with it.)
    for &spot in &fit {
        if let Some(points) = route(ground, from, spot, hover) {
            return Ok(Way { spot, points });
        }
    }
    let starts = exits(ground, from, you);
    refused.exits = starts.len();
    for &spot in &fit {
        for &start in &starts {
            if let Some(points) = route(ground, start, spot, hover) {
                return Ok(Way { spot, points: std::iter::once(start).chain(points).collect() });
            }
        }
        refused.no_way += 1;
    }
    Err(refused)
}

/// Ways out from `from`, level with it: towards you, then about it, as far
/// out as `OUT`, all it gets to; the furthest from where it is first (out
/// of what shuts it in), then the nearest you.
fn exits(ground: &Ground, from: Vec3, you: Vec3) -> Vec<Vec3> {
    let toward = Vec3::new(you.x - from.x, 0.0, you.z - from.z).normalize_or(Vec3::X);
    let mut found: Vec<(f32, Vec3)> = Vec::new();
    for k in 0..9 {
        let dir = if k == 0 { toward } else { Quat::from_rotation_y(k as f32 / 8.0 * std::f32::consts::TAU) * Vec3::X };
        for d in OUT {
            let e = from + dir * d;
            if !leg(ground, from, e) {
                break;
            }
            found.push((-d * 10.0 + e.distance(you) * 0.01, e));
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().take(EXITS).map(|(_, e)| e).collect()
}

/// The ground at `q` if it can stand there (else whether over the void, or
/// rough): there is ground (not the void),
/// flat under all its feet; in a built place, the floor found down from
/// `level` (your eye's height: the floor you are on, or one below).
fn standing(ground: &Ground, q: Vec2, level: f32) -> Result<Vec3, bool> {
    let probe = |p: Vec2| ground.under(Vec3::new(p.x, level, p.y)).ok_or(true);
    let y = probe(q)?;
    for k in 0..6 {
        let around = q + Vec2::from_angle(k as f32 / 6.0 * std::f32::consts::TAU) * FOOT;
        if (probe(around)? - y).abs() > FLAT {
            return Err(false);
        }
    }
    Ok(Vec3::new(q.x, y, q.y))
}

/// Its way from `from` to stand on `spot`: straight, or up over whatever is
/// in the way, across and down; None if no try gets through.
fn route(ground: &Ground, from: Vec3, spot: Vec3, hover: f32) -> Option<Vec<Vec3>> {
    let above = spot + Vec3::Y * (feet() + hover);
    let stand = spot + Vec3::Y * feet();
    for over in OVER {
        let top = from.y.max(above.y) + over;
        let points = if over == 0.0 { vec![above, stand] } else { vec![from.with_y(top), above.with_y(top), above, stand] };
        let mut at = from;
        let clear = points[..points.len() - 1].iter().all(|&p| {
            let ok = leg(ground, at, p);
            at = p;
            ok
        });
        if clear {
            return Some(points);
        }
    }
    None
}

/// Whether its body gets from `a` to `b`: nothing built in the way, and the
/// ground under it all along below its feet.
pub(super) fn leg(ground: &Ground, a: Vec3, b: Vec3) -> bool {
    if !ground.passes(a, b, BODY) {
        return false;
    }
    // (Straight up or down, the ground under it is the ground it left.)
    let flat = Vec2::new(b.x - a.x, b.z - a.z).length();
    if flat < 0.5 {
        return true;
    }
    let steps = (a.distance(b) / FELT).ceil() as usize;
    (1..=steps).all(|k| {
        let p = a.lerp(b, k as f32 / steps as f32);
        ground.under(p).is_none_or(|g| g + CLEAR < p.y - feet())
    })
}
