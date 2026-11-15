//! Where the ground is under a point, for what walks or lands: outdoors the
//! terrain (one height under each point); in the chasm (`--opt chasm`: a
//! built place, galleries over halls, ledges over the void), what is built,
//! found by a ray cast down from a little above the point, so a creature
//! finds the floor it is on and not one above or below it; over the void,
//! nothing.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{Args, terrain::WorldGen};

/// How far above a point its ground is looked for (a step up a flight; not
/// so far that the ray starts inside a gallery's slab overhead), and how far
/// below it.
const ABOVE: f32 = 1.5;
const BELOW: f32 = 40.0;
/// How far about a point the ground is felt for too (over a crack, or off
/// an edge).
const PROBE: f32 = 0.1;

#[derive(SystemParam)]
pub struct Ground<'w, 's> {
    world: Res<'w, WorldGen>,
    args: Res<'w, Args>,
    spatial: SpatialQuery<'w, 's>,
    mover: MoveAndSlide<'w, 's>,
}

impl Ground<'_, '_> {
    /// The ground's height under `p`; `None` over the void (nothing within
    /// `BELOW`), only in a built place.
    pub fn under(&self, p: Vec3) -> Option<f32> {
        // (In the chasm, and on the ground lab's, what is built is the
        // ground.)
        if !self.args.opt("chasm") && self.args.ground.is_none() {
            return Some(self.world.walk_height(p.x, p.z));
        }
        // By rays, exact: one under it, and four `PROBE` about it. Where the
        // one falls further than that below the highest of the four, it is
        // over a crack (a hairline one between a flight's slope and its
        // landing let a lone ray find the floor four storeys down) or off an
        // edge: the ground is theirs. (Felt with a thin ball, it stood off a
        // flat floor by up to 14 cm, from one centimetre to the next, on the
        // arena's plaza, its long thin triangles: what lay there sank and
        // popped up for ever, the gladiators', 2026-10-31.)
        let from = p + Vec3::Y * ABOVE;
        let ray = |at: Vec3| self.spatial.cast_ray(at, Dir3::NEG_Y, ABOVE + BELOW, true, &SpatialQueryFilter::default()).map(|hit| at.y - hit.distance);
        let around = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].into_iter().filter_map(|d| ray(from + d * PROBE)).reduce(f32::max);
        match (ray(from), around) {
            (Some(under), Some(around)) if around - under > PROBE => Some(around),
            (Some(under), _) => Some(under),
            (None, around) => around,
        }
    }

    /// Whether nothing built stands between `a` and `b` (in sight, in
    /// reach of a straight leap).
    pub fn clear(&self, a: Vec3, b: Vec3) -> bool {
        let to = b - a;
        Dir3::new(to).map_or(true, |d| self.spatial.cast_ray(a, d, to.length(), true, &SpatialQueryFilter::default()).is_none())
    }

    /// Whether a ball of `radius` about `p` is inside something built (a
    /// flyer's check that its ways keep clear).
    pub fn inside(&self, p: Vec3, radius: f32) -> bool {
        !self.spatial.shape_intersections(&Collider::sphere(radius), p, Quat::IDENTITY, &SpatialQueryFilter::default()).is_empty()
    }

    /// Whether a ball of `radius` gets from `a` to `b` with nothing built in
    /// its way: a body's way past a corner, not only a line of sight.
    pub fn passes(&self, a: Vec3, b: Vec3, radius: f32) -> bool {
        let to = b - a;
        let Ok(dir) = Dir3::new(to) else { return true };
        let config = ShapeCastConfig { max_distance: to.length(), ignore_origin_penetration: true, ..default() };
        self.spatial.cast_shape(&Collider::sphere(radius), a, Quat::IDENTITY, dir, &config, &SpatialQueryFilter::default()).is_none()
    }

    /// Where a body of `shape`, its middle at `from`, gets to moved by
    /// `delta`: in a built place as far as what is built lets it, sliding
    /// along what stops it (as you do); elsewhere all the way.
    pub fn sweep(&self, shape: &Collider, from: Vec3, delta: Vec3) -> Vec3 {
        if !self.args.opt("chasm") || delta.length_squared() < 1e-10 {
            return from + delta;
        }
        self.mover
            .move_and_slide(shape, from, Quat::IDENTITY, delta, Duration::from_secs(1), &MoveAndSlideConfig::default(), &SpatialQueryFilter::default(), |_| MoveAndSlideHitResponse::Accept)
            .position
    }

    /// What is under feet at `feet`, within `reach` below: the terrain, or
    /// anything built over it (a ledge, a roof); `None` if nothing is (over
    /// the void, high in a leap). (Where a stalker aims, at what you stand
    /// on: your feet's ground alone missed ledges, the user.)
    pub fn floor(&self, feet: Vec3, reach: f32) -> Option<f32> {
        let from = feet - Vec3::Y * 0.05;
        let built = self.spatial.cast_ray(from, Dir3::NEG_Y, reach, true, &SpatialQueryFilter::default()).map(|hit| from.y - hit.distance);
        let terrain = (!self.args.opt("chasm")).then(|| self.world.ground_height(feet.x, feet.z)).filter(|&h| h > feet.y - reach);
        match (built, terrain) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }

    /// Whether `a` sees `b`: nothing built between, and the ground nowhere
    /// over the line (sampled every 2 m; `clear` alone, outdoors, saw
    /// through hills).
    pub fn sight(&self, a: Vec3, b: Vec3) -> bool {
        if !self.clear(a, b) {
            return false;
        }
        if self.args.opt("chasm") {
            return true;
        }
        let n = ((a.distance(b) / 2.0).ceil() as usize).clamp(2, 150);
        (1..n).all(|k| {
            let p = a.lerp(b, k as f32 / n as f32);
            self.world.ground_height(p.x, p.z) < p.y - 0.1
        })
    }

    /// How far along `dir` from `from` a ray first meets what is built,
    /// within `max`.
    pub fn hit(&self, from: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        Dir3::new(dir).ok().and_then(|d| self.spatial.cast_ray(from, d, max, true, &SpatialQueryFilter::default()).map(|hit| hit.distance))
    }

    /// The ground's height under `p`, or far below it over the void (what
    /// stands there falls).
    pub fn at(&self, p: Vec3) -> f32 {
        self.under(p).unwrap_or(p.y - BELOW)
    }
}
