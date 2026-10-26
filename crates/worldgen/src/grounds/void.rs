//! Cylinders in the void (the user's, after the chasm, 2026-10-23: a huge
//! void, no floor in sight, and out of it cylinders on a grid, as of some
//! vast machine of unknown origin; what matters is how they are joined).
//! Unlike the chasm, no way down to follow, no rooms to line up: a field to
//! cross however one likes.
//!
//! Its joints are defined once, so nothing can misalign (the chasm's
//! lesson: free geometry meeting free geometry went makeshift, an exception
//! at every seed's angle): cylinders on a square grid, all alike; datum
//! levels shared by the whole field; at a level a cylinder may wear a
//! collar, a ring of floor round it; and a span always runs collar to
//! collar, radial to both, its ends landing on them. Variety only by
//! leaving things out and by discrete choices, never by nudging off the
//! grid. Two recipes for the spans: the machine (along the grid's axes, at
//! a level) and the web (also across the diagonals, and up or down a level
//! as ramps).
//!
//! It stands high (the lab's flat ground, its floor, lies far below, lost in
//! the haze, as the chasm's: see the game's `look.rs`).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use glam::{DVec2, DVec3};

use super::cuts::{Band, Cut, Leg, Wall};
use super::{Builder, SPAN, rand};

/// The grid (m), the cylinders' radius (m) and sides, and the chance one is
/// missing.
const GRID: f64 = 150.0;
const RADIUS: f64 = 22.0;
const SIDES: usize = 32;
const MISSING: f64 = 0.1;
/// The datum levels: the lowest (m), how far apart (m), how many.
pub const LEVEL: (f64, f64, usize) = (1200.0, 40.0, 8);
/// A cylinder's top: at one of the levels, from this one up.
const TOP_FROM: usize = 3;
/// Collars: the chance of one at a level under a cylinder's top, how far
/// out from the cylinder (m), how thick (m).
const COLLAR: (f64, f64, f64) = (0.5, 8.0, 3.0);
/// Spans: the chance of one where two collars face each other, how wide
/// (m), how thick (m).
const SPAN_CHANCE: f64 = 0.6;
const DECK: (f64, f64) = (7.0, 2.5);

/// Which spans.
pub enum Recipe {
    Machine,
    Web,
}

/// What collars and spans are made as (the user, 2026-10-23: solid, the
/// rings read as hoops and the spans as bars, a rack of pipes; more
/// engineered, thin members leaving gaps and showing the forces at work,
/// but not human: our own logics, not a bridge's trusses):
/// - `Solid`: slabs.
/// - `Lattice`: a deck over an open frame, frames within its frames (the
///   fractal rule's, frames within frames).
/// - `Tension`: decks hung by bundled stays from nodes on the cylinders
///   above.
/// - `Branch`: decks carried on struts branching up out of the cylinders
///   below.
/// - `Mixed`: lattice collars, branching spans (the user: the lattice the
///   best for collars, the branches for bridges).
/// - `Meander`: mixed, a path along each collar's members darker than the
///   rest, a motif repeated round the ring (the user's).
/// - `Inlay`: no collars (the user: even improved, they felt added on):
///   at each level a span reaches, the cylinder cut back to a core, a
///   groove round it to walk, a collar inlaid; each span the cylinders'
///   own stone, an arch seen from above, its edges curving out into the
///   cylinders, tangent to them (the user's sketch); carried as `Carry`
///   says; the grooves with piers left standing in them, or not, as each
///   cylinder's recipe has it; with the scale lab's `Cues` cut into its
///   drums.
pub enum Style {
    Solid,
    Lattice,
    Tension,
    Branch,
    Mixed,
    Meander,
    Inlay(Carry, Infill, Cues),
}

/// What fills an inlaid groove's openings, between its piers (the user: the
/// piers alone read as a prison's bars, neither machine nor classical):
/// nothing; a balcony out from the face, carried on a bay of the lattice
/// collars' frame within a frame; or a screen across the opening, the chasm
/// rooms' fretwork (both the user's). (Plain stone first, a parapet and a
/// screen hung from the ceiling: the user meant patterns.)
#[derive(Clone, Copy)]
pub enum Infill {
    Open,
    Balcony,
    Screen,
}

/// The scale lab's cues (the user, 2026-10-25: make scale readable), cut
/// into an inlaid cylinder's drums (see `cuts.rs`):
/// - `courses`: joints round the drums on one module, a storey's (a
///   ruler); at each datum level a course a groove's height, so the levels
///   read where no groove is cut. (They read as how the drum was built, not
///   as its size.)
/// - `seams`: where the drum's parts were joined (the user's, 2026-10-25:
///   the courses read as how it was built, not as its size): see `Seams`.
///
/// (Removed, 2026-10-25: shafts, clusters of slots cut down from under the
/// grooves, after the Citadel's (the user's reference), straight, leaning
/// or jogging; windows, blocks of small openings, some lit. Shallow, they
/// read as marks drawn on; 5 m deep, dark inside, as black stripes; with
/// louvres across them, as pigeon holes, the user: a recess in a convex
/// face is no shaft, the Citadel's are members standing in a gap between
/// layers, running on into the dark, and at a human scale.)
///
/// And `lamps`: one at each end of each walkway (see `Lamp`).
#[derive(Clone, Copy, Default)]
pub struct Cues {
    pub courses: bool,
    pub seams: Option<Seams>,
    pub lamps: Option<Lamp>,
}

/// The alien world's streetlight (the user's, 2026-10-25: alien, so it may
/// be weird, but roughly a body's size, as a streetlight is, so the player sizes
/// what is round it by it; light and dark the world's own, these its trees,
/// in a way). One at each end of each walkway (one, not a row: the field
/// stays desolate), lit where the walkway carries a pulse's channel, dark
/// where none runs. A walkway's at its right as it leaves its cylinder, out
/// past its curves, so its two stand on its two sides:
/// - `Monolith`: a slab of the drums' metal, a slot cut through it near its
///   top along the walkway, a glowing rod standing in the slot.
/// - `Cradle`: two prongs rising from the deck, curving in to each other
///   across the walkway, a glowing bead held between their tips.
/// - `Pendant`: in the groove, over the walkway's way in, a glowing bead
///   hanging from the ceiling on a rod, a little over head height.
/// - `Echo`: the field in small (the user's: seen from under it, a cylinder
///   with two walkways leaving it at right angles is a streetlight, its
///   pole, its arms, two lamps hanging off them): a pole, a groove round
///   its core; from the groove's floor two arms at right angles, out over
///   the walkway; on each a drum hanging, its groove's core lit; its foot
///   as `Foot` says.
#[derive(Clone, Copy)]
pub enum Lamp {
    Monolith,
    Cradle,
    Pendant,
    Echo(Foot),
}

/// How an echo meets the walkway (the user: plain, it stood on it, nothing
/// anchoring it; a mechanism, a plate or bolts, would be ours, not the
/// field's): `Plain`, standing on it; `Socket`, down through it in a round
/// well cut in it a little wider than the pole (as the cylinders go on
/// down into the void). (A fillet too, the deck curving up into it, as the
/// walkways curve into the cylinders: soft, cast, barely there from 12 m;
/// retired.)
#[derive(Clone, Copy)]
pub enum Foot {
    Plain,
    Socket,
}

/// How a drum was assembled, its seams cut as the courses' joints are: of
/// shells round it, each seam between two interlocking, jogging aside one
/// way and then the other each storey (the user's sketch); the shells
/// staves the drum's whole height, or rings a storey high, a joint round
/// it at each datum level, each ring's shells a half shell round from the
/// next's (as bricks are laid).
#[derive(Clone, Copy)]
pub enum Seams {
    Staves,
    Rings,
}

/// How an inlaid span is carried (the user, 2026-10-23: a ribbon, no weight
/// or bearing): an arch beneath it, springing from the cylinders, or the
/// branch spans' trees.
#[derive(Clone, Copy)]
pub enum Carry {
    Arch,
    Trees,
}

/// An inlaid span's half width (m) and the radius of the curves its edges
/// sweep out on to meet the cylinders (m); the grooves, so deep (to the
/// core) and so high (m). (On the way, the user's sketch read other ways:
/// the span's band cut on into the cylinder, a notch, 6 m deep, hardly
/// showing, past its middle a hall, read inverted; then the crescents
/// beside the band cut, the band left solid, too shallow to walk round to
/// another span; then a core with the band carried on in as a tongue, the
/// tongue a block pushed into the groove, added on again.)
const ARCH: (f64, f64) = (3.5, 20.0);
pub(super) const NOTCH: (f64, f64) = (6.0, 7.0);
/// An arched span's soffit: so far down under its deck where it meets the
/// cylinders (m), rising to the deck's own depth at its middle.
const SOFFIT: f64 = 20.0;
/// Piers left standing in a groove: so many round it (each cylinder one of
/// these, by chance; none where a span comes in), so wide (m).
const PIERS: ([usize; 3], f64) = ([0, 8, 16], 3.0);
/// A balcony: out from the face so far, its slab so thick, the frame under
/// it so deep (m).
const BALCONY: (f64, f64, f64) = (3.0, 0.8, 5.0);
/// A screen: its fretwork drawn as for an opening so many times smaller (a
/// room's, its bars then so much thicker), its bars so deep, set back so far
/// from the face (m).
const FRET: (f64, f64, f64) = (2.5, 0.3, 0.6);
/// A screen's widest empty square, at most (in its drawing's units: wider,
/// an opening with a gap in it, not a screen, the user). (Its bars darker
/// than the stone, to read apart from it, the user's: lost against the
/// dark gallery behind them; a metal, perhaps.)
const GAP: f32 = 0.9;
/// The cores' pulses: so far apart as they climb (m). The channels they run
/// along the walkways in, from core to core: so wide, standing so far proud
/// of the decks (m). A core's tone where no pulse climbs (see
/// `core_glow.wgsl`).
pub(super) const PULSE: f64 = 200.0;
const CHANNEL: (f64, f64) = (0.6, 0.03);
const STEADY: f32 = 2.0;
/// Courses' joints: so high, so deep (m); none under this height (lost in
/// the haze, and so many).
const JOINT: (f64, f64) = (0.3, 0.25);
const COURSES_FROM: f64 = 1000.0;
/// Seams: so many shells round a drum (each cylinder one of these); a
/// tab's jog aside (m), and its foot, top and way, as fractions of a
/// storey.
const SHELLS: [usize; 3] = [2, 3, 4];
const TAB: (f64, [(f64, f64, f64); 2]) = (5.0, [(0.2, 0.42, -1.0), (0.58, 0.8, 1.0)]);

/// Lamps: a lit one's glowing part's tone (see `core_glow.wgsl`), an unlit
/// one's; how far in from a walkway's edge one stands (m).
const LAMP: f32 = 3.0;
const UNLIT: f32 = 0.04;
const KERB: f64 = 1.2;
/// A monolith: so thick (along the walkway), so wide, so high; its slot
/// from so high to so high, so wide; its rod so thick (m).
const MONOLITH: (f64, f64, f64) = (0.5, 1.2, 4.2);
const SLOT: (f64, f64, f64) = (2.5, 3.8, 0.5);
const ROD: f64 = 0.16;
/// A cradle: its prongs so far apart at their feet and at their tips, so
/// high, so thick at their feet and at their tips; its bead so wide (m).
const CRADLE: ((f64, f64), f64, (f64, f64), f64) = ((1.3, 0.6), 3.8, (0.32, 0.18), 0.28);
/// A pendant: its rod so thick, down to so high over the floor; its bead
/// so wide, so long (m).
const PENDANT: (f64, f64, f64, f64) = (0.14, 3.6, 0.36, 0.9);
/// An echo: its pole so wide (radius), so high; its groove's floor so high
/// up it, the groove so high; its arms so far out (to a lamp's middle), so
/// wide, so deep at their middles, their soffits springing so far down at
/// their ends; its lamps' drums hanging so far under the arms, standing so
/// far over them (m), as wide as the pole, as the cylinders all are. (Its
/// cores as the cylinders' are, so far in. At first its lamps narrower than
/// its pole, its arms flat: a streetlight, an ordinary one.)
const ECHO: (f64, f64, f64, f64) = (0.25, 5.28, 4.56, 0.22);
const ARM: (f64, f64, f64, f64) = (1.4, 0.16, 0.1, 0.35);
const ECHO_DRUM: (f64, f64) = (0.7, 0.5);
/// An echo's socket: so much wider than its pole all round, so deep, its
/// strip of walkway so far along either side of it (m). Lamps stand so far out past a walkway's curves (m: a socket's strip
/// clear of them).
const SOCKET: (f64, f64, f64) = (0.12, 1.2, 1.0);
/// The lamps' round parts' sides (fewer than the cylinders': at a lamp's
/// size, shaded smooth, as round; the cylinders' 32 were 60% of an echo's
/// triangles).
const LAMP_SIDES: usize = 12;
const OUT: f64 = 1.5;

/// The cylinders' stone, and the inlaid spans' (the cylinders' own: lighter,
/// they looked added on, the user).
const STONE: f32 = 0.12;

/// A lattice collar's members, by bay (round the ring from its first post):
/// the chord along its foot; the post at its left; a stay from its corner
/// (0 top left, 1 top right, 2 foot right, 3 foot left) to its inner
/// frame's; the inner frame's edge from that corner to the next.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Member {
    Chord(usize),
    Post(usize),
    Stay(usize, usize),
    Inner(usize, usize),
}

/// Decks over members: how thick (m); the lattice's depth under its deck
/// (m) and its bays (m, about); members' and decks' tones.
const THIN: f64 = 1.5;
const FRAME: (f64, f64) = (8.0, 9.0);
const MEMBER: f32 = 0.13;
/// A meander's members, and the rest of its collar's (lighter than members
/// are, or the path hardly showed against them).
const PATH: f32 = 0.025;
const AROUND: f32 = 0.4;
const PLATE: f32 = 0.21;

pub fn build(b: &mut Builder, seed: u32, recipe: Recipe, style: Style) {
    let half = (SPAN * 0.5 / GRID).floor() as i32;
    let middle = b.centre;
    let centre = |i: i32, j: i32| middle + DVec2::new(i as f64, j as f64) * GRID;
    let r = |i: i32, j: i32, k: i32| rand(seed, i, j, 900 + k);
    let level = |k: usize| LEVEL.0 + LEVEL.1 * k as f64;
    let present = |i: i32, j: i32| i.abs() <= half && j.abs() <= half && r(i, j, 0) >= MISSING;
    let top = |i: i32, j: i32| TOP_FROM + (r(i, j, 1) * (LEVEL.2 - TOP_FROM) as f64) as usize;
    let collar = |i: i32, j: i32, k: usize| present(i, j) && k < top(i, j) && r(i, j, 10 + k as i32) < COLLAR.0;
    // The spans first: from each cylinder to its neighbours ahead (each
    // pair once), where both could wear collars at the levels it joins.
    let ahead: &[(i32, i32)] = match recipe {
        Recipe::Machine => &[(1, 0), (0, 1)],
        Recipe::Web => &[(1, 0), (0, 1), (1, 1), (1, -1)],
    };
    let mut spans = Vec::new();
    for j in -half..=half {
        for i in -half..=half {
            for (n, &(di, dj)) in ahead.iter().enumerate() {
                let (ni, nj) = (i + di, j + dj);
                for k in 0..LEVEL.2 {
                    // (The web: also a ramp from this level to the one
                    // above or below at the other end.)
                    let ends: &[i32] = match recipe {
                        Recipe::Machine => &[0],
                        Recipe::Web => &[0, 1, -1],
                    };
                    for &dk in ends {
                        let k2 = k as i32 + dk;
                        if k2 < 0 || k2 >= LEVEL.2 as i32 || !collar(i, j, k) || !collar(ni, nj, k2 as usize) {
                            continue;
                        }
                        // (Fewer ramps and diagonals than level spans.)
                        let chance = SPAN_CHANCE * if dk != 0 { 0.25 } else { 1.0 } * if di != 0 && dj != 0 { 0.5 } else { 1.0 };
                        if r(i, j, 100 + n as i32 * 40 + k as i32 * 3 + dk + 1) >= chance {
                            continue;
                        }
                        spans.push(((i, j, k), (ni, nj, k2 as usize)));
                    }
                }
            }
        }
    }
    // Collars only where a span lands (the user: those nothing reached, a
    // ring round a cylinder for no reason, cluttered the field).
    let reached: HashSet<(i32, i32, usize)> = spans.iter().flat_map(|&(a, z)| [a, z]).collect();
    // The pulses' network (inlaid; step 3 of the user's machine pass): a
    // pulse climbing a core, at a walkway's level, runs along it in a
    // channel to the next core and climbs on up it. So each core's pulses
    // are its feeder's, later by the channel's length: fed out from roots
    // (taken in a random order, each fed from the depths, its pulses at a
    // phase of its own) to their neighbours, each by every walkway to it at
    // or over where its feeder's own pulses start, its own starting at the
    // lowest. (Each core's phase, the level its pulses start at; each
    // span's channel, if it has one, whether it runs from its first end.)
    let mut feed: HashMap<(i32, i32), (f64, usize)> = HashMap::new();
    let mut lit: Vec<Option<bool>> = vec![None; spans.len()];
    if matches!(style, Style::Inlay(..)) {
        let mut order: Vec<(i32, i32)> = (-half..=half).flat_map(|j| (-half..=half).map(move |i| (i, j))).filter(|&(i, j)| present(i, j)).collect();
        order.sort_by(|a, z| r(a.0, a.1, 403).total_cmp(&r(z.0, z.1, 403)));
        // (From core face to core face.)
        let length = |a: (i32, i32), z: (i32, i32)| centre(a.0, a.1).distance(centre(z.0, z.1)) - 2.0 * (RADIUS - NOTCH.0);
        for root in order {
            if feed.contains_key(&root) {
                continue;
            }
            feed.insert(root, (r(root.0, root.1, 401), 0));
            let mut queue = VecDeque::from([root]);
            while let Some(x) = queue.pop_front() {
                let (phase, entry) = feed[&x];
                let mut fed: BTreeMap<(i32, i32), usize> = BTreeMap::new();
                for (n, &((ai, aj, k), (zi, zj, k2))) in spans.iter().enumerate() {
                    let to = if (ai, aj) == x { (zi, zj) } else if (zi, zj) == x { (ai, aj) } else { continue };
                    assert_eq!(k, k2, "inlaid spans are level: the machine's");
                    if k < entry || feed.contains_key(&to) {
                        continue;
                    }
                    lit[n] = Some((ai, aj) == x);
                    let lowest = fed.entry(to).or_insert(k);
                    *lowest = (*lowest).min(k);
                }
                for (to, k) in fed {
                    feed.insert(to, ((phase + length(x, to) / PULSE).fract(), k));
                    queue.push_back(to);
                }
            }
        }
    }
    // The cylinders, out of the depths; their collars.
    for j in -half..=half {
        for i in -half..=half {
            if !present(i, j) {
                continue;
            }
            let c = centre(i, j);
            b.shade(seed, (i, j), STONE, 0.015);
            if let Style::Inlay(_, infill, cues) = style {
                // (Each span's way out of this cylinder, by level.)
                let ways: Vec<(usize, DVec2)> = spans
                    .iter()
                    .filter_map(|&((ai, aj, ak), (zi, zj, zk))| match ((ai, aj) == (i, j), (zi, zj) == (i, j)) {
                        (true, _) => Some((ak, (centre(zi, zj) - c).normalize())),
                        (_, true) => Some((zk, (centre(ai, aj) - c).normalize())),
                        _ => None,
                    })
                    .collect();
                let piers = PIERS.0[(r(i, j, 300) * PIERS.0.len() as f64) as usize];
                // (The core's light: the machine shown, with restraint, where
                // the grooves expose it, the user; its pulses as fed.)
                let pattern = (r(i, j, 402) * 1e6) as u32;
                inlaid(b, c, level(top(i, j)), &ways, &level, (piers, infill, pattern), feed[&(i, j)], (cues, &|k| r(i, j, 500 + k)));
                continue;
            }
            b.cylinder(c, &ring(c, RADIUS), level(top(i, j)), -1.0, false);
            for k in 0..LEVEL.2 {
                if reached.contains(&(i, j, k)) {
                    ring_collar(b, &style, c, level(k), r(i, j, 60 + k as i32), &|n| r(i, j, 200 + k as i32 * 20 + n));
                }
            }
        }
    }
    for (n, ((i, j, k), (ni, nj, k2))) in spans.into_iter().enumerate() {
        if let Style::Inlay(carry, _, cues) = style {
            let sockets = match cues.lamps {
                Some(Lamp::Echo(Foot::Socket)) => vec![stand(centre(i, j), centre(ni, nj)), stand(centre(ni, nj), centre(i, j))],
                _ => Vec::new(),
            };
            arch(b, centre(i, j), centre(ni, nj), level(k), carry, &sockets);
            if let Some(kind) = cues.lamps {
                b.tone = STONE;
                lamp(b, kind, centre(i, j), centre(ni, nj), level(k), lit[n].is_some());
                lamp(b, kind, centre(ni, nj), centre(i, j), level(k), lit[n].is_some());
            }
            if let Some(forward) = lit[n] {
                let (from, to) = if forward { ((i, j), (ni, nj)) } else { ((ni, nj), (i, j)) };
                channel(b, centre(from.0, from.1), centre(to.0, to.1), level(k), feed[&from].0);
            }
        } else {
            span(b, &style, centre(i, j), level(k), centre(ni, nj), level(k2));
        }
    }
}

/// An inlaid cylinder at `c`, its top at `top`: solid, but cut back
/// `NOTCH.0` to a core, `NOTCH.1` high, at each level a span leaves it by
/// (`ways`: the level and the span's way out); so many piers left standing
/// round each groove, but where a span comes in, and between each two the
/// infill (a screen's fretwork `pattern`, the same in every opening); its
/// core where the grooves show it glowing (see `Patch::glows`), its pulses
/// at `phase`, from the level `entry` up; the scale lab's `cues` cut into
/// its drums (`pick`: its random numbers).
#[allow(clippy::too_many_arguments)]
fn inlaid(b: &mut Builder, c: DVec2, top: f64, ways: &[(usize, DVec2)], level: &dyn Fn(usize) -> f64, (piers, infill, pattern): (usize, Infill, u32), (phase, entry): (f64, usize), (cues, pick): (Cues, &dyn Fn(i32) -> f64)) {
    let (deep, high) = NOTCH;
    let core = ring(c, RADIUS - deep);
    let mut levels: Vec<usize> = ways.iter().map(|&(k, _)| k).collect();
    levels.sort();
    levels.dedup();
    let mut bottom = -1.0;
    let mut floats = false;
    let shells = SHELLS[(pick(-1) * SHELLS.len() as f64) as usize];
    for &k in &levels {
        let y = level(k);
        drum(b, c, (bottom, y), floats, (cues, shells));
        let stone = b.tone;
        b.tone = if k >= entry { phase as f32 } else { STEADY };
        b.glow(|b| b.cylinder(c, &core, y + high, y, false));
        b.tone = stone;
        // Its piers: into the core, a little short of the face.
        let width = PIERS.1 * 0.5;
        let clear = mouth() + (width / RADIUS).atan() + 0.05;
        let angle = |n: usize| n as f64 * std::f64::consts::TAU / piers as f64;
        let standing = |n: usize| !ways.iter().any(|&(l, u)| l == k && DVec2::from_angle(angle(n)).angle_to(u).abs() < clear);
        let face = RADIUS - 0.4;
        for n in (0..piers).filter(|&n| standing(n)) {
            let way = DVec2::from_angle(angle(n));
            let aside = way.perp() * width;
            let (inner, outer) = (c + way * (RADIUS - deep - 0.5), c + way * face);
            b.prism(&[inner - aside, outer - aside, outer + aside, inner + aside], y + high, y, false);
            // The infill to the next pier, if it stands: from one's side
            // to the other's.
            if !standing((n + 1) % piers) {
                continue;
            }
            // (Between the piers' sides, at `r` from the middle.)
            let between = |r: f64| {
                let half = (width / r).asin();
                (angle(n) + half, angle(n) + std::f64::consts::TAU / piers as f64 - half)
            };
            match infill {
                Infill::Open => {}
                Infill::Balcony => balcony(b, c, y, between(RADIUS)),
                Infill::Screen => screen(b, c, y, high, between(face - FRET.2), pattern),
            }
        }
        bottom = y + high;
        floats = true;
    }
    drum(b, c, (bottom, top), floats, (cues, shells));
}

/// A drum of an inlaid cylinder at `c`, from `y.0` up to `y.1` (its foot
/// floating over a groove, or down in the depths); the scale lab's `cues`
/// cut into it, its seams', if any, between so many `shells`.
fn drum(b: &mut Builder, c: DVec2, y: (f64, f64), floats: bool, (cues, shells): (Cues, usize)) {
    let round = ring(c, RADIUS);
    if !cues.courses && cues.seams.is_none() {
        b.cylinder(c, &round, y.1, y.0, floats);
        return;
    }
    let wall = Wall { c, radius: RADIUS, sides: SIDES };
    let around = wall.perimeter();
    let mut bands: Vec<Band> = if cues.courses { courses(y).iter().map(|&h| Band { y: (h - JOINT.0 * 0.5, h + JOINT.0 * 0.5), deep: JOINT.1 }).collect() } else { Vec::new() };
    let mut cuts = Vec::new();
    if let Some(seams) = cues.seams {
        let (base, apart, _) = LEVEL;
        let from = y.0.max(COURSES_FROM);
        let storey = |h: f64| ((h - base) / apart).floor() as i32;
        // Its pieces up it (between the rings' joints, if it has them),
        // each one's shells' seams a half shell round if its storey is odd.
        let mut edges = vec![from];
        if let Seams::Rings = seams {
            let joints: Vec<f64> = (storey(from) + 1..=storey(y.1)).map(|l| base + apart * l as f64).filter(|&h| h > from + 1.0 && h < y.1 - 1.0).collect();
            bands.extend(joints.iter().map(|&h| Band { y: (h - JOINT.0 * 0.5, h + JOINT.0 * 0.5), deep: JOINT.1 }));
            edges.extend(joints);
        }
        edges.push(y.1);
        for (n, w) in edges.windows(2).enumerate() {
            let (lo, hi) = (w[0], w[1]);
            let joint = |at: bool| if at { JOINT.0 * 0.5 } else { 0.0 };
            let ends = (lo + joint(n > 0), hi - joint(n + 2 < edges.len()));
            let tabs: Vec<(f64, f64, f64)> = (storey(lo)..=storey(hi))
                .flat_map(|l| TAB.1.iter().map(move |&(a, z, way)| (base + apart * (l as f64 + a), base + apart * (l as f64 + z), way)))
                .filter(|&(a, z, _)| a > lo + 1.0 && z < hi - 1.0)
                .collect();
            let stagger = match seams {
                Seams::Staves => 0,
                Seams::Rings => storey((lo + hi) * 0.5).rem_euclid(2) as usize,
            };
            for k in 0..shells {
                let x = (2 * (2 * k + stagger) + 1) as f64 * around / (4 * shells) as f64;
                seam(&mut cuts, x, ends, &tabs);
            }
        }
    }
    b.top(&round, y.1);
    if floats {
        b.underside(&round, y.0);
    }
    wall.build(b, y, &cuts, &bands);
}

/// Where the lamp for the walkway from the cylinder at `c` towards the one
/// at `to` stands, at that end: at its right, out past its curves.
fn stand(c: DVec2, to: DVec2) -> DVec2 {
    let u = (to - c).normalize();
    c + u * (sweeps().0.x + OUT) - u.perp() * (ARCH.0 - KERB)
}

/// A lamp of `kind` for the walkway from the cylinder at `c` towards the one
/// at `to`, at that end, its top at `y` (the groove's floor); lit, or not.
fn lamp(b: &mut Builder, kind: Lamp, c: DVec2, to: DVec2, y: f64, lit: bool) {
    let u = (to - c).normalize();
    let v = u.perp();
    let at = |p: DVec2, h: f64| DVec3::new(p.x, h, p.y);
    // (A glowing part, and where its light shines from, if anywhere.)
    let head = |b: &mut Builder, build: &dyn Fn(&mut Builder), light: Option<DVec3>| {
        if lit {
            let stone = b.tone;
            b.tone = LAMP;
            b.glow(|b| build(b));
            b.tone = stone;
            b.lights.extend(light);
        } else {
            let stone = b.tone;
            b.tone = UNLIT;
            build(b);
            b.tone = stone;
        }
    };
    let floor = y - 0.01;
    let p = stand(c, to);
    match kind {
        Lamp::Monolith => {
            let (thick, wide, high) = MONOLITH;
            let (low, top, slot) = SLOT;
            let (t, w) = (thick * 0.5, wide * 0.5);
            let post = (w - slot * 0.5) * 0.5;
            block(b, p, u, (t, w), (floor, floor + low));
            block(b, p, u, (t, w), (floor + top, floor + high));
            for s in [-1.0, 1.0] {
                block(b, p + v * s * (w - post), u, (t, post), (floor + low, floor + top));
            }
            head(b, &|b| block(b, p, u, (ROD * 0.5, ROD * 0.5), (floor + low, floor + top)), Some(at(p, floor + (low + top) * 0.5)));
        }
        Lamp::Cradle => {
            let ((feet, tips), high, (thick, thin), bead) = CRADLE;
            let steps = 10;
            // (Across the walkway, so seen along it they stand apart.)
            let across = DVec3::new(u.x, 0.0, u.y);
            for s in [-1.0, 1.0] {
                let sections: Vec<[DVec3; 4]> = (0..=steps)
                    .map(|n| {
                        let a = n as f64 / steps as f64 * std::f64::consts::FRAC_PI_2;
                        let off = (tips + (feet - tips) * a.cos()) * 0.5;
                        let q = at(p + v * s * off, floor + high * a.sin());
                        let along = (DVec3::new(v.x, 0.0, v.y) * -s * (feet - tips) * 0.5 * a.sin() + DVec3::Y * high * a.cos()).normalize();
                        let side = along.cross(across).normalize();
                        let h = (thick + (thin - thick) * n as f64 / steps as f64) * 0.5;
                        [q + side * h - across * h, q + side * h + across * h, q - side * h + across * h, q - side * h - across * h]
                    })
                    .collect();
                b.sweep(&sections);
            }
            let half = bead * 0.5;
            head(b, &|b| block(b, p, u, (half, half), (floor + high - half, floor + high + half)), Some(at(p, floor + high)));
        }
        Lamp::Pendant => {
            let (rod, from, bead, long) = PENDANT;
            let q = c + u * (RADIUS - NOTCH.0 * 0.5);
            block(b, q, u, (rod * 0.5, rod * 0.5), (y + from, y + NOTCH.1));
            head(b, &|b| block(b, q, u, (bead * 0.5, bead * 0.5), (y + from - long, y + from)), Some(at(q, y + from - long * 0.5)));
        }
        Lamp::Echo(foot) => {
            let (pole, high, groove, band) = ECHO;
            let (reach, wide, deep, spring) = ARM;
            let (hang, over) = ECHO_DRUM;
            let g = floor + groove;
            // (A drum at `q`, so wide, from `y.0` up to `y.1`, the groove
            // cut round its core.)
            let grooved = |b: &mut Builder, q: DVec2, r: f64, y: (f64, f64), light: Option<DVec3>| {
                let round = |r: f64| polygon(q, r, LAMP_SIDES);
                b.cylinder(q, &round(r), g, y.0, y.0 > floor);
                head(b, &|b| b.cylinder(q, &round(r * (RADIUS - NOTCH.0) / RADIUS), g + band, g, false), light);
                b.cylinder(q, &round(r), y.1, g + band, true);
            };
            match foot {
                Foot::Plain => grooved(b, p, pole, (floor, floor + high), None),
                // (Down to the socket's floor.)
                Foot::Socket => grooved(b, p, pole, (y - SOCKET.1, floor + high), None),
            }
            // (Out over the walkway, one either way along it.)
            for s in [-1.0, 1.0] {
                let way = (v + u * s).normalize();
                let q = p + way * reach;
                // (Its soffit as a span's: from face to face, half an
                // ellipse's quarter at each end.)
                let (side, gap) = (way.perp() * wide * 0.5, reach - 2.0 * pole);
                let steps = 12;
                let sections: Vec<[DVec3; 4]> = (0..=steps)
                    .map(|n| {
                        let x = pole + gap * n as f64 / steps as f64;
                        let e = (2.0 * (x - pole) / gap - 1.0).abs();
                        let low = g - deep - spring * (1.0 - (1.0 - e * e).max(0.0).sqrt());
                        let m = p + way * x;
                        [at(m - side, g), at(m + side, g), at(m + side, low), at(m - side, low)]
                    })
                    .collect();
                b.sweep(&sections);
                grooved(b, q, pole, (g - hang, g + over), Some(at(q, g + band * 0.5)));
            }
        }
    }
}

/// A walkway's strip round a socket: from `x.0` to `x.1` along it (from `a`,
/// `u` along, `side` across), the socket at `x.2` across it; its top at `y`;
/// `section`, the walkway's section at a point along it, from a height
/// down. (Its top round the socket, the socket's wall and floor, the strip
/// under them.)
fn socket(b: &mut Builder, (a, u, side): (DVec2, DVec2, DVec2), y: f64, x: (f64, f64, f64), section: &dyn Fn(f64, f64) -> [DVec3; 4]) {
    use std::f64::consts::TAU;
    let (gap, deep, _) = SOCKET;
    let (half, r) = (ARCH.0, ECHO.0 + gap);
    let (top, low) = (y - 0.01, y - deep);
    let o = DVec2::new((x.0 + x.1) * 0.5, x.2);
    let place = |p: DVec2| a + u * p.x + side * p.y;
    // (From the socket's middle out each way to its rim, and on to the
    // strip's edge; the strip's corners among the ways.)
    let mut ways: Vec<f64> = (0..LAMP_SIDES).map(|k| k as f64 * TAU / LAMP_SIDES as f64).collect();
    ways.extend([(x.0, -half), (x.1, -half), (x.1, half), (x.0, half)].map(|(cx, cw)| (cw - o.y).atan2(cx - o.x).rem_euclid(TAU)));
    ways.sort_by(f64::total_cmp);
    ways.dedup_by(|p, q| (*p - *q).abs() < 1e-9);
    let rim = |t: f64| place(o + DVec2::from_angle(t) * r);
    let edge = |t: f64| {
        let d = DVec2::from_angle(t);
        let along = if d.x > 1e-12 { (x.1 - o.x) / d.x } else if d.x < -1e-12 { (x.0 - o.x) / d.x } else { f64::INFINITY };
        let across = if d.y > 1e-12 { (half - o.y) / d.y } else if d.y < -1e-12 { (-half - o.y) / d.y } else { f64::INFINITY };
        place(o + d * along.min(across))
    };
    let middle = place(o);
    for n in 0..ways.len() {
        let (t0, t1) = (ways[n], ways[(n + 1) % ways.len()]);
        b.quad([at(rim(t0), top), at(edge(t0), top), at(edge(t1), top), at(rim(t1), top)], DVec3::Y);
        let inward = middle - (rim(t0) + rim(t1)) * 0.5;
        let q = [at(rim(t0), top), at(rim(t1), top), at(rim(t1), low), at(rim(t0), low)];
        b.smooth(DVec2::new(q[0].x, q[0].z), |p| DVec3::new(middle.x - p.x, 0.0, middle.y - p.z), |b| b.quad(q, DVec3::new(inward.x, 0.0, inward.y)));
    }
    b.top(&polygon(middle, r, LAMP_SIDES), low);
    // Its sides, down to the well's floor; under that, swept as the rest.
    for w in [-half, half] {
        let (p, q) = (place(DVec2::new(x.0, w)), place(DVec2::new(x.1, w)));
        b.quad([at(p, top), at(q, top), at(q, low), at(p, low)], DVec3::new(side.x * w, 0.0, side.y * w));
    }
    b.sweep(&[section(x.0, low), section(x.1, low)]);
}

/// A block standing at `p`, `u` its way along: so far along it and across
/// it either side of `p` (m), from `y.0` up to `y.1`.
fn block(b: &mut Builder, p: DVec2, u: DVec2, (along, across): (f64, f64), y: (f64, f64)) {
    let (a, s) = (u * along, u.perp() * across);
    b.prism(&[p - a - s, p + a - s, p + a + s, p - a + s], y.1, y.0, true);
}

/// A seam up a drum's wall at `x` along it, from `y.0` up to `y.1`, jogging
/// aside at each of `tabs` (its foot, its top, which way), into `cuts`.
fn seam(cuts: &mut Vec<Cut>, x: f64, y: (f64, f64), tabs: &[(f64, f64, f64)]) {
    let (wide, deep) = JOINT;
    let half = wide * 0.5;
    let up = |cuts: &mut Vec<Cut>, s: f64, (a, z): (f64, f64)| cuts.push(Cut { legs: vec![Leg { y: (a, z), s: (s - half, s - half) }], wide, deep });
    let across = |cuts: &mut Vec<Cut>, h: f64, (a, z): (f64, f64)| cuts.push(Cut { legs: vec![Leg { y: (h - half, h + half), s: (a.min(z) - half, a.min(z) - half) }], wide: (z - a).abs() + wide, deep });
    let mut from = y.0;
    for &(a, z, way) in tabs {
        let out = x + way * TAB.0;
        up(cuts, x, (from, a - half));
        across(cuts, a, (x, out));
        up(cuts, out, (a + half, z - half));
        across(cuts, z, (out, x));
        from = z + half;
    }
    up(cuts, x, (from, y.1));
}

/// The courses' joints within a drum from `y.0` to `y.1` (clear of its
/// ends): from each datum level, a groove's height up, then the rest of the
/// way to the next level in eight.
fn courses(y: (f64, f64)) -> Vec<f64> {
    let (base, apart, _) = LEVEL;
    let from = y.0.max(COURSES_FROM);
    let first = ((from - base) / apart).floor() as i32 - 1;
    let last = ((y.1 - base) / apart).ceil() as i32 + 1;
    (first..=last)
        .flat_map(|l| {
            let datum = base + apart * l as f64;
            std::iter::once(datum).chain((0..8).map(move |m| datum + NOTCH.1 + m as f64 * (apart - NOTCH.1) / 8.0))
        })
        .filter(|&h| h >= from && h > y.0 + 1.0 && h < y.1 - 1.0)
        .collect()
}

/// The band between angles `from` and `to` round `c`, `inner` to `outer`
/// from it, in steps of about `step` m along it.
fn sector(c: DVec2, (from, to): (f64, f64), (inner, outer): (f64, f64), step: f64) -> Vec<DVec2> {
    let steps = ((to - from) * outer / step).ceil().max(1.0) as usize;
    let arc = |r: f64| (0..=steps).map(move |s| c + DVec2::from_angle(from + (to - from) * s as f64 / steps as f64) * r);
    let mut outline: Vec<DVec2> = arc(outer).collect();
    outline.extend(arc(inner).collect::<Vec<_>>().into_iter().rev());
    outline
}

/// A balcony round the cylinder at `c` between angles `between`, its top at
/// the groove's floor `y` (a hair under, never its plane): a slab out from
/// the face, under it a bay of a lattice collar's frame, posts down from its
/// ends to a chord, spokes back into the cylinder, the frame within the
/// frame between.
fn balcony(b: &mut Builder, c: DVec2, y: f64, (from, to): (f64, f64)) {
    let (out, thick, deep) = BALCONY;
    b.prism(&sector(c, (from, to), (RADIUS - 0.5, RADIUS + out), 2.0), y - 0.01, y - thick, true);
    let (under, low) = (y - thick, y - thick - deep);
    let rim = RADIUS + out - 0.8;
    let (p, q) = (c + DVec2::from_angle(from) * rim, c + DVec2::from_angle(to) * rim);
    let stone = b.tone;
    b.tone = MEMBER;
    b.beam(at(p, low), at(q, low), 0.6, 0.6, 6);
    for e in [p, q] {
        b.beam(at(e, under), at(e, low), 0.7, 0.7, 6);
        b.beam(at(c + (e - c).normalize() * (RADIUS - 0.5), low), at(e, low), 0.6, 0.6, 6);
    }
    inner_frame(b, [at(p, under), at(q, under), at(q, low), at(p, low)], |_, _| MEMBER);
    b.tone = stone;
}

/// A screen round the cylinder at `c` between angles `between`, from the
/// groove's floor `y` to its ceiling, `high` over it: the chasm rooms'
/// fretwork (`pattern` its seed), drawn on the arc, each bar `FRET.1` deep
/// behind it, a little into the piers either side; the first of its seeds
/// on from `pattern` with no gap wider than `GAP`.
fn screen(b: &mut Builder, c: DVec2, y: f64, high: f64, (from, to): (f64, f64), pattern: u32) {
    let (scale, deep, back) = FRET;
    let r = RADIUS - 0.4 - back;
    let (from, to) = (from - 0.1 / r, to + 0.1 / r);
    let along = (to - from) * r;
    let (w, h) = ((along / scale) as f32, (high / scale) as f32);
    let bars = (pattern..pattern + 64)
        .map(|seed| crate::fret::fret(w, h, seed))
        .find(|bars| widest_gap(bars, w, h) <= GAP)
        .expect("no fretwork without a gap in 64 seeds: GAP too strict for these openings");
    for [u0, v0, u1, v1] in bars {
        let [u0, v0, u1, v1] = [u0, v0, u1, v1].map(|x| x as f64 * scale);
        let bar = sector(c, (from + u0 / r, from + u1 / r), (r - deep, r), 1.5);
        b.prism(&bar, y + v1.min(high), y + v0, true);
    }
}

/// A walkway's channel from the core at `from` to the one at `to`, at its
/// level `y`, down its middle: a pulse leaving the first core's at its
/// level (`phase`, its pulses') and running along it (see `Patch::glows`:
/// a channel's tone is less than 0, its pulse's phase there, -1, the less).
fn channel(b: &mut Builder, from: DVec2, to: DVec2, y: f64, phase: f64) {
    let u = (to - from).normalize();
    let (a, z) = (from + u * (RADIUS - NOTCH.0), to - u * (RADIUS - NOTCH.0));
    let (wide, proud) = CHANNEL;
    b.channel(a, z, y + proud, wide, proud + 0.3, |d| -(1.0 + phase + d / PULSE) as f32);
}

/// An inlaid span's end, round its cylinder: its edges' curves' middles
/// (along the span from the cylinder's middle, out from its line), and
/// where they touch the cylinder (likewise).
fn sweeps() -> (DVec2, DVec2) {
    let (half, sweep) = ARCH;
    let along = ((RADIUS + sweep).powi(2) - (half + sweep).powi(2)).sqrt();
    let middle = DVec2::new(along, half + sweep);
    (middle, middle * RADIUS / (RADIUS + sweep))
}

/// How far round from a span's way its end meets its cylinder (radians).
fn mouth() -> f64 {
    sweeps().1.to_angle()
}

/// An inlaid span from the cylinder at `a` to the one at `z`, its top at
/// `y` (a hair under the grooves' floors, never their plane): `ARCH.0`
/// either side of its line, and at each end its edges swept out on curves
/// of `ARCH.1` to meet the cylinder tangent to it, its end round the
/// cylinder a little into it; carried as `carry` says; a socket cut in it
/// at each of `sockets` (see `Foot`).
fn arch(b: &mut Builder, a: DVec2, z: DVec2, y: f64, carry: Carry, sockets: &[DVec2]) {
    let (half, sweep) = ARCH;
    let length = a.distance(z);
    let (u, side) = ((z - a) / length, (z - a).perp() / length);
    let (middle, touch) = sweeps();
    assert!(2.0 * middle.x < length, "an inlaid span's curves overlap: cylinders too close for ARCH");
    let steps = 12;
    // One end's edge, on side `s` (1 or -1), from the line to the cylinder,
    // in the span's terms (along it from `a`, across it).
    let curve = |s: f64| -> Vec<DVec2> {
        let middle = DVec2::new(middle.x, s * middle.y);
        let (from, to) = ((-s * DVec2::Y).to_angle(), (-middle).to_angle());
        let turn = (to - from + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        (0..=steps).map(|n| middle + DVec2::from_angle(from + turn * n as f64 / steps as f64) * sweep).collect()
    };
    let place = |p: DVec2| a + u * p.x + side * p.y;
    let far = |p: DVec2| DVec2::new(length - p.x, p.y);
    // Round its end: from the cylinder's touch on one side to the other's,
    // a little into it.
    let end = |centre: DVec2, from: DVec2, to: DVec2, out: &mut Vec<DVec2>| {
        let (p, q) = ((from - centre).to_angle(), (to - centre).to_angle());
        let turn = (q - p + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        for n in 0..=8 {
            out.push(centre + DVec2::from_angle(p + turn * n as f64 / 8.0) * (RADIUS - 0.5));
        }
    };
    let (low, high) = (curve(-1.0), curve(1.0));
    let mut outline = Vec::new();
    // Along the low edge from `a` to `z`, round `z`, back along the high
    // edge, round `a`.
    outline.extend(low.iter().rev().map(|&p| place(p)));
    outline.extend(low.iter().map(|&p| place(far(p))));
    end(z, place(far(low[steps])), place(far(high[steps])), &mut outline);
    outline.extend(high.iter().rev().map(|&p| place(far(p))));
    outline.extend(high.iter().map(|&p| place(p)));
    end(a, place(high[steps]), place(low[steps]), &mut outline);
    b.tone = STONE;
    match carry {
        Carry::Arch => {
            // Deck and arch one solid, swept along it: as wide as the deck
            // (from inside the cylinders, where its ends are hidden), its
            // soffit an ellipse's half from face to face, meeting each
            // upright, `SOFFIT` down, rising to the deck's depth midway.
            let width = |x: f64| {
                let x = x.min(length - x);
                if x >= middle.x {
                    half
                } else if x >= touch.x {
                    middle.y - (sweep * sweep - (x - middle.x).powi(2)).max(0.0).sqrt()
                } else {
                    touch.y
                }
            };
            let gap = length * 0.5 - RADIUS;
            let drop = |x: f64| {
                let s = (x.min(length - x) - RADIUS).max(0.0);
                SOFFIT * (1.0 - (1.0 - ((gap - s) / gap).powi(2)).max(0.0).sqrt())
            };
            let x0 = touch.x - 1.0;
            let n = 96;
            let section = |x: f64, top: f64| {
                let (w, foot) = (width(x), y - DECK.1 - drop(x));
                let (l, r) = (place(DVec2::new(x, -w)), place(DVec2::new(x, w)));
                [at(l, top), at(r, top), at(r, foot), at(l, foot)]
            };
            // (Closer together near the ends, where it curves most.)
            let mut xs: Vec<f64> = (0..=n).map(|k| x0 + (length - 2.0 * x0) * (1.0 - (std::f64::consts::PI * k as f64 / n as f64).cos()) * 0.5).collect();
            // Round each socket a strip of its own (along it, from, to, and
            // across it, the socket's middle), the rest swept in runs
            // between them.
            let strips: Vec<(f64, f64, f64)> = sockets.iter().map(|&s| ((s - a).dot(u), (s - a).dot(side))).map(|(x, w)| (x - SOCKET.2, x + SOCKET.2, w)).collect();
            xs.retain(|&x| !strips.iter().any(|s| x >= s.0 && x <= s.1));
            xs.extend(strips.iter().flat_map(|s| [s.0, s.1]));
            xs.sort_by(f64::total_cmp);
            let mut run = vec![xs[0]];
            for pair in xs.windows(2) {
                match strips.iter().find(|s| s.0 == pair[0] && s.1 == pair[1]) {
                    Some(&strip) => {
                        b.sweep(&run.iter().map(|&x| section(x, y - 0.01)).collect::<Vec<_>>());
                        socket(b, (a, u, side), y, strip, &section);
                        run = vec![pair[1]];
                    }
                    None => run.push(pair[1]),
                }
            }
            b.sweep(&run.iter().map(|&x| section(x, y - 0.01)).collect::<Vec<_>>());
            b.raise(&outline, y - 0.01);
        }
        Carry::Trees => {
            b.prism(&outline, y - 0.01, y - DECK.1, true);
            trees(b, a, z, y, y, a + u * RADIUS, z - u * RADIUS, DECK.1, half - 0.6);
        }
    }
}

/// A cylinder's outline (`SIDES` sides) round `c`.
fn ring(c: DVec2, radius: f64) -> Vec<DVec2> {
    polygon(c, radius, SIDES)
}

/// A round outline of so many sides round `c`.
fn polygon(c: DVec2, radius: f64, sides: usize) -> Vec<DVec2> {
    (0..sides).map(|s| c + DVec2::from_angle(s as f64 * std::f64::consts::TAU / sides as f64) * radius).collect()
}

/// A point at `p` (in plan) and height `y`.
fn at(p: DVec2, y: f64) -> DVec3 {
    DVec3::new(p.x, y, p.y)
}

/// A collar round the cylinder at `c`, its top at `y`, in `style`; `turn`
/// (0..1) turns its members a little, so collars differ; `pick` its own
/// random numbers (a meander's).
fn ring_collar(b: &mut Builder, style: &Style, c: DVec2, y: f64, turn: f64, pick: &dyn Fn(i32) -> f64) {
    let path = if matches!(style, Style::Meander) { walk(pick) } else { HashSet::new() };
    let rest = if matches!(style, Style::Meander) { AROUND } else { MEMBER };
    let shade = |m: Member| if path.contains(&m) { PATH } else { rest };
    let style = if matches!(style, Style::Mixed | Style::Meander) { &Style::Lattice } else { style };
    // (Its deck in segments, its inner edge half a metre into the cylinder:
    // overlapping, never just touching it.)
    let thick = if matches!(style, Style::Solid) { COLLAR.2 } else { THIN };
    let (inner, outer) = (ring(c, RADIUS - 0.5), ring(c, RADIUS + COLLAR.1));
    b.tone = PLATE;
    for s in 0..SIDES {
        let t = (s + 1) % SIDES;
        b.prism(&[inner[s], inner[t], outer[t], outer[s]], y, y - thick, true);
    }
    let under = y - thick;
    let dir = DVec2::from_angle;
    let offset = turn * std::f64::consts::TAU / 16.0;
    b.tone = MEMBER;
    match style {
        // (Mixed is made as lattice or branch, above.)
        Style::Solid | Style::Mixed | Style::Meander | Style::Inlay(..) => {}
        Style::Lattice => {
            // Its chord a frame's depth down, posts to it every other side,
            // in each bay a frame within the frame, held at its corners;
            // spokes back into the cylinder at the chord.
            // (A meander's members darker.)
            let low = y - FRAME.0;
            let rim = ring(c, RADIUS + COLLAR.1 - 0.8);
            for t in 0..SIDES / 2 {
                let (p, q) = (rim[2 * t], rim[(2 * t + 2) % SIDES]);
                b.tone = shade(Member::Chord(t));
                b.beam(at(p, low), at(q, low), 0.6, 0.6, 6);
                b.tone = shade(Member::Post(t));
                b.beam(at(p, under), at(p, low), 0.7, 0.7, 6);
                b.tone = rest;
                b.beam(at(c + (p - c).normalize() * (RADIUS - 0.5), low), at(p, low), 0.6, 0.6, 6);
                inner_frame(b, [at(p, under), at(q, under), at(q, low), at(p, low)], |k, stay| shade(if stay { Member::Stay(t, k) } else { Member::Inner(t, k) }));
            }
        }
        Style::Tension => {
            // Hung from nodes on the cylinder over it, each stay a bundle.
            for k in 0..8 {
                let a = offset + k as f64 * std::f64::consts::TAU / 8.0;
                let node = at(c + dir(a) * (RADIUS + 0.8), y + 22.0);
                b.beam(at(c + dir(a) * (RADIUS - 0.5), y + 22.0), node, 1.4, 1.1, 6);
                bundle(b, node, at(c + dir(a) * (RADIUS + COLLAR.1 - 0.6), y - thick * 0.5));
            }
        }
        Style::Branch => {
            // Carried on trees out of the cylinder under it: a trunk to a
            // node, branches from it to the deck's rim.
            for k in 0..8 {
                let a = offset + k as f64 * std::f64::consts::TAU / 8.0;
                let root = at(c + dir(a) * (RADIUS - 0.5), y - 24.0);
                let node = at(c + dir(a) * (RADIUS + 3.5), y - 10.0);
                b.beam(root, node, 1.1, 0.85, 7);
                for da in [-0.2, 0.0, 0.2] {
                    b.beam(node, at(c + dir(a + da) * (RADIUS + COLLAR.1 - 1.0), under + 0.3), 0.6, 0.35, 6);
                }
                // (A knot where they part.)
                b.beam(node - DVec3::Y * 0.8, node + DVec3::Y * 0.8, 1.3, 1.3, 7);
            }
        }
    }
}

/// A span from the cylinder at `a` (its deck at `ya`) to the one at `z` (at
/// `yz`), collar to collar, in `style`.
fn span(b: &mut Builder, style: &Style, a: DVec2, ya: f64, z: DVec2, yz: f64) {
    let style = if matches!(style, Style::Mixed | Style::Meander) { &Style::Branch } else { style };
    let along = (z - a).normalize();
    let side = along.perp();
    // (Into each collar a metre: its ends landing on them.)
    let reach = RADIUS + COLLAR.1 - 1.0;
    let (p, q) = (a + along * reach, z - along * reach);
    let thick = if matches!(style, Style::Solid) { DECK.1 } else { THIN };
    b.tone = PLATE;
    b.deck(p, ya, q, yz, DECK.0, thick);
    b.tone = MEMBER;
    let w = DECK.0 * 0.5 - 0.6;
    // A point along the deck (0 at `p`, 1 at `q`), so far aside, so far
    // under its top.
    let on = |t: f64, aside: f64, down: f64| at(p + (q - p) * t + side * aside, ya + (yz - ya) * t - down);
    match style {
        // (Mixed is made as lattice or branch, above.)
        Style::Solid | Style::Mixed | Style::Meander | Style::Inlay(..) => {}
        Style::Lattice => {
            // Chords a frame's depth down each side, posts at each bay, in
            // each bay a frame within the frame; ties across under it.
            let bays = ((p.distance(q) / FRAME.1).round() as usize).max(2);
            for e in [-w, w] {
                b.beam(on(0.0, e, FRAME.0), on(1.0, e, FRAME.0), 0.6, 0.6, 6);
                for n in 0..=bays {
                    let t = n as f64 / bays as f64;
                    b.beam(on(t, e, thick), on(t, e, FRAME.0), 0.6, 0.6, 6);
                    if n < bays {
                        let u = (n + 1) as f64 / bays as f64;
                        inner_frame(b, [on(t, e, thick), on(u, e, thick), on(u, e, FRAME.0), on(t, e, FRAME.0)], |_, _| MEMBER);
                    }
                }
            }
            for n in 0..=bays {
                let t = n as f64 / bays as f64;
                b.beam(on(t, -w, FRAME.0), on(t, w, FRAME.0), 0.5, 0.5, 6);
            }
        }
        Style::Tension => {
            // From a node on each cylinder over the span's end, a fan of
            // bundled stays to its edges.
            for (end, from, y) in [(a, 1.0, ya), (z, -1.0, yz)] {
                let node = at(end + along * from * (RADIUS + 0.8), y + 30.0);
                b.beam(at(end + along * from * (RADIUS - 0.5), y + 30.0), node, 1.6, 1.2, 6);
                for f in [0.12, 0.24, 0.36] {
                    let t = if from > 0.0 { f } else { 1.0 - f };
                    for e in [-w, w] {
                        bundle(b, node, on(t, e, thick * 0.5));
                    }
                }
            }
        }
        Style::Branch => trees(b, a, z, ya, yz, p, q, thick, w),
    }
}

/// The branch spans' trees under a deck from `p` (its top at `ya`) to `q`
/// (`yz`), `thick` deep, `w` either side of its line, between the cylinders
/// at `a` and `z`: from each cylinder, under the span's end, a trunk to a
/// node under the deck; from it, branches to its underside, the middle one
/// reaching the middle, where the two trees meet.
#[allow(clippy::too_many_arguments)]
fn trees(b: &mut Builder, a: DVec2, z: DVec2, ya: f64, yz: f64, p: DVec2, q: DVec2, thick: f64, w: f64) {
    let along = (z - a).normalize();
    let side = along.perp();
    let on = |t: f64, aside: f64, down: f64| at(p + (q - p) * t + side * aside, ya + (yz - ya) * t - down);
    for (end, from, y) in [(a, 1.0, ya), (z, -1.0, yz)] {
        let t0 = if from > 0.0 { 0.22 } else { 0.78 };
        let root = at(end + along * from * (RADIUS - 0.5), y - 28.0);
        let node = on(t0, 0.0, 10.0);
        b.beam(root, node, 1.3, 1.0, 7);
        b.beam(node - DVec3::Y * 0.9, node + DVec3::Y * 0.9, 1.5, 1.5, 7);
        for (f, e) in [(0.1, -w), (0.1, w), (0.34, -w), (0.34, w)] {
            let t = if from > 0.0 { f } else { 1.0 - f };
            b.beam(node, on(t, e, thick - 0.3), 0.6, 0.35, 6);
        }
        b.beam(node, on(0.5, 0.0, thick + 0.4), 0.8, 0.5, 6);
    }
}

/// A frame within the frame of corners `c` (a bay: two along its top, two
/// along its bottom): a smaller one, inset, held to its corners; `tone`
/// each member's (its corner, whether a stay or the edge from it).
fn inner_frame(b: &mut Builder, c: [DVec3; 4], tone: impl Fn(usize, bool) -> f32) {
    let mid = (c[0] + c[1] + c[2] + c[3]) * 0.25;
    let inner = c.map(|p| mid + (p - mid) * 0.5);
    for k in 0..4 {
        b.tone = tone(k, false);
        b.beam(inner[k], inner[(k + 1) % 4], 0.35, 0.35, 5);
        b.tone = tone(k, true);
        b.beam(c[k], inner[k], 0.3, 0.3, 5);
    }
    b.tone = MEMBER;
}

/// A meander round a lattice collar: a motif of one, two or four bays,
/// repeated round the ring; in each bay a way from its left side to its
/// right, along its chord or through its inner frame, either way round;
/// where one bay's way ends at the other height from where the next one's
/// starts, up or down the post between them. (`pick`: its random numbers.)
fn walk(pick: &dyn Fn(i32) -> f64) -> HashSet<Member> {
    let bays = SIDES / 2;
    let length = [1, 2, 4][(pick(0) * 3.0) as usize];
    // Each bay's way: the corner it comes in at (0 top, 3 foot), the one it
    // leaves by (1 top, 2 foot), and round its inner frame which way
    // (clockwise or not; none: along its chord).
    let motif: Vec<(usize, usize, Option<bool>)> = (0..length)
        .map(|n| {
            let r = |k: i32| pick(1 + n as i32 * 4 + k);
            let (from, to) = (if r(0) < 0.5 { 0 } else { 3 }, if r(1) < 0.5 { 1 } else { 2 });
            let chord = from == 3 && to == 2 && r(2) < 0.4;
            (from, to, if chord { None } else { Some(r(3) < 0.5) })
        })
        .collect();
    let mut path = HashSet::new();
    for t in 0..bays {
        let (from, to, way) = motif[t % length];
        match way {
            None => {
                path.insert(Member::Chord(t));
            }
            Some(clockwise) => {
                path.insert(Member::Stay(t, from));
                path.insert(Member::Stay(t, to));
                let mut k = from;
                while k != to {
                    if clockwise {
                        path.insert(Member::Inner(t, k));
                        k = (k + 1) % 4;
                    } else {
                        k = (k + 3) % 4;
                        path.insert(Member::Inner(t, k));
                    }
                }
            }
        }
        let (next, ..) = motif[(t + 1) % length];
        if (to == 1) != (next == 0) {
            path.insert(Member::Post((t + 1) % bays));
        }
    }
    path
}

/// A stay from `from` to `to`: three cables bundled, side by side.
fn bundle(b: &mut Builder, from: DVec3, to: DVec3) {
    let axis = (to - from).normalize_or_zero();
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    for k in 0..3 {
        let a = k as f64 * std::f64::consts::TAU / 3.0;
        let off = (u * a.cos() + v * a.sin()) * 0.32;
        b.beam(from + off, to + off, 0.22, 0.22, 5);
    }
}

/// The side of the largest square a fretwork's `bars` leave empty in its `w`
/// by `h` opening (in its own units).
fn widest_gap(bars: &[[f32; 4]], w: f32, h: f32) -> f32 {
    const CELL: f32 = 0.05;
    let (nx, ny) = ((w / CELL).ceil() as usize, (h / CELL).ceil() as usize);
    let mut barred = vec![false; nx * ny];
    for b in bars {
        let span = |lo: f32, hi: f32, n: usize| ((lo / CELL).floor().max(0.0) as usize)..((hi / CELL).ceil() as usize).min(n);
        for j in span(b[1], b[3], ny) {
            for i in span(b[0], b[2], nx) {
                barred[j * nx + i] = true;
            }
        }
    }
    // (Each cell's largest empty square with its corner there.)
    let mut side = vec![0u32; nx * ny];
    let mut widest = 0;
    for j in 0..ny {
        for i in 0..nx {
            if barred[j * nx + i] {
                continue;
            }
            let s = if i == 0 || j == 0 { 1 } else { 1 + side[j * nx + i - 1].min(side[(j - 1) * nx + i]).min(side[(j - 1) * nx + i - 1]) };
            side[j * nx + i] = s;
            widest = widest.max(s);
        }
    }
    widest as f32 * CELL
}


