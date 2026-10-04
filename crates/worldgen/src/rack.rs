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
//! clinging to the wall it lines (see [`network`]).

use glam::{Quat, Vec2, Vec3};

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
            let wall = Wall {
                start: a + inn * depth + e * depth * 0.5,
                along: e,
                out: -inn,
                length: length - depth,
                floor,
                height,
            };
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
        let solid = |center: Vec3, half: Vec3, albedo: f32| Solid { wedge: false, center, rotation: yaw, half, albedo };
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
            rack.tubes.push(Tube { from: base, to: base + Vec3::Y * top, radius, albedo: (tone + 0.02).min(0.4) });
            for j in (2..storeys).step_by(2) {
                let y = floor + j as f32 * s;
                rack.tubes.push(Tube {
                    from: Vec3::new(p.x, y - 0.3, p.y),
                    to: Vec3::new(p.x, y + 0.3, p.y),
                    radius: radius * 1.12,
                    albedo: frame_tone,
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
                                rack.tubes.push(Tube { from: base, to: turn, radius, albedo: shade });
                                rack.tubes.push(Tube { from: turn, to: out, radius, albedo: shade });
                                let top = Vec3::new(out.x, floor + height, out.z);
                                rack.tubes.push(Tube { from: out - Vec3::Y * radius, to: top, radius, albedo: shade });
                                rack.tubes.push(Tube {
                                    from: out - Vec3::Y * (radius + 0.25),
                                    to: out + Vec3::Y * (radius + 0.25),
                                    radius: radius * 1.25,
                                    albedo: frame_tone,
                                });
                                x += radius * 2.0 + 0.08;
                                m += 1;
                                continue;
                            }
                            rack.tubes.push(Tube { from: base, to: base + Vec3::Y * height, radius, albedo: shade });
                            for j in 1..storeys {
                                let y = floor + j as f32 * s;
                                let p = at(cx, c + inner * f, y);
                                rack.tubes.push(Tube {
                                    from: p - Vec3::Y * 0.2,
                                    to: p + Vec3::Y * 0.2,
                                    radius: radius * 1.3,
                                    albedo: frame_tone,
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
                        rack.tubes.push(Tube { from: ta, to: tb, radius, albedo: lit + 0.03 });
                        let dir = (tb - ta).normalize_or_zero();
                        for end in [ta + dir * 0.5, tb - dir * 0.5] {
                            rack.tubes.push(Tube {
                                from: end - dir * 0.12,
                                to: end + dir * 0.12,
                                radius: radius * 1.06,
                                albedo: frame_tone,
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
                        rack.tubes.push(Tube { from: base, to: base + Vec3::Y * height, radius, albedo: shade });
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

/// A vertical wall for a network to cling to: its foot runs from `start`
/// along `along` for `length` metres at `floor`, `height` high, facing
/// `out`.
pub struct Wall {
    pub start: Vec2,
    pub along: Vec2,
    pub out: Vec2,
    pub length: f32,
    pub floor: f32,
    pub height: f32,
}

impl Wall {
    /// A point `u` metres along the wall, `y` above its floor, `z` out
    /// from its face.
    fn at(&self, u: f32, y: f32, z: f32) -> Vec3 {
        let p = self.start + self.along * u + self.out * z;
        Vec3::new(p.x, self.floor + y, p.y)
    }
}

/// How dense the network is `u` metres along a wall: smooth, with bare
/// stretches between knots.
fn density(u: f32, seed: u32) -> f32 {
    let cell = 7.0;
    let k = (u / cell).floor();
    let t = u / cell - k;
    let t = t * t * (3.0 - 2.0 * t);
    let h = |k: f32| hash01(k as i32, 0, 0xde5, seed);
    let v = h(k) * (1.0 - t) + h(k + 1.0) * t;
    ((v - 0.3) / 0.45).clamp(0.0, 1.0)
}

/// A route of the network as it grows: where its end is (`u` along the
/// wall, `y` up, `z` off it), how thick its pipes are, how many run side by
/// side in its bundle, how many times it has split, and its own number.
#[derive(Clone, Copy)]
struct Route {
    u: f32,
    y: f32,
    z: f32,
    radius: f32,
    strands: u32,
    generation: u32,
    path: u32,
}

/// A network of pipes on a wall, no deeper than `depth` off it: bundles
/// rooted at its foot where the wall is dense, climbing, jogging sideways
/// at uneven angles from lane to lane (now and then running level), and
/// splitting, the bundle shared out between its branches until single
/// pipes split into thinner ones; now and then a route plugs back into the
/// wall. Routes keep their own depth off the wall, so they cross in front
/// of and behind each other; a few heavy brackets hold them. Between the
/// knots, bare wall.
pub fn network(rack: &mut Rack, wall: &Wall, depth: f32, tone: f32, seed: u32) {
    let budget = rack.tubes.len() + 5000;
    let collar_tone = (tone - 0.03).max(0.03);
    let yaw = Quat::from_rotation_y((-wall.along.y).atan2(wall.along.x));
    let top = wall.height - 0.3;
    let fits = |radius: f32, strands: u32| radius * 2.2 * strands as f32;
    // How far off the wall a route of this thickness may run.
    let depth_for = |radius: f32, h: f32| (radius + 0.15 + (depth - 2.0 * radius - 0.15).max(0.0) * h).max(radius + 0.1);

    // Roots along the foot of the wall: in the knots, heavy bundles.
    let mut stack: Vec<Route> = Vec::new();
    let mut u = 0.5;
    let mut k = 0;
    while u < wall.length - 0.5 {
        let h = |j: i32| hash01(k, j, 0x0e7, seed);
        let d = density(u, seed);
        if h(0) < d * 0.7 {
            let radius = (0.15 + 0.5 * h(1) * h(1) * d).min(depth * 0.3);
            let strands = 1 + (h(4) * h(4) * 6.0 * d) as u32;
            stack.push(Route { u, y: 0.0, z: depth_for(radius, h(2)), radius, strands, generation: 0, path: k as u32 + 1 });
            u += fits(radius, strands) + 0.3 + 2.0 * h(3);
        } else {
            u += 0.8;
        }
        k += 1;
    }

    // A segment of a route: each strand of the bundle, side by side across
    // the segment's direction in the wall's plane.
    let segment = |rack: &mut Rack, from: (f32, f32, f32), to: (f32, f32, f32), radius: f32, strands: u32, shade: f32| {
        let (du, dy) = (to.0 - from.0, to.1 - from.1);
        let length = (du * du + dy * dy).sqrt().max(1e-3);
        let (pu, py) = (-dy / length, du / length);
        let spacing = radius * 2.2;
        for s in 0..strands {
            let o = (s as f32 - (strands as f32 - 1.0) * 0.5) * spacing;
            // Strands of a wide bundle step a little in depth too.
            let dz = if s % 2 == 1 { radius * 0.6 } else { 0.0 };
            let a = wall.at(from.0 + pu * o, from.1 + py * o, from.2 + dz);
            let b = wall.at(to.0 + pu * o, to.1 + py * o, to.2 + dz);
            // A little overlap hides the joints at bends.
            let d = (b - a).normalize_or_zero() * radius * 0.5;
            rack.tubes.push(Tube { from: a - d, to: b + d, radius, albedo: shade });
        }
    };

    while let Some(route) = stack.pop() {
        if rack.tubes.len() > budget {
            return;
        }
        let Route { u, y, z, radius, strands, generation, path } = route;
        if y >= top {
            continue;
        }
        let r = |j: i32| hash01(path as i32, j, generation as i32, seed);
        let shade = (tone + 0.04 + (r(0) - 0.5) * 0.05).clamp(0.05, 0.4);
        let width = fits(radius, strands);
        let clamp_u = |u: f32| u.clamp(width * 0.5 + 0.2, (wall.length - width * 0.5 - 0.2).max(width * 0.5 + 0.2));
        let action = r(1);
        // A heavy bracket holding the bundle to the wall now and then.
        if r(2) < 0.1 {
            rack.solids.push(Solid {
                wedge: false,
                center: wall.at(u, y + radius, z * 0.5),
                rotation: yaw,
                half: Vec3::new(width * 0.5 + 0.15, radius * 1.1 + 0.08, z * 0.5),
                albedo: collar_tone,
            });
        }
        if generation > 0 && y > wall.height * 0.15 && action < 0.05 {
            // Into the wall.
            segment(rack, (u, y, z), (u, y, -0.2), radius, strands, shade);
        } else if action < 0.3 && (strands > 1 || (radius > 0.18 && generation < 6)) {
            // A split: the bundle shared out between two or three branches
            // leaning away across the wall, each at its own depth; a single
            // pipe splits into thinner ones.
            rack.tubes.push(Tube {
                from: wall.at(u, y - radius, z),
                to: wall.at(u, y + radius, z),
                radius: width * 0.5 + radius * 0.35,
                albedo: collar_tone,
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
                let side = if c % 2 == 0 { 1.0 } else { -1.0 };
                let du = side * (1.5 + 5.0 * rc(1)) * (0.5 + child_r * 2.0);
                let nu = clamp_u(u + du);
                let nz = depth_for(child_r, rc(2));
                let ny = (y + (nu - u).abs() * (0.3 + 1.4 * rc(3)) + 0.3).min(top);
                segment(rack, (u, y, z), (nu, ny, nz), child_r, child_n, shade);
                stack.push(Route { u: nu, y: ny, z: nz, radius: child_r, strands: child_n, generation: generation + 1, path: id });
            }
        } else if action < 0.55 {
            // A jog sideways into another lane, at an uneven angle, now and
            // then level; maybe nearer or further off the wall.
            let side = if r(4) < 0.5 { -1.0 } else { 1.0 };
            let du = side * (1.0 + 5.0 * r(5));
            let nu = clamp_u(u + du);
            let nz = (z + (r(6) - 0.5) * depth * 0.6).clamp(radius + 0.1, (depth - radius).max(radius + 0.1));
            let slope = if r(7) < 0.2 { 0.0 } else { 0.25 + 1.75 * r(9) };
            let ny = (y + (nu - u).abs() * slope).min(top);
            segment(rack, (u, y, z), (nu, ny, nz), radius, strands, shade);
            // A level run must still climb afterwards.
            let ny = if slope == 0.0 {
                let rise = (1.0 + 2.0 * r(10)).min(top - ny).max(0.0);
                segment(rack, (nu, ny, nz), (nu, ny + rise, nz), radius, strands, shade);
                ny + rise.max(0.3)
            } else {
                ny.max(y + 0.3)
            };
            stack.push(Route { u: nu, y: ny, z: nz, path: path.wrapping_mul(2).wrapping_add(1), ..route });
        } else {
            // A straight climb.
            let rise = (2.0 + 7.0 * r(8)).min(top - y);
            segment(rack, (u, y, z), (u, y + rise, z), radius, strands, shade);
            stack.push(Route { y: y + rise, path: path.wrapping_mul(2), ..route });
        }
    }
}
