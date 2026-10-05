//! Stairs between levels: flights laid along the cliffs of a box structure,
//! from a lower top up to a higher one, as on Manifold Garden's terraces.
//! The structure is read as a height map (in the frame of its main yaw);
//! wherever a low cell meets a cliff 3 to 40 metres high, and the low side
//! stays level and the cliff stays high for as long as the flight needs,
//! a solid flight of steps climbs along the cliff's foot to its top. The
//! stairs are what give the masses their size.

use std::collections::HashMap;

use glam::{Quat, Vec2, Vec3};

use crate::noise::hash01;
use crate::structure::Solid;

/// Height map cells (metres).
const CELL: f32 = 1.0;
/// Step rise and run (metres).
const RISE: f32 = 0.42;
const RUN: f32 = 0.5;
/// Flight width (metres): the cell row along the cliff, a little less.
const WIDTH: f32 = 1.8;

/// Flights of stairs for `solids`, at most `max` of them.
pub fn flights(solids: &[Solid], max: u32, seed: u32) -> Vec<Solid> {
    let mut out = Vec::new();
    if max == 0 || solids.is_empty() {
        return out;
    }
    // The frame: the yaw most solids share.
    let yaw_of = |s: &Solid| {
        let f = s.rotation * Vec3::X;
        f.z.atan2(f.x)
    };
    let mut counts: HashMap<i32, usize> = HashMap::new();
    for s in solids.iter().filter(|s| !s.wedge && !s.round) {
        *counts.entry((yaw_of(s).to_degrees() * 2.0).round() as i32).or_default() += 1;
    }
    let Some((&key, _)) = counts.iter().max_by_key(|(_, n)| **n) else { return out };
    let yaw = (key as f32 * 0.5).to_radians();
    let (ex, ez) = (Vec2::new(yaw.cos(), yaw.sin()), Vec2::new(-yaw.sin(), yaw.cos()));
    let to_local = |p: Vec3| Vec2::new(Vec2::new(p.x, p.z).dot(ex), Vec2::new(p.x, p.z).dot(ez));

    // The height map: the highest top over each cell, of the solids in the
    // frame (upright, turned only by the frame's yaw, give or take a degree).
    let mut height: HashMap<(i32, i32), f32> = HashMap::new();
    for s in solids.iter().filter(|s| !s.wedge && !s.round) {
        let up = s.rotation * Vec3::Y;
        if up.y < 0.999 {
            continue;
        }
        let d = (yaw_of(s) - yaw).rem_euclid(std::f32::consts::FRAC_PI_2);
        if d.min(std::f32::consts::FRAC_PI_2 - d) > 0.02 {
            continue;
        }
        let c = to_local(s.center);
        // Half extents in the frame (a quarter turn swaps x and z).
        let fx = s.rotation * Vec3::X;
        let along_x = to_local(s.center + fx) - c;
        let (hx, hz) = if along_x.x.abs() > along_x.y.abs() { (s.half.x, s.half.z) } else { (s.half.z, s.half.x) };
        let top = s.center.y + s.half.y;
        let (x0, x1) = (((c.x - hx) / CELL).ceil() as i32, ((c.x + hx) / CELL).floor() as i32);
        let (z0, z1) = (((c.y - hz) / CELL).ceil() as i32, ((c.y + hz) / CELL).floor() as i32);
        if (x1 - x0 + 1) as i64 * (z1 - z0 + 1) as i64 > 40_000 {
            continue;
        }
        for x in x0..=x1 {
            for z in z0..=z1 {
                let h = height.entry((x, z)).or_insert(f32::MIN);
                *h = h.max(top);
            }
        }
    }
    let at = |x: i32, z: i32| height.get(&(x, z)).copied();
    let to_world = |l: Vec2| ex * l.x + ez * l.y;
    let rotation = Quat::from_rotation_y(-yaw);

    // Candidate cells in a scrambled order.
    let mut cells: Vec<(i32, i32)> = height.keys().copied().collect();
    cells.sort_by_key(|&(x, z)| (hash01(x, z, 0x57a, seed) * 1e6) as i64);
    let mut used: HashMap<(i32, i32), ()> = HashMap::new();
    let mut made = 0;
    for (x, z) in cells {
        if made >= max {
            break;
        }
        let Some(low) = at(x, z) else { continue };
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let Some(high) = at(x + dx, z + dz) else { continue };
            let rise = high - low;
            if !(3.0..=40.0).contains(&rise) {
                continue;
            }
            // Along the cliff, one way or the other.
            let (ax, az) = (-dz, dx);
            let side = if hash01(x, z, 0x57b, seed) < 0.5 { 1 } else { -1 };
            let (ax, az) = (ax * side, az * side);
            let steps = (rise / RISE).ceil() as i32;
            let length = ((steps as f32 * RUN) / CELL).ceil() as i32 + 1;
            let ok = (0..=length).all(|t| {
                let (cx, cz) = (x + ax * t, z + az * t);
                let flat = at(cx, cz).is_some_and(|h| (h - low).abs() < 0.3);
                let cliff = at(cx + dx, cz + dz).is_some_and(|h| h >= high - 0.3);
                let free = !used.contains_key(&(cx, cz));
                flat && cliff && free
            });
            if !ok {
                continue;
            }
            // The flight: solid steps from the low top, in the cell row at the
            // cliff's foot, climbing along it.
            let rise_each = rise / steps as f32;
            let face = Vec2::new(dx as f32, dz as f32) * (CELL * 0.5);
            for s in 0..steps {
                let along = (s as f32 + 0.5) * RUN / CELL;
                let l = Vec2::new(x as f32 + ax as f32 * along, z as f32 + az as f32 * along) * CELL + face
                    - Vec2::new(dx as f32, dz as f32) * (WIDTH * 0.5);
                let top = low + rise_each * (s + 1) as f32;
                let w = to_world(l);
                let half_along = RUN * 0.5 + 0.02;
                let half = if ax != 0 { Vec3::new(half_along, (top - low) * 0.5, WIDTH * 0.5) } else { Vec3::new(WIDTH * 0.5, (top - low) * 0.5, half_along) };
                out.push(Solid { glow: 0.0,
                    detail: false,
                    wedge: false,
                    round: false,
                    center: Vec3::new(w.x, (low + top) * 0.5, w.y),
                    rotation,
                    half,
                    albedo: 0.16,
                });
            }
            for t in -2..=length + 2 {
                for k in -2..=2 {
                    used.insert((x + ax * t + az.abs() * k, z + az * t + ax.abs() * k), ());
                }
            }
            made += 1;
            break;
        }
    }
    out
}
