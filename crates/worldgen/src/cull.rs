//! Hidden faces of box solids: what of each box's faces no other box
//! covers. Faces between touching boxes, inside overlapping ones and under
//! coplanar faces of others are left out, so a structure built of many
//! boxes draws only its outer surface, and never two faces in one place
//! (which flicker against each other).
//!
//! Only boxes turned about y alone are considered, in groups that share a
//! frame up to quarter turns: in a group's frame every box is an
//! axis-aligned box.

use std::collections::HashMap;

use glam::{Quat, Vec3};

/// Faces closer than this (metres) are taken as one plane.
const EPS: f32 = 0.004;
/// Pieces of faces narrower than this are dropped rather than drawn.
const SLIVER: f32 = EPS;
/// A face cut into more pieces than this by boxes overlapping it (none in
/// its plane) is drawn whole: the boxes hide what they cover anyway.
const MAX_PIECES: usize = 4;
/// The grid the boxes are hashed in to find their neighbours (metres).
const CELL: f32 = 8.0;

/// One rectangle of a box's face left to draw: the box, the face's axis and
/// side, and the rectangle (u0, v0, u1, v1) on the two other axes, in the
/// group's frame.
pub struct Piece {
    pub solid: usize,
    pub axis: usize,
    pub positive: bool,
    pub rect: [f32; 4],
}

/// A group's frame and its boxes' extents in it.
pub struct Group {
    pub frame: Quat,
    pub boxes: Vec<(usize, Vec3, Vec3)>,
}

/// The yaw of a rotation about y alone, folded into a quarter turn; `None`
/// if it turns about another axis too.
fn yaw(rotation: Quat) -> Option<f32> {
    let q = rotation.normalize();
    if q.x.abs() > 1e-4 || q.z.abs() > 1e-4 {
        return None;
    }
    let a = 2.0 * q.y.atan2(q.w);
    Some(a.rem_euclid(std::f32::consts::FRAC_PI_2))
}

/// Groups boxes (`(center, rotation, half)`, `None` for solids that are not
/// boxes) by frame.
pub fn groups(boxes: &[Option<(Vec3, Quat, Vec3)>]) -> Vec<Group> {
    let mut by_yaw: HashMap<i64, Group> = HashMap::new();
    for (i, b) in boxes.iter().enumerate() {
        let Some((center, rotation, half)) = *b else { continue };
        let Some(a) = yaw(rotation) else { continue };
        // A yaw just under a quarter turn is the same frame as none.
        let key = ((a / 1e-4).round() as i64) % ((std::f32::consts::FRAC_PI_2 / 1e-4).round() as i64);
        let group = by_yaw.entry(key).or_insert_with(|| Group { frame: Quat::from_rotation_y(key as f32 * 1e-4), boxes: Vec::new() });
        let inv = group.frame.inverse();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let p = inv * (center + rotation * (Vec3::new(x, y, z) * half));
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
        group.boxes.push((i, lo, hi));
    }
    by_yaw.into_values().collect()
}

/// Removes `cut` from each rectangle in `rects`.
fn subtract(rects: &mut Vec<[f32; 4]>, cut: [f32; 4]) {
    let mut out = Vec::with_capacity(rects.len() + 3);
    for r in rects.drain(..) {
        let (u0, v0) = (r[0].max(cut[0]), r[1].max(cut[1]));
        let (u1, v1) = (r[2].min(cut[2]), r[3].min(cut[3]));
        if u1 - u0 <= SLIVER || v1 - v0 <= SLIVER {
            out.push(r);
            continue;
        }
        // The parts outside the cut: below and above it across the whole
        // width, then left and right of it within its height.
        for piece in [[r[0], r[1], r[2], v0], [r[0], v1, r[2], r[3]], [r[0], v0, u0, v1], [u1, v0, r[2], v1]] {
            if piece[2] - piece[0] > SLIVER && piece[3] - piece[1] > SLIVER {
                out.push(piece);
            }
        }
    }
    *rects = out;
}

/// The two axes other than `axis`.
pub fn others(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

/// The visible pieces of a group's boxes' faces.
pub fn visible(group: &Group) -> Vec<Piece> {
    let cell = |v: f32| (v / CELL).floor() as i32;
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (k, &(_, lo, hi)) in group.boxes.iter().enumerate() {
        for x in cell(lo.x - EPS)..=cell(hi.x + EPS) {
            for z in cell(lo.z - EPS)..=cell(hi.z + EPS) {
                grid.entry((x, z)).or_default().push(k);
            }
        }
    }
    let mut out = Vec::new();
    let mut near: Vec<usize> = Vec::new();
    for (k, &(solid, lo, hi)) in group.boxes.iter().enumerate() {
        near.clear();
        for x in cell(lo.x - EPS)..=cell(hi.x + EPS) {
            for z in cell(lo.z - EPS)..=cell(hi.z + EPS) {
                if let Some(list) = grid.get(&(x, z)) {
                    near.extend(list.iter().copied().filter(|&j| j != k));
                }
            }
        }
        near.sort_unstable();
        near.dedup();
        for axis in 0..3 {
            let (a, b) = others(axis);
            for positive in [false, true] {
                let plane = if positive { hi[axis] } else { lo[axis] };
                let whole = [lo[a], lo[b], hi[a], hi[b]];
                let mut rects = vec![whole];
                let mut coplanar = false;
                for &j in &near {
                    let (_, jlo, jhi) = group.boxes[j];
                    // Does box j fill the space just in front of this face?
                    let (front, behind) = if positive { (jhi[axis] - plane, plane - jlo[axis]) } else { (plane - jlo[axis], jhi[axis] - plane) };
                    let covers = if front > EPS {
                        behind > -EPS
                    } else if front > -EPS && behind > EPS {
                        // Its face lies in this plane, facing the same way:
                        // one of the two is drawn, the first.
                        coplanar |= j < k;
                        j < k
                    } else {
                        false
                    };
                    if !covers {
                        continue;
                    }
                    let cut = [jlo[a], jlo[b], jhi[a], jhi[b]];
                    subtract(&mut rects, cut);
                    if rects.is_empty() {
                        break;
                    }
                }
                if rects.len() > MAX_PIECES && !coplanar {
                    rects = vec![whole];
                }
                out.extend(rects.into_iter().map(|rect| Piece { solid, axis, positive, rect }));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(pieces: &[Piece]) -> f32 {
        pieces.iter().map(|p| (p.rect[2] - p.rect[0]) * (p.rect[3] - p.rect[1])).sum()
    }

    fn of(boxes: &[(Vec3, Vec3)]) -> Vec<Piece> {
        let input: Vec<_> = boxes.iter().map(|&(c, h)| Some((c, Quat::IDENTITY, h))).collect();
        groups(&input).iter().flat_map(visible).collect()
    }

    #[test]
    fn a_lone_box_keeps_its_faces() {
        let p = of(&[(Vec3::ZERO, Vec3::ONE)]);
        assert_eq!(p.len(), 6);
        assert!((area(&p) - 24.0).abs() < 1e-3);
    }

    #[test]
    fn touching_boxes_lose_the_faces_between() {
        let p = of(&[(Vec3::ZERO, Vec3::ONE), (Vec3::X * 2.0, Vec3::ONE)]);
        // Two 2x2x2 cubes side by side: a 4x2x2 box's surface.
        assert!((area(&p) - (2.0 * 16.0 + 8.0)).abs() < 1e-3, "{}", area(&p));
    }

    #[test]
    fn identical_boxes_draw_once() {
        let p = of(&[(Vec3::ZERO, Vec3::ONE), (Vec3::ZERO, Vec3::ONE)]);
        assert!((area(&p) - 24.0).abs() < 1e-3, "{}", area(&p));
    }

    #[test]
    fn a_box_inside_another_vanishes() {
        let p = of(&[(Vec3::ZERO, Vec3::ONE * 2.0), (Vec3::ZERO, Vec3::ONE)]);
        assert!((area(&p) - 96.0).abs() < 1e-3);
        assert!(p.iter().all(|p| p.solid == 0));
    }

    #[test]
    fn a_partly_covered_face_is_cut() {
        // A small box on top of a big one: the big top loses the footprint.
        let p = of(&[(Vec3::ZERO, Vec3::ONE * 2.0), (Vec3::Y * 2.5, Vec3::new(0.5, 0.5, 0.5))]);
        let top: f32 = p.iter().filter(|p| p.solid == 0 && p.axis == 1 && p.positive).map(|p| (p.rect[2] - p.rect[0]) * (p.rect[3] - p.rect[1])).sum();
        assert!((top - 15.0).abs() < 1e-2, "{top}");
    }

    #[test]
    fn quarter_turns_share_a_frame() {
        let turned = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let input = vec![Some((Vec3::ZERO, Quat::IDENTITY, Vec3::ONE)), Some((Vec3::X * 2.0, turned, Vec3::ONE))];
        let g = groups(&input);
        assert_eq!(g.len(), 1);
        assert!((area(&visible(&g[0])) - 40.0).abs() < 1e-3);
    }
}
