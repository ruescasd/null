//! Forms: buildings grown from plates.
//!
//! Where a style (see `structure.rs`) splits a box, a form works on a
//! polygon: a plate, or a piece of one. It extrudes it into a prism (straight,
//! battered or drawn to a point), insets it for a setback, cuts it into
//! blocks with streets between them, divides it into smaller plates, walls
//! its rim or raises it on pillars, and hands the results to further forms.
//! Plates are convex and every operation keeps them convex, so each piece is
//! a convex prism: one hull for collision.
//!
//! Forms are named and defined in the data file, so they can refer to each
//! other (and to themselves, for setbacks that repeat).

use std::collections::BTreeMap;

use glam::{Vec2, Vec3};
use serde::Deserialize;

use crate::mesh::ColumnMesh;
use crate::noise::hash01;
use crate::structure::{self, Library, Placement, Solid};

/// What to do with a polygon. Heights and distances are metres; a pair is a
/// range picked from at random.
#[derive(Clone, Debug, Deserialize)]
pub enum Form {
    /// Nothing (also available as "nothing").
    Nothing,
    /// A prism whose faces are carved into themselves (see `relief.rs`): a
    /// core, and a skin `depth` thick cut into recesses within recesses.
    /// `then` grows on top, on the whole polygon.
    Relief {
        height: (f32, f32),
        #[serde(default = "relief_depth")]
        depth: f32,
        #[serde(default)]
        tone: f32,
        #[serde(default)]
        then: Option<String>,
    },
    /// A Sierpinski pyramid in the biggest square that fits, `height` tall:
    /// a pyramid made of five half-size pyramids (four on the base, one on
    /// top), each made the same way, `depth` times; with `tetrahedron`, a
    /// tetrahedron made of four. No boxes, no right angles in its outline.
    Sierpinski {
        height: (f32, f32),
        #[serde(default = "sierpinski_depth")]
        depth: u32,
        #[serde(default)]
        tetrahedron: bool,
        #[serde(default)]
        tone: f32,
    },
    /// An ordered tower (see `tower.rs`), transcribed from Manifold Garden:
    /// symmetric, proportioned, with designed-in stairs and fretwork, in
    /// the biggest square that fits, about `height` tall. With `spacing`,
    /// towers `width` across repeat on a lattice that far apart.
    Tower {
        height: (f32, f32),
        #[serde(default)]
        width: f32,
        #[serde(default)]
        spacing: f32,
        #[serde(default)]
        tone: f32,
    },
    /// Round towers of bare wall given scale by a few human-sized things
    /// (see `bastion.rs`): rows of deep arches, a door, a stair spiralling
    /// up the outside, ledges, arched bridges between neighbours. Up to
    /// `count` towers about `height` tall.
    Bastions {
        height: (f32, f32),
        #[serde(default = "one_tower")]
        count: u32,
        #[serde(default)]
        tone: f32,
    },
    /// Wiring in its own right (see `curtain.rs`): a broken ring of walls
    /// round an open shaft, sheaves of long cables crossing the void and
    /// hanging down the walls, the walls carrying the pipe network sunk
    /// into their faces.
    Curtains {
        height: (f32, f32),
        #[serde(default)]
        tone: f32,
    },
    /// A mass made of cylinders (see `cluster.rs`): drums packed and fused
    /// on the polygon, about `height` tall at its middle, a few lying across,
    /// and a tangle of small tubes sagging between them and winding down
    /// their sides.
    Cluster {
        height: (f32, f32),
        #[serde(default)]
        tone: f32,
    },
    /// A network of pipes spreading out across the ground from the polygon's
    /// edges for `reach` metres, no higher than `depth` (see `rack.rs`):
    /// the city's pipes running out into the land.
    Roots {
        reach: (f32, f32),
        #[serde(default = "roots_depth")]
        depth: f32,
        /// Buried conduits instead: few, straight, only their crowns showing.
        #[serde(default)]
        buried: bool,
        #[serde(default)]
        tone: f32,
    },
    /// An open frame lining the polygon's edges, one bay `depth` deep, its
    /// bays packed with pipes, tanks, hoses and machinery (see `rack.rs`).
    /// With `core`, a dark mass fills the polygon inside the rack. `then`
    /// grows on top, on the whole polygon.
    Rack {
        height: (f32, f32),
        #[serde(default = "rack_storey")]
        storey: (f32, f32),
        #[serde(default = "rack_bay")]
        bay: (f32, f32),
        #[serde(default = "rack_depth")]
        depth: (f32, f32),
        #[serde(default = "rack_frame")]
        frame: f32,
        #[serde(default)]
        core: bool,
        /// No frame: a network of pipes on the core's faces, climbing,
        /// switching lanes, splitting, with bare wall between its knots.
        #[serde(default)]
        strange: bool,
        /// With `strange` and `core`: the pipes sunk into the core's faces,
        /// only their crowns showing.
        #[serde(default)]
        sunk: bool,
        /// With `strange`: conduits square in section rather than round.
        #[serde(default)]
        square: bool,
        #[serde(default)]
        tone: f32,
        #[serde(default)]
        then: Option<String>,
    },
    /// A prism of the polygon. `taper` is how much it narrows towards the
    /// top (0 straight, 1 to a point), `lean` how far its top may shift
    /// sideways; `then` grows on its top.
    Extrude {
        height: (f32, f32),
        #[serde(default)]
        taper: (f32, f32),
        #[serde(default)]
        lean: f32,
        /// Added to the building's tone.
        #[serde(default)]
        tone: f32,
        #[serde(default)]
        then: Option<String>,
    },
    /// A prism whose faces are covered with panels: a core, and on each
    /// face a grid of cells `panel` wide and `storey` high, each holding
    /// (with chance `fill`) a slab standing out `depth` from the face, with
    /// a tone of its own. The panels stay within the polygon. `then` grows
    /// on top, on the whole polygon.
    Facade {
        height: (f32, f32),
        #[serde(default = "default_panel")]
        panel: f32,
        #[serde(default = "default_storey")]
        storey: f32,
        #[serde(default = "default_depth")]
        depth: (f32, f32),
        #[serde(default = "default_fill")]
        fill: f32,
        #[serde(default)]
        tone: f32,
        #[serde(default)]
        then: Option<String>,
    },
    /// The polygon shrunk by `by` on every side.
    Inset { by: (f32, f32), then: String },
    /// The polygon moved `by` metres sideways, in a random direction (or
    /// along the polygon's longest edge with `along`): stacked, this makes
    /// cantilevers and overhangs.
    Shift {
        by: (f32, f32),
        #[serde(default)]
        along: bool,
        then: String,
    },
    /// A recessed band: a prism of the polygon shrunk by `by`, `height`
    /// high; `then` grows on top of it on the whole polygon again. Stacked
    /// between bands it makes the grooves that give tall things a scale.
    Neck {
        by: (f32, f32),
        height: (f32, f32),
        #[serde(default)]
        then: Option<String>,
    },
    /// Cut into blocks by straight cuts across the polygon's length, `depth`
    /// times, with `gap` between the pieces.
    Split {
        #[serde(default = "two")]
        depth: u32,
        #[serde(default)]
        gap: f32,
        /// Pieces narrower than this are not cut again.
        #[serde(default = "default_min_width")]
        min_width: f32,
        then: String,
    },
    /// Divided into smaller plates about `size` across, with `gap` between.
    Cells {
        size: f32,
        #[serde(default)]
        gap: f32,
        then: String,
    },
    /// A wall along each edge, `width` thick, broken by gates with the given
    /// chance; `inner` grows inside it. With `wall`, each stretch of wall
    /// grows that form instead of being a plain prism `height` high.
    Rim {
        width: f32,
        #[serde(default)]
        height: (f32, f32),
        #[serde(default = "default_gates")]
        gates: f32,
        #[serde(default)]
        inner: Option<String>,
        #[serde(default)]
        wall: Option<String>,
    },
    /// Square pillars at the corners and every `spacing` along the edges;
    /// `then` grows on the whole polygon at their top.
    Pillars {
        width: f32,
        height: (f32, f32),
        #[serde(default = "default_spacing")]
        spacing: f32,
        #[serde(default)]
        then: Option<String>,
    },
    /// One of these, by weight.
    Choose(Vec<(f32, String)>),
    /// All of these, on the same polygon.
    Stack(Vec<String>),
    /// A structure of a box style, in the biggest box that fits: `height`
    /// metres high, or with `proportion`, that many times the box's
    /// narrower side.
    Structure {
        style: String,
        #[serde(default)]
        height: (f32, f32),
        #[serde(default)]
        proportion: Option<(f32, f32)>,
    },
}

fn sierpinski_depth() -> u32 {
    5
}

fn one_tower() -> u32 {
    1
}

fn relief_depth() -> f32 {
    2.0
}

fn roots_depth() -> f32 {
    2.5
}

fn rack_storey() -> (f32, f32) {
    (3.5, 5.0)
}

fn rack_bay() -> (f32, f32) {
    (3.0, 5.0)
}

fn rack_depth() -> (f32, f32) {
    (2.2, 3.5)
}

fn rack_frame() -> f32 {
    0.45
}

fn two() -> u32 {
    2
}
fn default_min_width() -> f32 {
    8.0
}
fn default_gates() -> f32 {
    0.3
}
fn default_spacing() -> f32 {
    12.0
}
fn default_panel() -> f32 {
    3.0
}
fn default_storey() -> f32 {
    3.5
}
fn default_depth() -> (f32, f32) {
    (0.3, 1.2)
}
fn default_fill() -> f32 {
    0.55
}

/// The names a form refers to.
pub fn references(form: &Form) -> Vec<&str> {
    match form {
        Form::Nothing | Form::Structure { .. } => vec![],
        Form::Rack { then, .. } => then.iter().map(|s| s.as_str()).collect(),
        Form::Roots { .. }
        | Form::Cluster { .. }
        | Form::Curtains { .. }
        | Form::Bastions { .. }
        | Form::Tower { .. }
        | Form::Sierpinski { .. } => vec![],
        Form::Relief { then, .. } => then.iter().map(|s| s.as_str()).collect(),
        Form::Extrude { then, .. } | Form::Pillars { then, .. } | Form::Neck { then, .. } | Form::Facade { then, .. } => {
            then.iter().map(|s| s.as_str()).collect()
        }
        Form::Rim { inner, wall, .. } => inner.iter().chain(wall).map(|s| s.as_str()).collect(),
        Form::Inset { then, .. } | Form::Split { then, .. } | Form::Cells { then, .. } | Form::Shift { then, .. } => {
            vec![then]
        }
        Form::Choose(options) => options.iter().map(|(_, s)| s.as_str()).collect(),
        Form::Stack(forms) => forms.iter().map(|s| s.as_str()).collect(),
    }
}

/// A convex prism: a polygon at `y0`, drawn up to `y1` where it is scaled by
/// `top_scale` about its centroid and shifted by `lean`.
#[derive(Clone, Debug)]
pub struct Prism {
    pub points: Vec<Vec2>,
    pub y0: f32,
    pub y1: f32,
    pub top_scale: f32,
    pub lean: Vec2,
    pub albedo: f32,
}

impl Prism {
    fn top(&self) -> Vec<Vec2> {
        let c = centroid(&self.points);
        self.points.iter().map(|&p| c + (p - c) * self.top_scale + self.lean).collect()
    }

    /// Its corners, for a convex hull collider.
    pub fn hull_points(&self) -> Vec<Vec3> {
        let mut out: Vec<Vec3> = self.points.iter().map(|p| Vec3::new(p.x, self.y0, p.y)).collect();
        if self.top_scale < 0.02 {
            let apex = centroid(&self.points) + self.lean;
            out.push(Vec3::new(apex.x, self.y1, apex.y));
        } else {
            out.extend(self.top().iter().map(|p| Vec3::new(p.x, self.y1, p.y)));
        }
        out
    }
}

/// Adds prisms to a mesh, flat shaded like the structures.
pub fn mesh_into(mesh: &mut ColumnMesh, prisms: &[Prism]) {
    for prism in prisms {
        let n = prism.points.len();
        let c = centroid(&prism.points);
        let mid = Vec3::new(c.x, (prism.y0 + prism.y1) * 0.5, c.y);
        let mut face = |points: &[Vec3], ao: &[f32]| {
            let mut normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
            let center = points.iter().copied().sum::<Vec3>() / points.len() as f32;
            let flip = normal.dot(center - mid) < 0.0;
            if flip {
                normal = -normal;
            }
            let base = mesh.positions.len() as u32;
            for (p, &a) in points.iter().zip(ao) {
                mesh.positions.push(p.to_array());
                mesh.normals.push(normal.to_array());
                mesh.albedo.push(prism.albedo);
                mesh.ao.push(a);
            }
            mesh.face_size(points.len(), crate::mesh::polygon_width(points));
            for k in 1..points.len() as u32 - 1 {
                if flip {
                    mesh.indices.extend_from_slice(&[base, base + k + 1, base + k]);
                } else {
                    mesh.indices.extend_from_slice(&[base, base + k, base + k + 1]);
                }
            }
        };
        let bottom: Vec<Vec3> = prism.points.iter().map(|p| Vec3::new(p.x, prism.y0, p.y)).collect();
        face(&bottom, &vec![0.45; n]);
        let pointed = prism.top_scale < 0.02;
        let top: Vec<Vec3> = if pointed {
            let apex = c + prism.lean;
            vec![Vec3::new(apex.x, prism.y1, apex.y); n]
        } else {
            prism.top().iter().map(|p| Vec3::new(p.x, prism.y1, p.y)).collect()
        };
        if !pointed {
            face(&top, &vec![1.0; n]);
        }
        for i in 0..n {
            let j = (i + 1) % n;
            if (bottom[j] - bottom[i]).length_squared() < 1e-6 {
                continue;
            }
            if pointed {
                face(&[bottom[i], bottom[j], top[i]], &[0.7, 0.7, 1.0]);
            } else {
                face(&[bottom[i], bottom[j], top[j], top[i]], &[0.7, 0.7, 1.0, 1.0]);
            }
        }
    }
}

/// What grows from a polygon: prisms, solids (from box styles and racks)
/// and tubes (racks).
#[derive(Default)]
pub struct Growth {
    pub prisms: Vec<Prism>,
    pub solids: Vec<Solid>,
    pub tubes: Vec<crate::dressing::Tube>,
}

/// Recursion deeper than this stops, whatever the forms say.
const MAX_DEPTH: u32 = 64;
/// Polygons smaller than this (square metres) are dropped.
const MIN_AREA: f32 = 3.0;

pub struct Grower<'a> {
    pub library: &'a Library,
    /// Prisms still allowed.
    pub budget: usize,
    /// Leaves still allowed for box styles.
    pub leaves: usize,
    pub out: Growth,
}

impl Grower<'_> {
    /// Grows the named form on a polygon standing at height `floor`.
    pub fn grow(&mut self, name: &str, poly: &[Vec2], floor: f32, tone: f32, seed: u32, depth: u32) {
        if depth > MAX_DEPTH || self.budget == 0 || poly.len() < 3 || area(poly) < MIN_AREA {
            return;
        }
        let Some(form) = self.library.forms.get(name) else { return };
        let r = |k: i32| hash01(seed as i32, k, depth as i32, 0xf0f0);
        let pick = |range: (f32, f32), k: i32| range.0 + (range.1 - range.0) * r(k);
        let child = |i: u32| seed.wrapping_mul(0x9e37_79b9).wrapping_add(i.wrapping_mul(0x85eb_ca6b)) ^ depth;
        match form {
            Form::Nothing => {}
            Form::Relief { height, depth: d, tone: t, then } => {
                let tone = tone + t;
                let h = pick(*height, 1);
                let (core, solids) = crate::relief::build(poly, floor, h, *d, tone, child(5));
                if solids.len() + 1 > self.budget {
                    return;
                }
                self.budget -= solids.len() + 1;
                self.out.prisms.push(Prism {
                    points: core,
                    y0: floor,
                    y1: floor + h,
                    top_scale: 1.0,
                    lean: Vec2::ZERO,
                    albedo: (tone - 0.02).clamp(0.03, 0.4),
                });
                self.out.solids.extend(solids);
                if let Some(then) = then {
                    self.grow(then, poly, floor + h, tone, child(1), depth + 1);
                }
            }
            Form::Sierpinski { height, depth: levels, tetrahedron, tone: t } => {
                let Some((center, dir, half)) = inscribed_box(poly) else { return };
                let side = Vec2::new(-dir.y, dir.x);
                let s = half.min_element();
                let h = pick(*height, 1);
                let tone = tone + t;
                // (base outline, base height, height) of each piece, split
                // `levels` times.
                let mut pieces: Vec<(Vec<Vec2>, f32, f32)> = vec![if *tetrahedron {
                    (
                        (0..3)
                            .map(|k| {
                                let a = std::f32::consts::TAU * k as f32 / 3.0 + std::f32::consts::FRAC_PI_2;
                                center + (dir * a.cos() + side * a.sin()) * s
                            })
                            .collect(),
                        floor,
                        h,
                    )
                } else {
                    (
                        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                            .iter()
                            .map(|&(u, v)| center + (dir * u + side * v) * s)
                            .collect(),
                        floor,
                        h,
                    )
                }];
                for _ in 0..(*levels).min(8) {
                    let mut next = Vec::with_capacity(pieces.len() * 5);
                    for (base, y, h) in pieces {
                        let c = centroid(&base);
                        // Half-size copies at each corner of the base...
                        for &corner in &base {
                            next.push((base.iter().map(|&p| (p + corner) * 0.5).collect(), y, h * 0.5));
                        }
                        // ...and one on top, over the middle.
                        next.push((base.iter().map(|&p| (p + c) * 0.5).collect(), y + h * 0.5, h * 0.5));
                    }
                    pieces = next;
                }
                if pieces.len() > self.budget {
                    return;
                }
                self.budget -= pieces.len();
                for (k, (points, y0, h)) in pieces.into_iter().enumerate() {
                    let shade = tone + (hash01(k as i32, 3, 0x51e, seed) - 0.5) * 0.03;
                    self.out.prisms.push(Prism {
                        points,
                        y0,
                        y1: y0 + h,
                        top_scale: 0.0,
                        lean: Vec2::ZERO,
                        albedo: shade.clamp(0.03, 0.4),
                    });
                }
            }
            Form::Tower { height, width, spacing, tone: t } => {
                let Some((center, dir, half)) = inscribed_box(poly) else { return };
                let origin = Vec3::new(center.x, floor, center.y);
                let h = pick(*height, 1);
                let solids = if *spacing > 0.0 {
                    crate::tower::lattice(origin, dir, half, *width, h, *spacing, tone + t)
                } else {
                    crate::tower::build(origin, dir, half.min_element() * 2.0, h, tone + t)
                };
                if solids.len() > self.budget {
                    return;
                }
                self.budget -= solids.len();
                self.out.solids.extend(solids);
            }
            Form::Bastions { height, count, tone: t } => {
                let (walls, solids) = crate::bastion::build(poly, floor, pick(*height, 1), *count, tone + t, child(7));
                if walls.len() + solids.len() > self.budget {
                    return;
                }
                self.budget -= walls.len() + solids.len();
                self.out.prisms.extend(walls);
                self.out.solids.extend(solids);
            }
            Form::Curtains { height, tone: t } => {
                let (walls, cables, pipes) = crate::curtain::build(poly, floor, pick(*height, 1), tone + t, child(6));
                let pieces = walls.len() + cables.len() + pipes.len();
                if pieces > self.budget {
                    return;
                }
                self.budget -= pieces;
                self.out.prisms.extend(walls);
                self.out.solids.extend(cables);
                self.out.tubes.extend(pipes);
            }
            Form::Cluster { height, tone: t } => {
                let solids = crate::cluster::build(poly, floor, pick(*height, 1), tone + t, child(4));
                if solids.len() > self.budget {
                    return;
                }
                self.budget -= solids.len();
                self.out.solids.extend(solids);
            }
            Form::Roots { reach, depth: d, buried, tone: t } => {
                let roots = if *buried {
                    crate::rack::conduits(poly, floor, pick(*reach, 1), tone + t, child(3))
                } else {
                    crate::rack::roots(poly, floor, pick(*reach, 1), *d, tone + t, child(3))
                };
                let pieces = roots.solids.len() + roots.tubes.len();
                if pieces > self.budget {
                    return;
                }
                self.budget -= pieces;
                self.out.solids.extend(roots.solids);
                self.out.tubes.extend(roots.tubes);
            }
            Form::Rack { height, storey, bay, depth: d, frame, core, strange, sunk, square, tone: t, then } => {
                let tone = tone + t;
                let layout = crate::rack::Layout {
                    height: pick(*height, 1),
                    storey: pick(*storey, 2),
                    bay: pick(*bay, 3),
                    depth: pick(*d, 4),
                    frame: *frame,
                    strange: *strange,
                    sunk: *sunk,
                    square: *square,
                };
                let rack = crate::rack::build(poly, floor, layout, tone, child(2));
                let pieces = rack.solids.len() + rack.tubes.len();
                if pieces > self.budget {
                    return;
                }
                self.budget -= pieces;
                self.out.solids.extend(rack.solids);
                self.out.tubes.extend(rack.tubes);
                if *core && self.budget > 0 {
                    let inner = inset(poly, layout.depth);
                    if inner.len() >= 3 {
                        self.budget -= 1;
                        self.out.prisms.push(Prism {
                            points: inner,
                            y0: floor,
                            y1: floor + layout.height,
                            top_scale: 1.0,
                            lean: Vec2::ZERO,
                            albedo: (tone - 0.04).clamp(0.03, 0.4),
                        });
                    }
                }
                if let Some(then) = then {
                    self.grow(then, poly, floor + layout.height, tone, child(1), depth + 1);
                }
            }
            Form::Extrude { height, taper, lean, tone: t, then } => {
                let tone = tone + t;
                let a = r(4) * std::f32::consts::TAU;
                let prism = Prism {
                    points: poly.to_vec(),
                    y0: floor,
                    y1: floor + pick(*height, 1),
                    // A negative taper widens it towards the top.
                    top_scale: (1.0 - pick(*taper, 2)).clamp(0.0, 8.0),
                    lean: Vec2::new(a.cos(), a.sin()) * lean * r(3),
                    albedo: (tone + (r(5) - 0.5) * 0.03).clamp(0.03, 0.4),
                };
                let (top, y1) = (prism.top(), prism.y1);
                let pointed = prism.top_scale < 0.02;
                self.budget -= 1;
                self.out.prisms.push(prism);
                if let (Some(then), false) = (then, pointed) {
                    self.grow(then, &top, y1, tone, child(1), depth + 1);
                }
            }
            Form::Facade { height, panel, storey, depth: (d0, d1), fill, tone: t, then } => {
                let tone = tone + t;
                let core = Some(inset(poly, *d1)).filter(|c| c.len() >= 3).unwrap_or_else(|| poly.to_vec());
                let h = pick(*height, 1);
                if self.budget == 0 {
                    return;
                }
                self.budget -= 1;
                self.out.prisms.push(Prism {
                    points: core.clone(),
                    y0: floor,
                    y1: floor + h,
                    top_scale: 1.0,
                    lean: Vec2::ZERO,
                    albedo: (tone + (r(5) - 0.5) * 0.02).clamp(0.03, 0.4),
                });
                let rows = (h / storey.max(0.5)).round().max(1.0) as usize;
                let row_h = h / rows as f32;
                let outward: Vec<Vec2> = inward_normals(&core).into_iter().map(|n| -n).collect();
                let n = core.len();
                for i in 0..n {
                    let (a, b) = (core[i], core[(i + 1) % n]);
                    let length = (b - a).length();
                    if length < 0.5 {
                        continue;
                    }
                    let columns = (length / panel.max(0.5)).floor().max(1.0) as usize;
                    let along = (b - a) / length;
                    let w = length / columns as f32;
                    for c in 0..columns {
                        for row in 0..rows {
                            let k = (i * 7919 + c * 131 + row) as i32;
                            let q = |key: i32| hash01(seed as i32 ^ k, key, depth as i32, 0xfa5e);
                            if q(0) >= *fill || self.budget == 0 {
                                continue;
                            }
                            // A slab in its cell, a little inside its edges.
                            let d = d0 + (d1 - d0) * q(1);
                            let (t0, t1) = (c as f32 * w + w * 0.06, (c + 1) as f32 * w - w * 0.06);
                            let (p0, p1) = (a + along * t0, a + along * t1);
                            let slab = [p0, p1, p1 + outward[i] * d, p0 + outward[i] * d];
                            let slab = clip_to(&slab, poly);
                            if slab.len() < 3 {
                                continue;
                            }
                            self.budget -= 1;
                            self.out.prisms.push(Prism {
                                points: slab,
                                y0: floor + row as f32 * row_h + row_h * 0.06,
                                y1: floor + (row + 1) as f32 * row_h - row_h * 0.06,
                                top_scale: 1.0,
                                lean: Vec2::ZERO,
                                albedo: (tone + (q(2) - 0.5) * 0.08).clamp(0.03, 0.4),
                            });
                        }
                    }
                }
                // What grows on top takes the whole polygon again, so banded
                // towers keep their girth.
                if let Some(then) = then {
                    self.grow(then, poly, floor + h, tone, child(1), depth + 1);
                }
            }
            Form::Shift { by, along, then } => {
                let dir = if *along {
                    let n = poly.len();
                    let i = (0..n)
                        .max_by(|&a, &b| {
                            let len = |i: usize| (poly[(i + 1) % n] - poly[i]).length_squared();
                            len(a).total_cmp(&len(b))
                        })
                        .unwrap_or(0);
                    let e = (poly[(i + 1) % n] - poly[i]).normalize_or_zero();
                    if r(2) < 0.5 { e } else { -e }
                } else {
                    let a = r(2) * std::f32::consts::TAU;
                    Vec2::new(a.cos(), a.sin())
                };
                let offset = dir * pick(*by, 1);
                let moved: Vec<Vec2> = poly.iter().map(|&p| p + offset).collect();
                self.grow(then, &moved, floor, tone, child(1), depth + 1);
            }
            Form::Inset { by, then } => {
                let inner = inset(poly, pick(*by, 1));
                self.grow(then, &inner, floor, tone, child(1), depth + 1);
            }
            Form::Neck { by, height, then } => {
                // Too thin to recess: a band all the same, so what grows
                // on top still stands on something.
                let inner = Some(inset(poly, pick(*by, 1))).filter(|p| p.len() >= 3).unwrap_or_else(|| poly.to_vec());
                let top = floor + pick(*height, 2);
                if self.budget > 0 {
                    self.budget -= 1;
                    self.out.prisms.push(Prism {
                        points: inner,
                        y0: floor,
                        y1: top,
                        top_scale: 1.0,
                        lean: Vec2::ZERO,
                        albedo: (tone - 0.03).clamp(0.03, 0.4),
                    });
                }
                if let Some(then) = then {
                    self.grow(then, poly, top, tone, child(1), depth + 1);
                }
            }
            Form::Split { depth: cuts, gap, min_width, then } => {
                let mut pieces = vec![poly.to_vec()];
                for level in 0..*cuts {
                    let mut next = Vec::new();
                    for (i, piece) in pieces.into_iter().enumerate() {
                        let s = child(level * 1000 + i as u32);
                        match split(&piece, *min_width, s) {
                            Some((a, b)) => next.extend([a, b]),
                            None => next.push(piece),
                        }
                    }
                    pieces = next;
                }
                for (i, piece) in pieces.iter().enumerate() {
                    let piece = inset(piece, gap * 0.5);
                    self.grow(then, &piece, floor, tone, child(10_000 + i as u32), depth + 1);
                }
            }
            Form::Cells { size, gap, then } => {
                for (i, cell) in cells(poly, size.max(1.0), seed).iter().enumerate() {
                    let cell = inset(cell, gap * 0.5);
                    self.grow(then, &cell, floor, tone, child(i as u32), depth + 1);
                }
            }
            Form::Rim { width, height, gates, inner, wall: wall_form } => {
                let h = pick(*height, 1);
                let n = poly.len();
                let inward = inward_normals(poly);
                for i in 0..n {
                    let (a, b) = (poly[i], poly[(i + 1) % n]);
                    let length = (b - a).length();
                    if length < 0.5 {
                        continue;
                    }
                    // A gate: a gap in the middle of the edge.
                    let spans: Vec<(f32, f32)> = if length > width * 6.0 && r(10 + i as i32) < *gates {
                        let g = (4.0 / length).min(0.4) * 0.5;
                        vec![(0.0, 0.5 - g), (0.5 + g, 1.0)]
                    } else {
                        vec![(0.0, 1.0)]
                    };
                    for (t0, t1) in spans {
                        let (p, q) = (a.lerp(b, t0), a.lerp(b, t1));
                        let wall = [p, q, q + inward[i] * *width, p + inward[i] * *width];
                        let wall = clip_to(&wall, poly);
                        if let Some(form) = wall_form {
                            self.grow(form, &wall, floor, tone, child(100 + i as u32 * 2 + (t0 > 0.0) as u32), depth + 1);
                        } else if wall.len() >= 3 && area(&wall) > 0.2 && self.budget > 0 {
                            self.budget -= 1;
                            self.out.prisms.push(Prism {
                                points: wall,
                                y0: floor,
                                y1: floor + h,
                                top_scale: 1.0,
                                lean: Vec2::ZERO,
                                albedo: (tone - 0.01).clamp(0.03, 0.4),
                            });
                        }
                    }
                }
                if let Some(inner) = inner {
                    let inside = inset(poly, *width);
                    self.grow(inner, &inside, floor, tone, child(1), depth + 1);
                }
            }
            Form::Pillars { width, height, spacing, then } => {
                let h = pick(*height, 1);
                let n = poly.len();
                let inward = inward_normals(poly);
                let c = centroid(poly);
                let mut spots: Vec<(Vec2, Vec2)> = Vec::new();
                for i in 0..n {
                    let (a, b) = (poly[i], poly[(i + 1) % n]);
                    let dir = (b - a).normalize_or_zero();
                    let length = (b - a).length();
                    // At the corner, pulled in...
                    spots.push((a + (c - a).normalize_or_zero() * width * 1.2, dir));
                    // ...and along the edge.
                    let count = (length / spacing.max(width * 2.0)).floor() as i32;
                    for k in 1..count {
                        let t = k as f32 / count as f32;
                        spots.push((a.lerp(b, t) + inward[i] * width * 0.8, dir));
                    }
                }
                for (at, dir) in spots {
                    let side = Vec2::new(-dir.y, dir.x);
                    let half = width * 0.5;
                    let square = [
                        at - dir * half - side * half,
                        at + dir * half - side * half,
                        at + dir * half + side * half,
                        at - dir * half + side * half,
                    ];
                    let square = clip_to(&square, poly);
                    if square.len() >= 3 && self.budget > 0 {
                        self.budget -= 1;
                        self.out.prisms.push(Prism {
                            points: square,
                            y0: floor,
                            y1: floor + h,
                            top_scale: 1.0,
                            lean: Vec2::ZERO,
                            albedo: (tone - 0.02).clamp(0.03, 0.4),
                        });
                    }
                }
                if let Some(then) = then {
                    self.grow(then, poly, floor + h, tone, child(1), depth + 1);
                }
            }
            Form::Choose(options) => {
                let total: f32 = options.iter().map(|(w, _)| w.max(0.0)).sum();
                let mut x = r(1) * total;
                for (w, name) in options {
                    x -= w.max(0.0);
                    if x <= 0.0 {
                        self.grow(name, poly, floor, tone, child(1), depth + 1);
                        break;
                    }
                }
            }
            Form::Stack(names) => {
                for (i, name) in names.iter().enumerate() {
                    self.grow(name, poly, floor, tone, child(i as u32), depth + 1);
                }
            }
            Form::Structure { style, height, proportion } => {
                let Some((center, dir, half)) = inscribed_box(poly) else { return };
                let tall = match proportion {
                    Some(p) => pick(*p, 1) * half.min_element() * 2.0,
                    None => pick(*height, 1),
                };
                let size = (half.x * 2.0, tall, half.y * 2.0);
                let placement = Placement {
                    style: style.clone(),
                    at: (0.0, 0.0),
                    size,
                    yaw: (-dir.y).atan2(dir.x).to_degrees(),
                    seed,
                    sink: 0.0,
                };
                let solids = structure::build(self.library, &placement, self.leaves);
                self.leaves = self.leaves.saturating_sub(solids.len());
                let offset = Vec3::new(center.x, floor, center.y);
                self.out.solids.extend(solids.into_iter().map(|s| Solid { center: s.center + offset, ..s }));
            }
        }
    }
}

// Convex polygon helpers. Polygons may wind either way.

pub fn area(poly: &[Vec2]) -> f32 {
    signed_area(poly).abs()
}

fn signed_area(poly: &[Vec2]) -> f32 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        a += p.x * q.y - q.x * p.y;
    }
    a * 0.5
}

pub fn centroid(poly: &[Vec2]) -> Vec2 {
    let a = signed_area(poly);
    if a.abs() < 1e-6 {
        return poly.iter().copied().sum::<Vec2>() / poly.len().max(1) as f32;
    }
    let mut c = Vec2::ZERO;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        c += (p + q) * (p.x * q.y - q.x * p.y);
    }
    c / (6.0 * a)
}

/// Keeps the part of `poly` where `n·p >= c`.
fn clip(poly: &[Vec2], n: Vec2, c: f32) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for k in 0..poly.len() {
        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
        let (sa, sb) = (n.dot(a) - c, n.dot(b) - c);
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    out
}

/// Each edge's unit normal pointing into the polygon.
pub fn inward_normals(poly: &[Vec2]) -> Vec<Vec2> {
    let sign = signed_area(poly).signum();
    (0..poly.len())
        .map(|i| {
            let e = poly[(i + 1) % poly.len()] - poly[i];
            Vec2::new(-e.y, e.x).normalize_or_zero() * sign
        })
        .collect()
}

/// The polygon shrunk by `d` on every side.
pub fn inset(poly: &[Vec2], d: f32) -> Vec<Vec2> {
    if d <= 0.0 {
        return poly.to_vec();
    }
    let mut out = poly.to_vec();
    for (i, n) in inward_normals(poly).into_iter().enumerate() {
        out = clip(&out, n, n.dot(poly[i]) + d);
        if out.len() < 3 {
            return Vec::new();
        }
    }
    out
}

/// `shape` clipped to the inside of the convex polygon `within`.
pub fn clip_to(shape: &[Vec2], within: &[Vec2]) -> Vec<Vec2> {
    let mut out = shape.to_vec();
    for (i, n) in inward_normals(within).into_iter().enumerate() {
        out = clip(&out, n, n.dot(within[i]));
        if out.len() < 3 {
            return Vec::new();
        }
    }
    out
}

/// The polygon's extent along a direction.
fn extent(poly: &[Vec2], dir: Vec2) -> f32 {
    let (lo, hi) = poly.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.dot(dir)), hi.max(p.dot(dir))));
    hi - lo
}

/// Cuts a polygon in two across its length, along one of its own edge
/// directions (or square to one), so cuts follow the plate's geometry.
fn split(poly: &[Vec2], min_width: f32, seed: u32) -> Option<(Vec<Vec2>, Vec<Vec2>)> {
    let r = |k: i32| hash01(seed as i32, k, 7, 0x5b11);
    let mut best: Option<(f32, Vec2)> = None;
    for i in 0..poly.len() {
        let e = (poly[(i + 1) % poly.len()] - poly[i]).normalize_or_zero();
        for (k, n) in [e, Vec2::new(-e.y, e.x)].into_iter().enumerate() {
            let score = extent(poly, n) * (0.75 + 0.5 * r(i as i32 * 2 + k as i32));
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, n));
            }
        }
    }
    let (_, n) = best?;
    let length = extent(poly, n);
    if length < min_width * 2.0 {
        return None;
    }
    let at = n.dot(centroid(poly)) + (r(100) - 0.5) * 0.4 * length;
    let a = clip(poly, n, at);
    let b = clip(poly, -n, -at);
    (a.len() >= 3 && b.len() >= 3).then_some((a, b))
}

/// Voronoi cells of jittered points about `size` apart, inside the polygon.
fn cells(poly: &[Vec2], size: f32, seed: u32) -> Vec<Vec<Vec2>> {
    let (mut min, mut max) = (poly[0], poly[0]);
    for p in poly {
        min = min.min(*p);
        max = max.max(*p);
    }
    let (nx, nz) = (((max.x - min.x) / size).ceil() as i32, ((max.y - min.y) / size).ceil() as i32);
    if nx * nz > 400 {
        return vec![poly.to_vec()];
    }
    let mut points = Vec::new();
    for j in 0..nz.max(1) {
        for i in 0..nx.max(1) {
            let r = |k: i32| hash01(i, j * 4 + k, seed as i32, 0xce11);
            points.push(min + Vec2::new(i as f32 + 0.5 + (r(0) - 0.5) * 0.8, j as f32 + 0.5 + (r(1) - 0.5) * 0.8) * size);
        }
    }
    let mut out = Vec::new();
    for (i, &p) in points.iter().enumerate() {
        let mut cell = poly.to_vec();
        for (j, &q) in points.iter().enumerate() {
            if i == j || p.distance_squared(q) > (size * 3.0).powi(2) {
                continue;
            }
            // The side of the bisector nearer p.
            let n = p - q;
            cell = clip(&cell, n, n.dot((p + q) * 0.5));
            if cell.len() < 3 {
                break;
            }
        }
        if cell.len() >= 3 {
            out.push(cell);
        }
    }
    out
}

/// A large box inside the polygon, aligned with its longest edge: centre,
/// unit direction of its x axis, half extents.
pub(crate) fn inscribed_box(poly: &[Vec2]) -> Option<(Vec2, Vec2, Vec2)> {
    let n = poly.len();
    let longest = (0..n).max_by(|&a, &b| {
        let len = |i: usize| (poly[(i + 1) % n] - poly[i]).length_squared();
        len(a).total_cmp(&len(b))
    })?;
    let dir = (poly[(longest + 1) % n] - poly[longest]).normalize_or_zero();
    let side = Vec2::new(-dir.y, dir.x);
    let c = centroid(poly);
    let mut half = Vec2::new(extent(poly, dir), extent(poly, side)) * 0.5;
    let inside = |p: Vec2| {
        inward_normals(poly).iter().enumerate().all(|(i, n)| n.dot(p - poly[i]) >= -1e-3)
    };
    for _ in 0..30 {
        let corners = [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
        if corners.iter().all(|&(u, v)| inside(c + dir * half.x * u + side * half.y * v)) {
            return (half.min_element() > 1.0).then_some((c, dir, half));
        }
        half *= 0.92;
    }
    None
}

/// Checks that every form's references resolve.
pub fn check(forms: &BTreeMap<String, Form>, styles: &BTreeMap<String, structure::Style>) -> Result<(), String> {
    for (name, form) in forms {
        for r in references(form) {
            if r != "nothing" && !forms.contains_key(r) {
                return Err(format!("form '{name}': unknown form '{r}'"));
            }
        }
        if let Form::Structure { style, .. } = form
            && !styles.contains_key(style)
        {
            return Err(format!("form '{name}': unknown style '{style}'"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexagon(radius: f32) -> Vec<Vec2> {
        (0..6)
            .map(|i| {
                let a = i as f32 / 6.0 * std::f32::consts::TAU + 0.2;
                Vec2::new(a.cos(), a.sin()) * radius
            })
            .collect()
    }

    #[test]
    fn operations_keep_pieces_inside() {
        let poly = hexagon(16.0);
        let inner = inset(&poly, 2.0);
        assert!(area(&inner) < area(&poly) && area(&inner) > 0.0);
        let (a, b) = split(&poly, 2.0, 3).unwrap();
        assert!((area(&a) + area(&b) - area(&poly)).abs() < 0.1);
        let cells = cells(&poly, 6.0, 5);
        assert!(cells.len() > 4);
        let total: f32 = cells.iter().map(|c| area(c)).sum();
        assert!((total - area(&poly)).abs() < 0.5, "{total} vs {}", area(&poly));
        let (c, dir, half) = inscribed_box(&poly).unwrap();
        assert!(c.length() < 1.0 && half.min_element() > 5.0 && (dir.length() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn forms_grow_prisms() {
        let library = Library::parse(
            r#"(
                styles: { "block": (divisions: (2, 2, 2), keep: All, depth: 1, leaves: [(on: Any, module: "box")]) },
                forms: {
                    "tower": Extrude(height: (10, 20), then: "setback"),
                    "setback": Choose([(1, "nothing"), (3, "step")]),
                    "step": Inset(by: (1, 2), then: "tower"),
                    "blocks": Split(depth: 3, gap: 2, min_width: 3, then: "tower"),
                    "court": Rim(width: 1.5, height: (3, 5), gates: 1, inner: "shards"),
                    "shards": Cells(size: 4, gap: 0.5, then: "shard"),
                    "shard": Extrude(height: (2, 8), taper: (1, 1), lean: 1),
                    "table": Pillars(width: 1, height: (5, 6), then: "top"),
                    "top": Extrude(height: (1, 1)),
                    "old": Structure(style: "block", height: (10, 10)),
                    "all": Stack(["blocks", "court", "table", "old"]),
                },
            )"#,
        )
        .unwrap();
        let mut grower = Grower { library: &library, budget: 10_000, leaves: 1000, out: Growth::default() };
        grower.grow("all", &hexagon(20.0), 0.0, 0.12, 1, 0);
        let prisms = &grower.out.prisms;
        assert!(prisms.len() > 40, "{} prisms", prisms.len());
        assert!(!grower.out.solids.is_empty());
        // Everything stays over the plate.
        let plate = hexagon(20.0);
        for prism in prisms {
            for &p in &prism.points {
                let inside = inward_normals(&plate).iter().enumerate().all(|(i, n)| n.dot(p - plate[i]) >= -1e-3);
                assert!(inside, "{p} outside the plate");
            }
            assert!(prism.y1 > prism.y0);
        }
        let mut mesh = ColumnMesh::default();
        mesh_into(&mut mesh, prisms);
        assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()));
    }
}
