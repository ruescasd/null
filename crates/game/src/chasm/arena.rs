//! The arena (`--opt arena`, which brings `--opt chasm`, whose built places
//! it shares): the combat lab, a monumental building to fight in, a
//! colosseum on an empty plaza lost in the haze. Round its floor (64 m
//! across) the cavea rises in giant's steps, 4 m high and 6 m deep, too
//! big for whatever sat on them, people-sized stairs cut into them. Behind
//! them the shell, 40 m high in five storeys: an arcade round the outside, a
//! ring corridor behind it, rooms, stair halls, and passages out onto the
//! terraces (the vomitoria). Three gates, slots through the cavea far too
//! tall for anything that comes through them; and a wedge of it all cut
//! cleanly away, its storeys open in section (the user's two picks for its
//! strangeness: the wrong scale, and the clean cut). Its first stage, a
//! blockout. Everything in metres, from the floor's middle, angles in
//! degrees round from +x towards +z.


use bevy::prelude::*;
use manifold_csg::{CrossSection, Manifold, cross_section::FillRule};

use super::{Geometry, Parts, across, placed};

/// Where the floor's middle is: high over the lab's ground, out of sight
/// below the haze, as a region of the chasm is.
pub const CENTRE: Vec3 = Vec3::new(1000.0, 1200.0, 1000.0);
/// Its stone's code as a plate's (see `plate_code` in worldgen, and
/// terrain.wgsl): the coarsest level's, so its floors are pieced at 32 and
/// 8 m as the ground's coarsest plates are; its number its own.
pub const PLATE: i32 = 4 * 7;
/// The floor's radius; the giant steps' rise and depth, and how many.
const FLOOR: f32 = 32.0;
const RISE: f32 = 4.0;
const DEEP: f32 = 6.0;
const TIERS: i32 = 6;
/// The shell: from the top terrace's back out; its walls' thickness; where
/// its rooms and its corridor are; a storey's height, how many, the top.
const INNER: f32 = FLOOR + DEEP * TIERS as f32;
const OUTER: f32 = 84.0;
const WALL: f32 = 1.5;
const ROOM_IN: f32 = INNER + WALL;
const ROOM_OUT: f32 = 77.5;
const HALL_IN: f32 = 78.5;
const HALL_OUT: f32 = OUTER - WALL;
const STOREY: f32 = 8.0;
const STOREYS: i32 = 5;
const TOP: f32 = STOREY * STOREYS as f32;
/// A storey's floor slab.
const SLAB: f32 = 1.0;
/// Rooms round a storey (10 degrees each), and arches round the arcade.
const ROOMS: i32 = 36;
const BAYS: i32 = 72;
/// The cut: how far either side of +x it reaches (degrees).
const CUT: f32 = 20.0;
/// The gates (degrees): how wide, how high.
const GATES: [f32; 3] = [90.0, 180.0, 270.0];
const GATE_HALF: f32 = 4.0;
const GATE_HIGH: f32 = 22.0;
/// The aisles' lines (degrees), each flight a few degrees to one side of
/// its line or the other in turn, so they zigzag up; how wide.
const AISLES: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const ZIGZAG: f32 = 4.0;
const AISLE_HALF: f32 = 1.5;
/// The vomitoria: where (degrees) and at what height, each from a
/// terrace's back (or the floor's edge) out into the shell's rooms.
const VOMITORIA: [(f32, f32); 7] = [(67.5, 0.0), (202.5, 0.0), (292.5, 0.0), (112.5, 8.0), (247.5, 8.0), (157.5, 16.0), (292.5, 16.0)];
/// The rooms, a storey to a line, a letter to a room from 0 degrees round
/// (the stage's working pass on them: kinds by plan, not by chance). `c` a
/// cell, a door onto the corridor; `e` doors also through its walls to its
/// neighbours that are `e` too, a run of them in line (an enfilade); `h`
/// with its neighbours that are `h` one hall, a pier where each wall was;
/// `o` open onto the corridor between two piers; `s` a stair hall, a
/// flight up to the storey over it (from the last, to the roof); `p` a
/// gate's pen. In capitals, no lamp (under the top terrace; above it, the
/// day comes in by its arches and windows). The rooms in the cut are cells.
/// (In quarters, nine rooms each, the last three from a gate.)
const PLANS: [&str; STOREYS as usize] = [
    concat!("ccceeseee", "phhhCoooc", "pcEEEschh", "pCeeeeccc"),
    concat!("ccchhhhco", "peeescHHc", "pooceeeEc", "pchhsCccc"),
    concat!("cccoocsCc", "peeeechhh", "pcooCcsee", "peecHHccc"),
    concat!("ccceeeech", "hhcooscee", "eechhhhco", "ooceesccc"),
    concat!("cccosooce", "eeechhhhc", "eeeescooc", "hhhceeecc"),
];
/// The floor's layouts (`--set floor N`), each a different fight on it.
pub const FLOORS: [&str; 4] = ["open", "pillars", "levels", "the pit"];
/// `pillars`: columns of the wrong scale holding nothing up, six in a ring
/// 11 m out as high as the top terrace, twelve 23 m out by turns 8 and 16
/// m high (to break the lines of sight); their radius.
const COLUMN: f32 = 1.5;
/// `levels`: a dais in the middle as high as the first terrace, and two
/// spines from it out to that terrace, walked along the top, passages
/// through them below (a fight over and under); the dais's radius, the
/// spines' angles (clear of the gates, the aisles, the vomitoria).
const DAIS: f32 = 8.0;
const SPINES: [f32; 2] = [112.5, 247.5];
/// `the pit`: the giant steps going on down below the floor, from its rim
/// (the floor left a ring outside it, a step's depth), their aisles too;
/// how many.
const PIT: f32 = FLOOR - DEEP;
const PIT_TIERS: i32 = 3;
/// Segments round a whole turn.
const SEGMENTS: i32 = 256;
/// A stair's riser (as the chasm's; its treads 0.25 m as theirs, from its
/// run).
const RISER: f32 = 0.2;
/// A stair hall's flights and landing: how deep their slabs are.
const WAIST: f32 = 0.35;

/// Outward at `deg`, and along the turn there.
fn radial(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    Vec3::new(a.cos(), 0.0, a.sin())
}

fn at(r: f32, deg: f32, y: f32) -> Vec3 {
    radial(deg) * r + Vec3::Y * y
}

/// Stood up, (x, y, z) to (x, z, -y): a plan drawn as (x, -z) and extruded,
/// or a side view turned (the turn's axis manifold's z), made to stand.
fn upright(m: &Manifold) -> Manifold {
    m.transform(&[1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0])
}

fn section(pts: &[[f32; 2]]) -> CrossSection {
    CrossSection::from_polygons_with_fill_rule(&[pts.iter().map(|p| [p[0] as f64, p[1] as f64]).collect()], FillRule::NonZero)
}

/// An outline in plan (x, z) standing from `y0` to `y1`.
fn prism(outline: &[Vec2], y0: f32, y1: f32) -> Manifold {
    let pts: Vec<[f32; 2]> = outline.iter().map(|p| [p.x, -p.y]).collect();
    upright(&section(&pts).extrude((y1 - y0) as f64)).translate(0.0, y0 as f64, 0.0)
}

/// A side view (radius, height) turned round the middle.
fn turned(side: &[[f32; 2]]) -> Manifold {
    upright(&Manifold::revolve(&section(side), SEGMENTS, 360.0))
}

fn ring(r0: f32, r1: f32, y0: f32, y1: f32) -> Manifold {
    turned(&[[r0, y0], [r1, y0], [r1, y1], [r0, y1]])
}

/// A ring's piece from `a0` to `a1` degrees.
fn sector(r0: f32, r1: f32, (a0, a1): (f32, f32), y0: f32, y1: f32) -> Manifold {
    let n = (((a1 - a0) / 360.0 * SEGMENTS as f32).ceil() as usize).max(1);
    let arc = |r: f32| (0..=n).map(move |i| radial(a0 + (a1 - a0) * i as f32 / n as f32) * r).map(|p| Vec2::new(p.x, p.z));
    let outline: Vec<Vec2> = arc(r1).chain(arc(r0).collect::<Vec<_>>().into_iter().rev()).collect();
    prism(&outline, y0, y1)
}

/// A box along the radius at `deg`, from `r0` out to `r1`, `half` to either
/// side.
fn spoke(deg: f32, r0: f32, r1: f32, half: f32, y0: f32, y1: f32) -> Manifold {
    let (u, t) = (radial(deg), radial(deg + 90.0));
    let c = |r: f32, s: f32| {
        let p = u * r + t * s;
        Vec2::new(p.x, p.z)
    };
    prism(&[c(r0, -half), c(r1, -half), c(r1, half), c(r0, half)], y0, y1)
}

/// A box at `r` out at `deg`: `along` to either side round the turn, `out`
/// to either side along the radius.
fn post(deg: f32, r: f32, along: f32, out: f32, y0: f32, y1: f32) -> Manifold {
    let (u, t) = (radial(deg), radial(deg + 90.0));
    let c = |a: f32, b: f32| {
        let p = u * (r + b) + t * a;
        Vec2::new(p.x, p.z)
    };
    prism(&[c(-along, -out), c(-along, out), c(along, out), c(along, -out)], y0, y1)
}

/// An opening along the radius at `deg`: upright sides `half` apart from
/// `y0` to `y1`, a half circle over.
fn arched(deg: f32, r0: f32, r1: f32, half: f32, y0: f32, y1: f32) -> Manifold {
    let round = Manifold::cylinder((r1 - r0) as f64, half as f64, half as f64, 32, false);
    let (u, t) = (radial(deg), radial(deg + 90.0));
    spoke(deg, r0, r1, half, y0, y1).union(&placed(&round, at(r0, deg, y1), -t, u))
}

/// A flight's side view: up from (0, 0) to (`run`, `rise`), a riser and a
/// tread at a time; under it, solid down to 0, or (`waist`) a slab that
/// deep under the steps' inner corners, open beneath.
fn flight_side(run: f32, rise: f32, waist: Option<f32>) -> CrossSection {
    let n = (rise / RISER).round() as i32;
    let (r, t) = (rise / n as f32, run / n as f32);
    let (foot, head) = waist.map_or((0.0, 0.0), |w| (-w, rise - w));
    let mut pts = vec![[0.0, foot]];
    for i in 0..n {
        pts.push([t * i as f32, r * (i + 1) as f32]);
        pts.push([t * (i + 1) as f32, r * (i + 1) as f32]);
    }
    pts.push([run, head]);
    section(&pts)
}

/// A flight from `origin` along `x` (`z` across it, `x` cross up, `z0` to
/// `z1`): its steps (seen) and the slope over them (walked on), a little
/// under the floors at its ends (as the chasm's: flush, a walker sliding
/// over the floor can catch on its end).
fn flight(origin: Vec3, x: Vec3, z: (f32, f32), run: f32, rise: f32) -> (Manifold, Manifold) {
    flight_over(origin, x, z, run, rise, None)
}

/// A flight as `flight`, its steps on a slab `waist` deep, open beneath (a
/// stair hall's, the user's: to walk under), its slope as deep as that.
fn flight_over(origin: Vec3, x: Vec3, (z0, z1): (f32, f32), run: f32, rise: f32, waist: Option<f32>) -> (Manifold, Manifold) {
    let z = x.cross(Vec3::Y);
    let deep = waist.unwrap_or(1.0);
    let slope = section(&[[0.0, -0.03], [run, rise - 0.03], [run, rise - deep], [0.0, -deep]]);
    (placed(&across(&flight_side(run, rise, waist), z0, z1), origin, x, z), placed(&across(&slope, z0, z1), origin, x, z))
}

/// Where a straight cut `half` to either side must start to open a curved
/// face at radius `r` all across (the user: begun at the face's middle,
/// the face was left standing at the cut's sides, a skin narrowing the
/// mouth).
fn short_of(r: f32, half: f32) -> f32 {
    (r * r - half * half).sqrt() - 0.01
}

/// Half a radial wall's thickness (1 m) at the rooms' middle, in degrees.
fn wall_half() -> f32 {
    (0.5 / ((ROOM_IN + ROOM_OUT) * 0.5)).to_degrees()
}

/// Room `i` round storey `s` (see `PLANS`): its kind, and whether unlit.
fn room(s: i32, i: i32) -> (char, bool) {
    let k = PLANS[s as usize].as_bytes()[i.rem_euclid(ROOMS) as usize] as char;
    (k.to_ascii_lowercase(), k.is_ascii_uppercase())
}

/// Whether `deg` is in the cut (or within `margin` of it).
fn in_cut(deg: f32, margin: f32) -> bool {
    let d = (deg + 180.0).rem_euclid(360.0) - 180.0;
    d.abs() <= CUT + margin
}

/// How far the lamps reach (m): a room's, a corridor's or a tunnel's, a
/// gate's pen's. (Short, so that no point is reached by more lamps than cast
/// shadows at once, see `crowded`: a lamp without its shadow lights through
/// walls, and lamps taking their shadows by turns as you walked made what
/// was lit jump, the user, in the corridor at 270 degrees.)
const REACH: (f32, f32, f32) = (11.0, 12.0, 20.0);

/// The most lamps reaching any point of the building, and where (see
/// `REACH`): sampled every metre out, every 2 degrees round, at eye height
/// on each storey and terrace. A lamp a cone (`spots`) reaches only what is
/// within its cone, down.
pub fn crowded(lamps: &[(Vec3, f32)], spots: bool) -> (usize, Vec3) {
    let within = (super::SPOT.0).cos();
    let mut worst = (0, Vec3::ZERO);
    for r in 30..85 {
        for d in (0..360).step_by(2) {
            for y in [0.5, 4.0, 8.5, 12.0, 16.5, 20.0, 24.5, 28.0, 32.5, 36.0] {
                let p = at(r as f32, d as f32, y);
                let n = lamps.iter().filter(|(c, reach)| c.distance(p) < *reach && (!spots || (p - *c).normalize_or(Vec3::Y).dot(Vec3::NEG_Y) > within)).count();
                if n > worst.0 {
                    worst = (n, p);
                }
            }
        }
    }
    worst
}

/// A lamp's brightness for its reach (a passage lamp's, as `tiles`).
fn lamp(range: f32) -> f32 {
    0.0006 * (range / 14.0).powi(2) * 6.0
}

/// Where you start: out on the plaza before the cut, looking in.
pub fn start() -> [f32; 5] {
    let eye = CENTRE + at(170.0, 0.0, 1.7);
    [eye.x, eye.y, eye.z, 90.0, 6.0]
}

/// A gate's leaf (see `leaves`): a slab of glossy black as wide as its gate,
/// as tall and 2 m more, standing in it at its pen's mouth, shut; sunk into
/// the floor, its top flush with it, open (a round's, see `combat::round`).
#[derive(Component)]
pub struct Leaf {
    /// Where it stands shut.
    pub shut: Vec3,
}

/// A leaf's size (m): wide, tall, thick; how far it sinks to stand open.
pub const LEAF: Vec3 = Vec3::new(GATE_HALF * 2.0 + 0.1, GATE_HIGH + 2.0, 1.0);
pub const SINK: f32 = GATE_HIGH;

/// The gates' leaves, shut: where each stands (its middle), and turned so
/// its face is across its gate (in the shell's inner wall, at the pen).
pub fn leaves() -> Vec<(Vec3, Quat)> {
    GATES.iter().map(|&g| (CENTRE + at(INNER + 0.75, g, GATE_HIGH * 0.5 - 1.0), Quat::from_rotation_y((90.0 - g).to_radians()))).collect()
}

/// Where a gate's pen is, behind it, on its floor (for what comes through).
pub fn pens() -> Vec<Vec3> {
    GATES.iter().map(|&g| CENTRE + at((ROOM_IN + ROOM_OUT) * 0.5, g, 0.0)).collect()
}

/// A place on the floor, whatever its layout (where the navigation's check
/// starts).
pub fn ground() -> Vec3 {
    CENTRE + at(FLOOR - 3.0, 200.0, 0.0)
}

/// What its navmeshes are checked for: every landmark reached from the
/// floor.
pub(crate) fn checks(layout: usize) -> crate::navmesh::Checks {
    vec![("from the floor".into(), landmarks(layout).into_iter().map(|(name, p)| (name, ground(), p)).collect())]
}

/// Places that must all be reached from the floor (the navigation's
/// check): the top terrace, each storey's corridor, the roof, the pens,
/// and the floor `layout`'s own; by name.
pub fn landmarks(layout: usize) -> Vec<(String, Vec3)> {
    let mut out = vec![("the top terrace".to_string(), at(INNER - 3.0, 100.0, RISE * TIERS as f32))];
    match layout {
        2 => {
            out.push(("the dais".to_string(), Vec3::Y * RISE));
            out.extend(SPINES.iter().map(|&deg| (format!("the spine at {deg}"), at(20.0, deg, RISE))));
        }
        3 => out.push(("the pit's bottom".to_string(), Vec3::Y * -RISE * PIT_TIERS as f32)),
        _ => {}
    }
    for s in 0..STOREYS {
        out.push((format!("storey {s}'s corridor"), at((HALL_IN + HALL_OUT) * 0.5, 100.0, STOREY * s as f32)));
    }
    out.push(("the roof".to_string(), at((INNER + OUTER) * 0.5, 100.0, TOP)));
    out.extend(GATES.iter().map(|&g| (format!("the pen at {g}"), at((ROOM_IN + ROOM_OUT) * 0.5, g, 0.0))));
    out.into_iter().map(|(n, p)| (n, p + CENTRE)).collect()
}

/// A floor layout's pieces: solid (the floor itself among them), what is
/// cut from it, its flights (seen, and walked on).
#[derive(Default)]
struct Floor {
    solid: Vec<Manifold>,
    cuts: Vec<Manifold>,
    steps: Vec<Manifold>,
    slopes: Vec<Manifold>,
}

/// The floor laid out as `FLOORS[layout]` (everything within its edge).
fn floor(layout: usize) -> Floor {
    let mut out = Floor::default();
    let flight_up = |out: &mut Floor, origin: Vec3, x: Vec3| {
        let (seen, walked) = flight(origin, x, (-AISLE_HALF + 0.01, AISLE_HALF - 0.01), 5.0, RISE);
        out.steps.push(seen);
        out.slopes.push(walked);
    };
    let flat = ring(0.0, FLOOR, -2.0, 0.0);
    match layout {
        0 => out.solid.push(flat),
        1 => {
            out.solid.push(flat);
            let column = |r: f32, deg: f32, high: f32| {
                let p = at(r, deg, 0.0);
                upright(&Manifold::cylinder(high as f64, COLUMN as f64, COLUMN as f64, 48, false)).translate(p.x as f64, 0.0, p.z as f64)
            };
            out.solid.extend((0..6).map(|k| column(11.0, 30.0 + 60.0 * k as f32, RISE * TIERS as f32)));
            out.solid.extend((0..12).map(|k| column(23.0, 15.0 + 30.0 * k as f32, if k % 2 == 0 { 8.0 } else { 16.0 })));
        }
        2 => {
            out.solid.push(flat);
            out.solid.push(ring(0.0, DAIS, 0.0, RISE));
            // (Each spine into the dais and the first terrace a little, so
            // they are one; two passages through it, 2.5 m wide.)
            for deg in SPINES {
                out.solid.push(spoke(deg, DAIS - 1.0, FLOOR + 0.5, 1.5, 0.0, RISE));
                for r in [15.0, 24.0] {
                    out.cuts.push(post(deg, r, 2.0, 1.25, 0.0, 2.8));
                }
            }
            // Up onto the dais from the floor, a flight cut into it on
            // either side, facing the cut and the gate across from it.
            for deg in [0.0, 180.0] {
                out.cuts.push(spoke(deg, DAIS - 5.0, DAIS + 0.5, AISLE_HALF, 0.0, RISE + 0.01));
                flight_up(&mut out, at(DAIS, deg, 0.0), -radial(deg));
            }
        }
        3 => {
            let bottom = -RISE * PIT_TIERS as f32;
            let mut side = vec![[0.0, bottom - 1.0], [FLOOR, bottom - 1.0], [FLOOR, 0.0]];
            for k in 0..PIT_TIERS {
                let (r, y) = (PIT - DEEP * k as f32, -RISE * (k + 1) as f32);
                if k == 0 {
                    side.push([r, 0.0]);
                }
                side.push([r, y]);
                side.push([if k == PIT_TIERS - 1 { 0.0 } else { r - DEEP }, y]);
            }
            out.solid.push(turned(&side));
            // (The aisles on down, zigzagging on from the cavea's first
            // flight.)
            for line in AISLES {
                for k in 0..PIT_TIERS {
                    let deg = line + if k % 2 == 0 { ZIGZAG } else { -ZIGZAG };
                    let (r, y) = (PIT - DEEP * k as f32, -RISE * (k + 1) as f32);
                    out.cuts.push(spoke(deg, short_of(r, AISLE_HALF), r + 5.0, AISLE_HALF, y, y + RISE + 0.01));
                    flight_up(&mut out, at(r, deg, y), radial(deg));
                }
            }
        }
        _ => panic!("--set floor {layout}: there is no such floor; the floors are {}", FLOORS.iter().enumerate().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")),
    }
    out
}

/// The arena built, its floor laid out as `FLOORS[layout]`: its surfaces
/// and lights (`spots`, its lamps cones shining down, else spheres), where
/// it can be walked, and a line on what it is.
pub(in crate::chasm) fn build(layout: usize, spots: bool) -> (Parts, Vec<String>) {
    let mut parts = Parts::default();
    let mut cuts = Vec::new();
    let mut steps = Vec::new();
    let mut slopes = Vec::new();
    let mut lamps: Vec<(Vec3, f32)> = Vec::new();
    let hw = wall_half();

    // The cavea: the giant steps, from the floor's edge up to the shell
    // (into it a metre, so the two are one).
    let mut side = vec![[FLOOR, 0.0]];
    for k in 0..TIERS {
        let (r, y) = (FLOOR + DEEP * k as f32, RISE * (k + 1) as f32);
        side.push([r, y]);
        side.push([r + DEEP, y]);
    }
    side.push([INNER + 1.0, RISE * TIERS as f32]);
    side.push([INNER + 1.0, 0.0]);
    let cavea = turned(&side);
    let shell = ring(INNER, OUTER, 0.0, TOP);

    // Each storey: the corridor round it, its rooms (each its kind, see
    // `PLANS`), a door from each room to the corridor, the arcade's arches;
    // on the top terrace's storey, arches out onto it; on the last, windows
    // over the cavea. (Piers stood where walls were cut away.)
    let mut piers = Vec::new();
    for s in 0..STOREYS {
        let f = STOREY * s as f32;
        let ceiling = f + STOREY - SLAB;
        cuts.push(ring(HALL_IN, HALL_OUT, f, ceiling));
        for i in 0..ROOMS {
            let c = 360.0 / ROOMS as f32 * i as f32;
            let ((kind, dark), next) = (room(s, i), room(s, i + 1).0);
            let half = 5.0 - hw;
            cuts.push(sector(ROOM_IN, ROOM_OUT, (c - half, c + half), f, ceiling));
            // (A gate's pen is a storey's room three storeys high: no door
            // into it above its floor.)
            if kind != 'p' || f == 0.0 {
                cuts.push(spoke(c, ROOM_OUT - 0.5, HALL_IN + 0.5, 1.0, f, f + 3.0));
            }
            // Open onto the corridor: its wall there cut away, two piers
            // left, three openings.
            if kind == 'o' {
                cuts.push(sector(ROOM_OUT - 0.01, HALL_IN + 0.01, (c - half, c + half), f, ceiling));
                piers.extend([-1.0, 1.0].map(|side| post(c + side * half / 3.0, (ROOM_OUT + HALL_IN) * 0.5, 0.5, 0.5, f, ceiling)));
            }
            // Through the wall to the next room: in a hall, the wall gone
            // but for a pier in its middle; in an enfilade, a door, in line
            // with the others.
            if kind == 'h' && next == 'h' {
                cuts.push(sector(ROOM_IN, ROOM_OUT, (c, c + 10.0), f, ceiling));
                piers.push(post(c + 5.0, (ROOM_IN + ROOM_OUT) * 0.5, 0.5, 1.0, f, ceiling));
            } else if kind == 'e' && next == 'e' {
                cuts.push(post(c + 5.0, (ROOM_IN + ROOM_OUT) * 0.5, 1.5, 1.0, f, f + 3.0));
            }
            // (Not in a stair hall: its flight goes up along that wall, and
            // went across the arch or the window, as the user saw.)
            let opens = kind != 's';
            if opens && f == RISE * TIERS as f32 {
                cuts.push(arched(c, INNER - 0.5, ROOM_IN + 0.5, 2.0, f, f + 3.5));
            } else if opens && s == STOREYS - 1 {
                cuts.push(spoke(c, INNER - 0.5, ROOM_IN + 0.5, 1.5, f + 1.0, f + 4.0));
            }
            // (In a stair hall, out past its flights: in the room's middle,
            // it hung at the head of one going up.)
            if f < RISE * TIERS as f32 && kind != 'p' && !dark {
                lamps.push((at(if kind == 's' { ROOM_OUT - 1.0 } else { (ROOM_IN + ROOM_OUT) * 0.5 }, c, ceiling - 0.6), REACH.0));
            }
        }
        for j in 0..BAYS {
            cuts.push(arched(360.0 / BAYS as f32 * j as f32, HALL_OUT - 0.5, OUTER + 0.5, 2.0, f, f + 3.5));
        }
        // (The corridors under the cavea's top lit; above it, by day,
        // through the arcade.)
        if f < RISE * TIERS as f32 {
            for j in 0..12 {
                lamps.push((at((HALL_IN + HALL_OUT) * 0.5, 30.0 * j as f32 + 15.0, ceiling - 0.6), REACH.1));
            }
        }
    }

    // The gates: a slot through the cavea and into the shell, as high as
    // five men stood on one another's shoulders four times over; behind
    // each a pen, its room three storeys high. (Over each, the top
    // terrace goes on across: what is left of the cavea above the slot.)
    for g in GATES {
        cuts.push(spoke(g, FLOOR - 1.0, ROOM_IN + 0.5, GATE_HALF, 0.0, GATE_HIGH));
        cuts.push(sector(ROOM_IN, ROOM_OUT, (g - 5.0 + hw, g + 5.0 - hw), 0.0, GATE_HIGH + SLAB));
        lamps.push((at((ROOM_IN + ROOM_OUT) * 0.5, g, GATE_HIGH - 0.6), REACH.2));
    }

    // The vomitoria: from a terrace's back (the floor's edge) out under
    // the terraces above into a room of the shell.
    for (deg, y) in VOMITORIA {
        let from = FLOOR + DEEP * (y / RISE);
        cuts.push(spoke(deg, short_of(from, 1.5), ROOM_IN + 0.5, 1.5, y, y + 3.0));
        lamps.push((at((from + ROOM_IN) * 0.5, deg, y + 2.6), REACH.1));
    }

    // The aisles: a flight cut into each giant step's riser, from the
    // terrace below to the next (the first from the floor), 20 steps in
    // 5 m of the next one's 6.
    for line in AISLES {
        for k in 0..TIERS {
            let deg = line + if k % 2 == 0 { -ZIGZAG } else { ZIGZAG };
            let (r, y) = (FLOOR + DEEP * k as f32, RISE * k as f32);
            cuts.push(spoke(deg, short_of(r, AISLE_HALF), r + 5.0, AISLE_HALF, y, y + RISE + 0.01));
            let (seen, walked) = flight(at(r, deg, y), radial(deg), (-AISLE_HALF + 0.01, AISLE_HALF - 0.01), 5.0, RISE);
            steps.push(seen);
            slopes.push(walked);
        }
    }

    // The stair halls: two flights along a room, half a storey each, side
    // by side, a landing between them (the user's, as the court's
    // stairwell: one flight a storey high filled the room, its ends 0.6 m
    // from its walls); the second up through a hole in the floor over it
    // and the landing. (Over a straight flight, a hole begun halfway up
    // it: going down, the user's head met its edge.)
    let mut landings = Vec::new();
    for s in 0..STOREYS {
        let f = STOREY * s as f32;
        for c in (0..ROOMS).filter(|&i| room(s, i).0 == 's').map(|i| 360.0 / ROOMS as f32 * i as f32) {
            // (In the room: `x` round it from its middle, `out` from its
            // inner wall.)
            let (u, t) = (radial(c), radial(c + 90.0));
            let p = |x: f32, out: f32| at(ROOM_IN, c, f) + t * x + u * out;
            let plan = |(x0, x1): (f32, f32), (o0, o1): (f32, f32)| {
                let q = |x: f32, o: f32| {
                    let v = p(x, o);
                    Vec2::new(v.x, v.z)
                };
                [q(x0, o0), q(x0, o1), q(x1, o1), q(x1, o0)]
            };
            // (Each flight, and the landing, a slab borne only where it
            // meets a wall or a floor, open beneath to walk under: the
            // user's. The first into the inner wall a little, as the
            // landing, the wall's curve leaving no gap.)
            let half = STOREY * 0.5;
            let (first, second) = (p(-3.5, 0.0), p(1.5, 2.5) + Vec3::Y * half);
            for (seen, walked) in [flight_over(first, t, (-2.5, 0.1), 5.0, half, Some(WAIST)), flight_over(second, -t, (0.0, 2.5), 5.0, half, Some(WAIST))] {
                steps.push(seen);
                slopes.push(walked);
            }
            // The landing, on into the room's end wall (ended short of it,
            // a drop the bot ran off); a parapet along the open side of
            // each flight and of the landing.
            landings.push(prism(&plan((1.5, 6.5), (-0.5, 5.0)), f + half - WAIST, f + half));
            landings.push(prism(&plan((1.5, 6.5), (5.0, 5.5)), f + half - WAIST, f + half + 1.0));
            let rail_on = section(&[[0.0, -WAIST], [5.0, half - WAIST], [5.0, half + 1.0], [0.0, 1.0]]);
            landings.push(placed(&across(&rail_on, -2.5, -2.2), first, t, -u));
            landings.push(placed(&across(&rail_on, 2.5, 3.0), second, -t, u));
            // The hole over the second flight, its head borne on the hole's
            // edge; on the floor above, a parapet round it but at that head.
            let (top, rail) = (f + STOREY, f + STOREY + 1.0);
            cuts.push(prism(&plan((-3.5, 2.5), (2.4, 5.0)), top - SLAB - 0.5, top + 0.01));
            landings.extend([plan((-3.6, 3.0), (1.9, 2.4)), plan((2.5, 3.0), (2.4, 5.0)), plan((-3.6, 3.0), (5.0, 5.5))].map(|outline| prism(&outline, top, rail)));
        }
    }

    // All of it, cut away in the wedge; on the plaza, parapets round the
    // top.
    let wedge = {
        let far = |deg: f32| {
            let p = radial(deg) * 400.0;
            Vec2::new(p.x, p.z)
        };
        prism(&[Vec2::ZERO, far(-CUT), far(0.0), far(CUT)], -1.0, TOP + 10.0)
    };
    let building = cavea.union(&shell).difference(&Manifold::batch_union(&cuts));
    let parapets = Manifold::batch_union(&[ring(INNER, INNER + 0.5, TOP, TOP + 1.2), ring(OUTER - 0.5, OUTER, TOP, TOP + 1.2)]);
    let above = Manifold::batch_union(&[building, parapets, Manifold::batch_union(&piers), Manifold::batch_union(&landings)]).difference(&wedge);
    // On the plaza, out from the floor's edge; the floor as laid out.
    let plaza = ring(FLOOR, 600.0, -2.0, 0.0);
    let floor = floor(layout);
    let mut all = vec![above, plaza];
    all.extend(floor.solid);
    let mut solid = Manifold::batch_union(&all);
    if !floor.cuts.is_empty() {
        solid = solid.difference(&Manifold::batch_union(&floor.cuts));
    }
    steps.extend(floor.steps);
    slopes.extend(floor.slopes);
    lamps.retain(|(p, _)| !in_cut(p.z.atan2(p.x).to_degrees(), 1.0));

    // In the world: in twelve pieces round (so what is out of view, or out
    // of a lamp's reach, is not drawn).
    let place = |m: &Manifold| m.translate(CENTRE.x as f64, CENTRE.y as f64, CENTRE.z as f64);
    for k in 0..12 {
        let piece = sector(0.0, 700.0, (30.0 * k as f32, 30.0 * (k + 1) as f32), -20.0, TOP + 10.0);
        let mut g = Geometry::default();
        g.solid(&place(&solid.intersection(&piece)));
        parts.chunks.push(g);
    }
    parts.steps.solid(&place(&Manifold::batch_union(&steps)));
    for s in &slopes {
        parts.slope(&place(s));
    }
    for &(p, range) in &lamps {
        let p = p + CENTRE;
        if spots {
            parts.spots.push((p, Vec3::NEG_Y, range, lamp(range)));
        } else {
            parts.lights.push((p, range, lamp(range)));
        }
        parts.glow.oriented(p + Vec3::Y * 0.5, Vec3::X * 0.3, Vec3::Y * 0.05, Vec3::Z * 0.3);
    }
    let (most, where_) = crowded(&lamps, spots);
    let report = vec![format!(
        "{} m across, {} m high, the floor {}; {} triangles, {} flights, {} lamps (at most {most} reaching one point, at {:.0})",
        OUTER * 2.0,
        TOP,
        FLOORS[layout],
        parts.chunks.iter().map(|g| g.indices.len() / 3).sum::<usize>(),
        slopes.len(),
        lamps.len(),
        where_ + CENTRE
    )];
    (parts, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rooms' plans as `PLANS` says they are: a letter to a room, a
    /// pen where a gate is and only there, cells in the cut.
    #[test]
    fn plans() {
        for s in 0..STOREYS {
            let f = STOREY * s as f32;
            assert_eq!(PLANS[s as usize].len(), ROOMS as usize, "storey {s}'s plan: a letter to a room");
            for i in 0..ROOMS {
                let c = 360.0 / ROOMS as f32 * i as f32;
                let (kind, _) = room(s, i);
                assert!("cehosp".contains(kind), "storey {s}, room {i}: no kind {kind}");
                assert_eq!(kind == 'p', GATES.contains(&c) && f < GATE_HIGH, "storey {s}, room {i} ({c} degrees): a pen where a gate is, and only there");
                if in_cut(c, 5.0) {
                    assert_eq!(kind, 'c', "storey {s}, room {i} ({c} degrees): in the cut, a cell");
                }
            }
        }
    }

    /// No point of the building is reached by more lamps than cast shadows at
    /// once (see `REACH`): failing, the lamps' layout or their reach must
    /// change, not the test.
    #[test]
    fn lamps_never_crowd() {
        for spots in [false, true] {
            let (parts, _) = build(0, spots);
            let lamps: Vec<(Vec3, f32)> = parts.lights.iter().map(|&(p, reach, _)| (p - CENTRE, reach)).chain(parts.spots.iter().map(|&(p, _, reach, _)| (p - CENTRE, reach))).collect();
            let (most, at) = crowded(&lamps, spots);
            assert!(most <= super::super::SHADOWED, "{most} lamps reach {:?}, more than the {} that cast shadows at once", at + CENTRE, super::super::SHADOWED);
        }
    }

    /// Every floor builds, and on every body's navmesh every landmark is
    /// reached from it.
    #[test]
    fn floors_reached() {
        use crate::navmesh::{Body, Floors, bounds, unreached};
        for layout in 0..FLOORS.len() {
            let (parts, _) = build(layout, true);
            let colliders = super::super::solid(&parts, false);
            for body in Body::ALL {
                let floors = Floors::new(&crate::navmesh::build(&colliders, body.size(), bounds(&colliders)).unwrap(), body.size().0, false).unwrap();
                for (group, _, lost) in unreached(&floors, &checks(layout)) {
                    assert!(lost.is_empty(), "the floor {}, the {body:?}: {group}, {lost:?} not reached", FLOORS[layout]);
                }
            }
        }
    }
}
