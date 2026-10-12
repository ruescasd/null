//! A region of the chasm built for real (`--opt chasm --opt region`; step
//! 3b): the entrance and the first places down the spine (`--set spine=N`,
//! 3), and the places paths join to them (but `--opt noside`), with the
//! paths between them all. Each place its room (see `rooms::place`), at its
//! true place and depth; each step of a path inside the rock a tile (see
//! `rooms::tiles`): a passage, a stair a storey up the cell, a portal where
//! it comes out on the chasm; out there, plainly (for now), a ledge's shelf
//! along the face, a bridge's deck across, a balcony's slab, a glass
//! stepwell (a room glazed on the chasm). The rock is the blockout's
//! surfaces, open where what is built into it opens; inside it nothing but
//! what is built. You start on the rim at the entrance, a stair down into
//! the rock.

use std::collections::HashSet;

use bevy::prelude::*;

use super::super::{CELL, Holes, ORIGIN, RIM, blockout, tree};
use super::{Cells, DIRS, Door, Kind, Place, STOREY, What, network, spec};
use crate::chasm::Parts;
use crate::chasm::rooms::tiles::{self, Arm, Opening, Tile};
use crate::chasm::rooms::{self, Fixtures, RoomViews};

/// Where a storey's floor is (m up): storey 0's a storey under the rim.
fn floor(s: i32) -> f32 {
    RIM - (s + 1) as f32 * STOREY
}

/// A storey's band: from its floor's slab to its top (a storey high).
fn band(top: i32, bottom: i32) -> (f32, f32) {
    (floor(bottom) - 1.0, floor(top) + STOREY - 1.0)
}

/// Which side of a cell a way goes out by (as `tiles` and `Spec` number
/// them).
fn side(d: IVec2) -> usize {
    match (d.x, d.y) {
        (-1, 0) => 0,
        (1, 0) => 1,
        (0, -1) => 2,
        _ => 3,
    }
}

/// A cell's corner, and its middle (at height 0).
fn corner(c: IVec2) -> Vec3 {
    ORIGIN + Vec3::new(c.x as f32 * CELL, 0.0, c.y as f32 * CELL)
}

fn middle(c: IVec2) -> Vec3 {
    corner(c) + Vec3::new(CELL * 0.5, 0.0, CELL * 0.5)
}

fn flat(d: IVec2) -> Vec3 {
    Vec3::new(d.x as f32, 0.0, d.y as f32)
}

/// The places and paths of the region: the entrance and the spine's first
/// `spine` places; with `side`, the places a path joins to them; and every
/// path between two of them.
fn chosen(net: &super::Network, spine: usize, side: bool) -> (HashSet<usize>, Vec<usize>) {
    let mut places: HashSet<usize> = HashSet::from([0]);
    for &p in net.spine.iter().take(spine) {
        places.insert(net.paths[p].0);
        places.insert(net.paths[p].1);
    }
    if side {
        let core = places.clone();
        for (a, b, _, _) in &net.paths {
            if core.contains(a) && *a != 0 {
                places.insert(*b);
            }
            if core.contains(b) && *b != 0 {
                places.insert(*a);
            }
        }
    }
    let paths = (0..net.paths.len()).filter(|&i| places.contains(&net.paths[i].0) && places.contains(&net.paths[i].1)).collect();
    (places, paths)
}

/// Where you start: on the rim past the entrance's stair, looking down it.
fn start_at(net: &super::Network, paths: &[usize]) -> [f32; 5] {
    let entrance = net.places[0].at;
    let first = paths.iter().map(|&p| &net.paths[p]).find(|p| p.0 == 0 || p.1 == 0);
    let inward = first.map_or(IVec2::X, |(a, _, steps, _)| if *a == 0 { steps[1].at - steps[0].at } else { steps[steps.len() - 2].at - steps[steps.len() - 1].at });
    let eye = middle(entrance) - flat(inward) * (CELL * 0.5 + 3.0) + Vec3::Y * (RIM + 1.7);
    let look = middle(entrance) + Vec3::Y * (RIM - 4.0) - eye;
    [eye.x, eye.y, eye.z, (-look.x).atan2(-look.z).to_degrees(), look.y.atan2(Vec2::new(look.x, look.z).length()).to_degrees()]
}

/// The way a walker takes through the region (`--opt walkbot`): each path
/// from its first step out of a place to its last, cell to cell at its
/// floors (a stair's cell at half way), out through each portal and along
/// what is out there; a glass stepwell jumped across (its stair is its
/// own); each path after the first begun afresh, jumped to.
pub fn route(seed: u32, spine: usize, side: bool) -> Vec<(Vec3, String)> {
    let plan = tree(seed);
    let net = network(&plan, seed);
    let cells = Cells(&plan);
    let (_, paths) = chosen(&net, spine, side);
    let mut way = Vec::new();
    for (k, &pi) in paths.iter().enumerate() {
        let (a, b, steps, _) = &net.paths[pi];
        let n = steps.len();
        let at = |i: usize| {
            let (c, s) = (steps[i], steps[i.saturating_sub(1)]);
            let y = if i > 0 && c.storey != s.storey { (floor(c.storey) + floor(s.storey)) * 0.5 } else { floor(c.storey) };
            middle(c.at) + Vec3::Y * y
        };
        let (first, last) = (if *a == 0 { 0 } else { 1 }, if *b == 0 { n - 1 } else { n - 2 });
        if *a == 0 {
            // (From the rim beside the entrance's stair's top, down it.)
            let inward = flat(steps[1].at - steps[0].at);
            way.push((middle(steps[0].at) - inward * (CELL * 0.5 + 2.0) + Vec3::Y * RIM, format!("jump to the rim at the entrance (path {pi})")));
            way.push((middle(steps[0].at) - inward * (CELL * 0.5 - 0.5) + Vec3::Y * (RIM - 0.25), format!("path {pi}: the entrance's stair's top")));
            way.push((middle(steps[0].at) + Vec3::Y * (RIM - 3.0), format!("path {pi}: down the entrance's stair")));
        } else {
            way.push((at(first), format!("jump to path {pi} begins")));
        }
        let _ = k;
        let mut i = first + 1;
        while i <= last {
            let st = steps[i];
            let label = format!("path {pi} step {i} {:?}", st.what);
            if let What::Out(kind) = st.what {
                let Some(d) = DIRS.into_iter().find(|&d| cells.void(st.at + d)) else { i += 1; continue };
                let face = |c: IVec2| middle(c) + flat(d) * (CELL * 0.5) + Vec3::Y * floor(st.storey);
                let mut e = i;
                while e + 1 <= last && matches!(steps[e + 1].what, What::Out(_)) {
                    e += 1;
                }
                way.push((at(i), label.clone()));
                match kind {
                    Kind::Ledge => {
                        way.push((face(st.at) + flat(d) * 1.5, format!("{label}: out on the ledge")));
                        way.push((face(steps[e].at) + flat(d) * 1.5, format!("{label}: along the ledge")));
                        way.push((at(e), format!("{label}: back in")));
                    }
                    Kind::Bridge => {
                        way.push((face(st.at) + flat(d) * 0.5, format!("{label}: onto the bridge")));
                        for j in i + 1..e {
                            way.push((at(j), format!("{label}: over the chasm")));
                        }
                        way.push((at(e), format!("{label}: across")));
                    }
                    Kind::Balcony => {
                        way.push((face(st.at) + flat(d) * 2.5, format!("{label}: out on the balcony")));
                        way.push((face(steps[e].at) + flat(d) * 2.5, format!("{label}: along it")));
                        way.push((at(e), format!("{label}: back in")));
                    }
                    Kind::Glass => {
                        way.push((at(e + 1), format!("jump to {label}: past the stepwell")));
                    }
                }
                i = e + 1;
                continue;
            }
            way.push((at(i), label));
            i += 1;
        }
    }
    way
}

/// Where you start in the region; `inside` (`--opt inside`), on the first
/// path's floor a few cells in, looking along it.
pub fn start(seed: u32, spine: usize, side: bool, inside: bool) -> [f32; 5] {
    let plan = tree(seed);
    let net = network(&plan, seed);
    let (_, paths) = chosen(&net, spine, side);
    if inside && let Some((_, _, steps, _)) = paths.first().map(|&p| &net.paths[p]) {
        let k = (steps.len() / 2).max(1).min(steps.len() - 2);
        let (here, next) = (steps[k], steps[k + 1]);
        let eye = middle(here.at) + Vec3::Y * (floor(here.storey) + 1.7);
        let look = middle(next.at) + Vec3::Y * (floor(next.storey) + 1.5) - eye;
        return [eye.x, eye.y, eye.z, (-look.x).atan2(-look.z).to_degrees(), look.y.atan2(Vec2::new(look.x, look.z).length()).to_degrees()];
    }
    start_at(&net, &paths)
}

/// The region built: its surfaces and lights, what its rooms hold, where to
/// see them from, a line on each room.
pub(in crate::chasm) fn build(seed: u32, spine: usize, side_links: bool) -> (Parts, Fixtures, RoomViews, Vec<String>) {
    let plan = tree(seed);
    let net = network(&plan, seed);
    let cells = Cells(&plan);
    let (places, paths) = chosen(&net, spine, side_links);
    let mut parts = Parts::default();
    let mut fixtures = Fixtures::default();
    let mut views = RoomViews::default();
    let mut report = Vec::new();
    let mut holes = Holes::default();
    let mut tiles = Vec::new();
    let mut boxes: Vec<(String, Vec3, Vec3)> = Vec::new();
    // (What is built in a cell, from `top` to `bottom` storeys, opens the
    // faces it stands at: it is their surface there.)
    let open = |holes: &mut Holes, c: IVec2, (top, bottom): (i32, i32)| {
        for (k, d) in DIRS.into_iter().enumerate() {
            if cells.void(c + d) {
                holes.faces.entry((c, k)).or_default().push(band(top, bottom));
            }
        }
    };
    let entrance = net.places[0].at;
    let mut doors: Vec<Vec<Door>> = vec![Vec::new(); net.places.len()];
    let mut walked = 0;
    for &pi in &paths {
        let (a, b, steps, _) = &net.paths[pi];
        let n = steps.len();
        doors[*a].push(Door { cell: steps[0].at, out: steps[1].at - steps[0].at, storey: steps[0].storey });
        doors[*b].push(Door { cell: steps[n - 1].at, out: steps[n - 2].at - steps[n - 1].at, storey: steps[n - 1].storey });
        let room = |i: usize| (i == 0 && *a != 0) || (i == n - 1 && *b != 0);
        let out = |i: usize| matches!(steps[i].what, What::Out(_));
        let glass = |i: usize| matches!(steps[i].what, What::Out(Kind::Glass));
        let void = |i: usize| cells.void(steps[i].at);
        // (Where two steps meet: at the earlier's storey; a stair's cell
        // climbs from the storey it is come to at to its own.)
        let edge = |i: usize, j: usize| steps[i.min(j)].storey;
        walked += n;
        // (A view along the way, midway along it, where it is inside.)
        if let Some(m) = (n / 2..n - 1).find(|&m| matches!(steps[m].what, What::In) && !room(m)) {
            let (c, next) = (steps[m], steps[m + 1]);
            let y = floor(c.storey) + 1.7;
            views.0.push((format!("path{pi}_inside"), middle(c.at) + Vec3::Y * y, middle(next.at) + Vec3::Y * (floor(next.storey) + 1.5)));
        }
        let mut i = 0;
        while i < n {
            let st = steps[i];
            // Out on the chasm: what is out there, plainly.
            if out(i) && (i == 0 || !out(i - 1)) {
                let mut e = i;
                while e + 1 < n && out(e + 1) {
                    e += 1;
                }
                exposure(&steps[i..=e], &cells, &steps[i - 1], &steps[(e + 1).min(n - 1)], &mut parts, &mut holes, &mut fixtures, &mut views, &mut report, &mut boxes, pi);
            }
            if room(i) || glass(i) || void(i) || (out(i) && i > 0 && i + 1 < n && out(i - 1) && out(i + 1)) {
                i += 1;
                continue;
            }
            let mut arms: Vec<(usize, i32, Opening)> = Vec::new();
            for j in [i.wrapping_sub(1), i + 1] {
                if j >= n {
                    continue;
                }
                let d = steps[j].at - st.at;
                if out(i) && out(j) && !glass(j) {
                    // Out onto the chasm, toward the void nearest where the
                    // way goes on.
                    if let Some(dir) = DIRS.into_iter().filter(|&e| cells.void(st.at + e)).min_by_key(|&e| (st.at + e - steps[j].at).length_squared())
                        && !arms.iter().any(|a| a.2 == Opening::Portal)
                    {
                        arms.push((side(dir), edge(i, j), Opening::Portal));
                    }
                } else if d.abs().element_sum() == 1 {
                    arms.push((side(d), edge(i, j), if room(j) || glass(j) { Opening::Door } else { Opening::Pass }));
                }
            }
            if arms.is_empty() {
                i += 1;
                continue;
            }
            let up = st.at == entrance && st.storey == 0;
            let top = arms.iter().map(|a| a.1).min().unwrap();
            let bottom = arms.iter().map(|a| a.1).max().unwrap();
            let (y0, y1) = if up { (floor(0) - 1.0, RIM) } else { band(top, bottom) };
            let origin = corner(st.at) + Vec3::Y * y0;
            tiles.push(Tile { origin, height: y1 - y0, arms: arms.iter().map(|&(side, s, kind)| Arm { side, floor: floor(s) - y0, kind }).collect(), lamp: i % 2 == 0 || arms.iter().any(|a| a.2 == Opening::Portal), up });
            open(&mut holes, st.at, (top, bottom));
            if up {
                holes.tops.insert(st.at);
            }
            i += 1;
        }
    }
    // The places' rooms, each with the doors of the paths built.
    let mut ps: Vec<usize> = places.iter().copied().filter(|&p| p != 0 && !doors[p].is_empty()).collect();
    ps.sort();
    for p in ps {
        let place = net.places[p];
        let spec = spec(&place, &doors[p], format!("place{p}"));
        let origin = corner(place.at) + Vec3::Y * (floor(place.storeys.1) - 1.0);
        report.push(rooms::place(&spec, origin, p, None, &mut parts, &mut fixtures, &mut views));
        boxes.push((format!("place{p}"), origin, origin + Vec3::new(spec.size.x, spec.height, spec.size.y)));
        for c in place_cells(&place) {
            open(&mut holes, c, place.storeys);
        }
    }
    for t in &tiles {
        boxes.push(("tile".into(), t.origin, t.origin + Vec3::new(CELL, t.height, CELL)));
    }
    // (What is built must not overlap: each box against the others.)
    let mut overlaps = 0;
    for i in 0..boxes.len() {
        for j in i + 1..boxes.len() {
            let (lo, hi) = (boxes[i].1.max(boxes[j].1), boxes[i].2.min(boxes[j].2));
            if (hi - lo).cmpgt(Vec3::splat(0.01)).all() {
                overlaps += 1;
                if overlaps <= 12 {
                    report.push(format!("OVERLAP {} at {:.0} and {} at {:.0}", boxes[i].0, boxes[i].1, boxes[j].0, boxes[j].1));
                }
            }
        }
    }
    report.push(format!("{overlaps} overlaps"));
    tiles::build(&tiles, &mut parts);
    blockout(&plan, &holes, &mut parts.stone);
    report.push(format!("the region: {} places, {} paths, {} steps, {} tiles", places.len(), paths.len(), walked, tiles.len()));
    let [x, y, z, yaw, pitch] = start_at(&net, &paths);
    let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
    let eye = Vec3::new(x, y, z);
    views.0.insert(0, ("entrance".into(), eye, eye + Vec3::new(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()) * 10.0));
    (parts, fixtures, views, report)
}

fn place_cells(p: &Place) -> Vec<IVec2> {
    (0..p.size.y).flat_map(|z| (0..p.size.x).map(move |x| p.at + IVec2::new(x, z))).collect()
}

/// What is out on the chasm for a run of steps out there (`before` the step
/// it is come to from, `after` the one it goes on to), plainly: a shelf
/// along the face (a ledge), a deck across (a bridge), a slab out from it
/// (a balcony), each a storey's floor's height, lamps at its ends; or a
/// glass stepwell, a room glazed on the chasm.
#[allow(clippy::too_many_arguments)]
fn exposure(run: &[super::Step], cells: &Cells, before: &super::Step, after: &super::Step, parts: &mut Parts, holes: &mut Holes, fixtures: &mut Fixtures, views: &mut RoomViews, report: &mut Vec<String>, boxes: &mut Vec<(String, Vec3, Vec3)>, path: usize) {
    let What::Out(kind) = run[0].what else { return };
    let first = run[0];
    let Some(d) = DIRS.into_iter().find(|&d| cells.void(first.at + d)) else { return };
    let y = floor(first.storey);
    let face = |c: IVec2| middle(c) + flat(d) * (CELL * 0.5);
    let deck = |parts: &mut Parts, p: Vec3, q: Vec3| {
        let (lo, hi) = (p.min(q), p.max(q));
        let (c, e) = ((lo + hi) * 0.5, (hi - lo) * 0.5);
        parts.stone.oriented(c, Vec3::X * e.x, Vec3::Y * e.y, Vec3::Z * e.z);
        // (Walked on: spots over it in the columns the floors' are in.)
        let s = crate::chasm::nav::SPACING;
        let first = |a: f32| ((a - 0.375) / s).ceil() * s + 0.375;
        // (Up to a tenth of a metre from its ends: the spots of the tile it
        // meets are half a metre on, joined.)
        let mut z = first(lo.z + 0.1);
        while z < hi.z - 0.1 {
            let mut x = first(lo.x + 0.1);
            while x < hi.x - 0.1 {
                parts.walkable.push(Vec3::new(x, hi.y, z));
                x += s;
            }
            z += s;
        }
    };
    let lamp = |parts: &mut Parts, p: Vec3| {
        parts.lights.push((p, 18.0, 0.0006 * (18.0f32 / 14.0).powi(2) * 2.0));
        parts.glow.oriented(p, Vec3::X * 0.15, Vec3::Y * 0.15, Vec3::Z * 0.15);
    };
    let thick = Vec3::Y * 1.2;
    // (Views: out through its portal from inside, and back at the face
    // from out on it.)
    if kind != Kind::Glass {
        let eye = middle(first.at) - flat(d) * 2.0 + Vec3::Y * (y + 1.7);
        views.0.push((format!("path{path}_portal"), eye, eye + flat(d) * 10.0 - Vec3::Y * 2.0));
        let out = face(first.at) + flat(d) * 2.0 + Vec3::Y * (y + 1.7);
        let side = flat(IVec2::new(d.y, d.x)) * 6.0;
        views.0.push((format!("path{path}_out"), out + side, face(first.at) + Vec3::Y * (y + 1.0)));
    }
    match kind {
        Kind::Ledge => {
            let last = run[run.len() - 1];
            let t = flat((last.at - first.at).signum());
            deck(parts, face(first.at) - t * 2.5 + Vec3::Y * y - thick, face(last.at) + t * 2.5 + flat(d) * 3.0 + Vec3::Y * y);
            for c in [first.at, last.at] {
                lamp(parts, face(c) + flat(d) * 1.0 + Vec3::Y * (y + 4.2));
            }
        }
        Kind::Bridge => {
            let across = run[run.len() - 1];
            let side = flat(IVec2::new(d.y, d.x)) * 1.75;
            let far = middle(across.at) - flat(d) * (CELL * 0.5);
            deck(parts, face(first.at) - side + Vec3::Y * y - thick, far + side + Vec3::Y * y);
            for p in [face(first.at), far] {
                lamp(parts, p + Vec3::Y * (y + 4.2));
            }
        }
        Kind::Balcony => {
            let back = run[run.len() - 1];
            let t = flat((back.at - first.at).signum());
            deck(parts, face(first.at) - t * 2.5 + Vec3::Y * y - thick, face(back.at) + t * 2.5 + flat(d) * 5.0 + Vec3::Y * y);
            lamp(parts, face(first.at) + flat(d) * 2.5 + t * 4.0 + Vec3::Y * (y + 4.2));
        }
        Kind::Glass => {
            // A room of the two cells, the storeys between, its doors where
            // the way comes in and goes on, glazed on the chasm.
            let (a, b) = (run[0], run[run.len() - 1]);
            let place = Place { at: a.at.min(b.at), size: (b.at - a.at).abs() + IVec2::ONE, storeys: (a.storey.min(b.storey), a.storey.max(b.storey)) };
            let ds = [Door { cell: a.at, out: before.at - a.at, storey: a.storey }, Door { cell: b.at, out: after.at - b.at, storey: b.storey }];
            let spec = spec(&place, &ds, format!("stepwell{path}"));
            let origin = corner(place.at) + Vec3::Y * (floor(place.storeys.1) - 1.0);
            report.push(rooms::place(&spec, origin, 0, Some(side(d)), parts, fixtures, views));
            boxes.push((format!("stepwell{path}"), origin, origin + Vec3::new(spec.size.x, spec.height, spec.size.y)));
            for c in place_cells(&place) {
                for (k, e) in DIRS.into_iter().enumerate() {
                    if cells.void(c + e) {
                        holes.faces.entry((c, k)).or_default().push(band(place.storeys.0, place.storeys.1));
                    }
                }
            }
        }
    }
}
