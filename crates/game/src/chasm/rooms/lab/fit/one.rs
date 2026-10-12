//! Rooms on one level fitted to their doors (the network's places whose
//! doors are all on one storey: 62% of them): the lab's hypostyle, rotunda
//! and crossing, each taking its footprint, height and doors as given
//! rather than inventing them. Each on the door's level; the storeys above
//! it its height (proportions from the plan, the rest left as rock or a
//! lantern, a dome, a tower). A rotunda or a crossing that its doors or
//! footprint do not allow is not made (see `fitted`, which falls back to a
//! hypostyle).

use bevy::prelude::*;

use super::super::{B, Detail, Room, T, half, strength, thickness};
use super::{SLAB, Spec, light};

/// The level all the doors are on, if they are on one.
pub(super) fn level(spec: &Spec) -> Option<f32> {
    let y = spec.doors.first()?.2;
    spec.doors.iter().all(|d| (d.2 - y).abs() < 0.01).then_some(y)
}

/// A door through its wall and `extra` metres on into the rock (a passage
/// in to the room), 2 m wide, arched, with stepped reveals.
fn door(b: &mut B, spec: &Spec, (wall, u, y): (usize, f32, f32), extra: f32) {
    let (sx, sz) = (spec.size.x, spec.size.y);
    let (axis, n) = match wall {
        0 => (0, 0.0),
        1 => (0, sx - T - extra),
        2 => (2, 0.0),
        _ => (2, sz - T - extra),
    };
    b.door(axis, u - 1.0, 2.0, y, 2.5, n, T + extra);
}

/// A point `d` in from a door's wall at its middle, `up` above its sill.
fn inward(spec: &Spec, (wall, u, y): (usize, f32, f32), d: f32, up: f32) -> Vec3 {
    let (sx, sz) = (spec.size.x, spec.size.y);
    match wall {
        0 => Vec3::new(T + d, y + up, u),
        1 => Vec3::new(sx - T - d, y + up, u),
        2 => Vec3::new(u, y + up, T + d),
        _ => Vec3::new(u, y + up, sz - T - d),
    }
}

/// Views: from inside the first door (past its passage, `extra` deep), at
/// the room's middle a little up; from inside each door, the same.
fn views(b: &mut B, spec: &Spec, mid: Vec3, extra: &[f32]) {
    for (k, &d) in spec.doors.iter().enumerate() {
        let eye = inward(spec, d, 1.0 + extra.get(k).copied().unwrap_or(0.0), 1.7);
        b.view(["floor", "door2", "door3", "door4", "door5", "door6"][k.min(5)], eye, mid);
    }
}

/// A hypostyle: the hall filling the footprint, on the doors' level, as
/// high as a third of its mean side (5 m at least); columns by the orders
/// (a tenth of its height thick) at the crossings of bays dividing each
/// side evenly, near six thicknesses each; a clearing of a bay or two in
/// the middle, under a lantern rising into what height is left; columns
/// kept out of every door's way in; pilasters answering them along the
/// walls, niches between; coffers, beams on the columns, bands.
pub(super) fn hypostyle(spec: &Spec) -> Room {
    let (sx, sz) = (spec.size.x, spec.size.y);
    let floor = level(spec).unwrap_or(1.0);
    let room = spec.height - SLAB - floor;
    let (w, d) = (sx - 2.0 * T, sz - 2.0 * T);
    let h = half(((w + d) * 0.5 / 3.0).max(5.0)).min(room);
    let dd = thickness(h, 10.0);
    let target = (dd * 6.0).max(4.0);
    let (nx, nz) = (((w / target).round() as i32).max(2), ((d / target).round() as i32).max(2));
    let (bx, bz) = (w / nx as f32, d / nz as f32);
    let mut b = B::new(Vec3::new(sx, spec.height, sz), Detail { reveals: true, coffers: true, bands: true, pilasters: true, beams: true, screens: false });
    b.carve(Vec3::new(T, floor, T), Vec3::new(sx - T, floor + h, sz - T));
    // The clearing: the middle bay, or the middle two, each way.
    let middle = |n: i32, bay: f32, from: f32| {
        let k = if n % 2 == 1 { 1 } else { 2 };
        let a = from + (n - k) as f32 * 0.5 * bay;
        (a, a + k as f32 * bay)
    };
    let (cx, cz) = (middle(nx, bx, T), middle(nz, bz, T));
    let lantern = (room - h).min(bx.max(bz) * 1.2);
    if lantern >= 2.0 {
        b.carve(Vec3::new(cx.0, floor + h, cz.0), Vec3::new(cx.1, floor + h + lantern * 0.5, cz.1));
        b.carve(Vec3::new(cx.0 + 0.75, floor + h, cz.0 + 0.75), Vec3::new(cx.1 - 0.75, floor + h + lantern, cz.1 - 0.75));
    }
    b.coffers((T, sx - T), (T, sz - T), floor + h, bx.min(bz));
    let beam = half(bx.min(bz) / 10.0).max(0.5);
    // Columns: at the bays' crossings, but in the clearing and in a door's
    // way in (2 m either side of it, a bay and a half in).
    let in_the_way = |x: f32, z: f32| {
        spec.doors.iter().any(|&(wall, u, _)| {
            let (across, along) = match wall {
                0 => (z - u, x - T),
                1 => (z - u, sx - T - x),
                2 => (x - u, z - T),
                _ => (x - u, sz - T - z),
            };
            across.abs() < 2.0 + dd * 0.5 && along < 1.5 * bx.max(bz)
        })
    };
    let mut columns = 0;
    let mut posts = Vec::new();
    for i in 1..nx {
        for j in 1..nz {
            let (x, z) = (T + i as f32 * bx, T + j as f32 * bz);
            let cleared = x > cx.0 + 0.01 && x < cx.1 - 0.01 && z > cz.0 + 0.01 && z < cz.1 - 0.01;
            if !cleared && !in_the_way(x, z) {
                b.column(x, z, dd, (floor, floor + h - beam));
                posts.push((i, j));
                columns += 1;
            }
        }
    }
    // Beams on them, along their rows and files, between neighbours both
    // standing.
    for &(i, j) in &posts {
        for (di, dj) in [(1, 0), (0, 1)] {
            if posts.contains(&(i + di, j + dj)) {
                let (x0, z0) = (T + i as f32 * bx, T + j as f32 * bz);
                let (x1, z1) = (T + (i + di) as f32 * bx, T + (j + dj) as f32 * bz);
                let w2 = dd * 0.5;
                b.solid(Vec3::new(x0.min(x1) - w2, floor + h - beam, z0.min(z1) - w2), Vec3::new(x0.max(x1) + w2, floor + h, z0.max(z1) + w2));
            }
        }
    }
    b.rhythm((T, sx - T), (T, sz - T), (floor, floor + h), bx, Some(bx), true);
    b.bands((T, sx - T), (T, sz - T), (floor, floor + h));
    for &dr in &spec.doors {
        door(&mut b, spec, dr, 0.0);
    }
    b.note(format!("{}: a hypostyle {sx:.0} by {sz:.0} m, {h:.1} m high, {columns} columns {dd:.1} m thick, bays {bx:.1} by {bz:.1} m; a clearing under a lantern {lantern:.1} m higher; {} doors", spec.name, spec.doors.len()));
    light(&mut b, spec.size, floor, floor + h);
    if lantern >= 2.0 {
        let c = Vec3::new((cx.0 + cx.1) * 0.5, floor + h + lantern - 1.0, (cz.0 + cz.1) * 0.5);
        b.lamp(c, h + lantern + w * 0.5, strength(h + lantern + w * 0.5) * 2.0);
    }
    let mid = Vec3::new(sx * 0.5, floor + h * 0.4, sz * 0.5);
    views(&mut b, spec, mid, &[]);
    b.finish(floor)
}

/// A rotunda: a drum as wide as the footprint allows (near square only: no
/// more than 4:3), its wall as high as its radius (the Pantheon's) or as
/// the height allows, a stepped dome over it in what is left, an oculus in
/// its crown; the corners solid; each door a passage cut straight in to
/// the drum. None if a door is too near a corner to reach the drum.
pub(super) fn rotunda(spec: &Spec) -> Option<Room> {
    let (sx, sz) = (spec.size.x, spec.size.y);
    if sx.max(sz) / sx.min(sz) > 1.34 {
        return None;
    }
    let floor = level(spec)?;
    let room = spec.height - SLAB - floor;
    let rad = (sx.min(sz) - 2.0 * T) * 0.5;
    let (cx, cz) = (sx * 0.5, sz * 0.5);
    // (Each door's passage: from its wall to the drum, a little into it.)
    let mut passages = Vec::new();
    for &(wall, u, y) in &spec.doors {
        let (off, to_centre) = match wall {
            0 | 1 => (u - cz, cx - T),
            _ => (u - cx, cz - T),
        };
        if off.abs() > rad - 1.5 {
            return None;
        }
        passages.push(((wall, u, y), to_centre - (rad * rad - off * off).sqrt() + 0.5));
    }
    let wall_h = half(rad.min(room - 2.0)).max(4.0).min(room);
    let mut b = B::new(Vec3::new(sx, spec.height, sz), Detail { reveals: true, coffers: true, bands: true, ..Default::default() });
    b.ring(cx, cz, (0.0, rad), (floor, floor + wall_h), false);
    b.round_bands((cx, cz), rad, (floor, floor + wall_h));
    // The dome: stepped in, ring by ring, to an oculus, in the height left.
    let above = room - wall_h;
    let levels = ((above / 1.5).floor() as i32).clamp(0, 6);
    let (dr, dh) = (half(rad * 0.75 / levels.max(1) as f32).max(0.5), if levels > 0 { above / levels as f32 } else { 0.0 });
    for k in 1..=levels {
        let y = floor + wall_h + (k - 1) as f32 * dh;
        b.ring(cx, cz, (0.0, rad - k as f32 * dr), (y, y + dh), false);
    }
    let oculus = (rad - (levels + 1) as f32 * dr).max(1.0);
    b.ring(cx, cz, (0.0, oculus), (floor + wall_h + levels as f32 * dh, spec.height), false);
    b.sky(Vec3::new(cx, spec.height, cz), oculus + 1.0, room + 12.0, strength(room + 12.0) * 6.0);
    for &(d, extra) in &passages {
        door(&mut b, spec, d, extra);
    }
    let extras: Vec<f32> = passages.iter().map(|p| p.1).collect();
    b.note(format!("{}: a rotunda {:.0} m across, its wall {wall_h:.1} m high, a dome stepped {levels} times to an oculus; {} doors, each a passage in", spec.name, rad * 2.0, spec.doors.len()));
    light(&mut b, spec.size, floor, floor + wall_h);
    views(&mut b, spec, Vec3::new(cx, floor + wall_h * 0.6, cz), &extras);
    Some(b.finish(floor))
}

/// A crossing: two halls a third of the narrower side wide (6 m at least)
/// crossing, wall to wall, each through the doors in the walls it ends at, vaulted along their ways where the
/// height allows (flat and coffered where not); over the crossing a tower
/// stepped in, in what height is left; the corners solid. A door in an
/// arm's end opens straight in; elsewhere, a passage cut straight in to the
/// arm across its way. None if the footprint is too small for the arms to
/// leave corners.
pub(super) fn crossing(spec: &Spec) -> Option<Room> {
    let (sx, sz) = (spec.size.x, spec.size.y);
    let floor = level(spec)?;
    let room = spec.height - SLAB - floor;
    let (w, d) = (sx - 2.0 * T, sz - 2.0 * T);
    let arm = half((w.min(d) / 3.0).max(6.0));
    if w.min(d) - arm < 6.0 {
        return None;
    }
    // (Each arm through the doors in the walls it ends at, the most it can
    // take, the nearer the middle the better; along a side wall if that is
    // where they are, the crossing then off its middle or a T. Through the
    // middle alone, or kept off the side walls, nearly every door came in
    // by a passage: doors are mostly in corner cells.)
    let place = |side: f32, walls: [usize; 2]| {
        let (lo, hi) = (T + arm * 0.5, side - T - arm * 0.5);
        let mut at: Vec<f32> = spec.doors.iter().filter(|d| walls.contains(&d.0)).map(|d| d.1.clamp(lo, hi)).collect();
        at.push(side * 0.5);
        let takes = |c: f32| spec.doors.iter().filter(|d| walls.contains(&d.0) && (d.1 - c).abs() <= arm * 0.5 - 1.2).count();
        at.into_iter().max_by(|a, b| takes(*a).cmp(&takes(*b)).then(((b - side * 0.5).abs()).total_cmp(&(a - side * 0.5).abs()))).unwrap_or(side * 0.5)
    };
    let (cx, cz) = (place(sx, [2, 3]), place(sz, [0, 1]));
    let (x0, x1, z0, z1) = (cx - arm * 0.5, cx + arm * 0.5, cz - arm * 0.5, cz + arm * 0.5);
    let vaulted = room >= arm * 1.2;
    let h = if vaulted { half(arm * 1.2).min(room) } else { room.min(half(arm * 1.2)) };
    let mut b = B::new(Vec3::new(sx, spec.height, sz), Detail { reveals: true, coffers: true, bands: true, pilasters: true, beams: !vaulted, screens: false });
    // The arms: along z (x0..x1) and along x (z0..z1), wall to wall.
    let arms = [((x0, x1), (T, sz - T), 2usize), ((T, sx - T), (z0, z1), 0usize)];
    for &((a, c), (e, f), axis) in &arms {
        if vaulted {
            if axis == 2 {
                b.vault(2, a, c - a, floor, h - arm * 0.5, e, f - e);
            } else {
                b.vault(0, e, f - e, floor, h - arm * 0.5, a, c - a);
            }
        } else {
            b.carve(Vec3::new(a, floor, e), Vec3::new(c, floor + h, f));
            b.coffers((a, c), (e, f), floor + h, half(arm / 3.0).max(2.0));
            b.beams((a, c), (e, f), floor + h, half(arm * 0.75).max(3.0));
        }
    }
    b.carve(Vec3::new(x0, floor, z0), Vec3::new(x1, floor + h, z1));
    // The tower over the crossing, stepped in, in the height left.
    let tower = room - h;
    let steps = ((tower / 2.0).floor() as i32).clamp(0, 4);
    for k in 0..steps {
        let inset = 0.75 * (k + 1) as f32;
        let y = floor + h + tower * k as f32 / steps as f32;
        b.carve(Vec3::new(x0 + inset, y, z0 + inset), Vec3::new(x1 - inset, y + tower / steps as f32, z1 - inset));
    }
    let wall = if vaulted { h - arm * 0.5 } else { h };
    let bay = half(arm * 0.75).max(3.0);
    for &((a, c), (e, f), _) in &arms {
        b.rhythm((a, c), (e, f), (floor, floor + wall), bay, None, true);
        if !vaulted {
            b.bands((a, c), (e, f), (floor, floor + h));
        }
    }
    // Doors: in an arm's end, straight in; else a passage to the other arm.
    let mut passages = 0;
    let mut extras = Vec::new();
    for &(wall_i, u, y) in &spec.doors {
        let extra = match wall_i {
            2 if (u - cx).abs() > arm * 0.5 - 1.2 => z0 - T + 0.5,
            3 if (u - cx).abs() > arm * 0.5 - 1.2 => sz - T - z1 + 0.5,
            0 if (u - cz).abs() > arm * 0.5 - 1.2 => x0 - T + 0.5,
            1 if (u - cz).abs() > arm * 0.5 - 1.2 => sx - T - x1 + 0.5,
            _ => 0.0,
        };
        if extra > 0.0 {
            passages += 1;
        }
        extras.push(extra);
        door(&mut b, spec, (wall_i, u, y), extra);
    }
    b.note(format!(
        "{}: two halls {arm:.0} m wide crossing, {h:.1} m high, {}, a tower {tower:.1} m over the crossing; {} doors, {passages} by a passage",
        spec.name,
        if vaulted { "vaulted" } else { "flat, coffered" },
        spec.doors.len()
    ));
    light(&mut b, spec.size, floor, floor + wall);
    if steps > 0 {
        b.lamp(Vec3::new(cx, floor + h + tower - 1.0, cz), h + tower + 4.0, strength(h + tower + 4.0) * 3.0);
    }
    views(&mut b, spec, Vec3::new(cx, floor + h * 0.6, cz), &extras);
    Some(b.finish(floor))
}
