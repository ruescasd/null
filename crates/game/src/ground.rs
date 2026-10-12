//! Where the ground is under a point, for what walks or lands: outdoors the
//! terrain (one height under each point); in the chasm (`--opt chasm`: a
//! built place, galleries over halls, ledges over the void), what is built,
//! found by a ray cast down from a little above the point, so a creature
//! finds the floor it is on and not one above or below it; over the void,
//! nothing.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{Args, terrain::WorldGen};

/// How far above a point its ground is looked for (a step up a flight; not
/// so far that the ray starts inside a gallery's slab overhead), and how far
/// below it.
const ABOVE: f32 = 1.5;
const BELOW: f32 = 40.0;

#[derive(SystemParam)]
pub struct Ground<'w, 's> {
    world: Res<'w, WorldGen>,
    args: Res<'w, Args>,
    spatial: SpatialQuery<'w, 's>,
}

impl Ground<'_, '_> {
    /// The ground's height under `p`; `None` over the void (nothing within
    /// `BELOW`), only in a built place.
    pub fn under(&self, p: Vec3) -> Option<f32> {
        if !self.args.opt("chasm") {
            return Some(self.world.ground_height(p.x, p.z));
        }
        let from = p + Vec3::Y * ABOVE;
        self.spatial.cast_ray(from, Dir3::NEG_Y, ABOVE + BELOW, true, &SpatialQueryFilter::default()).map(|hit| from.y - hit.distance)
    }

    /// The ground's height under `p`, or far below it over the void (what
    /// stands there falls).
    pub fn at(&self, p: Vec3) -> f32 {
        self.under(p).unwrap_or(p.y - BELOW)
    }
}
