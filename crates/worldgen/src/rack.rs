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
//! A strange rack drops what makes that human (shelves, machinery, tanks,
//! chimneys, a storey rhythm at human height) for things that are not: a bay
//! may hold a smaller rack, which may hold a smaller one (frames within
//! frames); pipes branch like trees as they rise; bundles twist round each
//! other; capsules are strung up like vertebrae.

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
    /// How many racks this one is nested in.
    pub level: u32,
}

/// Racks nest this deep at most.
const MAX_LEVEL: u32 = 2;

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
    // Strange.
    Branching,
    Twisted,
    Nested,
    Vertebrae,
}

/// Builds a rack along the edges of `poly` (convex), from `floor` up.
pub fn build(poly: &[Vec2], floor: f32, layout: Layout, tone: f32, seed: u32) -> Rack {
    let mut rack = Rack::default();
    let Layout { height, storey, bay, depth, frame: c, strange, level } = layout;
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
        // Leave the corners to the neighbouring edges' end columns.
        let usable = length - 2.0 * depth;
        if usable < depth.min(2.0) {
            continue;
        }
        let e = (b - a) / length;
        let inn = inward[i];
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
        // Storey levels, uneven in a strange rack.
        let levels: Vec<f32> = (0..=storeys)
            .map(|j| {
                let jitter = if strange && j > 0 && j < storeys { (r0(j as i32, 9) - 0.5) * 0.8 } else { 0.0 };
                floor + (j as f32 + jitter) * s
            })
            .collect();

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
            let y = levels[j] - if j == storeys { c * 0.4 } else { 0.0 };
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
        // every other storey; in a strange rack, a twisted bundle instead.
        if strange {
            let p = a + (e + inn) * depth * 0.5;
            let wind = depth * 0.28;
            twist(&mut rack, Vec3::new(p.x, floor, p.y), e, inn, wind, height, tone + 0.02, r0(-1, 3), level);
        } else {
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
            let can_nest = level < MAX_LEVEL && span > 1.5 && inner > 0.8;
            let fill = if strange {
                match pick {
                    p if p < 0.28 && can_nest => Fill::Nested,
                    p if p < 0.5 => Fill::Branching,
                    p if p < 0.68 => Fill::Twisted,
                    p if p < 0.8 => Fill::Vertebrae,
                    p if p < 0.9 => Fill::Hoses,
                    _ => Fill::Empty,
                }
            } else {
                match pick {
                    p if p < 0.33 => Fill::Pipes,
                    p if p < 0.55 => Fill::Tanks,
                    p if p < 0.75 => Fill::Hoses,
                    p if p < 0.87 => Fill::Machine,
                    _ => Fill::Empty,
                }
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
                    let rows: &[f32] =
                        if inner > 2.2 { &[0.33, 0.67] } else { &[0.5] };
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
                Fill::Nested => {
                    // A smaller rack lining the bay itself.
                    let corners = [(x0, c), (x1, c), (x1, depth - c), (x0, depth - c)]
                        .map(|(x, z)| start + e * x + inn * z);
                    let sub_depth = span.min(inner) * (0.28 + 0.1 * r(k as i32, 80));
                    let sub = build(
                        &corners,
                        floor,
                        Layout {
                            height,
                            storey: s / (2.0 + r(k as i32, 81) * 2.0),
                            bay: span / (2.0 + r(k as i32, 82) * 2.0),
                            depth: sub_depth,
                            frame: sub_depth / 4.0,
                            strange,
                            level: level + 1,
                        },
                        tone + 0.02,
                        seed ^ (i as u32 * 7919 + k as u32 * 104_729 + 1),
                    );
                    rack.solids.extend(sub.solids);
                    rack.tubes.extend(sub.tubes);
                }
                Fill::Branching => {
                    // Trunks that split into thinner pipes as they rise, and
                    // split again, spreading across the bay.
                    let trunks = 1 + (r(k as i32, 90) * 2.0) as i32;
                    let max_r = (span.min(inner) * 0.22).min(0.8);
                    for t in 0..trunks {
                        let along = x0 + span * (t as f32 + 0.5) / trunks as f32;
                        let p = Vec2::new(along, c + inner * 0.5);
                        branch(
                            &mut rack,
                            &at,
                            (x0, x1, c, depth - c),
                            p,
                            floor,
                            floor + height,
                            max_r,
                            lit,
                            frame_tone,
                            seed ^ (i as u32 * 31 + k as u32 * 977 + t as u32 * 13),
                        );
                    }
                }
                Fill::Twisted => {
                    let mid = at((x0 + x1) * 0.5, c + inner * 0.5, floor);
                    let wind = span.min(inner) * 0.28;
                    twist(&mut rack, mid, e, inn, wind, height, lit, r(k as i32, 95), level);
                }
                Fill::Vertebrae => {
                    // Capsules strung up the bay on a narrow spine, like
                    // vertebrae.
                    let radius = span.min(inner) * (0.3 + 0.12 * r(k as i32, 100));
                    let mid = at((x0 + x1) * 0.5, c + inner * 0.5, floor);
                    let spine = radius * 0.3;
                    rack.tubes.push(Tube { from: mid, to: mid + Vec3::Y * height, radius: spine, albedo: frame_tone });
                    let mut y = 0.3;
                    let mut m = 0;
                    while m < 64 {
                        let long = radius * (1.2 + 1.6 * r(k as i32 * 64 + m, 101));
                        if y + long > height - 0.2 {
                            break;
                        }
                        let p = mid + Vec3::Y * y;
                        rack.tubes.push(Tube { from: p, to: p + Vec3::Y * long, radius, albedo: lit });
                        rack.tubes.push(Tube {
                            from: p + Vec3::Y * (long * 0.5 - 0.1),
                            to: p + Vec3::Y * (long * 0.5 + 0.1),
                            radius: radius * 1.12,
                            albedo: frame_tone,
                        });
                        y += long + radius * (0.3 + 0.5 * r(k as i32 * 64 + m, 102));
                        m += 1;
                    }
                }
                Fill::Empty => {
                    for j in 0..storeys {
                        let y = levels[j];
                        let (dx, dy) = (span, levels[j + 1] - levels[j]);
                        let length = (dx * dx + dy * dy).sqrt();
                        let tilt = dy.atan2(dx);
                        let both = r(k as i32 * 64 + j as i32, 70) < 0.6;
                        for sign in if both { &[1.0, -1.0][..] } else { &[1.0][..] } {
                            let rotation = yaw * Quat::from_rotation_z(tilt * sign);
                            rack.solids.push(Solid {
                                wedge: false,
                                center: at((x0 + x1) * 0.5, c * 0.5, y + dy * 0.5),
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

/// A bundle of pipes winding round each other up a vertical axis from
/// `base`, `wind` from it, as high as `height`.
#[allow(clippy::too_many_arguments)]
fn twist(rack: &mut Rack, base: Vec3, e: Vec2, inn: Vec2, wind: f32, height: f32, tone: f32, r: f32, level: u32) {
    let count = 3 + (r * 3.0) as usize;
    let pitch = (4.0 + 6.0 * r) * (0.5f32).powi(level as i32);
    let radius = wind * (0.7 / count as f32 * 2.0).min(0.55);
    let step = (pitch / 6.0).clamp(0.4, 1.6);
    let steps = (height / step).ceil().max(1.0) as usize;
    let (ex, ez) = (Vec3::new(e.x, 0.0, e.y), Vec3::new(inn.x, 0.0, inn.y));
    let point = |p: usize, t: f32| {
        let y = (t * step).min(height);
        let a = std::f32::consts::TAU * (y / pitch + p as f32 / count as f32);
        base + ex * (a.cos() * wind) + ez * (a.sin() * wind) + Vec3::Y * y
    };
    for p in 0..count {
        let shade = (tone + 0.03 * (p as f32 / count as f32 - 0.5)).clamp(0.05, 0.4);
        for k in 0..steps {
            let (from, to) = (point(p, k as f32), point(p, k as f32 + 1.0));
            // A little overlap hides the joints.
            let d = (to - from).normalize_or_zero() * radius * 0.4;
            rack.tubes.push(Tube { from: from - d, to: to + d, radius, albedo: shade });
        }
    }
}

/// A pipe rising from `p` (in the edge's along/across metres) at `y0` that
/// splits into two or three thinner ones, which split again, within the
/// bay's `bounds` (along from, to; across from, to), up to `y1`.
#[allow(clippy::too_many_arguments)]
fn branch(
    rack: &mut Rack,
    at: &dyn Fn(f32, f32, f32) -> Vec3,
    bounds: (f32, f32, f32, f32),
    p: Vec2,
    y0: f32,
    y1: f32,
    radius: f32,
    tone: f32,
    collar: f32,
    seed: u32,
) {
    // (position, bottom, radius, depth, path)
    let mut stack = vec![(p, y0, radius, 0u32, 1u32)];
    while let Some((p, y, radius, depth, path)) = stack.pop() {
        if rack.tubes.len() > 200_000 {
            return;
        }
        let r = |k: i32| hash01(path as i32, k, depth as i32, seed);
        let remaining = y1 - y;
        let last = depth >= 4 || radius < 0.08 || remaining < 3.0;
        let rise = if last { remaining } else { remaining * (0.2 + 0.3 * r(0)) };
        let from = at(p.x, p.y, y);
        let top = from + Vec3::Y * rise;
        rack.tubes.push(Tube { from, to: top, radius, albedo: tone });
        if last {
            continue;
        }
        // The junction, and the children leaning out from it.
        rack.tubes.push(Tube {
            from: top - Vec3::Y * radius,
            to: top + Vec3::Y * radius,
            radius: radius * 1.35,
            albedo: collar,
        });
        let children = 2 + (r(1) * 2.0) as u32;
        let child_r = radius * (0.55 + 0.1 * r(2));
        let spread = radius * 4.0;
        for c in 0..children {
            let a = std::f32::consts::TAU * (c as f32 / children as f32 + r(3));
            let q = Vec2::new(
                (p.x + a.cos() * spread).clamp(bounds.0 + child_r, bounds.1 - child_r),
                (p.y + a.sin() * spread).clamp(bounds.2 + child_r, bounds.3 - child_r),
            );
            let lift = spread * 1.2;
            let (start, end) = (at(p.x, p.y, y + rise), at(q.x, q.y, y + rise + lift));
            rack.tubes.push(Tube { from: start, to: end, radius: child_r, albedo: tone });
            stack.push((q, y + rise + lift, child_r, depth + 1, path * 4 + c));
        }
    }
}
