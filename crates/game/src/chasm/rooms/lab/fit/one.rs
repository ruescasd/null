//! Rooms on one level fitted to their doors (the network's places whose
//! doors are all on one storey: 62% of them): the lab's hypostyle, rotunda
//! and crossing, each taking its footprint, height and doors as given
//! rather than inventing them. Each on the door's level; the storeys above
//! it its height (proportions from the plan, the rest left as rock or a
//! lantern, a dome, a tower). A rotunda or a crossing that its doors or
//! footprint do not allow is not made (see `fitted`, which falls back to a
//! hypostyle).

use bevy::prelude::*;

use super::super::{B, Detail, Hang, Room, T, half, strength, thickness};
use super::{SLAB, Spec, intrusion, light, painting};

/// The level all the doors are on, if they are on one.
pub(super) fn level(spec: &Spec) -> Option<f32> {
    let y = spec.doors.first()?.2;
    spec.doors.iter().all(|d| (d.2 - y).abs() < 0.01).then_some(y)
}

/// A door through its wall, 2 m wide, arched, its reveals stepped on the
/// wall's faces; or, with `extra` metres on into the rock, a plain arched
/// passage in to the room, its reveals on the outer face only. (The reveals at the passage's far end, where it
/// meets the room, were cut through by a curved wall, a drum's, into
/// notches.)
fn door(b: &mut B, spec: &Spec, (wall, u, y): (usize, f32, f32), extra: f32) {
    let (sx, sz) = (spec.size.x, spec.size.y);
    let (axis, n, on) = match wall {
        0 => (0, 0.0, T),
        1 => (0, sx - T, sx - T - extra),
        2 => (2, 0.0, T),
        _ => (2, sz - T, sz - T - extra),
    };
    if extra <= 0.0 {
        b.door(axis, u - 1.0, 2.0, y, 2.5, n, T);
        return;
    }
    // (Its reveals on the wall's outer face only, where the way comes to it:
    // on its inner face, 4 m wide, they showed as slits beside the
    // passage, in a rotunda's portal.)
    b.arch(axis, u - 1.0, 2.0, y, 2.5, n, T);
    b.arch(axis, u - 1.0, 2.0, y, 2.5, on, extra);
    let outer = |d: f32| if n > 0.0 { n + T - d } else { 0.0 };
    for (k, d) in [(1.0, 0.5), (2.0, 0.25)] {
        b.arch(axis, u - 1.0 - k * 0.5, 2.0 + k, y, 2.5, outer(d), d);
    }
}

/// Clears a door's sill of what was laid along the walls after it was cut
/// (the plinth band), from its wall `extra` and half a metre on in.
fn sill(b: &mut B, spec: &Spec, (wall, u, y): (usize, f32, f32), extra: f32) {
    let (a, c) = (inward(spec, (wall, u - 1.0, y), -T, 0.0), inward(spec, (wall, u + 1.0, y), extra + 0.5, 0.75));
    b.carve(a.min(c), a.max(c));
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
/// the room's middle a little up; from inside each door, the same; from the
/// middle, at each door's mouth.
fn views(b: &mut B, spec: &Spec, mid: Vec3, extra: &[f32]) {
    for (k, &d) in spec.doors.iter().enumerate() {
        let eye = inward(spec, d, 1.0 + extra.get(k).copied().unwrap_or(0.0), 1.7);
        b.view(["floor", "door2", "door3", "door4", "door5", "door6"][k.min(5)], eye, mid);
    }
    // (And from the middle, at each door's mouth.)
    for (k, &d) in spec.doors.iter().enumerate() {
        let mouth = inward(spec, d, extra.get(k).copied().unwrap_or(0.0), 1.75);
        b.view(["mouth1", "mouth2", "mouth3", "mouth4", "mouth5", "mouth6"][k.min(5)], Vec3::new(mid.x, d.2 + 1.7, mid.z), mouth);
    }
}

/// A hypostyle: the hall filling the footprint, on the doors' level, as
/// high as a third of its mean side (5 m at least); stocky columns (a
/// sixth of its height thick) at the crossings of bays dividing each side
/// evenly, near four and a half thicknesses each; a clearing of a bay or two in
/// the middle, under a lantern rising into what height is left; columns
/// kept out of every door's way in; pilasters answering them along the
/// walls, niches between; coffers, beams on the columns, bands.
pub(super) fn hypostyle(spec: &Spec) -> Room {
    let (sx, sz) = (spec.size.x, spec.size.y);
    let floor = level(spec).unwrap_or(1.0);
    let room = spec.height - SLAB - floor;
    let (w, d) = (sx - 2.0 * T, sz - 2.0 * T);
    let h = half(((w + d) * 0.5 / 3.0).max(5.0)).min(room);
    // (Stocky, a forest: a sixth of the height thick, four and a half
    // thicknesses apart. By the classical orders, a tenth, in halls this
    // low they were half a metre thick, thin as poles: the user.)
    let dd = thickness(h, 6.0);
    let target = (dd * 4.5).max(4.0);
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
    let can = |i: i32, j: i32| {
        let (x, z) = (T + i as f32 * bx, T + j as f32 * bz);
        let cleared = x > cx.0 + 0.01 && x < cx.1 - 0.01 && z > cz.0 + 0.01 && z < cz.1 - 0.01;
        !cleared && !in_the_way(x, z)
    };
    // (Only where its mirrors across the hall's middle lines can stand too:
    // kept wherever no door's way in was, a column could stand alone in
    // its row, out of place: the user.)
    b.round = spec.name.bytes().fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32)) % 2 == 1;
    for i in 1..nx {
        for j in 1..nz {
            if can(i, j) && can(nx - i, j) && can(i, nz - j) && can(nx - i, nz - j) {
                b.column(T + i as f32 * bx, T + j as f32 * bz, dd, (floor, floor + h - beam));
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
    b.note(format!("{}: a hypostyle {sx:.0} by {sz:.0} m, {h:.1} m high, {columns} columns {dd:.1} m thick ({} mouldings), bays {bx:.1} by {bz:.1} m; a clearing under a lantern {lantern:.1} m higher; {} doors", spec.name, if b.round { "round" } else { "square" }, spec.doors.len()));
    light(&mut b, spec.size, floor, floor + h);
    if lantern >= 2.0 {
        let c = Vec3::new((cx.0 + cx.1) * 0.5, floor + h + lantern - 1.0, (cz.0 + cz.1) * 0.5);
        b.lamp(c, h + lantern + w * 0.5, strength(h + lantern + w * 0.5) * 2.0);
    }
    let mid = Vec3::new(sx * 0.5, floor + h * 0.4, sz * 0.5);
    if let Some(seed) = painting(spec) {
        b.hang(vec![((T, sx - T), (T, sz - T))], (floor, floor + h), seed, Hang::Bay);
    }
    if let Some(seed) = intrusion(spec) {
        b.intrude(seed);
    }
    views(&mut b, spec, mid, &[]);
    b.finish(floor)
}

/// A rotunda: a drum as wide as the footprint allows (near square only: no
/// more than 4:3), its wall as high as its radius (the Pantheon's) or as
/// the height allows, a stepped dome over it in what is left, an oculus in
/// its crown; the corners solid, but those a door is near, hollowed into
/// chambers opening into the drum, the door into them; other doors a
/// passage cut straight in to a portal in the drum, a flat-backed recess. None if a door is too near a corner to reach the drum.
pub(super) fn rotunda(spec: &Spec) -> Option<Room> {
    let (sx, sz) = (spec.size.x, spec.size.y);
    if sx.max(sz) / sx.min(sz) > 1.34 {
        return None;
    }
    let floor = level(spec)?;
    let room = spec.height - SLAB - floor;
    let rad = (sx.min(sz) - 2.0 * T) * 0.5;
    let (cx, cz) = (sx * 0.5, sz * 0.5);
    // Corner chambers: each corner a door is near hollowed out from the
    // walls to just inside the drum (its corner a metre past the drum's
    // diagonal), the door opening into it, it into the drum. (Doors are
    // mostly in corner cells; a passage from one, with a portal, dug 6 m
    // through the corner's rock, and two from one corner's walls hollowed
    // it out between them.)
    let diag = rad / std::f32::consts::SQRT_2;
    let (kx, kz) = (cx - diag + 1.0 - T, cz - diag + 1.0 - T);
    let chamber_box = |k: usize| -> (Vec3, Vec3) {
        let (x0, x1) = if k == 0 || k == 3 { (T, T + kx) } else { (sx - T - kx, sx - T) };
        let (z0, z1) = if k < 2 { (T, T + kz) } else { (sz - T - kz, sz - T) };
        (Vec3::new(x0, floor, z0), Vec3::new(x1, floor + 4.5, z1))
    };
    let mut chambers = [false; 4];
    // Each door: into a corner chamber, or by a passage to a portal in the
    // drum (a recess with a flat back, as deep as the drum's curve across
    // it, the passage opening in its back; cut straight into the curve, an
    // arched passage's mouth was a warped edge).
    let mut ways: Vec<((usize, f32, f32), Option<(f32, f32, f32)>)> = Vec::new();
    for &(wall, u, y) in &spec.doors {
        let (along_k, side) = match wall {
            0 | 1 => (kz, sz),
            _ => (kx, sx),
        };
        let low = u + 1.0 <= T + along_k - 0.5;
        let high = u - 1.0 >= side - T - along_k + 0.5;
        if low || high {
            let k = match (wall, high) {
                (0, false) | (2, false) => 0,
                (1, false) | (2, true) => 1,
                (1, true) | (3, true) => 2,
                _ => 3,
            };
            chambers[k] = true;
            ways.push(((wall, u, y), None));
            continue;
        }
        let (off, to_centre) = match wall {
            0 | 1 => (u - cz, cx - T),
            _ => (u - cx, cz - T),
        };
        let pw = if off.abs() + 2.0 < rad - 0.5 { 2.0 } else { 1.5 };
        if off.abs() + pw > rad - 0.5 {
            return None;
        }
        let depth = |o: f32| to_centre - (rad * rad - o * o).sqrt();
        // (Its back where the drum comes nearest across it, half a metre
        // short, so the back is rock all across; its front past where the
        // drum is furthest.)
        ways.push(((wall, u, y), Some(((depth((off.abs() - pw).max(0.0)) - 0.5).max(0.25), depth(off.abs() + pw) + 0.5, pw))));
    }
    let wall_h = half(rad.min(room - 2.0)).max(4.0).min(room);
    let stars = planetarium(spec);
    let mut b = B::new(Vec3::new(sx, spec.height, sz), Detail { reveals: true, coffers: true, bands: true, ..Default::default() });
    // (The doors and passages first, then the drum: cut after it, their
    // arches' shapes reaching into it stood there as slivers in the open.)
    let portal = |d: (usize, f32, f32), (back, front, pw): (f32, f32, f32), top: f32| {
        let (p, q) = (inward(spec, (d.0, d.1 - pw, d.2), back, 0.0), inward(spec, (d.0, d.1 + pw, d.2), front, top));
        (p.min(q), p.max(q))
    };
    for &(d, way) in &ways {
        door(&mut b, spec, d, way.map_or(0.0, |w| w.0));
        if let Some(w) = way {
            let (p, q) = portal(d, w, 4.5);
            b.carve(p, q);
        }
    }
    for k in (0..4).filter(|&k| chambers[k]) {
        let (p, q) = chamber_box(k);
        b.carve(p, q);
    }
    b.ring(cx, cz, (0.0, rad), (floor, floor + wall_h), false);
    b.round_bands((cx, cz), rad, (floor, floor + wall_h));
    // (The drum's plinth laid across the portals' and chambers' floors:
    // cleared.)
    for &(d, way) in &ways {
        if let Some(w) = way {
            let (p, q) = portal(d, w, 0.75);
            b.carve(p, q);
        }
        sill(&mut b, spec, d, way.map_or(0.0, |w| w.0));
    }
    for k in (0..4).filter(|&k| chambers[k]) {
        let (p, q) = chamber_box(k);
        b.carve(p, Vec3::new(q.x, floor + 0.75, q.z));
        b.lamp((p + q) * 0.5 + Vec3::Y * 1.0, kx.max(kz) * 2.0 + 4.0, strength(kx.max(kz) * 2.0 + 4.0) * if stars { 0.1 } else { 0.5 });
    }
    // Engaged columns round the drum, twelve places about it, each sunk
    // two fifths into the wall, by the orders (a tenth of the wall's
    // height thick), up to its cornice; but by a door's mouth or a
    // chamber's opening. (The rotundas were bare, the user; niches cut into
    // the curve, their half-domes in quarter-metre layers, read as stacks
    // of coins.)
    let dc = thickness(wall_h, 10.0);
    let mouths: Vec<f32> = spec.doors.iter().map(|&d| {
        let p = inward(spec, d, 0.0, 0.0);
        (p.z - cz).atan2(p.x - cx)
    }).collect();
    let openings: Vec<f32> = (0..4).filter(|&k| chambers[k]).map(|k| [-0.75f32, -0.25, 0.25, 0.75][k] * std::f32::consts::PI).collect();
    let mut columns = 0;
    for k in 0..12 {
        let a = (k as f32 + 0.5) * std::f32::consts::TAU / 12.0 - std::f32::consts::PI;
        let near = |b: f32| {
            let d = (a - b).rem_euclid(std::f32::consts::TAU);
            d.min(std::f32::consts::TAU - d) < (dc + 2.5) / rad
        };
        if mouths.iter().chain(&openings).any(|&b| near(b)) {
            continue;
        }
        let at = rad - dc * 0.5 * 0.2;
        b.column(cx + a.cos() * at, cz + a.sin() * at, dc, (floor, floor + wall_h - 1.0));
        columns += 1;
    }
    // The dome: stepped in, ring by ring, to an oculus, in the height left.
    let above = room - wall_h;
    let levels = ((above / 1.5).floor() as i32).clamp(0, 6);
    let (dr, dh) = (half(rad * 0.75 / levels.max(1) as f32).max(0.5), if levels > 0 { above / levels as f32 } else { 0.0 });
    for k in 1..=levels {
        let y = floor + wall_h + (k - 1) as f32 * dh;
        b.ring(cx, cz, (0.0, rad - k as f32 * dr), (y, y + dh), false);
    }
    // (A planetarium's dome closed, dark: no oculus.)
    if !stars {
        let oculus = (rad - (levels + 1) as f32 * dr).max(1.0);
        b.ring(cx, cz, (0.0, oculus), (floor + wall_h + levels as f32 * dh, spec.height), false);
        b.sky(Vec3::new(cx, spec.height, cz), oculus + 1.0, room + 12.0, strength(room + 12.0) * 6.0);
    }
    // A planetarium: the projector's light over a pedestal in the middle, half way up
    // the room or near it, as far from the floor as from the walls and the
    // dome (low, the floor round it was bright, the walls far and faint);
    // lit only by it (its stars, and the little light it lets out all
    // round: see `projector.rs`), and the chambers' lamps turned low.
    let projector = stars.then(|| {
        let up = (room * 0.45).clamp(3.5, wall_h);
        let at = Vec3::new(cx, floor + up, cz);
        b.column(cx, cz, thickness(up, 8.0).max(1.25), (floor, floor + up - 1.5));
        let reach = 2.0 * (rad * rad + up.max(room - up).powi(2)).sqrt() + 4.0;
        b.view("projector", Vec3::new(cx + rad * 0.6, floor + 1.7, cz + rad * 0.3), at);
        b.view("stars", Vec3::new(cx - rad * 0.7, floor + 1.7, cz), Vec3::new(cx + rad, floor + wall_h, cz));
        b.note("a planetarium: dark, its dome closed, stars thrown from over an empty pedestal in the middle");
        (at, reach)
    });
    let extras: Vec<f32> = ways.iter().map(|w| w.1.map_or(0.0, |w| w.0)).collect();
    b.note(format!(
        "{}: a rotunda {:.0} m across, its wall {wall_h:.1} m high, {columns} engaged columns round it, a dome stepped {levels} times{}; {} doors, {} into corner chambers ({} of them), the rest by passages to portals",
        spec.name,
        rad * 2.0,
        if stars { "" } else { " to an oculus" },
        spec.doors.len(),
        ways.iter().filter(|w| w.1.is_none()).count(),
        chambers.iter().filter(|c| **c).count()
    ));
    if !stars {
        light(&mut b, spec.size, floor, floor + wall_h);
    }
    views(&mut b, spec, Vec3::new(cx, floor + wall_h * 0.6, cz), &extras);
    let mut r = b.finish(floor);
    r.projector = projector;
    Some(r)
}

/// Whether a rotunda is a planetarium (half of them, by its place's name).
fn planetarium(spec: &Spec) -> bool {
    let h = spec.name.bytes().fold(0x811c9dc5u32 ^ 0x51a7, |h, c| (h ^ c as u32).wrapping_mul(16777619));
    (h >> 7) % 2 == 0
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
    // take, the nearer the middle the better, a metre from a side wall at
    // least, the crossing then off its middle. Through the middle alone,
    // nearly every door came in by a passage: doors are mostly in corner
    // cells. Right along a side wall, its own side wall was in the plane of
    // the other arm's end wall, and the two arms' rhythms of pilasters and
    // niches, out of step, cut into each other: an arch left standing on
    // its own.)
    let place = |side: f32, walls: [usize; 2]| {
        let (lo, hi) = (T + arm * 0.5 + 1.0, side - T - arm * 0.5 - 1.0);
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
    // Doors: in an arm's end, straight in; else a passage to the other arm.
    // (Cut first, then the arms: cut after, their arches' shapes reaching
    // into an arm stood there as slivers in the open.)
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
    // Over a vaulted arm, a transverse arch at each bay (where its
    // pilasters stand), half a metre wide, 0.4 m deep under the vault,
    // springing where it springs; but in the crossing and at the arm's ends.
    // (The vaulted arms' beams: the user wanted beams on the ceiling.)
    if vaulted {
        let bay = half(arm * 0.75).max(3.0);
        let spring = h - arm * 0.5;
        let mut arches = 0;
        for &((a, c), (e, f), axis) in &arms {
            let (from, to, skip) = if axis == 2 { (e, f, (z0, z1)) } else { (a, c, (x0, x1)) };
            let len = to - from;
            let n = (len / bay).floor() as i32;
            let start = half((len - n as f32 * bay) * 0.5);
            for k in 0..=n {
                let u = from + start + k as f32 * bay;
                if u < from + 0.6 || u > to - 0.6 || (u > skip.0 - 0.5 && u < skip.1 + 0.5) {
                    continue;
                }
                // (Each foot carried by a corbel out of the wall, two steps;
                // with no wall under a foot, over an opening, no arch. Its
                // feet stood in the air where no pilaster happened to be:
                // the user.)
                let y = floor + spring;
                let feet: [(Vec3, Vec3); 2] = if axis == 2 { [(Vec3::new(a, y, u), Vec3::X), (Vec3::new(c, y, u), Vec3::NEG_X)] } else { [(Vec3::new(u, y, e), Vec3::Z), (Vec3::new(u, y, f), Vec3::NEG_Z)] };
                if !feet.iter().all(|&(p, into)| [0.3, 0.8, 1.3].iter().all(|&dy| b.is_solid(p - into * 0.1 - Vec3::Y * dy))) {
                    continue;
                }
                arches += 1;
                let top = floor + spring + arm * 0.5;
                if axis == 2 {
                    b.solid(Vec3::new(a, floor + spring, u - 0.25), Vec3::new(c, top, u + 0.25));
                    b.vault(2, a + 0.4, c - a - 0.8, floor, spring, u - 0.25, 0.5);
                } else {
                    b.solid(Vec3::new(u - 0.25, floor + spring, e), Vec3::new(u + 0.25, top, f));
                    b.vault(0, e + 0.4, f - e - 0.8, floor, spring, u - 0.25, 0.5);
                }
                let across = if axis == 2 { Vec3::Z } else { Vec3::X };
                // (Its steps: 0.4 m out for the half metre under the foot,
                // 0.2 m for the half metre under that.)
                for &(p, into) in &feet {
                    for (out, (below, above)) in [(0.4f32, (0.5f32, 0.0f32)), (0.2, (1.0, 0.5))] {
                        let (q0, q1) = (p - across * 0.25 - Vec3::Y * below, p + across * 0.25 + into * out - Vec3::Y * above);
                        b.solid(q0.min(q1), q0.max(q1));
                    }
                }
            }
        }
        b.note(format!("{arches} transverse arches over the arms"));
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
    for (&d, &extra) in spec.doors.iter().zip(&extras) {
        sill(&mut b, spec, d, extra);
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
    if let Some(seed) = painting(spec) {
        b.hang(arms.iter().map(|a| (a.0, a.1)).collect(), (floor, floor + wall), seed, Hang::Bay);
    }
    if let Some(seed) = intrusion(spec) {
        b.intrude(seed);
    }
    views(&mut b, spec, Vec3::new(cx, floor + h * 0.6, cz), &extras);
    Some(b.finish(floor))
}
