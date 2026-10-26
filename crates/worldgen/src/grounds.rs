//! The ground lab: kinds of ground made by different techniques, each a
//! patch round a point, to judge side by side (the user, 2026-10-21: new
//! districts found by trying techniques, not only by imagining districts).
//! Each technique is cheap and rough on purpose; those that survive are
//! developed into districts.
//!
//! A patch is meshes (gathered by tile) and the height of its top, on a
//! grid, for what stands on it (the player's start, the camera).
//!
//! The techniques (`build`'s names):
//! - `tilted`: the tilted-native ground (see `tilted.rs`).
//! - `canyons`: a plateau cut by a network of straight-sided canyons, deep
//!   and wide, branching into narrower and shallower ones (enclosure).
//! - `bridged`: the canyons with levels of their own (enclosure and
//!   levels): natural bridges left across them, grooves round their walls
//!   (strata), and their mesas' tops inverted, cut into pits with ribs
//!   between them (against the blocky look).
//! - `nested`: the structures' fractal rule run at the scale of the land:
//!   plateaus within plateaus, channels between them; `sunken`, the same
//!   cut downward (pits within pits, ribs between); `contrast`, most blocks
//!   left blank, a few split all the way down (knots).
//! - `stacked`: ground in layers, decks on pillars over a floor, voids
//!   between them; `floors`, the decks continuous over many pillars, a
//!   megastructure's floors.
//!
//! - `void_machine`, `void_web`: cylinders on a grid rising out of a void,
//!   joined by spans collar to collar: along the grid's axes (the machine),
//!   or also across its diagonals and up or down a level (the web);
//!   `void_lattice`, `void_tension`, `void_branch`, the machine's collars and
//!   spans made as members, not slabs, and `void_mixed`, lattice collars
//!   with branching spans, and `void_meander` the same, a darker path
//!   meandering along each collar's members; and `void_inlay`, no collars,
//!   a groove round a core where a span lands, each span an arch from above
//!   swept into the cylinders and from the side, springing from them, and
//!   `void_inlay_trees`, the spans on the branch spans' trees (see
//!   `grounds/void.rs`). And the scale lab: `void_inlay_screen` with its
//!   cues cut into the cylinders' drums, `void_scale_courses` (see
//!   `void::Cues`); and seams where the drums were assembled,
//!   `void_seams_staves`, `void_seams_rings` (`void::Seams`); and a lamp at
//!   each end of each walkway, `void_lamp_monolith`, `void_lamp_cradle`,
//!   `void_lamp_pendant`, `void_lamp_echo` (`void::Lamp`), its foot in a
//!   socket, `void_lamp_echo_socket` (`void::Foot`).
//!
//! The axes found so far (the user, 2026-10-21): levels (several heights
//! under a point) as a district's identity, not only a landmark's; and
//! enclosure, full (canyons) or partial (nested).
//!
//! Removed: round 1's `spots` (reaction-diffusion spots: a tank farm,
//! "nothing going for it") and `penrose` (a Penrose tiling's rhombi, heights
//! nested by supertile: rubble; its cracks suggested the canyons); round 2's
//! `turing` (a reaction-diffusion labyrinth raised into walls, traced by
//! marching squares: from above its curves and sameness failed, between its
//! walls it was promising; the canyons took its place); and round 5's
//! tops for the bridged mesas, tilted (each small block tipped its mesa's
//! way) and spires (a sparse stone forest), "not convinced": the inverted
//! tops took their place. And the void's T-square collars (2026-10-23):
//! drums of flat faces under the decks, a T-square fractal on each, as
//! plates in relief (greebles at three levels, an emblem at two), as frames
//! of bars (fretwork, the best of them, but each square's bars its own), as
//! holes in a plate, and as its merged outline in one band (a Greek key):
//! none better than the plain lattice, the user; the meander, a path of
//! tone along the lattice, took their place.

mod canyons;
mod cuts;
mod floors;
mod nested;
mod stacked;
mod void;

use std::collections::HashMap;

use glam::{DVec2, DVec3};

use crate::tilted::{SheetMesh, Tilted, clip, face, triangulate};

/// The names `build` knows.
pub const NAMES: [&str; 27] = ["tilted", "canyons", "bridged", "nested", "sunken", "contrast", "stacked", "floors", "void_machine", "void_web", "void_lattice", "void_tension", "void_branch", "void_mixed", "void_meander", "void_inlay", "void_inlay_balcony", "void_inlay_screen", "void_inlay_trees", "void_scale_courses", "void_seams_staves", "void_seams_rings", "void_lamp_monolith", "void_lamp_cradle", "void_lamp_pendant", "void_lamp_echo", "void_lamp_echo_socket"];

/// Whether a kind stands over the void (its floor far below, lost in the
/// haze: the game draws the haze nearer for it).
pub fn over_void(name: &str) -> bool {
    name.starts_with("void")
}

/// The void kinds' datum levels: the lowest (m), how far apart, how many;
/// and how high a groove cut at a level is (m).
pub const VOID_LEVELS: (f64, f64, usize) = void::LEVEL;
pub const VOID_GROOVE: f64 = void::NOTCH.1;
pub const VOID_PULSE: f64 = void::PULSE;

/// How wide a patch is (m), and its tiles (m).
pub const SPAN: f64 = 2048.0;
const TILE: f64 = 256.0;
/// The top grid's cells (m).
const CELL: f64 = 4.0;

/// A patch of ground: its meshes, by tile, and its top.
pub struct Patch {
    pub pieces: Vec<SheetMesh>,
    /// What glows from within (the void's cores): each vertex's tone its
    /// pulse's phase (0..1).
    pub glows: Vec<SheetMesh>,
    /// What is glossy black, as the ichor is (the void's streetlights: the
    /// world's black things against the machine's metal).
    pub gloss: Vec<SheetMesh>,
    /// Where lamps shine from (the void's: see `void::Lamp`).
    pub lights: Vec<[f32; 3]>,
    top: Top,
}

impl Patch {
    /// The height of its top at a point (0 off it, or where nothing is).
    pub fn height(&self, x: f64, z: f64) -> f64 {
        self.top.at(DVec2::new(x, z))
    }
}

/// A patch of the named ground round `centre`; an error naming those there
/// are if there is no such.
pub fn build(name: &str, centre: (f64, f64), seed: u32) -> Result<Patch, String> {
    let centre = DVec2::new(centre.0, centre.1);
    let mut b = Builder::new(centre);
    match name {
        "tilted" => {
            // (Raised so its lowest sheets stand on the lab's flat ground.)
            let lift = 160.0;
            let tilted = Tilted::new(crate::WORLD_SIZE, seed);
            let pieces = tilted.patch((centre.x, centre.y), SPAN, lift, -1.0);
            b.top.fill(|p| tilted.height(p) + lift);
            return Ok(Patch { pieces, glows: Vec::new(), gloss: Vec::new(), lights: Vec::new(), top: b.top });
        }
        "canyons" => canyons::build(&mut b, seed, &canyons::PLAIN),
        "bridged" => {
            canyons::build(&mut b, seed, &canyons::BRIDGED);
            canyons::bridges(&mut b, seed);
        }
        "nested" => nested::build(&mut b, seed, nested::RAISED),
        "sunken" => nested::sunken(&mut b, seed),
        "contrast" => nested::build(&mut b, seed, nested::CONTRAST),
        "stacked" => stacked::build(&mut b, seed),
        "floors" => floors::build(&mut b, seed),
        "void_machine" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Solid),
        "void_web" => void::build(&mut b, seed, void::Recipe::Web, void::Style::Solid),
        "void_lattice" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Lattice),
        "void_tension" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Tension),
        "void_branch" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Branch),
        "void_mixed" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Mixed),
        "void_meander" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Meander),
        "void_inlay" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Inlay(void::Carry::Arch, void::Infill::Open, void::Cues::default())),
        "void_inlay_balcony" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Inlay(void::Carry::Arch, void::Infill::Balcony, void::Cues::default())),
        "void_inlay_screen" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Inlay(void::Carry::Arch, void::Infill::Screen, void::Cues::default())),
        "void_inlay_trees" => void::build(&mut b, seed, void::Recipe::Machine, void::Style::Inlay(void::Carry::Trees, void::Infill::Open, void::Cues::default())),
        "void_scale_courses" => {
            let cues = void::Cues { courses: true, ..void::Cues::default() };
            void::build(&mut b, seed, void::Recipe::Machine, void::Style::Inlay(void::Carry::Arch, void::Infill::Screen, cues));
        }
        "void_seams_staves" | "void_seams_rings" => {
            let seams = if name.ends_with("staves") { void::Seams::Staves } else { void::Seams::Rings };
            let cues = void::Cues { seams: Some(seams), ..void::Cues::default() };
            void::build(&mut b, seed, void::Recipe::Machine, void::Style::Inlay(void::Carry::Arch, void::Infill::Screen, cues));
        }
        "void_lamp_monolith" | "void_lamp_cradle" | "void_lamp_pendant" | "void_lamp_echo" | "void_lamp_echo_socket" => {
            let lamp = match name {
                "void_lamp_monolith" => void::Lamp::Monolith,
                "void_lamp_cradle" => void::Lamp::Cradle,
                "void_lamp_pendant" => void::Lamp::Pendant,
                "void_lamp_echo_socket" => void::Lamp::Echo(void::Foot::Socket),
                _ => void::Lamp::Echo(void::Foot::Plain),
            };
            let cues = void::Cues { lamps: Some(lamp), ..void::Cues::default() };
            void::build(&mut b, seed, void::Recipe::Machine, void::Style::Inlay(void::Carry::Arch, void::Infill::Screen, cues));
        }
        _ => return Err(format!("no ground called {name:?}: there are {}", NAMES.join(", "))),
    }
    Ok(b.finish())
}

/// The highest top over a grid of cells round the patch's middle.
struct Top {
    origin: DVec2,
    n: usize,
    h: Vec<f64>,
}

impl Top {
    fn new(centre: DVec2) -> Self {
        let n = (SPAN / CELL).ceil() as usize;
        Self { origin: centre - DVec2::splat(SPAN * 0.5), n, h: vec![0.0; n * n] }
    }

    fn cell(&self, p: DVec2) -> Option<usize> {
        let q = (p - self.origin) / CELL;
        let (i, j) = (q.x.floor(), q.y.floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.n && (j as usize) < self.n).then(|| j as usize * self.n + i as usize)
    }

    fn at(&self, p: DVec2) -> f64 {
        self.cell(p).map_or(0.0, |k| self.h[k])
    }

    /// Each cell's middle's height, from `f`.
    fn fill(&mut self, f: impl Fn(DVec2) -> f64) {
        for j in 0..self.n {
            for i in 0..self.n {
                let p = self.origin + (DVec2::new(i as f64, j as f64) + 0.5) * CELL;
                self.h[j * self.n + i] = f(p);
            }
        }
    }

    /// Raised to `y` over a polygon (where its cells' middles fall in it).
    fn raise(&mut self, outline: &[DVec2], y: f64) {
        let (mut lo, mut hi) = (outline[0], outline[0]);
        for p in outline {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        let a = ((lo - self.origin) / CELL).floor().max(DVec2::ZERO);
        let b = ((hi - self.origin) / CELL).ceil().min(DVec2::splat(self.n as f64));
        for j in a.y as usize..b.y as usize {
            for i in a.x as usize..b.x as usize {
                let p = self.origin + (DVec2::new(i as f64, j as f64) + 0.5) * CELL;
                if inside(outline, p) {
                    let k = j * self.n + i;
                    self.h[k] = self.h[k].max(y);
                }
            }
        }
    }
}

/// Whether a point is inside a polygon (even-odd).
fn inside(poly: &[DVec2], p: DVec2) -> bool {
    let mut odd = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            odd = !odd;
        }
    }
    odd
}

/// Gathers a patch's solids into meshes by tile, and its top; each solid
/// in the tone set when it is added (its albedo: as the plates, each its
/// own, so forms read apart).
struct Builder {
    centre: DVec2,
    tiles: HashMap<(i32, i32), SheetMesh>,
    /// What glows (see `Patch::glows`), and whether what is built now does.
    glows: HashMap<(i32, i32), SheetMesh>,
    glowing: bool,
    /// What is glossy black (see `Patch::gloss`), and whether what is built
    /// now is (but what glows, glows).
    gloss: HashMap<(i32, i32), SheetMesh>,
    glossy: bool,
    /// Where lamps shine from.
    lights: Vec<DVec3>,
    top: Top,
    tone: f32,
}

impl Builder {
    fn new(centre: DVec2) -> Self {
        Self { centre, tiles: HashMap::new(), glows: HashMap::new(), glowing: false, gloss: HashMap::new(), glossy: false, lights: Vec::new(), top: Top::new(centre), tone: 0.18 }
    }

    /// A tone round `base`, varied by `spread` with `k`'s hash.
    fn shade(&mut self, seed: u32, k: (i32, i32), base: f32, spread: f32) {
        self.tone = base + spread * (rand(seed, k.0, k.1, 99) as f32 * 2.0 - 1.0);
    }

    /// Whether a point is on the patch.
    fn on(&self, p: DVec2) -> bool {
        let d = (p - self.centre).abs();
        d.x <= SPAN * 0.5 && d.y <= SPAN * 0.5
    }

    /// What `build` builds glowing (see `Patch::glows`), its tone the glow's.
    fn glow(&mut self, build: impl FnOnce(&mut Self)) {
        self.glowing = true;
        build(self);
        self.glowing = false;
    }

    /// What `build` builds glossy black (see `Patch::gloss`).
    fn gloss(&mut self, build: impl FnOnce(&mut Self)) {
        self.glossy = true;
        build(self);
        self.glossy = false;
    }

    /// A glowing bar from `a` to `z` (as a deck: its top at `top`, `width`
    /// wide, `thick` deep), each point's tone `tone` of how far along it.
    fn channel(&mut self, a: DVec2, z: DVec2, top: f64, width: f64, thick: f64, tone: impl Fn(f64) -> f32) {
        self.glowing = true;
        let start = self.mesh((a + z) * 0.5).positions.len();
        self.deck(a, top, z, top, width, thick);
        let along = (z - a).normalize_or_zero();
        let m = self.mesh((a + z) * 0.5);
        for k in start..m.positions.len() {
            let p = m.positions[k];
            m.tones[k] = tone((DVec2::new(p[0] as f64, p[2] as f64) - a).dot(along));
        }
        self.glowing = false;
    }

    fn mesh(&mut self, at: DVec2) -> &mut SheetMesh {
        let key = ((at.x / TILE).floor() as i32, (at.y / TILE).floor() as i32);
        let tiles = if self.glowing {
            &mut self.glows
        } else if self.glossy {
            &mut self.gloss
        } else {
            &mut self.tiles
        };
        tiles.entry(key).or_insert_with(|| SheetMesh {
            middle: (DVec2::new(key.0 as f64, key.1 as f64) + 0.5) * TILE,
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            tones: Vec::new(),
        })
    }

    /// A flat top over a polygon at `y`.
    fn top(&mut self, outline: &[DVec2], y: f64) {
        if outline.len() < 3 {
            return;
        }
        let at = |p: DVec2| DVec3::new(p.x, y, p.y);
        let tone = self.tone;
        let m = self.mesh(middle(outline));
        for t in triangulate(outline) {
            face(m, [at(outline[t[0]]), at(outline[t[1]]), at(outline[t[2]])], DVec3::Y);
        }
        m.tones.resize(m.positions.len(), tone);
        self.top.raise(outline, y);
    }

    /// An underside over a polygon at `y`, facing down.
    fn underside(&mut self, outline: &[DVec2], y: f64) {
        if outline.len() < 3 {
            return;
        }
        let at = |p: DVec2| DVec3::new(p.x, y, p.y);
        let tone = self.tone;
        let m = self.mesh(middle(outline));
        for t in triangulate(outline) {
            face(m, [at(outline[t[0]]), at(outline[t[1]]), at(outline[t[2]])], -DVec3::Y);
        }
        m.tones.resize(m.positions.len(), tone);
    }

    /// A wall along `a` to `b`, from `low` up to `high`, facing `out` (a
    /// side, in the ground's plane).
    fn wall(&mut self, a: DVec2, b: DVec2, low: f64, high: f64, out: DVec2) {
        if high - low < 1e-6 || a.distance_squared(b) < 1e-10 {
            return;
        }
        let tone = self.tone;
        let m = self.mesh((a + b) * 0.5);
        let hint = DVec3::new(out.x, 0.0, out.y);
        let (a0, b0, a1, b1) = (DVec3::new(a.x, low, a.y), DVec3::new(b.x, low, b.y), DVec3::new(a.x, high, a.y), DVec3::new(b.x, high, b.y));
        face(m, [a0, b0, b1], hint);
        face(m, [a0, b1, a1], hint);
        m.tones.resize(m.positions.len(), tone);
    }

    /// A flat quad (its corners in order round it), facing `out`.
    fn quad(&mut self, q: [DVec3; 4], out: DVec3) {
        let tone = self.tone;
        let m = self.mesh(DVec2::new(q[0].x, q[0].z));
        face(m, [q[0], q[1], q[2]], out);
        face(m, [q[0], q[2], q[3]], out);
        m.tones.resize(m.positions.len(), tone);
    }

    /// Whatever `build` makes in the mesh at `at`, shaded smooth: each of its
    /// points' normals `normal` of where it is (a round thing's, faceted,
    /// shaded as the curve it stands for).
    fn smooth(&mut self, at: DVec2, normal: impl Fn(DVec3) -> DVec3, build: impl FnOnce(&mut Self)) {
        let from = self.mesh(at).positions.len();
        build(self);
        let m = self.mesh(at);
        for k in from..m.positions.len() {
            let p = m.positions[k];
            m.normals[k] = normal(DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).normalize().as_vec3().to_array();
        }
    }

    /// A prism as `prism` makes it, its outline a cylinder's round `c`: its
    /// sides shaded smooth.
    fn cylinder(&mut self, c: DVec2, outline: &[DVec2], top: f64, bottom: f64, floats: bool) {
        if outline.len() < 3 || top <= bottom {
            return;
        }
        self.top(outline, top);
        let out = |p: DVec3| DVec3::new(p.x - c.x, 0.0, p.z - c.y);
        let n = outline.len();
        for i in 0..n {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            self.smooth((a + b) * 0.5, out, |s| s.wall(a, b, bottom, top, (a + b) * 0.5 - c));
        }
        if floats {
            self.underside(outline, bottom);
        }
    }

    /// A beam from `p` to `q`: a tube of `sides` sides, `r0` m round at `p`
    /// tapering to `r1` at `q`, its ends closed. (Members of any slant:
    /// struts, stays, a lattice's bars.)
    fn beam(&mut self, p: DVec3, q: DVec3, r0: f64, r1: f64, sides: usize) {
        let axis = (q - p).normalize_or_zero();
        if axis == DVec3::ZERO {
            return;
        }
        let u = axis.any_orthonormal_vector();
        let v = axis.cross(u);
        let round = |c: DVec3, r: f64| -> Vec<DVec3> { (0..sides).map(|k| { let a = k as f64 * std::f64::consts::TAU / sides as f64; c + (u * a.cos() + v * a.sin()) * r }).collect() };
        let (a, b) = (round(p, r0), round(q, r1));
        let tone = self.tone;
        let m = self.mesh(DVec2::new((p.x + q.x) * 0.5, (p.z + q.z) * 0.5));
        for k in 0..sides {
            let j = (k + 1) % sides;
            let out = ((a[k] + a[j]) * 0.5 - p).normalize_or_zero();
            face(m, [a[k], a[j], b[j]], out);
            face(m, [a[k], b[j], b[k]], out);
            if k > 0 && j > 0 {
                face(m, [a[0], a[k], a[j]], -axis);
                face(m, [b[0], b[k], b[j]], axis);
            }
        }
        m.tones.resize(m.positions.len(), tone);
    }

    /// A solid swept through `sections` (each its corners round: top left,
    /// top right, foot right, foot left), its ends closed.
    fn sweep(&mut self, sections: &[[DVec3; 4]]) {
        let tone = self.tone;
        let first = sections[0][0];
        let m = self.mesh(DVec2::new(first.x, first.z));
        let centre = |s: &[DVec3; 4]| (s[0] + s[1] + s[2] + s[3]) * 0.25;
        for pair in sections.windows(2) {
            let (s, t) = (pair[0], pair[1]);
            let inside = (centre(&s) + centre(&t)) * 0.5;
            for k in 0..4 {
                let j = (k + 1) % 4;
                let quad = [s[k], s[j], t[j], t[k]];
                let hint = (quad[0] + quad[1] + quad[2] + quad[3]) * 0.25 - inside;
                face(m, [quad[0], quad[1], quad[2]], hint);
                face(m, [quad[0], quad[2], quad[3]], hint);
            }
        }
        let n = sections.len();
        for (s, next) in [(sections[0], sections[1]), (sections[n - 1], sections[n - 2])] {
            let hint = centre(&s) - centre(&next);
            face(m, [s[0], s[1], s[2]], hint);
            face(m, [s[0], s[2], s[3]], hint);
        }
        m.tones.resize(m.positions.len(), tone);
    }

    /// The ground's top raised to `y` over a polygon (where something is
    /// built whose top the patch's own meshes don't give).
    fn raise(&mut self, outline: &[DVec2], y: f64) {
        self.top.raise(outline, y);
    }

    /// A deck from `a` (its top at `ya`) to `b` (at `yb`): `width` m wide,
    /// `thick` m deep, level or sloping (a ramp), its ends square to it.
    fn deck(&mut self, a: DVec2, ya: f64, b: DVec2, yb: f64, width: f64, thick: f64) {
        let side = (b - a).perp().normalize_or_zero() * (width * 0.5);
        let at = |p: DVec2, y: f64| DVec3::new(p.x, y, p.y);
        let top = [at(a - side, ya), at(b - side, yb), at(b + side, yb), at(a + side, ya)];
        let low = top.map(|p| p - DVec3::Y * thick);
        let up = DVec3::Y;
        let along = DVec3::new(b.x - a.x, 0.0, b.y - a.y).normalize_or_zero();
        let out = DVec3::new(side.x, 0.0, side.y).normalize_or_zero();
        let tone = self.tone;
        let m = self.mesh((a + b) * 0.5);
        for (quad, hint) in [(top, up), ([low[0], low[1], low[2], low[3]], -up), ([top[0], top[1], low[1], low[0]], -out), ([top[3], top[2], low[2], low[3]], out), ([top[0], top[3], low[3], low[0]], -along), ([top[1], top[2], low[2], low[1]], along)] {
            face(m, [quad[0], quad[1], quad[2]], hint);
            face(m, [quad[0], quad[2], quad[3]], hint);
        }
        m.tones.resize(m.positions.len(), tone);
        self.top.raise(&[a - side, b - side, b + side, a + side], ya.max(yb));
    }

    /// A prism: a polygon's top at `top`, its sides down to `bottom`, and
    /// its underside if it floats.
    fn prism(&mut self, outline: &[DVec2], top: f64, bottom: f64, floats: bool) {
        if outline.len() < 3 || top <= bottom {
            return;
        }
        let ccw = crate::tilted::area(outline) > 0.0;
        self.top(outline, top);
        let n = outline.len();
        for i in 0..n {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            let out = (b - a).perp().normalize_or_zero() * if ccw { -1.0 } else { 1.0 };
            self.wall(a, b, bottom, top, out);
        }
        if floats {
            self.underside(outline, bottom);
        }
    }

    fn finish(self) -> Patch {
        let lights = self.lights.iter().map(|p| p.as_vec3().to_array()).collect();
        Patch { pieces: self.tiles.into_values().collect(), glows: self.glows.into_values().collect(), gloss: self.gloss.into_values().collect(), lights, top: self.top }
    }
}

/// The Voronoi cell of site (`i`, `j`) on a grid of `spacing` m from
/// `origin`, each site strayed up to `jitter` of a cell (hashed with `key`):
/// a square cut by its neighbours' bisectors. With its site.
fn voronoi(seed: u32, key: i32, origin: DVec2, spacing: f64, jitter: f64, i: i32, j: i32) -> (DVec2, Vec<DVec2>) {
    let site = |i: i32, j: i32| {
        let r = |k: i32| rand(seed, i, j, key * 16 + k);
        origin + (DVec2::new(i as f64, j as f64) + 0.5 + DVec2::new(r(1) - 0.5, r(2) - 0.5) * jitter) * spacing
    };
    let s = site(i, j);
    let mut cell: Vec<DVec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|&(x, z)| s + DVec2::new(x, z) * spacing * 1.5).collect();
    for dj in -2..=2 {
        for di in -2..=2 {
            if (di, dj) == (0, 0) || cell.len() < 3 {
                continue;
            }
            let o = site(i + di, j + dj);
            cell = clip(&cell, (s + o) * 0.5, o - s);
        }
    }
    (s, cell)
}

/// A convex polygon (counter-clockwise) shrunk by `d` m on every side;
/// nothing if that leaves nothing.
fn inset(p: &[DVec2], d: f64) -> Vec<DVec2> {
    let mut out = p.to_vec();
    for i in 0..p.len() {
        if out.len() < 3 {
            return Vec::new();
        }
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let inward = (b - a).perp().normalize_or_zero();
        out = clip(&out, a + inward * d, -inward);
    }
    if out.len() < 3 || crate::tilted::area(&out).abs() < 1.0 { Vec::new() } else { out }
}

/// A convex polygon cut to another (convex, counter-clockwise).
fn within(p: &[DVec2], by: &[DVec2]) -> Vec<DVec2> {
    let mut out = p.to_vec();
    for i in 0..by.len() {
        if out.len() < 3 {
            return Vec::new();
        }
        let (a, b) = (by[i], by[(i + 1) % by.len()]);
        out = clip(&out, a, -(b - a).perp());
    }
    out
}

/// A polygon's middle (its points' mean).
fn middle(p: &[DVec2]) -> DVec2 {
    p.iter().copied().sum::<DVec2>() / p.len() as f64
}

/// Our own random numbers: a hash of integers to [0, 1).
fn rand(seed: u32, a: i32, b: i32, k: i32) -> f64 {
    crate::noise::hash01(a, b, k, seed) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounds_build() {
        // Every kind builds, has ground, and stands on the patch.
        let centre = DVec2::splat(8192.0);
        for name in NAMES {
            let patch = build(name, (centre.x, centre.y), 1).unwrap();
            let triangles: usize = patch.pieces.iter().map(|m| m.indices.len() / 3).sum();
            let tops = patch.top.h.iter().filter(|&&h| h > 0.5).count();
            println!("{name}: {} tiles, {triangles} triangles, top over {:.0}% of it", patch.pieces.len(), tops as f64 / patch.top.h.len() as f64 * 100.0);
            assert!(triangles > 1000, "{name}: {triangles} triangles");
            assert!(patch.height(centre.x, centre.y) >= 0.0);
        }
        assert!(build("nothing", (centre.x, centre.y), 1).is_err());
    }
}

