//! The brute's arms and waist band: cables, not its mesh (the user,
//! 2026-11-10: the lash read as pasted on, it must come out of its own
//! cabling; and pay out from the band). The mesh's arm loops and band are
//! cut out by `tools/impaled_mesh.py`, which writes the cables that take
//! their place to `creatures/impaled.cables`: each its side (1 its left
//! arm, -1 its right, 0 a coil of the band), its thickness, its path at
//! rest (an arm's from its root in the shoulder to its end in the band, a
//! coil's round the waist once; mesh units). Drawn as tubes on the brute,
//! moving and bending with it; in its grasp its cables are flung by their
//! shoulders (`attack.rs`), the band's coils unwinding as they pay out; a
//! cable dropped lies on the ground, drawn apart from it (`Litter`). Its
//! cables are drawn on the GPU from their points (`tube.rs`): only their
//! points are written, and only while they move.
//!
//! A flung cable's head (the user, 2026-11-12: lost among the parts of it
//! lagging behind) is a pale shard, as those it is impaled with, clamped in
//! its loop's bend, pointing on and barbed back (`Tips`).

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
/// Its cables' heads: how big (times the shard `tip` makes, m). (They
/// dripped ichor as they went; the user, 2026-11-12: it added nothing.)
const TIP: f32 = 0.55;


/// The cable the arm on `side` flings: its outermost (the longest loop) not
/// `out` already.
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

/// Its shoulders, which move forward and back (mesh units: from how far out
/// from its middle, and how high, to where it is all theirs).
const SHOULDERS: ((f32, f32), (f32, f32)) = ((0.08, 0.45), (0.0, 0.40));

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// A point of its body (mesh units) in `pose` (its shoulders forward, its
/// left (x) and its right (y), mesh units; back, less than none): each
/// shoulder moved, more the further out and higher (the line between them
/// bowing, seen from above); its body not leaning (the user, 2026-11-10:
/// the force is drawn from the shoulders' arch). (Its normals as they were:
/// a slide hardly turns them.)
pub(super) fn bent(p: Vec3, pose: Vec2) -> Vec3 {
    let forward = if p.x > 0.0 { pose.x } else { pose.y };
    if forward == 0.0 {
        return p;
    }
    p + Vec3::Z * (forward * shouldered(p))
}

/// How much of its shoulder's move a point of its body (mesh units) moves
/// with (0..1): more the further out and higher. Its body's meshes are
/// skinned by it (`adopt`), so the same bend is done on the GPU.
fn shouldered(p: Vec3) -> f32 {
    let ((x0, x1), (y0, y1)) = SHOULDERS;
    smooth(x0, x1, p.x.abs()) * smooth(y0, y1, p.y)
}

/// Smoothed `SMOOTH` times, point by point, its ends and point `keep` held.
fn smoothed(points: &mut [Vec3], keep: usize) {
    for _ in 0..SMOOTH {
        let before = points.to_vec();
        for i in 1..points.len().saturating_sub(1) {
            if i != keep {
                points[i] = before[i].lerp((before[i - 1] + before[i + 1]) * 0.5, 0.5);
            }
        }
    }
}

/// The way a cable's head has come (m, the world's): its trail, sagged, to
/// its head.
fn way(h: &super::attack::Head) -> impl DoubleEndedIterator<Item = Vec3> + Clone + '_ {
    h.trail.iter().map(|(p, sag)| *p - Vec3::Y * *sag).chain(std::iter::once(h.at))
}

/// A cable dropped (m, the world's; `rot` its brute's): its loop cut at the
/// elbow, out along its way to its head and back beside itself, smoothed;
/// and which point is its head.
pub(super) fn dropped(h: &super::attack::Head, rot: Quat) -> (Vec<Vec3>, usize) {
    let c = &cables()[h.cable];
    let beside = rot * Vec3::X * (c.side * STRANDS);
    let mut points: Vec<Vec3> = way(h).collect();
    let tip = points.len() - 1;
    points.extend(way(h).rev().skip(1).map(|p| p + beside));
    smoothed(&mut points, tip);
    (points, tip)
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
    let way: Vec<Vec3> = way(h).map(|p| back * (p - pos)).collect();
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
    smoothed(&mut points, tip);
    points
}

/// A brute's arms drawn: its cables, as tubes, in its own frame; whether
/// they were last drawn moving (to be drawn at rest again, once).
#[derive(Component)]
pub(in crate::combat) struct Arms {
    moving: bool,
}

/// Its cables' heads, flung and dropped, each drawn where it is (the
/// world's frame, as its `Litter`): the head's mesh and look, the heads
/// made (each a `Tip`, kept to be shown again), and how many were last
/// shown.
#[derive(Component)]
pub(in crate::combat) struct Tips {
    brute: Entity,
    mesh: Handle<Mesh>,
    look: Handle<StandardMaterial>,
    made: Vec<Entity>,
    shown: usize,
}

/// One of its cables' heads, drawn.
#[derive(Component)]
pub(in crate::combat) struct Tip;

/// A cable's head: a shard, faceted, its point along +z, its back at the
/// origin (in its loop's bend), three barbs swept back about it (a grapnel;
/// as its feet are star-cut); triangles (m).
fn tip() -> &'static [[Vec3; 3]] {
    static TRIS: OnceLock<Vec<[Vec3; 3]>> = OnceLock::new();
    TRIS.get_or_init(|| {
        let mut tris = Vec::new();
        // (A bipyramid: a ring of `radii` about `at` across `axis`, a point
        // `ahead` along it, one `behind`.)
        let mut spike = |at: Vec3, axis: Vec3, radii: &[f32], ahead: f32, behind: f32, twist: f32| {
            let axis = axis.normalize();
            let u = axis.any_orthonormal_vector();
            let v = axis.cross(u);
            let n = radii.len();
            let ring: Vec<Vec3> = (0..n)
                .map(|k| {
                    let a = (k as f32 + twist) / n as f32 * std::f32::consts::TAU;
                    at + (u * a.cos() + v * a.sin()) * radii[k]
                })
                .collect();
            let (front, back) = (at + axis * ahead, at - axis * behind);
            for k in 0..n {
                let (a, b) = (ring[k], ring[(k + 1) % n]);
                tris.push([a, b, front]);
                tris.push([b, a, back]);
            }
        };
        spike(Vec3::Z * 0.15, Vec3::Z, &[0.075, 0.058, 0.07, 0.052, 0.064], 0.46, 0.15, 0.0);
        for k in 0..3 {
            let a = (k as f32 + 0.15) / 3.0 * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            spike(Vec3::Z * 0.24 + out * 0.04, -Vec3::Z * 0.75 + out * 0.65, &[0.026, 0.02, 0.024, 0.018], 0.24 - 0.03 * k as f32, 0.05, 0.3);
        }
        tris
    })
}

/// A head, as one mesh, flat-shaded.
fn tip_mesh() -> Mesh {
    let (mut positions, mut normals) = (Vec::new(), Vec::new());
    for p in tip() {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or(Vec3::Y);
        for v in p {
            positions.push(v.to_array());
            normals.push(n.to_array());
        }
    }
    let n = positions.len() as u32;
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(bevy::mesh::Indices::U32((0..n).collect()))
}

/// A head pointing along `dir`, rolled its own way (by `k`).
fn pointing(dir: Vec3, k: usize) -> Quat {
    Quat::from_rotation_arc(Vec3::Z, dir.normalize_or(Vec3::Z)) * Quat::from_rotation_z(k as f32 * 2.1)
}



/// The cables a brute dropped, drawn where they lie (the world's frame,
/// apart from it: they stay as it goes); whether all lay still when last
/// drawn (then they are not drawn again till one stirs).
#[derive(Component)]
pub(in crate::combat) struct Litter {
    brute: Entity,
    still: bool,
}

/// Its cables, their points as its grasp has them (m, its own frame; bent
/// with its body), and thickness.
fn strands(g: &super::attack::Grasp, pos: Vec3, rot: Quat, scale: f32, pose: Vec2) -> Vec<(Vec<Vec3>, f32)> {
    let out = |k: usize| g.heads.iter().find(|h| h.cable == k);
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
/// metres); its cables' heads.
pub(super) fn make(commands: &mut Commands, tubes: &mut super::tube::Kit, materials: &mut Assets<StandardMaterial>, brute: Entity, scale: f32) {
    let at_rest = strands(&default(), Vec3::ZERO, Quat::IDENTITY, scale, Vec2::ZERO);
    // (Black, glossy; the user, 2026-11-12, kept so after grey and pale
    // collars were tried.)
    let cable = || StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.035), perceptual_roughness: 0.32, reflectance: 0.5, ..default() };
    // (As the shards it is impaled with.)
    let pale = materials.add(StandardMaterial { base_color: Color::srgb(0.78, 0.78, 0.8), perceptual_roughness: 0.35, reflectance: 0.5, ..default() });
    commands.spawn((Arms { moving: false }, super::tube::make(tubes, cable(), at_rest.iter().map(|(p, r)| (&p[..], *r))), Transform::default(), ChildOf(brute)));
    commands.spawn((Litter { brute, still: true }, super::tube::make(tubes, cable(), []), Transform::default()));
    let tip = tubes.meshes.add(tip_mesh());
    commands.spawn((Tips { brute, mesh: tip, look: pale, made: Vec::new(), shown: 0 }, Transform::default(), Visibility::default()));
}

/// Each brute's arms drawn again while they move, and once more at rest
/// after; the cables it dropped, while any of them stirs (and gone with
/// it); its cables' heads.
#[allow(clippy::too_many_arguments)]
pub(in crate::combat) fn draw(
    mut commands: Commands,
    brutes: Query<&super::Impaled>,
    mut arms: Query<(&mut Arms, &mut super::tube::Tubes, &mut Mesh3d, &ChildOf), Without<Litter>>,
    mut litters: Query<(Entity, &mut Litter, &mut super::tube::Tubes, &mut Mesh3d), Without<Arms>>,
    mut tips: Query<(Entity, &mut Tips)>,
    mut heads: Query<(&mut Transform, &mut Visibility), With<Tip>>,
    mut kit: super::tube::Kit,
) {
    for (mut a, mut tubes, mut mesh, parent) in &mut arms {
        let Ok(b) = brutes.get(parent.parent()) else { continue };
        let moving = super::attack::moving(&b.grasp);
        if !moving && !a.moving {
            continue;
        }
        a.moving = moving;
        let (pos, rot) = b.flight.drawn();
        let now = strands(&b.grasp, pos, rot, super::scale(), b.pose);
        super::tube::write(&mut kit, &mut tubes, &mut mesh, now.iter().map(|(p, r)| (&p[..], *r)));
    }
    for (entity, mut t) in &mut tips {
        let Ok(b) = brutes.get(t.brute) else {
            commands.entity(entity).despawn();
            continue;
        };
        if b.grasp.heads.is_empty() && b.grasp.litter.is_empty() && t.shown == 0 {
            continue;
        }
        let size = TIP * super::scale() / 1.37;
        // (Flung, pointing the way it goes, or the way its cable came;
        // dropped, along its cable's end, thinning with it.)
        let flung = b.grasp.heads.iter().map(|h| {
            let came = h.trail.last().map_or(Vec3::ZERO, |(p, _)| h.at - *p);
            let dir = if h.velocity().length_squared() > 0.01 { h.velocity() } else { came };
            (h.at, pointing(dir, h.cable), size)
        });
        let dropped = b.grasp.litter.iter().filter(|l| l.tip > 0 && l.tip < l.points.len() && l.thick() > 0.0).map(|l| (l.points[l.tip], pointing(l.points[l.tip] - l.points[l.tip - 1], l.tip), size * l.thick() / l.radius()));
        let Tips { mesh, look, made, shown, .. } = &mut *t;
        let mut n = 0;
        for (at, turn, size) in flung.chain(dropped) {
            let to = Transform { translation: at, rotation: turn, scale: Vec3::splat(size) };
            match made.get(n) {
                Some(&e) => {
                    if let Ok((mut placed, mut seen)) = heads.get_mut(e) {
                        placed.set_if_neq(to);
                        seen.set_if_neq(Visibility::Inherited);
                    }
                }
                None => made.push(commands.spawn((Tip, Mesh3d(mesh.clone()), MeshMaterial3d(look.clone()), to, ChildOf(entity))).id()),
            }
            n += 1;
        }
        for &e in &made[n..] {
            if let Ok((_, mut seen)) = heads.get_mut(e) {
                seen.set_if_neq(Visibility::Hidden);
            }
        }
        *shown = n;
    }
    for (entity, mut l, mut tubes, mut mesh) in &mut litters {
        let Ok(b) = brutes.get(l.brute) else {
            commands.entity(entity).despawn();
            continue;
        };
        let still = b.grasp.litter.iter().all(|s| s.still());
        if still && l.still {
            continue;
        }
        l.still = still;
        super::tube::write(&mut kit, &mut tubes, &mut mesh, b.grasp.litter.iter().map(|s| (&s.points[..], s.thick())));
    }
}


/// One of its body's own meshes (from its file: the body, the shards),
/// skinned to its shoulders: its left and right shoulder's joints (moved
/// forward and back as its shoulders are, in its mesh's own units), and the
/// pose they were last moved to.
///
/// (Its meshes' points were bent on the CPU each time its shoulders moved,
/// and the meshes made again: the renderer took them in anew each frame of
/// an attack, the user, 2026-11-12: a fifth of the frame. Skinned, only
/// two joints move.)
#[derive(Component)]
pub(in crate::combat) struct Bent {
    brute: Entity,
    shoulders: [Entity; 2],
    pose: Vec2,
}

/// Its body's meshes, once in, made its own and skinned to its shoulders
/// (each point to its middle and to its own side's shoulder, by
/// `shouldered`: three joints, children of the mesh, at rest where it is);
/// its body's skin as it has it (`skin.rs`); its shards hidden if it has
/// them so.
pub(in crate::combat) fn adopt(
    mut commands: Commands,
    brutes: Query<(Entity, &super::Impaled)>,
    children: Query<&Children>,
    // (Not its arms, nor the shards shot into it.)
    shapes: Query<(&Mesh3d, Option<&MeshMaterial3d<StandardMaterial>>), (Without<Bent>, Without<Arms>, Without<crate::combat::Needle>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bindposes: ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    mut at_rest: Local<Option<Handle<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>>,
) {
    // (Its joints at rest where its mesh is: none of them moves it.)
    let at_rest = at_rest.get_or_insert_with(|| bindposes.add(bevy::mesh::skinning::SkinnedMeshInverseBindposes::from(vec![Mat4::IDENTITY; 3]))).clone();
    for (brute, b) in &brutes {
        for e in children.iter_descendants(brute) {
            let Ok((shape, look)) = shapes.get(e) else { continue };
            let Some(mut mesh) = meshes.get(&shape.0).cloned() else { continue };
            // (Its body: the one with a texture; its shards the other.)
            let made = look.and_then(|m| materials.get(&m.0)).cloned();
            match made {
                Some(made) if made.base_color_texture.is_some() => {
                    if let Some(skin) = super::skin::paint(b.body, &mut mesh, &made) {
                        commands.entity(e).insert(MeshMaterial3d(materials.add(skin)));
                    }
                }
                Some(_) if !b.shards => {
                    commands.entity(e).insert(Visibility::Hidden);
                }
                _ => {}
            }
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(rest)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
                panic!("the impaled brute's mesh has no positions to bend");
            };
            let (joints, weights): (Vec<[u16; 4]>, Vec<[f32; 4]>) = rest
                .iter()
                .map(|p| {
                    let p = Vec3::from(*p);
                    let w = shouldered(p);
                    ([0, if p.x > 0.0 { 1 } else { 2 }, 0, 0], [1.0 - w, w, 0.0, 0.0])
                })
                .unzip();
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, bevy::mesh::VertexAttributeValues::Uint16x4(joints));
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
            let own = meshes.add(mesh);
            let joint = |commands: &mut Commands| commands.spawn((Transform::IDENTITY, ChildOf(e))).id();
            let (middle, left, right) = (joint(&mut commands), joint(&mut commands), joint(&mut commands));
            commands.entity(e).insert((Mesh3d(own), bevy::mesh::skinning::SkinnedMesh { inverse_bindposes: at_rest.clone(), joints: vec![middle, left, right] }, Bent { brute, shoulders: [left, right], pose: Vec2::ZERO }));
        }
    }
}

/// Its body bent to its pose now: its shoulders' joints moved forward and
/// back (only when that changes).
pub(in crate::combat) fn bend(brutes: Query<&super::Impaled>, mut bents: Query<&mut Bent>, mut joints: Query<&mut Transform, Without<Bent>>) {
    for mut b in &mut bents {
        let pose = brutes.get(b.brute).map_or(Vec2::ZERO, |i| i.pose);
        if (pose - b.pose).abs().max_element() < 1e-4 {
            continue;
        }
        b.pose = pose;
        for (joint, forward) in b.shoulders.iter().zip([pose.x, pose.y]) {
            if let Ok(mut t) = joints.get_mut(*joint) {
                t.translation = Vec3::Z * forward;
            }
        }
    }
}
