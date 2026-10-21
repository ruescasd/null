//! Slabs (the user's, after Valheim's Mistlands): big plates thrust up out of
//! the ground, tilted, their foot buried, towering over all round them; in
//! fields of several, each its own way about the field's (several,
//! off-kilter, read as fragments of something larger, the user, with the
//! mega backdrops). Of the world's own shapes: a big plate's outline, so they
//! speak the plates' language. Two kinds, to compare (the game's `--set
//! slabs N`):
//! 1. slabs: plates, long (the user: they point at the sky, their length
//!    dominates), tapering and thinning to their top; how long and how steep
//!    varied widely, to judge which;
//! 2. cliff blocks: the ground itself tilted up (the user: they read as
//!    objects, not landscape; the real ones are "half-way to sideways
//!    cliffs", much thicker, varied across their width too): thick as they
//!    are wide, of parallel bands along their length, each its own height,
//!    tilt and broken end (ledges), deep strata stepping on their flanks,
//!    less steep, more buried.
//!
//! Detail, till they escape the polygon (bare, they read as sheets of card
//! in open country, the user: "more and more detail"):
//! - strata, layers each its own thickness and tone, each a little in from
//!   the one under it, thinning towards its tip;
//! - each broken off at its top end along its own slanting line, staggered;
//! - every edge rough, jagged at every scale (as the lightning, the relief),
//!   the broken ones the most;
//! - its faces in relief, plates within plates (Voronoi cells, each proud or
//!   sunk), as the ground's own: a piece of it torn up and stood on end;
//! - cracked across into pieces, each a little apart and pivoted, as if it
//!   broke as it rose;
//! - rubble round its foot, where it came up through the ground.

use glam::{DQuat, DVec2, DVec3, Vec3};

use super::{Cache, ColumnMesh, GRID, PlateWorld};

/// Fields: the world in squares this wide (m), so many of them a field, and
/// in one, the chance a big plate there is a slab at its middle, falling to
/// none this far from it (m).
const FIELD: (f64, f64, f64, f64) = (1024.0, 0.2, 0.55, 420.0);
/// How far a slab's heading strays from its field's (degrees), and how much
/// of its plate it keeps (shrunk to its middle).
const STRAY: f64 = 40.0;
const SHRINK: f64 = 0.75;

/// A kind's shape, each at least and at most: how steep (degrees); its
/// plate's outline stretched along it and narrowed across (times), narrower
/// towards its top end and thinner there (of its width, its thickness, at
/// the tip); how thick (m, or of its width); how much of its length buried;
/// its strata (how many, how far each may stand in from the one under it,
/// m); its bands along it (how many; each raised or sunk, of its thickness,
/// tilted off the others, degrees, and broken off shorter, of its length,
/// at most); its rubble (how many blocks, how big, m).
struct Shape {
    tilt: (f64, f64),
    along: (f64, f64),
    across: (f64, f64),
    taper: (f64, f64),
    thin: (f64, f64),
    thick: (f64, f64),
    thick_of_width: bool,
    buried: (f64, f64),
    layers: (u32, u32),
    inset: f64,
    bands: (u32, u32),
    band_lift: f64,
    band_tilt: f64,
    band_short: f64,
    rubble: (u32, u32),
    rubble_size: (f64, f64),
}

const SLABS: Shape = Shape {
    tilt: (20.0, 70.0),
    along: (1.6, 3.2),
    across: (0.45, 0.6),
    taper: (0.3, 0.6),
    thin: (0.3, 0.6),
    thick: (6.0, 14.0),
    thick_of_width: false,
    buried: (0.2, 0.4),
    layers: (2, 4),
    inset: 4.0,
    bands: (1, 1),
    band_lift: 0.0,
    band_tilt: 0.0,
    band_short: 0.0,
    rubble: (6, 12),
    rubble_size: (2.0, 8.0),
};

const CLIFFS: Shape = Shape {
    tilt: (18.0, 50.0),
    along: (1.4, 2.4),
    across: (0.55, 0.8),
    taper: (0.1, 0.3),
    thin: (0.1, 0.3),
    thick: (0.45, 0.85),
    thick_of_width: true,
    buried: (0.3, 0.5),
    layers: (3, 6),
    inset: 8.0,
    bands: (2, 4),
    band_lift: 0.15,
    band_tilt: 4.0,
    band_short: 0.25,
    rubble: (10, 18),
    rubble_size: (3.0, 14.0),
};

/// Each layer broken off its top end (of its length, at least and at
/// most), along a line slanting up to so many degrees.
const BROKEN: (f64, f64) = (0.06, 0.3);
const SLANT: f64 = 30.0;
/// Rough edges: how many halvings deep, and how far each middle is pushed
/// aside (of the stretch's length): its sides, its broken ends and cracks.
const ROUGH: (u32, f64) = (3, 0.07);
const JAGGED: (u32, f64) = (5, 0.2);
/// Relief on its top face and its underside: cells this far apart (m), each
/// standing proud or sunk by so many steps of so much (m), rooted so far
/// into its face (m) for its sides.
const RELIEF: (f64, f64) = (14.0, 0.35);
const RELIEF_STEPS: (i32, i32) = (-1, 3);
const RELIEF_ROOT: f64 = 0.6;
/// Cracks across it: the chance of one, of a second; where (of its length
/// above the ground, kept from either end); each piece above one how far
/// apart (m), shifted aside (m), pivoted (degrees), at most.
const CRACKS: (f64, f64) = (0.65, 0.3);
const CRACK_AT: f64 = 0.15;
const CRACK_GAP: (f64, f64) = (0.5, 3.0);
const CRACK_SHIFT: f64 = 1.5;
const CRACK_PIVOT: f64 = 6.0;
/// Rubble: how far out from its line along the ground (m, beyond its
/// width), and how much of each block buried.
const RUBBLE_OUT: f64 = 14.0;
const RUBBLE_SUNK: f64 = 0.4;
/// How dark: its strata (of black), at least and at most.
const ALBEDO: (f32, f32) = (0.07, 0.14);

/// A layer of a piece: its outline (along the slab's heading, across it;
/// counter-clockwise), from how far under the slab's top face to how far
/// (m, `w` from 0 down), its tone.
struct Layer {
    outline: Vec<DVec2>,
    w: (f64, f64),
    albedo: f32,
}

/// A piece of a slab (a band of it, between its cracks): its layers, and how
/// it lies: from where along the slab (`from`, its `u`), at a point along
/// the ground from the slab's middle and a height (`s0`, `y0`), at an angle
/// (radians), how far on from there (m), shifted aside (m), raised (m).
struct Piece {
    layers: Vec<Layer>,
    from: f64,
    s0: f64,
    y0: f64,
    angle: f64,
    gap: f64,
    shift: f64,
    lift: f64,
}

/// A block of rubble: where (its middle, in the world), its outline (round
/// its middle), how thick, how turned.
struct Rock {
    at: DVec3,
    outline: Vec<DVec2>,
    thick: f64,
    turn: DQuat,
    albedo: f32,
}

/// A slab: its pieces, its rubble, its big plate (its seed), its middle,
/// how steep (radians), which way it rises (a unit), its length (along it,
/// from and to), and how thin at its tip (of its thickness).
pub(super) struct Slab {
    pieces: Vec<Piece>,
    rubble: Vec<Rock>,
    g: (i32, i32),
    middle: DVec2,
    tilt: f64,
    heading: DVec2,
    length: (f64, f64),
    thin: f64,
    width: f64,
}

/// What `slabs_near` tells of a slab: where, how steep (degrees), how long
/// and how wide (m), how many pieces.
#[derive(Clone, Copy, Debug)]
pub struct SlabInfo {
    pub middle: DVec2,
    pub tilt: f64,
    pub length: f64,
    pub width: f64,
    pub pieces: usize,
}

impl PlateWorld {
    /// The slab big plate `g` (its outline `poly`) is, if it is one.
    pub(super) fn slab(&self, cache: &mut Cache, g: (i32, i32), poly: &[DVec2]) -> Option<Slab> {
        let shape = match self.slabs {
            1 => &SLABS,
            2 => &CLIFFS,
            _ => return None,
        };
        if self.lab || poly.len() < 3 {
            return None;
        }
        let middle = poly.iter().copied().sum::<DVec2>() / poly.len() as f64;
        // Its field, and how near the field's middle.
        let n = (self.size / FIELD.0) as i32;
        let f = ((middle.x / FIELD.0).floor() as i32, (middle.y / FIELD.0).floor() as i32);
        let r = |k: i32| field_hash(f.0.rem_euclid(n), f.1.rem_euclid(n), k, self.seed);
        if r(0) > FIELD.1 {
            return None;
        }
        let focus = DVec2::new((f.0 as f64 + 0.25 + 0.5 * r(1)) * FIELD.0, (f.1 as f64 + 0.25 + 0.5 * r(2)) * FIELD.0);
        let chance = FIELD.2 * (1.0 - middle.distance(focus) / FIELD.3).max(0.0);
        let own = |k: i32| self.rand(0, g, 40 + k);
        if own(0) >= chance {
            return None;
        }
        let lerp = |(lo, hi): (f64, f64), t: f64| lo + (hi - lo) * t;
        let pick = |(lo, hi): (u32, u32), t: f64| (lo + (t * (hi - lo + 1) as f64) as u32).min(hi);
        let a = r(3) * std::f64::consts::TAU + (own(1) - 0.5) * 2.0 * STRAY.to_radians();
        let heading = DVec2::new(a.cos(), a.sin());
        let across = DVec2::new(-heading.y, heading.x);
        let tilt = lerp(shape.tilt, own(3)).to_radians();
        // Its outline round its middle, along its heading and across it:
        // stretched along, narrowed across, tapering to its top.
        let (long, narrow, taper) = (lerp(shape.along, own(7)), lerp(shape.across, own(8)), lerp(shape.taper, own(9)));
        let mut base: Vec<DVec2> = poly.iter().map(|&p| (p - middle) * SHRINK).map(|p| DVec2::new(p.dot(heading) * long, p.dot(across) * narrow)).collect();
        if area(&base) < 0.0 {
            base.reverse();
        }
        let (lo, hi) = base.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        for p in &mut base {
            p.y *= 1.0 - taper * (p.x - lo) / (hi - lo).max(1.0);
        }
        let (v_lo, v_hi) = base.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        let width = v_hi - v_lo;
        // Clear of sites, canals and their stations, where it stands.
        let clear = |p: DVec2| self.sites.at(p, 30.0).is_none() && self.canals.surface(p, &|c| self.canal_floor(c)).is_none();
        let world = |p: DVec2| middle + heading * p.x * tilt.cos() + across * p.y;
        if !clear(middle) || !base.iter().all(|&p| clear(world(p)) && clear(world(p * 0.5))) {
            return None;
        }
        // Buried: its foot under the ground, `buried` of its length; there,
        // on the ground, each band's pivot.
        let u_b = lo + (hi - lo) * lerp(shape.buried, own(5));
        let ground = self.surface(cache, middle.x, middle.y, 0);
        let thick = lerp(shape.thick, own(2)) * if shape.thick_of_width { width } else { 1.0 };
        // Cracked across (the whole of it): where (above the ground, clear
        // of either end), each crack's line slanting its own way.
        let mut picks = Vec::new();
        if own(10) < CRACKS.0 {
            picks.push(own(11));
            if own(12) < CRACKS.1 {
                picks.push(own(13));
            }
        }
        let (above, top) = (u_b + (hi - u_b) * CRACK_AT, hi - (hi - lo) * (BROKEN.1 + shape.band_short + 2.0 * CRACK_AT));
        let mut cracks: Vec<(f64, DVec2)> = picks
            .iter()
            .enumerate()
            .map(|(k, &t)| {
                let slant = ((self.rand(0, g, 200 + k as i32) - 0.5) * SLANT).to_radians();
                (above + (top - above).max(0.0) * t, DVec2::new(slant.cos(), slant.sin()))
            })
            .collect();
        cracks.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Its bands across it, each its own way.
        let bands = pick(shape.bands, own(16));
        let cuts: Vec<f64> = (1..bands).map(|b| v_lo + width * (b as f64 + (self.rand(0, g, 600 + b as i32) - 0.5) * 0.5) / bands as f64).collect();
        let mut pieces = Vec::new();
        for b in 0..bands {
            let qb = |k: i32| self.rand(0, g, 700 + 10 * b as i32 + k);
            // (Its seams with the bands beside it rough, as its sides.)
            let mut band = base.clone();
            if b > 0 {
                band = clip(&band, DVec2::new(0.0, cuts[b as usize - 1]), DVec2::new(0.0, -1.0)).0;
            }
            if b + 1 < bands {
                band = clip(&band, DVec2::new(0.0, cuts[b as usize]), DVec2::new(0.0, 1.0)).0;
            }
            if band.len() < 3 {
                continue;
            }
            let lift = if bands > 1 { (qb(0) - 0.5) * 2.0 * shape.band_lift * thick } else { 0.0 };
            let band_tilt = if bands > 1 { ((qb(1) - 0.5) * 2.0 * shape.band_tilt).to_radians() } else { 0.0 };
            let short = shape.band_short * qb(2) * (hi - lo);
            // Its strata, from its top face down: each its share of the
            // thickness, each in from the one under it (the lowest the
            // whole), each broken off its own way.
            let count = pick(shape.layers, qb(3));
            let shares: Vec<f64> = (0..count).map(|k| 0.6 + qb(10 + k as i32)).collect();
            let total: f64 = shares.iter().sum();
            let size = (hi - lo).max(1.0);
            let mut w = 0.0;
            let mut layers = Vec::new();
            for k in 0..count {
                let q = |j: i32| self.rand(0, g, 1000 + 100 * b as i32 + 10 * k as i32 + j);
                let depth = thick * shares[k as usize] / total;
                let up = (count - 1 - k) as f64;
                let inset = shape.inset * up * (0.4 + 0.6 * q(0)) / (count.max(2) - 1) as f64;
                let middle_v = band.iter().map(|p| p.y).sum::<f64>() / band.len() as f64;
                let outline: Vec<DVec2> = band.iter().map(|&p| DVec2::new(p.x * (1.0 - inset / size), middle_v + (p.y - middle_v) * (1.0 - inset / width.max(1.0)) + (q(1) - 0.5) * inset * 0.5)).collect();
                let cut = hi - short - (hi - lo) * lerp(BROKEN, q(2));
                let slant = ((q(3) - 0.5) * 2.0 * SLANT).to_radians();
                let (kept, broken) = clip(&outline, DVec2::new(cut, 0.0), DVec2::new(slant.cos(), slant.sin()));
                if kept.len() >= 3 {
                    let tone = ALBEDO.0 + (ALBEDO.1 - ALBEDO.0) * q(4) as f32;
                    let jagged: Vec<DVec2> = broken.iter().map(|&i| kept[i]).collect();
                    layers.push((kept, jagged, (-(w + depth), -w), tone));
                }
                w += depth;
            }
            // Its pieces between the cracks, each hanging from the one under
            // it: the first pivoted on the ground at its foot, each above a
            // crack a little apart and pivoted at it.
            let mut frame = (u_b, u_b * tilt.cos(), ground, tilt + band_tilt, 0.0);
            let bounds: Vec<Option<(f64, DVec2)>> = std::iter::once(None).chain(cracks.iter().copied().map(Some)).collect();
            for (j, &below) in bounds.iter().enumerate() {
                let next = cracks.get(j).copied();
                let q = |k: i32| self.rand(0, g, 2000 + 100 * b as i32 + 10 * j as i32 + k);
                let mut parts = Vec::new();
                for (k, (outline, jagged, w, tone)) in layers.iter().enumerate() {
                    let mut part = outline.clone();
                    let mut marks = jagged.clone();
                    if let Some((u, normal)) = below {
                        let (kept, cut) = clip(&part, DVec2::new(u, 0.0), -normal);
                        marks.extend(cut.iter().map(|&i| kept[i]));
                        part = kept;
                    }
                    if let Some((u, normal)) = next {
                        let (kept, cut) = clip(&part, DVec2::new(u, 0.0), normal);
                        marks.extend(cut.iter().map(|&i| kept[i]));
                        part = kept;
                    }
                    if part.len() < 3 {
                        continue;
                    }
                    let marked: Vec<usize> = (0..part.len()).filter(|&i| marks.iter().any(|&p| p.distance(part[i]) < 1e-6)).collect();
                    // (Rough, and if that crosses itself, less so, then
                    // plain: a narrow piece between a crack and its broken
                    // end, both jagged, crossed.)
                    let seed = (g.0 * 7 + j as i32 + 31 * b as i32, g.1, k as i32);
                    let rough = [1.0, 0.5].iter().map(|&s| roughen(&part, &marked, seed, self.seed, s)).find(|o| simple(o)).unwrap_or(part);
                    parts.push(Layer { outline: rough, w: *w, albedo: *tone });
                }
                // Where it hangs: on the crack under it, as the piece under
                // it lies (or, the first, at its foot on the ground).
                let (from, s0, y0, angle, shift) = frame;
                let (gap, extra_shift, here) = match below {
                    None => (0.0, 0.0, frame),
                    Some((u, _)) => {
                        let du = u - from;
                        let pivot = ((q(2) - 0.5) * 2.0 * CRACK_PIVOT).to_radians();
                        (lerp(CRACK_GAP, q(0)), (q(1) - 0.5) * 2.0 * CRACK_SHIFT, (u, s0 + du * angle.cos(), y0 + du * angle.sin(), angle + pivot, shift))
                    }
                };
                frame = (here.0, here.1, here.2, here.3, here.4 + extra_shift);
                if !parts.is_empty() {
                    pieces.push(Piece { layers: parts, from: frame.0, s0: frame.1, y0: frame.2, angle: frame.3, gap, shift: frame.4, lift });
                }
            }
        }
        if pieces.is_empty() {
            return None;
        }
        // Rubble round its foot, along its line on the ground.
        let count = pick(shape.rubble, own(14));
        let mut rubble = Vec::new();
        for k in 0..count {
            let q = |j: i32| self.rand(0, g, 400 + 10 * k as i32 + j);
            let v = (q(0) - 0.5) * (width + RUBBLE_OUT);
            let out = (q(1) - 0.5) * 2.0 * RUBBLE_OUT;
            let flat = middle + heading * (u_b * tilt.cos() + out) + across * v;
            if !clear(flat) {
                continue;
            }
            let size = lerp(shape.rubble_size, q(2) * q(2));
            let corners = 5 + (q(3) * 3.0) as usize;
            let mut outline: Vec<DVec2> = (0..corners)
                .map(|c| {
                    let a = (c as f64 + 0.3 * self.rand(0, g, 500 + 10 * k as i32 + c as i32)) / corners as f64 * std::f64::consts::TAU;
                    DVec2::new(a.cos(), a.sin()) * size * 0.5 * (0.7 + 0.3 * q(4))
                })
                .collect();
            if area(&outline) < 0.0 {
                outline.reverse();
            }
            let thick = size * (0.3 + 0.4 * q(5));
            let ground = self.surface(cache, flat.x, flat.y, 0);
            let turn = DQuat::from_euler(glam::EulerRot::YXZ, q(6) * std::f64::consts::TAU, (q(7) - 0.5) * 1.2, (q(8) - 0.5) * 1.2);
            let tone = ALBEDO.0 + (ALBEDO.1 - ALBEDO.0) * q(9) as f32;
            rubble.push(Rock { at: DVec3::new(flat.x, ground + thick * (0.5 - RUBBLE_SUNK), flat.y), outline, thick, turn, albedo: tone });
        }
        Some(Slab { pieces, rubble, g, middle, tilt, heading, length: (lo, hi), thin: lerp(shape.thin, own(15)), width })
    }

    /// A slab's solid, into a column's mesh (positions from `origin`): each
    /// piece's layers, their tops, undersides and sides; its rubble. Flat
    /// faces, crisp.
    pub(super) fn emit_slab(&self, mesh: &mut ColumnMesh, s: &Slab, origin: (f64, f64)) {
        let across = DVec2::new(-s.heading.y, s.heading.x);
        let span = (s.length.1 - s.length.0).max(1.0);
        for (j, piece) in s.pieces.iter().enumerate() {
            let (sin, cos) = piece.angle.sin_cos();
            // A point (along the slab `u`, across it `v`, under its top face
            // `w`, thinning towards its tip): as its piece lies.
            let at = |p: DVec2, w: f64| -> Vec3 {
                let w = w * (1.0 - s.thin * ((p.x - s.length.0) / span).clamp(0.0, 1.0)) + piece.lift;
                let du = p.x - piece.from + piece.gap;
                let flat = s.middle + s.heading * (piece.s0 + du * cos - w * sin) + across * (p.y + piece.shift);
                let y = piece.y0 + du * sin + w * cos;
                Vec3::new((flat.x - origin.0) as f32, y as f32, (flat.y - origin.1) as f32)
            };
            // A direction in its plane, and square to it.
            let dir = |d: DVec2| -> Vec3 {
                let flat = s.heading * (d.x * cos) + across * d.y;
                Vec3::new(flat.x as f32, (d.x * sin) as f32, flat.y as f32).normalize_or(Vec3::Y)
            };
            let up = Vec3::new(-(s.heading.x * sin) as f32, cos as f32, -(s.heading.y * sin) as f32).normalize();
            let last = piece.layers.len() - 1;
            for (k, layer) in piece.layers.iter().enumerate() {
                let o = &layer.outline;
                // Its faces: the top's (the highest layer's) and the
                // underside's (the lowest's) in relief, the rest plain.
                for (w, side, ao, relief) in [(layer.w.1, 1.0, 1.0, k == 0), (layer.w.0, -1.0, 0.35, k == last)] {
                    let normal = up * side as f32;
                    if relief {
                        let seed = (s.g.0 * 7 + j as i32, s.g.1, k as i32 * 2 + (side > 0.0) as i32);
                        for (cell, steps) in cells(o, seed, self.seed) {
                            let (top, root) = (w + side * steps as f64 * RELIEF.1, w - side * RELIEF_ROOT);
                            prism(mesh, &cell, &|p, t| at(p, if t { top } else { root }), normal, &dir, layer.albedo, ao);
                        }
                        continue;
                    }
                    let first = mesh.positions.len() as u32;
                    for &p in o {
                        push(mesh, at(p, w), normal, layer.albedo, ao);
                    }
                    for t in triangulate(o) {
                        tri(mesh, [first + t[0] as u32, first + t[1] as u32, first + t[2] as u32], normal);
                    }
                }
                sides(mesh, o, &|p, t| at(p, if t { layer.w.1 } else { layer.w.0 }), &dir, layer.albedo * 0.9, 0.7);
            }
        }
        for rock in &s.rubble {
            let place = |p: DVec2, t: bool| -> Vec3 {
                let local = DVec3::new(p.x, if t { rock.thick * 0.5 } else { -rock.thick * 0.5 }, p.y);
                let w = rock.at + rock.turn * local;
                Vec3::new((w.x - origin.0) as f32, w.y as f32, (w.z - origin.1) as f32)
            };
            let up = (rock.turn * DVec3::Y).as_vec3();
            let dir = |d: DVec2| (rock.turn * DVec3::new(d.x, 0.0, d.y)).as_vec3().normalize_or(Vec3::Y);
            // (Its outline in its own x, z: counter-clockwise seen from
            // above is clockwise in x, z, so its sides face in: turned.)
            let flipped: Vec<DVec2> = rock.outline.iter().rev().copied().collect();
            prism(mesh, &flipped, &place, up, &dir, rock.albedo, 0.8);
            let first = mesh.positions.len() as u32;
            for &p in &flipped {
                push(mesh, place(p, false), -up, rock.albedo, 0.3);
            }
            for t in triangulate(&flipped) {
                tri(mesh, [first + t[0] as u32, first + t[1] as u32, first + t[2] as u32], -up);
            }
        }
    }

    /// The slabs within `r` m of a point: for finding and judging them.
    pub fn slabs_near(&self, x: f64, z: f64, r: f64) -> Vec<SlabInfo> {
        let mut cache = Cache::default();
        let g0 = GRID[0];
        let (gx0, gz0) = (((x - r) / g0).floor() as i32 - 1, ((z - r) / g0).floor() as i32 - 1);
        let (gx1, gz1) = (((x + r) / g0).ceil() as i32 + 1, ((z + r) / g0).ceil() as i32 + 1);
        let mut found = Vec::new();
        for gz in gz0..=gz1 {
            for gx in gx0..=gx1 {
                let poly = self.big_cell((gx, gz));
                if let Some(s) = self.slab(&mut cache, (gx, gz), &poly)
                    && s.middle.distance(DVec2::new(x, z)) <= r
                {
                    found.push(SlabInfo { middle: s.middle, tilt: s.tilt.to_degrees(), length: s.length.1 - s.length.0, width: s.width, pieces: s.pieces.len() });
                }
            }
        }
        found
    }
}

fn push(mesh: &mut ColumnMesh, p: Vec3, normal: Vec3, albedo: f32, ao: f32) {
    mesh.positions.push(p.to_array());
    mesh.normals.push(normal.to_array());
    mesh.albedo.push(albedo);
    mesh.ao.push(ao);
}

/// A triangle facing `normal`, whatever its winding.
fn tri(mesh: &mut ColumnMesh, i: [u32; 3], normal: Vec3) {
    let p = |k: u32| Vec3::from(mesh.positions[k as usize]);
    let face = (p(i[1]) - p(i[0])).cross(p(i[2]) - p(i[0]));
    if face.dot(normal) >= 0.0 {
        mesh.indices.extend_from_slice(&i);
    } else {
        mesh.indices.extend_from_slice(&[i[0], i[2], i[1]]);
    }
}

/// The sides of an outline (counter-clockwise) between its top and its
/// bottom (`at(p, true)` and `at(p, false)`), each facing out (`dir` turns an
/// outline direction into the world).
fn sides(mesh: &mut ColumnMesh, o: &[DVec2], at: &dyn Fn(DVec2, bool) -> Vec3, dir: &dyn Fn(DVec2) -> Vec3, albedo: f32, ao: f32) {
    let n = o.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let edge = o[j] - o[i];
        if edge.length_squared() < 1e-8 {
            continue;
        }
        // (Counter-clockwise: out is to the edge's right.)
        let out = dir(DVec2::new(edge.y, -edge.x));
        let first = mesh.positions.len() as u32;
        for p in [at(o[i], true), at(o[j], true), at(o[j], false), at(o[i], false)] {
            push(mesh, p, out, albedo, ao);
        }
        tri(mesh, [first, first + 1, first + 2], out);
        tri(mesh, [first, first + 2, first + 3], out);
    }
}

/// An outline's top (facing `normal`) and its sides down to its bottom.
fn prism(mesh: &mut ColumnMesh, o: &[DVec2], at: &dyn Fn(DVec2, bool) -> Vec3, normal: Vec3, dir: &dyn Fn(DVec2) -> Vec3, albedo: f32, ao: f32) {
    let first = mesh.positions.len() as u32;
    for &p in o {
        push(mesh, at(p, true), normal, albedo, ao);
    }
    for t in triangulate(o) {
        tri(mesh, [first + t[0] as u32, first + t[1] as u32, first + t[2] as u32], normal);
    }
    sides(mesh, o, at, dir, albedo * 0.9, ao * 0.8);
}

/// A polygon's signed area (counter-clockwise, positive).
fn area(p: &[DVec2]) -> f64 {
    (0..p.len()).map(|i| p[i].perp_dot(p[(i + 1) % p.len()])).sum::<f64>() * 0.5
}

/// A polygon cut by a line (through `at`, square to `normal`): what lies on
/// its near side, and which of its edges the cut made (by the index of
/// their first corner).
fn clip(p: &[DVec2], at: DVec2, normal: DVec2) -> (Vec<DVec2>, Vec<usize>) {
    let side = |q: DVec2| (q - at).dot(normal);
    let (mut out, mut cut) = (Vec::new(), Vec::new());
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa <= 0.0 {
            out.push(a);
        }
        if (sa <= 0.0) != (sb <= 0.0) {
            // (Leaving, the next edge runs along the cut to where it comes
            // back.)
            if sa <= 0.0 {
                cut.push(out.len());
            }
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    (out, cut)
}

/// Each edge split in half again and again, each half's middle pushed
/// aside: the cut (`broken`) ones jagged, the rest rough.
fn roughen(p: &[DVec2], broken: &[usize], seed: (i32, i32, i32), world: u32, scale: f64) -> Vec<DVec2> {
    let mut out = Vec::new();
    for i in 0..p.len() {
        let (levels, amount) = if broken.contains(&i) { JAGGED } else { ROUGH };
        let mut way = vec![p[i], p[(i + 1) % p.len()]];
        for level in 0..levels as i32 {
            let mut finer = Vec::with_capacity(way.len() * 2);
            for (k, w) in way.windows(2).enumerate() {
                let (a, b) = (w[0], w[1]);
                let aside = (b - a).perp().normalize_or_zero();
                let h = crate::noise::hash01(seed.0 * 31 + seed.2, level * 4096 + i as i32 * 256 + k as i32, seed.1, world ^ 0x51ab) as f64;
                finer.extend_from_slice(&[a, (a + b) * 0.5 + aside * (h - 0.5) * 2.0 * amount * scale * a.distance(b)]);
            }
            finer.push(*way.last().unwrap());
            way = finer;
        }
        out.extend_from_slice(&way[..way.len() - 1]);
    }
    out
}

/// Whether an outline does not cross itself.
fn simple(o: &[DVec2]) -> bool {
    let n = o.len();
    for i in 0..n {
        for j in i + 2..n {
            if (j + 1) % n == i {
                continue;
            }
            let (a, b, c, d) = (o[i], o[(i + 1) % n], o[j], o[(j + 1) % n]);
            if (b - a).perp_dot(c - a) * (b - a).perp_dot(d - a) < 0.0 && (d - c).perp_dot(a - c) * (d - c).perp_dot(b - c) < 0.0 {
                return false;
            }
        }
    }
    true
}

/// A face (`outline`) in relief: split into Voronoi cells about `RELIEF.0`
/// apart, each with how many steps it stands proud (or, below 0, sunk).
fn cells(outline: &[DVec2], seed: (i32, i32, i32), world: u32) -> Vec<(Vec<DVec2>, i32)> {
    let spacing = RELIEF.0;
    let (min, max) = outline.iter().fold((DVec2::MAX, DVec2::MIN), |(a, b), &p| (a.min(p), b.max(p)));
    let (i0, j0) = ((min.x / spacing).floor() as i32 - 1, (min.y / spacing).floor() as i32 - 1);
    let (i1, j1) = ((max.x / spacing).ceil() as i32 + 1, (max.y / spacing).ceil() as i32 + 1);
    let r = |i: i32, j: i32, k: i32| crate::noise::hash01(seed.0 * 131 + i, seed.1 * 131 + j, seed.2 * 16 + k, world ^ 0x7e1f) as f64;
    let site = |i: i32, j: i32| DVec2::new((i as f64 + 0.5 + (r(i, j, 0) - 0.5) * 0.7) * spacing, (j as f64 + 0.5 + (r(i, j, 1) - 0.5) * 0.7) * spacing);
    let mut out = Vec::new();
    for j in j0..=j1 {
        for i in i0..=i1 {
            let s = site(i, j);
            let mut piece = outline.to_vec();
            for dj in -2..=2 {
                for di in -2..=2 {
                    if (di, dj) == (0, 0) || piece.len() < 3 {
                        continue;
                    }
                    let other = site(i + di, j + dj);
                    piece = clip(&piece, (s + other) * 0.5, (other - s).normalize_or_zero()).0;
                }
            }
            if piece.len() >= 3 && area(&piece).abs() > 1e-6 {
                let steps = RELIEF_STEPS.0 + (r(i, j, 2) * (RELIEF_STEPS.1 - RELIEF_STEPS.0 + 1) as f64) as i32;
                out.push((piece, steps.min(RELIEF_STEPS.1)));
            }
        }
    }
    out
}

/// A simple polygon's triangles (ear clipping), counter-clockwise.
fn triangulate(p: &[DVec2]) -> Vec<[usize; 3]> {
    let mut idx: Vec<usize> = (0..p.len()).collect();
    if area(p) < 0.0 {
        idx.reverse();
    }
    let inside = |q: DVec2, a: DVec2, b: DVec2, c: DVec2| (b - a).perp_dot(q - a) > 0.0 && (c - b).perp_dot(q - b) > 0.0 && (a - c).perp_dot(q - c) > 0.0;
    let mut tris = Vec::new();
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&i| {
            let (a, b, c) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            (p[b] - p[a]).perp_dot(p[c] - p[b]) > 1e-9 && !idx.iter().any(|&j| j != a && j != b && j != c && inside(p[j], p[a], p[b], p[c]))
        });
        // (None found: not simple; the rest as a fan, rather than nothing.)
        let Some(i) = ear else {
            for k in 1..m - 1 {
                tris.push([idx[0], idx[k], idx[k + 1]]);
            }
            return tris;
        };
        tris.push([idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]]);
        idx.remove(i);
    }
    tris.push([idx[0], idx[1], idx[2]]);
    tris
}

/// A hash of a field's square (0..1).
fn field_hash(x: i32, z: i32, k: i32, seed: u32) -> f64 {
    crate::noise::hash01(x, k + 7300, z, seed) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slabs_in_fields() {
        // Over a 6 km square: slabs, gathered in fields, none on a canal;
        // how long and how steep spread wide (to judge which).
        let mut world = PlateWorld::new(crate::WORLD_SIZE, 1);
        world.slabs = 1;
        let found = world.slabs_near(8192.0, 8192.0, 3000.0);
        let mut fields: Vec<(i32, i32)> = found.iter().map(|s| ((s.middle.x / FIELD.0).floor() as i32, (s.middle.y / FIELD.0).floor() as i32)).collect();
        fields.sort();
        fields.dedup();
        let range = |f: &dyn Fn(&SlabInfo) -> f64| found.iter().map(f).fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
        let (tilt, length, width) = (range(&|s| s.tilt), range(&|s| s.length), range(&|s| s.width));
        let cracked = found.iter().filter(|s| s.pieces > 1).count();
        println!("{} slabs in {} fields; tilt {:.0}-{:.0} deg, {:.0}-{:.0} m long, {:.0}-{:.0} m wide; {cracked} cracked", found.len(), fields.len(), tilt.0, tilt.1, length.0, length.1, width.0, width.1);
        assert!(!found.is_empty(), "no slabs");
        assert!(found.len() as f64 / fields.len() as f64 >= 2.0, "slabs alone, not in fields");
        for s in &found {
            assert!(world.canal_at(s.middle.x as f32, s.middle.y as f32).is_none(), "a slab on a canal at {}", s.middle);
            assert!(s.length > s.width * 1.5, "a slab {:.0} m long, {:.0} wide: not long", s.length, s.width);
        }
    }

    #[test]
    fn slabs_are_whole() {
        // Each slab's layers, of either kind: whole faces (their triangles
        // cover their outline's area: no ear left out, none twice), relief
        // cells that tile them, and outlines that do not cross themselves.
        for kind in [1, 2] {
            whole(kind);
        }
    }

    fn whole(kind: u8) {
        let mut world = PlateWorld::new(crate::WORLD_SIZE, 1);
        world.slabs = kind;
        let mut cache = Cache::default();
        let mut seen = 0;
        for gz in 30..60 {
            for gx in 30..70 {
                let poly = world.big_cell((gx, gz));
                let Some(s) = world.slab(&mut cache, (gx, gz), &poly) else { continue };
                seen += 1;
                for piece in &s.pieces {
                    for (k, l) in piece.layers.iter().enumerate() {
                        let o = &l.outline;
                        assert!(simple(o), "slab {gx},{gz} layer {k}: its outline crosses itself");
                        let whole = area(o).abs();
                        let tris: f64 = triangulate(o).iter().map(|t| area(&[o[t[0]], o[t[1]], o[t[2]]]).abs()).sum();
                        assert!((tris - whole).abs() < whole * 1e-6, "slab {gx},{gz} layer {k}: its triangles {tris} m2, its outline {whole}");
                        let tiled: f64 = cells(o, (gx, gz, k as i32), world.seed).iter().map(|(c, _)| area(c).abs()).sum();
                        assert!((tiled - whole).abs() < whole * 0.01, "slab {gx},{gz} layer {k}: its cells {tiled} m2, its face {whole}");
                    }
                }
            }
        }
        println!("kind {kind}: {seen} slabs whole");
        assert!(seen > 10);
    }
}
