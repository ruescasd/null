//! The stalker's head: an open lattice cage (the user's concept, a skull of
//! Voronoi struts, dark inside; reconstructed as a mesh from its pictures
//! and baked into `data/stalker_head.bin`). Built by formula, a skull came
//! out an egg (and before it, heads of stacked rings read as toys: a torch,
//! a bulb, a duck): a head's shape is authored, not generated.
//!
//! Flat shaded, pale outside; the faces turned in, towards its hollow, black
//! (shaded only, they washed out pale in the sun), so the cage reads dark
//! inside through its openings.
//!
//! In two, cut at its narrow waist, the joint: the skull above turns with
//! the head, the legs below stand on the neck, fastened to the body by
//! cables (see `hunter/cables.rs`). (Whole on the head, its legs swung round
//! with it as it turned and looked down, dragging the cables, the user.)
//!
//! Baked in its own frame (x right, y up, z forward, the face; height 1
//! from its lowest point, centred across); the skull returned in the head
//! bone's frame (x right, y along it, where it looks, z down), the legs in
//! the neck's (x right, y along it, up, z forward), both from the neck's
//! end.

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

/// How tall the cage is, crown to the tips of its legs (m).
const HEIGHT: f32 = 0.56;
/// Where the neck's end meets it, up from its lowest point (of its
/// height): the waist, between the legs below and the skull above.
const WAIST: f32 = 0.34;
/// The skull's middle, up from its lowest point (of its height): what the
/// faces turn in towards, along its axis between `WAIST` and the crown.
const HOLLOW: (f32, f32) = (0.45, 0.8);
/// How far a face must turn in to be black (the cosine).
const INWARD: f32 = -0.2;

/// A part of the cage: its pale mesh and its black.
pub(super) struct Piece {
    pub pale: Mesh,
    pub dark: Mesh,
}

/// The baked mesh: its triangle count, then each corner's x, y, z.
fn decode(bytes: &[u8]) -> Vec<[Vec3; 3]> {
    let count = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 4 + count * 36, "data/stalker_head.bin is not a baked head: bake it again from its GLB");
    let f = |k: usize| f32::from_le_bytes(bytes[4 + k * 4..8 + k * 4].try_into().unwrap());
    (0..count).map(|t| std::array::from_fn(|c| Vec3::new(f(t * 9 + c * 3), f(t * 9 + c * 3 + 1), f(t * 9 + c * 3 + 2)))).collect()
}

fn baked() -> Vec<[Vec3; 3]> {
    decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/stalker_head.bin")))
}

/// From the cage's own frame, sized and set on the neck's end: to the head
/// bone's (the skull), to the neck's (the legs).
fn to_head(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y)
}
fn to_neck(v: Vec3) -> Vec3 {
    v
}
fn placed(v: Vec3) -> Vec3 {
    (v - Vec3::Y * WAIST) * HEIGHT
}

/// The skull and the legs (see the module's notes), in black and white by
/// `tone` (`--set cagetone`, to judge): 0 pale, its inside black; 1 inked,
/// the struts' outer faces pale, their sides black; 2 the body's black
/// rising up its struts and lower skull to a pale crown; 3 black, pale only
/// where a strut faces straight out.
pub(super) fn build(tone: u32) -> (Piece, Piece) {
    let mut pieces: [(Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<[f32; 3]>, Vec<[f32; 3]>); 2] = Default::default();
    for [a, b, c] in baked() {
        let n = (b - a).cross(c - a);
        if n.length_squared() < 1e-14 {
            continue;
        }
        let n = n.normalize();
        let middle = (a + b + c) / 3.0;
        let legs = middle.y < WAIST;
        let frame = if legs { to_neck } else { to_head };
        let out = (middle - Vec3::new(0.0, middle.y.clamp(HOLLOW.0, HOLLOW.1), 0.0)).normalize_or(Vec3::Z);
        let facing = n.dot(out);
        let corners = [a, b, c].map(|p| frame(placed(p)).to_array());
        let normal = frame(n).to_array();
        let (pale, pale_normals, colors, dark, dark_normals) = &mut pieces[legs as usize];
        let black = facing < INWARD
            || match tone {
                1 => facing < 0.75,
                2 => middle.y < 0.55,
                3 => facing < 0.92,
                _ => false,
            };
        if black {
            dark.extend(corners);
            dark_normals.extend([normal; 3]);
        } else {
            let shade = 0.55 + 0.45 * facing.max(0.0);
            pale.extend(corners);
            pale_normals.extend([normal; 3]);
            colors.extend([[shade, shade, shade, 1.0]; 3]);
        }
    }
    let [skull, legs] = pieces.map(|(pale, pale_normals, colors, dark, dark_normals)| Piece {
        pale: Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pale)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, pale_normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors),
        dark: Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, dark)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, dark_normals),
    });
    (skull, legs)
}

/// The tips of its legs (front, back, left, right), where the neck's cables
/// fasten, in the neck's frame from its end (m): each the middle of the
/// corners within a little of its lowest point, by the way it reaches out.
pub(in crate::combat) fn legs() -> [Vec3; 4] {
    static LEGS: std::sync::OnceLock<[Vec3; 4]> = std::sync::OnceLock::new();
    *LEGS.get_or_init(|| {
        let mut sums = [(Vec3::ZERO, 0.0f32); 4];
        for p in baked().into_iter().flatten().filter(|p| p.y < 0.015) {
            let k = if p.z.abs() > p.x.abs() { if p.z > 0.0 { 0 } else { 1 } } else if p.x < 0.0 { 2 } else { 3 };
            sums[k].0 += p;
            sums[k].1 += 1.0;
        }
        sums.map(|(sum, n)| {
            assert!(n > 0.0, "data/stalker_head.bin: a leg missing (the head stands on four, front, back and sides)");
            to_neck(placed(sum / n))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::{Intent, Plan, Rig};

    /// The stalker's neck struts end inside the top of its torso, walking
    /// and looking about.
    #[test]
    fn struts_in_the_torso() {
        for look in [Vec3::new(0.0, 1.5, 5.0), Vec3::new(4.0, 1.0, 1.0), Vec3::new(-3.0, 3.0, -2.0)] {
            let mut rig = Rig::new(Plan::stalker(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
            let mut worst = f32::MIN;
            for k in 0..800 {
                let intent = Intent { velocity: Vec3::Z * 1.2, look: rig.root + look, ..Default::default() };
                rig.update(1.0 / 100.0, intent, &|_, _| 0.0, &|from, step| from + step);
                if k < 200 {
                    continue;
                }
                let (torso, neck) = (rig.bones[2], rig.bones[3]);
                for tip in legs() {
                    let p = neck.b + neck.rotation() * tip;
                    let axis = torso.b - torso.a;
                    let t = ((p - torso.a).dot(axis) / axis.length_squared()).clamp(0.0, 1.0);
                    // (How far out of the torso, as a capsule, negative in.)
                    worst = worst.max(p.distance(torso.a + axis * t) - torso.radius);
                    // (And below its top, square to its axis.)
                    worst = worst.max((p - torso.b).dot(axis.normalize()));
                }
            }
            println!("looking at {look}: deepest strut tip {worst:.3} m from inside");
            assert!(worst < -0.01, "looking at {look}: a strut tip {worst} m out of the torso");
        }
    }
}
