//! The brute's arms and waist band: cables, not its mesh (the user,
//! 2026-11-10: the lash read as pasted on, it must come out of its own
//! cabling; and pay out from the band). The mesh's arm loops and band are
//! cut out by `tools/impaled_mesh.py`, which writes the cables that take
//! their place to `creatures/impaled.cables`: each its side (1 its left
//! arm, -1 its right, 0 a coil of the band), its thickness, its path at
//! rest (an arm's from its root in the shoulder to its end in the band, a
//! coil's round the waist once; mesh units). Drawn as tubes on the brute,
//! moving and bending with it; in a lash its arms are flung by their
//! shoulders (`attack.rs`), the band's coils unwinding as they pay out.

use std::sync::OnceLock;

use bevy::prelude::*;

/// One cable at rest.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Cable {
    /// Which arm: 1 its left (+x), -1 its right; 0 a coil of its band.
    pub side: f32,
    pub radius: f32,
    /// From its root in the shoulder to its end in the waist band (mesh
    /// units).
    pub rest: Vec<Vec3>,
}

fn file() -> std::path::PathBuf {
    super::super::creatures_folder().join("impaled.cables")
}

/// Its cables, read once (failing loudly: the file and what is wrong; make
/// it with `python tools/impaled_mesh.py`).
pub(super) fn cables() -> &'static [Cable] {
    static READ: OnceLock<Vec<Cable>> = OnceLock::new();
    READ.get_or_init(|| {
        let path = file();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e} (made by `python tools/impaled_mesh.py`)", path.display()));
        parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    })
}

/// Cables as the file has them: one a line, `SIDE RADIUS x,y,z x,y,z ...`;
/// `#` to the end of a line a comment.
pub(super) fn parse(text: &str) -> Result<Vec<Cable>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let fail = |what: &str| format!("line {}: {what}", n + 1);
        let mut words = line.split_whitespace();
        let side: f32 = words.next().and_then(|w| w.parse().ok()).ok_or_else(|| fail("no side (1, -1 or 0) first"))?;
        if side != 1.0 && side != -1.0 && side != 0.0 {
            return Err(fail("its side is 1 or -1 (an arm) or 0 (the band)"));
        }
        let radius: f32 = words.next().and_then(|w| w.parse().ok()).filter(|r: &f32| *r > 0.0).ok_or_else(|| fail("no radius after its side"))?;
        let rest: Vec<Vec3> = words
            .map(|w| {
                let v: Vec<f32> = w.split(',').map(str::parse).collect::<Result<_, _>>().map_err(|_| fail(&format!("{w} is not x,y,z")))?;
                <[f32; 3]>::try_from(v).map(Vec3::from).map_err(|_| fail(&format!("{w} is not three numbers")))
            })
            .collect::<Result<_, _>>()?;
        if rest.len() < 2 {
            return Err(fail("fewer than two points"));
        }
        out.push(Cable { side, radius, rest });
    }
    if out.is_empty() {
        return Err("no cables".into());
    }
    Ok(out)
}

/// How many times a cable's way is smoothed (point by point, it folded);
/// how many cables of each arm a coil of the band feeds, and how much of a
/// coil can unwind (of it); how far apart the two strands of a flung
/// cable's loop lie (m).
const SMOOTH: usize = 6;
const FED: usize = 3;
const FEED: f32 = 0.9;
const STRANDS: f32 = 0.06;

/// The cable the arm on `side` flings: its outermost (the longest loop) not
/// `out` already (still coming back from the last).
pub(super) fn lasher(side: f32, out: &[usize]) -> Option<usize> {
    debug_assert!(side != 0.0);
    let outward = |c: &Cable| c.rest.iter().map(|p| p.x * c.side).fold(f32::MIN, f32::max);
    (0..cables().len()).filter(|&k| cables()[k].side == side && !out.contains(&k)).max_by(|&a, &b| outward(&cables()[a]).total_cmp(&outward(&cables()[b])))
}

/// Where along it (which point) its loop's elbow is: furthest out. From
/// there its loop is flung.
fn elbow_at(c: &Cable) -> usize {
    (0..c.rest.len()).max_by(|&a, &b| (c.rest[a].x * c.side).total_cmp(&(c.rest[b].x * c.side))).unwrap_or(0).min(c.rest.len() - 2)
}

/// Where cable `k`'s elbow is (m, its own frame), its body in `pose`.
pub(super) fn elbow(k: usize, scale: f32, pose: Vec2) -> Vec3 {
    let c = &cables()[k];
    bent(c.rest[elbow_at(c)], pose) * scale
}

/// Which coil of its band feeds which arm cable: on each side its three
/// outermost cables, each its own coil, those starting on that side (the
/// rest of the cables only stretch; a coil left over stays as it is).
fn feeds() -> &'static [(usize, usize)] {
    static PAIRS: OnceLock<Vec<(usize, usize)>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        let all = cables();
        let outward = |k: usize| all[k].rest.iter().map(|p| p.x * all[k].side).fold(f32::MIN, f32::max);
        let mut pairs = Vec::new();
        for side in [1.0f32, -1.0] {
            let mut arm: Vec<usize> = (0..all.len()).filter(|&k| all[k].side == side).collect();
            arm.sort_by(|&a, &b| outward(b).total_cmp(&outward(a)));
            let coils = (0..all.len()).filter(|&j| all[j].side == 0.0 && all[j].rest[0].x * side > 0.0);
            pairs.extend(arm.into_iter().zip(coils).take(FED));
        }
        pairs
    })
}

/// A cable's length (m).
fn length(points: &[Vec3]) -> f32 {
    points.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// Where along `points` (as far as `at` m along) it is, and the index of
/// the first point beyond.
fn point_along(points: &[Vec3], at: f32) -> (Vec3, usize) {
    let mut gone = 0.0;
    for i in 1..points.len() {
        let step = points[i - 1].distance(points[i]);
        if gone + step >= at {
            return (points[i - 1].lerp(points[i], ((at - gone) / step.max(1e-6)).clamp(0.0, 1.0)), i);
        }
        gone += step;
    }
    (points[points.len() - 1], points.len())
}

/// How far the coil feeding arm cable `k` (if one does) is unwound with
/// its head out its way so far (m): twice its way (the loop goes out and
/// back), up to `FEED` of the coil.
fn paid(k: usize, way: f32, scale: f32) -> f32 {
    feeds().iter().find(|(a, _)| *a == k).map_or(0.0, |&(_, j)| {
        let coil: Vec<Vec3> = cables()[j].rest.iter().map(|p| *p * scale).collect();
        (2.0 * way).min(length(&coil) * FEED)
    })
}

/// Its waist band, where its body bends (mesh units: the pivot, and from
/// where to where up it the bend comes in, below it none, above it all);
/// its shoulders, which move forward and back (mesh units: from how far out
/// from its middle, and how high, to where it is all theirs).
const WAIST: Vec3 = Vec3::new(0.0, 0.10, -0.05);
const BEND: (f32, f32) = (-0.05, 0.30);
const SHOULDERS: ((f32, f32), (f32, f32)) = ((0.08, 0.45), (0.0, 0.40));

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// A point of its body (mesh units) in `pose` (x: leaning forward about its
/// waist band, rad; y: its shoulders forward, mesh units; back, less than
/// none): its shoulders moved first, more the further out and higher (the
/// line between them bowing, seen from above), then all above its waist
/// band turned about it, below the band as it was, between them by a share,
/// smoothly (the user, 2026-11-10: shoulders and body in the lash).
pub(super) fn bent(p: Vec3, pose: Vec2) -> Vec3 {
    if pose == Vec2::ZERO {
        return p;
    }
    let ((x0, x1), (y0, y1)) = SHOULDERS;
    let shoulder = smooth(x0, x1, p.x.abs()) * smooth(y0, y1, p.y);
    let p = p + Vec3::Z * (pose.y * shoulder);
    WAIST + Quat::from_rotation_x(pose.x * smooth(BEND.0, BEND.1, p.y)) * (p - WAIST)
}

/// How a direction (a normal) at `p` turns with `bent` (its lean; its
/// shoulders' slide left as it was).
pub(super) fn bent_dir(p: Vec3, d: Vec3, pose: Vec2) -> Vec3 {
    if pose.x == 0.0 {
        return d;
    }
    Quat::from_rotation_x(pose.x * smooth(BEND.0, BEND.1, p.y)) * d
}

/// Arm cable `k` flung out (m, its own frame; `pos`, `rot` its own):
/// whole, from its root in its shoulder down its arm to its elbow, out
/// along the way its head has come to its head, back along that way beside
/// itself (a loop, its two strands side by side), and from its elbow on to
/// where it leaves the band, with what of its coil is paid out. Smoothed,
/// its ends and its head held.
fn flung(k: usize, h: &super::attack::Head, pos: Vec3, rot: Quat, scale: f32, pose: Vec2) -> Vec<Vec3> {
    let c = &cables()[k];
    let rest: Vec<Vec3> = c.rest.iter().map(|p| bent(*p, pose) * scale).collect();
    let e = elbow_at(c);
    let back = rot.inverse();
    let mut way: Vec<Vec3> = h.trail.iter().map(|(p, sag)| back * (*p - Vec3::Y * *sag - pos)).collect();
    way.push(back * (h.at - pos));
    let beside = (rot.inverse() * Vec3::X * c.side) * STRANDS;
    let mut points: Vec<Vec3> = rest[..=e].to_vec();
    points.extend(way.iter().copied());
    let tip = points.len() - 1;
    points.extend(way.iter().rev().skip(1).map(|p| *p + beside));
    points.extend(rest[e..].iter().map(|p| *p + beside * 0.5));
    if let Some(&(_, j)) = feeds().iter().find(|(a, _)| *a == k) {
        let coil: Vec<Vec3> = cables()[j].rest.iter().map(|p| bent(*p, pose) * scale).collect();
        let (exit, beyond) = point_along(&coil, paid(k, length(&way), scale));
        points.extend_from_slice(&coil[1..beyond.min(coil.len())]);
        points.push(exit);
    }
    for _ in 0..SMOOTH {
        let before = points.clone();
        for i in 1..points.len() - 1 {
            if i != tip {
                points[i] = before[i].lerp((before[i - 1] + before[i + 1]) * 0.5, 0.5);
            }
        }
    }
    points
}

/// A brute's arms drawn: its cables, as tubes, in its own frame; whether
/// they were last drawn lashing (to be drawn at rest again, once).
#[derive(Component)]
pub(in crate::combat) struct Arms {
    mesh: Handle<Mesh>,
    lashing: bool,
}

/// Its cables, their points as its grasp has them (m, its own frame; bent
/// with its body), and thickness.
fn strands(g: &super::attack::Grasp, pos: Vec3, rot: Quat, scale: f32, pose: Vec2) -> Vec<(Vec<Vec3>, f32)> {
    let out = |k: usize| g.heads.iter().find(|h| h.cable == k && h.state != super::attack::State::Home);
    cables()
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let rest = || c.rest.iter().map(|p| bent(*p, pose) * scale).collect::<Vec<Vec3>>();
            let points = match (out(k), feeds().iter().find(|(_, j)| *j == k).and_then(|&(a, _)| out(a).map(|h| (a, h)))) {
                (Some(h), _) => flung(k, h, pos, rot, scale, pose),
                // (A coil feeding a flung cable: what of it is not paid out.)
                (None, Some((a, h))) => {
                    let coil = rest();
                    let way: f32 = h.trail.windows(2).map(|w| w[0].0.distance(w[1].0)).sum::<f32>() + h.trail.last().map_or(0.0, |(p, _)| p.distance(h.at));
                    let (exit, beyond) = point_along(&coil, paid(a, way, scale));
                    std::iter::once(exit).chain(coil[beyond.min(coil.len() - 1)..].iter().copied()).collect()
                }
                _ => rest(),
            };
            (points, c.radius * scale)
        })
        .collect()
}

/// Its arms made, at rest, on `brute` (its scale `scale`: mesh units to
/// metres).
pub(super) fn make(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, brute: Entity, scale: f32) {
    let mut mesh = super::field::placeholder();
    let at_rest = strands(&default(), Vec3::ZERO, Quat::IDENTITY, scale, Vec2::ZERO);
    crate::rope::tubes(&mut mesh, at_rest.iter().map(|(p, r)| (&p[..], (*r, *r), 1.0)));
    let mesh = meshes.add(mesh);
    // (Its body's black, glossy.)
    let look = materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.035), perceptual_roughness: 0.32, reflectance: 0.5, ..default() });
    commands.spawn((Arms { mesh: mesh.clone(), lashing: false }, Mesh3d(mesh), MeshMaterial3d(look), Transform::default(), ChildOf(brute)));
}

/// Each brute's arms drawn again while its grasp is out, and once more at
/// rest after.
pub(in crate::combat) fn draw(brutes: Query<&super::Impaled>, mut arms: Query<(&mut Arms, &ChildOf)>, mut meshes: ResMut<Assets<Mesh>>) {
    for (mut a, parent) in &mut arms {
        let Ok(b) = brutes.get(parent.parent()) else { continue };
        let moving = b.grasp.phase != super::attack::Phase::Rest;
        if !moving && !a.lashing {
            continue;
        }
        a.lashing = moving;
        if let Some(mut mesh) = meshes.get_mut(&a.mesh) {
            let (pos, rot) = b.flight.drawn();
            let now = strands(&b.grasp, pos, rot, super::scale(), b.pose);
            crate::rope::tubes(&mut mesh, now.iter().map(|(p, r)| (&p[..], (*r, *r), 1.0)));
        }
    }
}

/// One of its body's own meshes (from its file: the body, the shards), its
/// own copy, bent as its body moves in a lash: its points and normals as
/// they were, and the pose it was last bent to.
#[derive(Component)]
pub(in crate::combat) struct Bent {
    brute: Entity,
    mesh: Handle<Mesh>,
    rest: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    pose: Vec2,
}

/// Its body's meshes, once in, made its own (each brute bends its own) and
/// kept as they were, to be bent.
pub(in crate::combat) fn adopt(
    mut commands: Commands,
    brutes: Query<Entity, With<super::Impaled>>,
    children: Query<&Children>,
    // (Not its arms, nor the shards shot into it.)
    shapes: Query<&Mesh3d, (Without<Bent>, Without<Arms>, Without<crate::combat::Needle>)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for brute in &brutes {
        for e in children.iter_descendants(brute) {
            let Ok(shape) = shapes.get(e) else { continue };
            let Some(mesh) = meshes.get(&shape.0).cloned() else { continue };
            let read = |a| match mesh.attribute(a) {
                Some(bevy::mesh::VertexAttributeValues::Float32x3(v)) => Some(v.clone()),
                _ => None,
            };
            let (Some(rest), Some(normals)) = (read(Mesh::ATTRIBUTE_POSITION), read(Mesh::ATTRIBUTE_NORMAL)) else {
                panic!("the impaled brute's mesh has no positions or normals to bend");
            };
            let own = meshes.add(mesh);
            commands.entity(e).insert((Mesh3d(own.clone()), Bent { brute, mesh: own, rest, normals, pose: Vec2::ZERO }));
        }
    }
}

/// Its body bent to its pose now (only when that changes).
pub(in crate::combat) fn bend(brutes: Query<&super::Impaled>, mut bents: Query<&mut Bent>, mut meshes: ResMut<Assets<Mesh>>) {
    for mut b in &mut bents {
        let pose = brutes.get(b.brute).map_or(Vec2::ZERO, |i| i.pose);
        if (pose - b.pose).abs().max_element() < 1e-4 {
            continue;
        }
        b.pose = pose;
        let Some(mut mesh) = meshes.get_mut(&b.mesh) else { continue };
        let pos: Vec<[f32; 3]> = b.rest.iter().map(|p| bent(Vec3::from(*p), pose).to_array()).collect();
        let nor: Vec<[f32; 3]> = b.rest.iter().zip(&b.normals).map(|(p, n)| bent_dir(Vec3::from(*p), Vec3::from(*n), pose).to_array()).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    }
}
