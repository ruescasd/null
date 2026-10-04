//! Racks: open frames whose bays are packed with pipes, tanks, hoses and
//! machinery, the way BLAME!'s shafts are. The pipes are not bolted onto a
//! surface: they are what the frame holds.
//!
//! A rack lines the edges of a convex polygon, one bay deep: columns at the
//! bay boundaries on its outer and inner lines, beams along both lines and
//! across at every storey. Each column of bays is filled with one kind of
//! thing, so runs carry on from storey to storey: vertical pipes with
//! collars where they pass a storey, tanks lying on shelves, bundles of thin
//! hoses, boxes of machinery, or nothing (a dark gap into the depth) but a
//! cross brace. Bays vary in width; a big cylinder rises in each corner;
//! now and then pipes leave the frame at some storey and climb outside it.
//! Each edge stands to its own height, the corner cylinders above them all.
//!
//! A strange rack has no frame and no lanes: it is a network of pipes
//! clinging to the wall it lines (see [`network`]). The same network can
//! spread across the ground from a footprint's edges, like roots ([`roots`]),
//! or run buried under it, only their crowns showing ([`conduits`]).

use glam::{Mat3, Quat, Vec2, Vec3};

use crate::dressing::Tube;
use crate::forms::inward_normals;
use crate::noise::hash01;
use crate::structure::Solid;

/// How a rack is laid out (metres).
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub height: f32,
    pub storey: f32,
    pub bay: f32,
    pub depth: f32,
    /// Columns and beams are this thick.
    pub frame: f32,
    pub strange: bool,
}

/// What a rack is made of.
#[derive(Default)]
pub struct Rack {
    pub solids: Vec<Solid>,
    pub tubes: Vec<Tube>,
}

#[derive(Clone, Copy, PartialEq)]
enum Fill {
    Pipes,
    Tanks,
    Hoses,
    Machine,
    Empty,
}

/// Builds a rack along the edges of `poly` (convex), from `floor` up.
pub fn build(poly: &[Vec2], floor: f32, layout: Layout, tone: f32, seed: u32) -> Rack {
    let mut rack = Rack::default();
    let Layout { height, storey, bay, depth, frame: c, strange } = layout;
    if poly.len() < 3 || height < 1.0 || depth < c * 3.0 {
        return rack;
    }
    let storeys = (height / storey.max(1.0)).round().max(1.0) as usize;
    let s = height / storeys as f32;
    let inward = inward_normals(poly);
    let frame_tone = (tone - 0.03).max(0.03);
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let length = (b - a).length();
        let e = (b - a) / length.max(1e-3);
        let inn = inward[i];
        if strange {
            // The wall is the core's face, `depth` in from the edge; the
            // network fills the space between.
            let wall = Surface::wall(a + inn * depth + e * depth * 0.5, e, -inn, length - depth, floor, height);
            network(&mut rack, &wall, depth, tone, seed ^ (i as u32).wrapping_mul(0x9e37_79b9));
            continue;
        }
        // Leave the corners to the neighbouring edges' end columns.
        let usable = length - 2.0 * depth;
        if usable < depth.min(2.0) {
            continue;
        }
        // Each edge stands to its own height, so the rack steps around.
        let storeys = ((storeys as f32 * (0.55 + 0.45 * hash01(i as i32, 7, 0x7ae, seed))).round() as usize).max(1);
        let height = storeys as f32 * s;
        let start = a + e * depth;
        let bays = (usable / bay.max(1.0)).round().max(1.0) as usize;
        let w = usable / bays as f32;
        let r0 = |k: i32, j: i32| hash01(i as i32 * 131 + k, j, 0x7ad, seed);
        // Bay boundaries, each moved up to a third of a bay.
        let bounds: Vec<f32> = (0..=bays)
            .map(|k| if k == 0 || k == bays { k as f32 * w } else { (k as f32 + (r0(k as i32, 0) - 0.5) * 0.66) * w })
            .collect();
        let yaw = Quat::from_rotation_y((-e.y).atan2(e.x));
        // A box along the edge: centre at `along` metres from the start,
        // `across` metres in from the outer line, `y` up; half sizes along
        // the edge, up and across.
        let at = |along: f32, across: f32, y: f32| {
            let p = start + e * along + inn * across;
            Vec3::new(p.x, y, p.y)
        };
        let solid = |center: Vec3, half: Vec3, albedo: f32| Solid { wedge: false, round: false, center, rotation: yaw, half, albedo };
        let r = |k: i32, j: i32| hash01(i as i32 * 131 + k, j, 0x7ac, seed);

        // The frame: columns, beams along both lines and across.
        for &x in &bounds {
            for across in [c * 0.5, depth - c * 0.5] {
                rack.solids.push(solid(
                    at(x, across, floor + height * 0.5),
                    Vec3::new(c * 0.5, height * 0.5, c * 0.5),
                    frame_tone,
                ));
            }
        }
        for j in 0..=storeys {
            let y = floor + j as f32 * s - if j == storeys { c * 0.4 } else { 0.0 };
            for across in [c * 0.5, depth - c * 0.5] {
                rack.solids.push(solid(
                    at(usable * 0.5, across, y),
                    Vec3::new(usable * 0.5, c * 0.4, c * 0.5),
                    frame_tone,
                ));
            }
            for &x in &bounds {
                rack.solids.push(solid(
                    at(x, depth * 0.5, y),
                    Vec3::new(c * 0.45, c * 0.35, depth * 0.5),
                    frame_tone,
                ));
            }
        }

        // A big cylinder in the corner this edge starts from, collared at
        // every other storey.
        {
            let radius = depth * (0.35 + 0.15 * r0(-1, 1));
            let p = a + (e + inn) * depth * 0.5;
            let base = Vec3::new(p.x, floor, p.y);
            // Corners rise to the full height and a little over, like stacks.
            let top = layout.height + s * (0.3 + r0(-1, 2));
            rack.tubes.push(Tube { from: base, to: base + Vec3::Y * top, radius, albedo: (tone + 0.02).min(0.4), glow: 0.0 });
            for j in (2..storeys).step_by(2) {
                let y = floor + j as f32 * s;
                rack.tubes.push(Tube {
                    from: Vec3::new(p.x, y - 0.3, p.y),
                    to: Vec3::new(p.x, y + 0.3, p.y),
                    radius: radius * 1.12,
                    albedo: frame_tone,
                    glow: 0.0,
                });
            }
        }

        // The bays.
        let inner = depth - 2.0 * c;
        for k in 0..bays {
            let x0 = bounds[k] + c;
            let x1 = bounds[k + 1] - c;
            let span = x1 - x0;
            if span < 0.6 {
                continue;
            }
            let pick = r(k as i32, 1);
            let fill = match pick {
                p if p < 0.33 => Fill::Pipes,
                p if p < 0.55 => Fill::Tanks,
                p if p < 0.75 => Fill::Hoses,
                p if p < 0.87 => Fill::Machine,
                _ => Fill::Empty,
            };
            let lit = (tone + 0.05 + (r(k as i32, 2) - 0.5) * 0.04).clamp(0.05, 0.4);
            let shelf = |rack: &mut Rack, y: f32| {
                rack.solids.push(solid(
                    at((x0 + x1) * 0.5, depth * 0.5, y + 0.06),
                    Vec3::new(span * 0.5, 0.06, inner * 0.5),
                    frame_tone,
                ));
            };
            match fill {
                Fill::Pipes => {
                    // Packed across the bay in one or two rows, each pipe
                    // running the full height, collared at every storey.
                    let rows: &[f32] = if inner > 2.2 { &[0.33, 0.67] } else { &[0.5] };
                    let max_r = (inner / rows.len() as f32 * 0.42).min(0.65);
                    let escapes = rows.len() > 1 && r(k as i32, 60) < 0.3;
                    let escape_at = 1 + (r(k as i32, 61) * (storeys.saturating_sub(2)) as f32) as usize;
                    for (row, &f) in rows.iter().enumerate() {
                        let mut x = x0;
                        let mut m = 0;
                        while m < 24 {
                            let min_r = (max_r * 0.5).min(0.15);
                            let radius = min_r + (max_r - min_r).max(0.0) * r(k as i32 * 64 + m, 10 + row as i32);
                            if x + radius * 2.0 > x1 {
                                break;
                            }
                            let cx = x + radius;
                            let shade = (lit + (r(k as i32 * 64 + m, 20) - 0.5) * 0.05).clamp(0.05, 0.4);
                            let base = at(cx, c + inner * f, floor);
                            // The outer row of some bays leaves the frame at a
                            // storey: out through the face, then up outside it.
                            let escape = row == 0 && escapes && storeys > 2;
                            if escape {
                                let y = floor + escape_at as f32 * s + 0.6 + radius;
                                let out = at(cx, -(0.6 + radius * 2.0), y);
                                let turn = at(cx, c + inner * f, y);
                                rack.tubes.push(Tube { from: base, to: turn, radius, albedo: shade, glow: 0.0 });
                                rack.tubes.push(Tube { from: turn, to: out, radius, albedo: shade, glow: 0.0 });
                                let top = Vec3::new(out.x, floor + height, out.z);
                                rack.tubes.push(Tube { from: out - Vec3::Y * radius, to: top, radius, albedo: shade, glow: 0.0 });
                                rack.tubes.push(Tube {
                                    from: out - Vec3::Y * (radius + 0.25),
                                    to: out + Vec3::Y * (radius + 0.25),
                                    radius: radius * 1.25,
                                    albedo: frame_tone,
                                    glow: 0.0,
                                });
                                x += radius * 2.0 + 0.08;
                                m += 1;
                                continue;
                            }
                            rack.tubes.push(Tube { from: base, to: base + Vec3::Y * height, radius, albedo: shade, glow: 0.0 });
                            for j in 1..storeys {
                                let y = floor + j as f32 * s;
                                let p = at(cx, c + inner * f, y);
                                rack.tubes.push(Tube {
                                    from: p - Vec3::Y * 0.2,
                                    to: p + Vec3::Y * 0.2,
                                    radius: radius * 1.3,
                                    albedo: frame_tone,
                                    glow: 0.0,
                                });
                            }
                            x += radius * 2.0 + 0.08;
                            m += 1;
                        }
                    }
                }
                Fill::Tanks => {
                    // Lying along the bay on a shelf at each storey, banded
                    // near their ends; now and then a storey left empty.
                    let radius = (s * 0.42).min(inner * 0.45);
                    for j in 0..storeys {
                        let y = floor + j as f32 * s;
                        shelf(&mut rack, y);
                        if r(k as i32 * 64 + j as i32, 30) < 0.2 || radius < 0.3 {
                            continue;
                        }
                        let cy = y + 0.12 + radius;
                        let (ta, tb) = (at(x0 + 0.15, depth * 0.5, cy), at(x1 - 0.15, depth * 0.5, cy));
                        rack.tubes.push(Tube { from: ta, to: tb, radius, albedo: lit + 0.03, glow: 0.0 });
                        let dir = (tb - ta).normalize_or_zero();
                        for end in [ta + dir * 0.5, tb - dir * 0.5] {
                            rack.tubes.push(Tube {
                                from: end - dir * 0.12,
                                to: end + dir * 0.12,
                                radius: radius * 1.06,
                                albedo: frame_tone,
                                glow: 0.0,
                            });
                        }
                    }
                }
                Fill::Hoses => {
                    // Many thin runs, loosely packed.
                    let count = ((span * inner) / 0.35).round().clamp(4.0, 40.0) as i32;
                    for m in 0..count {
                        let radius = 0.06 + 0.1 * r(k as i32 * 64 + m, 40);
                        let x = x0 + radius + (span - 2.0 * radius) * r(k as i32 * 64 + m, 41);
                        let z = c + radius + (inner - 2.0 * radius) * r(k as i32 * 64 + m, 42);
                        let base = at(x, z, floor);
                        let shade = (lit - 0.02 + 0.05 * r(k as i32 * 64 + m, 43)).clamp(0.05, 0.4);
                        rack.tubes.push(Tube { from: base, to: base + Vec3::Y * height, radius, albedo: shade, glow: 0.0 });
                    }
                }
                Fill::Machine => {
                    // A box on a shelf at each storey, a smaller one on it.
                    for j in 0..storeys {
                        let y = floor + j as f32 * s;
                        shelf(&mut rack, y);
                        let q = |m: i32| r(k as i32 * 64 + j as i32 * 4 + m, 50);
                        if q(0) < 0.25 {
                            continue;
                        }
                        let hh = s * (0.25 + 0.25 * q(1));
                        let hx = span * (0.3 + 0.18 * q(2));
                        let hz = inner * (0.3 + 0.15 * q(3));
                        let cx = x0 + span * 0.5 + (span * 0.5 - hx) * (q(4) * 2.0 - 1.0);
                        rack.solids.push(solid(at(cx, depth * 0.5, y + 0.12 + hh), Vec3::new(hx, hh, hz), lit));
                        rack.solids.push(solid(
                            at(cx, depth * 0.5, y + 0.12 + hh * 2.0 + hh * 0.2),
                            Vec3::new(hx * 0.5, hh * 0.2, hz * 0.6),
                            frame_tone,
                        ));
                    }
                }
                Fill::Empty => {
                    for j in 0..storeys {
                        let y = floor + j as f32 * s;
                        let (dx, dy) = (span, s);
                        let length = (dx * dx + dy * dy).sqrt();
                        let tilt = dy.atan2(dx);
                        let both = r(k as i32 * 64 + j as i32, 70) < 0.6;
                        for sign in if both { &[1.0, -1.0][..] } else { &[1.0][..] } {
                            let rotation = yaw * Quat::from_rotation_z(tilt * sign);
                            rack.solids.push(Solid {
                                wedge: false,
                                round: false,
                                center: at((x0 + x1) * 0.5, c * 0.5, y + s * 0.5),
                                rotation,
                                half: Vec3::new(length * 0.5, c * 0.25, c * 0.25),
                                albedo: frame_tone,
                            });
                        }
                    }
                }
            }
        }
    }
    rack
}

/// A surface for a network to grow on: a wall it climbs, or the ground it
/// spreads across. Points on it are `u` metres across (along the wall, or
/// sideways on the ground), `y` metres in the direction it grows (up the
/// wall, or out from where it starts) and `z` metres off it.
pub struct Surface {
    origin: Vec3,
    across: Vec3,
    grow: Vec3,
    off: Vec3,
    width: f32,
    reach: f32,
    ground: bool,
}

impl Surface {
    /// A vertical wall whose foot runs from `start` along `along` for
    /// `length` metres at `floor`, `height` high, facing `out`.
    pub fn wall(start: Vec2, along: Vec2, out: Vec2, length: f32, floor: f32, height: f32) -> Self {
        Surface {
            origin: Vec3::new(start.x, floor, start.y),
            across: Vec3::new(along.x, 0.0, along.y),
            grow: Vec3::Y,
            off: Vec3::new(out.x, 0.0, out.y),
            width: length,
            reach: height,
            ground: false,
        }
    }

    /// Level ground at `floor`, spreading `reach` metres out (`out`) from a
    /// line that runs from `start` along `along` for `length` metres.
    pub fn ground(start: Vec2, along: Vec2, out: Vec2, length: f32, floor: f32, reach: f32) -> Self {
        Surface {
            origin: Vec3::new(start.x, floor, start.y),
            across: Vec3::new(along.x, 0.0, along.y),
            grow: Vec3::new(out.x, 0.0, out.y),
            off: Vec3::Y,
            width: length,
            reach,
            ground: true,
        }
    }

    fn at(&self, u: f32, y: f32, z: f32) -> Vec3 {
        self.origin + self.across * u + self.grow * y + self.off * z
    }

    /// The rotation that turns a box's x, y, z to across, grow, off.
    fn rotation(&self) -> Quat {
        Quat::from_mat3(&Mat3::from_cols(self.across, self.grow, self.across.cross(self.grow)))
    }
}

/// How dense the network is `u` metres across its surface: smooth, with
/// bare stretches between knots.
fn density(u: f32, seed: u32) -> f32 {
    let cell = 7.0;
    let k = (u / cell).floor();
    let t = u / cell - k;
    let t = t * t * (3.0 - 2.0 * t);
    let h = |k: f32| hash01(k as i32, 0, 0xde5, seed);
    let v = h(k) * (1.0 - t) + h(k + 1.0) * t;
    ((v - 0.3) / 0.45).clamp(0.0, 1.0)
}

/// Pipe kinds: plain, dark, pale, and banded (dark rings along it).
const KINDS: u32 = 4;
const BANDED: u32 = 3;

fn kind_of(h: f32) -> u32 {
    match h {
        h if h < 0.45 => 0,
        h if h < 0.65 => 1,
        h if h < 0.85 => 2,
        _ => BANDED,
    }
}

/// A route of the network as it grows: where its end is, how thick its
/// pipes are, how many run side by side in its bundle and of what kind,
/// how many times it has split, and its own number.
#[derive(Clone, Copy)]
struct Route {
    u: f32,
    y: f32,
    z: f32,
    radius: f32,
    strands: u32,
    kind: u32,
    generation: u32,
    path: u32,
}

/// A network of pipes on a surface, no further than `depth` off it:
/// bundles rooted along its start where it is dense, growing on (up a
/// wall, out across the ground), jogging sideways at uneven angles from
/// lane to lane (now and then square across), and splitting, the bundle
/// shared out between its branches until single pipes split into thinner
/// ones; now and then a route dives into the surface. Routes keep their own
/// distance off it, so they cross over and under each other; brackets hold
/// them (on the ground, supports). Pipes come in kinds: plain, dark, pale,
/// banded; a wide bundle mixes them. On the ground some routes lie half
/// sunk, and they thin out and dive in as they go. Between the knots, bare
/// surface.
pub fn network(rack: &mut Rack, surface: &Surface, depth: f32, tone: f32, seed: u32) {
    let budget = rack.tubes.len() + 5000;
    let collar_tone = (tone - 0.03).max(0.03);
    let rotation = surface.rotation();
    let top = surface.reach - 0.3;
    let fits = |radius: f32, strands: u32| radius * 2.2 * strands as f32;
    // How far off the surface a route of this thickness runs: on the ground,
    // often half sunk.
    let depth_for = |radius: f32, h: f32| {
        if surface.ground && h < 0.3 {
            radius * 0.4
        } else {
            let h = if surface.ground { (h - 0.3) / 0.7 } else { h };
            (radius + 0.15 + (depth - 2.0 * radius - 0.15).max(0.0) * h).max(radius + 0.1)
        }
    };
    let shade_of = |kind: u32, h: f32| {
        let base = tone + 0.04 + (h - 0.5) * 0.05;
        let shade = match kind {
            1 => base - 0.06,
            2 => base + 0.1,
            _ => base,
        };
        shade.clamp(0.04, 0.45)
    };

    // Roots along the start: in the knots, heavy bundles.
    let mut stack: Vec<Route> = Vec::new();
    let mut u = 0.5;
    let mut k = 0;
    while u < surface.width - 0.5 {
        let h = |j: i32| hash01(k, j, 0x0e7, seed);
        // The ground near a structure is never bare, and its roots are
        // heavier.
        let d = if surface.ground { 0.35 + 0.65 * density(u, seed) } else { density(u, seed) };
        let heavy = if surface.ground { 1.6 } else { 1.0 };
        let roots = if surface.ground { 0.7 } else { 0.5 };
        if h(0) < d * roots {
            let radius = ((0.15 + 0.5 * h(1) * h(1) * d) * heavy).min(depth * 0.3);
            let strands = 1 + (h(4) * h(4) * 6.0 * d * heavy) as u32;
            let kind = kind_of(h(5));
            stack.push(Route { u, y: 0.0, z: depth_for(radius, h(2)), radius, strands, kind, generation: 0, path: k as u32 + 1 });
            u += fits(radius, strands) + 0.3 + 2.0 * h(3);
        } else {
            u += 0.8;
        }
        k += 1;
    }

    // A segment of a route: each strand of the bundle, side by side across
    // the segment's direction on the surface; every third strand of a wide
    // bundle of another kind; banded pipes ringed.
    let segment = |rack: &mut Rack, from: (f32, f32, f32), to: (f32, f32, f32), route: &Route, h: f32| {
        let (du, dy) = (to.0 - from.0, to.1 - from.1);
        let length = (du * du + dy * dy).sqrt().max(1e-3);
        let (pu, py) = (-dy / length, du / length);
        let radius = route.radius;
        let spacing = radius * 2.2;
        for s in 0..route.strands {
            let o = (s as f32 - (route.strands as f32 - 1.0) * 0.5) * spacing;
            let kind = if s % 3 == 2 { (route.kind + 2) % KINDS } else { route.kind };
            // Strands of a wide bundle step a little off the surface too.
            let dz = if s % 2 == 1 { radius * 0.6 } else { 0.0 };
            let a = surface.at(from.0 + pu * o, from.1 + py * o, from.2 + dz);
            let b = surface.at(to.0 + pu * o, to.1 + py * o, to.2 + dz);
            let dir = (b - a).normalize_or_zero();
            // A little overlap hides the joints at bends.
            let d = dir * radius * 0.5;
            rack.tubes.push(Tube { from: a - d, to: b + d, radius, albedo: shade_of(kind, h), glow: 0.0 });
            if kind == BANDED {
                let span = (b - a).length();
                let rings = (span / 1.1).floor() as i32;
                for k in 1..=rings {
                    let p = a + dir * (k as f32 * 1.1);
                    rack.tubes.push(Tube { from: p - dir * 0.08, to: p + dir * 0.08, radius: radius * 1.18, albedo: collar_tone, glow: 0.0 });
                }
            }
        }
    };

    while let Some(route) = stack.pop() {
        if rack.tubes.len() > budget {
            return;
        }
        let Route { u, y, z, radius, strands, kind, generation, path } = route;
        let r = |j: i32| hash01(path as i32, j, generation as i32, seed);
        if y >= top {
            // On the ground, the end of the reach: into it.
            if surface.ground {
                segment(rack, (u, y, z), (u, y + 1.0, -radius * 2.0), &route, r(0));
            }
            continue;
        }
        let width = fits(radius, strands);
        // On a wall routes stay on it; on the ground they may fan out past
        // the ends of the line they started from, the further out the more.
        let spill = if surface.ground { y * 0.6 } else { 0.0 };
        let (lo, hi) = (width * 0.5 + 0.2 - spill, surface.width - width * 0.5 - 0.2 + spill);
        let clamp_u = |u: f32| u.clamp(lo, hi.max(lo));
        let action = r(1);
        let sunk = z < radius;
        // A heavy bracket holding the bundle (on the ground, a support).
        let brackets = if surface.ground { 0.3 } else { 0.1 };
        if !sunk && r(2) < brackets {
            rack.solids.push(Solid {
                wedge: false,
                round: false,
                center: surface.at(u, y + radius, z * 0.5),
                rotation,
                half: Vec3::new(width * 0.5 + 0.15, radius * 1.1 + 0.08, z * 0.5),
                albedo: collar_tone,
            });
        }
        // Into the surface: on the ground, more often the further out.
        let dive = if surface.ground { 0.02 + 0.1 * (y / surface.reach).powi(2) } else { 0.05 };
        if generation > 0 && y > surface.reach * 0.15 && action < dive {
            segment(rack, (u, y, z), (u, y + radius * 2.0, -radius * 2.0), &route, r(0));
        } else if action < 0.3 && (strands > 1 || (radius > 0.18 && generation < 6)) {
            // A split: the bundle shared out between two or three branches
            // leaning away across the surface, each at its own distance off
            // it; a single pipe splits into thinner ones.
            rack.tubes.push(Tube {
                from: surface.at(u, y - radius, z),
                to: surface.at(u, y + radius, z),
                radius: width * 0.5 + radius * 0.35,
                albedo: collar_tone,
                glow: 0.0,
            });
            let children = if strands > 1 { 2 } else { 2 + (r(3) * 2.0) as u32 };
            let mut left = strands;
            for c in 0..children {
                let id = path.wrapping_mul(4).wrapping_add(c);
                let rc = |j: i32| hash01(id as i32, j, generation as i32 + 1, seed);
                let (child_r, child_n) = if strands > 1 {
                    let n = if c + 1 == children { left } else { (strands / 2).max(1) };
                    left -= n.min(left);
                    (radius, n)
                } else {
                    (radius * (0.5 + 0.2 * rc(0)), 1)
                };
                if child_n == 0 {
                    continue;
                }
                let child_kind = if rc(4) < 0.2 { kind_of(rc(5)) } else { kind };
                let side = if c % 2 == 0 { 1.0 } else { -1.0 };
                let fan = if surface.ground { 2.0 } else { 1.0 };
                let du = side * (1.5 + 5.0 * rc(1)) * (0.5 + child_r * 2.0) * fan;
                let nu = clamp_u(u + du);
                let nz = depth_for(child_r, rc(2));
                let ny = (y + (nu - u).abs() * (0.3 + 1.4 * rc(3)) + 0.3).min(top);
                let child = Route {
                    u: nu,
                    y: ny,
                    z: nz,
                    radius: child_r,
                    strands: child_n,
                    kind: child_kind,
                    generation: generation + 1,
                    path: id,
                };
                segment(rack, (u, y, z), (nu, ny, nz), &child, rc(0));
                stack.push(child);
            }
        } else if action < 0.55 {
            // A jog sideways into another lane, at an uneven angle, now and
            // then square across; maybe nearer or further off the surface.
            let side = if r(4) < 0.5 { -1.0 } else { 1.0 };
            let du = side * (1.0 + 5.0 * r(5));
            let nu = clamp_u(u + du);
            let low = if surface.ground && sunk { z } else { radius + 0.1 };
            let nz = if sunk { z } else { (z + (r(6) - 0.5) * depth * 0.6).clamp(low, (depth - radius).max(low)) };
            let slope = if r(7) < 0.2 { 0.0 } else { 0.25 + 1.75 * r(9) };
            let ny = (y + (nu - u).abs() * slope).min(top);
            segment(rack, (u, y, z), (nu, ny, nz), &route, r(0));
            // A square run must still go on afterwards.
            let ny = if slope == 0.0 {
                let rise = (1.0 + 2.0 * r(10)).min(top - ny).max(0.0);
                segment(rack, (nu, ny, nz), (nu, ny + rise, nz), &route, r(0));
                ny + rise.max(0.3)
            } else {
                ny.max(y + 0.3)
            };
            stack.push(Route { u: nu, y: ny, z: nz, path: path.wrapping_mul(2).wrapping_add(1), ..route });
        } else {
            // Straight on.
            let rise = (2.0 + 7.0 * r(8)).min(top - y);
            segment(rack, (u, y, z), (u, y + rise, z), &route, r(0));
            stack.push(Route { y: y + rise, path: path.wrapping_mul(2), ..route });
        }
    }
}

/// A network spreading across the ground from each edge of `poly`
/// (convex), outwards for `reach` metres, no higher than `depth` off it:
/// the city's pipes running out into the land, thinning and diving in.
pub fn roots(poly: &[Vec2], floor: f32, reach: f32, depth: f32, tone: f32, seed: u32) -> Rack {
    let mut rack = Rack::default();
    let inward = inward_normals(poly);
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let length = (b - a).length();
        if length < 2.0 {
            continue;
        }
        let ground = Surface::ground(a, (b - a) / length, -inward[i], length, floor, reach);
        network(&mut rack, &ground, depth, tone, seed ^ (i as u32).wrapping_mul(0x85eb_ca6b));
    }
    rack
}

/// Buried conduits spreading across the ground from each edge of `poly`
/// (convex), out to `reach` metres: few and far between, sunk until only
/// their crowns show (low ridges, well under a step high), running straight
/// for long stretches, turning now and then at an oblique angle, branching
/// rarely, and fading further out; a flush hatch plate wherever one turns,
/// branches or ends.
pub fn conduits(poly: &[Vec2], floor: f32, reach: f32, tone: f32, seed: u32) -> Rack {
    let mut rack = Rack::default();
    let inward = inward_normals(poly);
    let collar_tone = (tone - 0.03).max(0.03);
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let length = (b - a).length();
        if length < 4.0 {
            continue;
        }
        let ground = Surface::ground(a, (b - a) / length, -inward[i], length, floor, reach);
        let rotation = ground.rotation();
        let seed = seed ^ (i as u32).wrapping_mul(0x85eb_ca6b);
        // A crown showing 0.35 of the radius above the ground.
        let sink = |radius: f32| -radius * 0.65;
        let hatch = |rack: &mut Rack, u: f32, y: f32, size: f32| {
            rack.solids.push(Solid {
                wedge: false,
                round: false,
                center: ground.at(u, y, 0.0),
                rotation,
                half: Vec3::new(size, size, 0.08),
                albedo: collar_tone,
            });
        };
        // (u, y, heading, radius, strands, kind, path)
        let mut stack = Vec::new();
        let mut u = 2.0 + 6.0 * hash01(i as i32, 0, 0xc0d, seed);
        let mut k = 0;
        while u < length - 2.0 {
            let h = |j: i32| hash01(k, j, 0xc0e, seed);
            let radius = 0.35 + 0.55 * h(1);
            let strands = 1 + (h(2) * h(2) * 3.0) as u32;
            let heading = (h(3) - 0.5) * 0.5;
            stack.push((u, 0.0f32, heading, radius, strands, kind_of(h(4)), k as u32 + 1));
            u += 10.0 + 14.0 * h(0);
            k += 1;
        }
        while let Some((u, y, heading, radius, strands, kind, path)) = stack.pop() {
            if rack.tubes.len() > 3000 {
                break;
            }
            let r = |j: i32| hash01(path as i32, j, 0xc0f, seed);
            let size = radius * strands as f32 * 1.2 + 0.4;
            let lit = hash01(path as i32 >> 2, 7, 0xc10, seed) < 0.45;
            let run = (10.0 + 20.0 * r(0)).min(reach - y);
            if run < 1.0 {
                hatch(&mut rack, u, y, size);
                continue;
            }
            let (du, dy) = (heading.sin() * run, heading.cos() * run);
            let (nu, ny) = (u + du, y + dy);
            // The strands side by side, crowns just showing.
            let side = Vec2::new(dy, -du).normalize_or_zero();
            for s in 0..strands {
                let o = (s as f32 - (strands as f32 - 1.0) * 0.5) * radius * 2.3;
                let kind = if s % 3 == 2 { (kind + 2) % KINDS } else { kind };
                let lift = match kind {
                    1 => -0.06,
                    2 => 0.1,
                    _ => 0.0,
                };
                let shade = (tone + 0.04 + (r(1) - 0.5) * 0.04 + lift).clamp(0.04, 0.45);
                let from = ground.at(u + side.x * o, y + side.y * o, sink(radius));
                let to = ground.at(nu + side.x * o, ny + side.y * o, sink(radius));
                let dir = (to - from).normalize_or_zero();
                rack.tubes.push(Tube { from: from - dir * radius, to: to + dir * radius, radius, albedo: shade, glow: 0.0 });
                // Some conduits carry a light filament along their crowns.
                if lit && s == 0 {
                    let crown = Vec3::Y * (radius + 0.01);
                    rack.tubes.push(Tube { from: from + crown, to: to + crown, radius: 0.06, albedo: 0.05, glow: 0.5 });
                }
                if kind == BANDED {
                    let rings = ((to - from).length() / 1.5) as i32;
                    for k in 1..=rings {
                        let p = from + dir * (k as f32 * 1.5);
                        rack.tubes.push(Tube { from: p - dir * 0.1, to: p + dir * 0.1, radius: radius * 1.15, albedo: collar_tone, glow: 0.0 });
                    }
                }
            }
            // At the node: end (more likely further out), branch, turn, or go
            // straight on.
            let action = r(2);
            let far = ny / reach;
            if action < 0.08 + 0.3 * far * far {
                hatch(&mut rack, nu, ny, size);
                continue;
            }
            let turn = |a: f32| (heading + a).clamp(-1.0, 1.0);
            if action < 0.2 + 0.3 * far * far && radius > 0.3 {
                hatch(&mut rack, nu, ny, size);
                let side = if r(3) < 0.5 { -1.0 } else { 1.0 };
                stack.push((nu, ny, turn(side * (0.5 + 0.5 * r(4))), radius * 0.7, 1, kind, path.wrapping_mul(4).wrapping_add(1)));
                stack.push((nu, ny, heading, radius, strands, kind, path.wrapping_mul(4).wrapping_add(2)));
            } else if action < 0.55 {
                hatch(&mut rack, nu, ny, size);
                let side = if r(5) < 0.5 { -1.0 } else { 1.0 };
                stack.push((nu, ny, turn(side * (0.35 + 0.3 * r(6))), radius, strands, kind, path.wrapping_mul(4).wrapping_add(3)));
            } else {
                stack.push((nu, ny, heading, radius, strands, kind, path.wrapping_mul(4)));
            }
        }
    }
    rack
}
