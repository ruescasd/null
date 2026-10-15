//! The cloth lab: whether cloth can be done properly (the user: we don't
//! know if cloth matters till we know it can be; a solid attempt). With
//! `--opt clothlab`, two sheets dropped again and again on a table, one
//! over a ball, one over a box: they should fall into clean folds, over
//! themselves and never through, and lie still. With `--opt cape` (and
//! `--opt porter`), the porter wears a cape, live: held along its
//! shoulders, pinned under its load, falling from them under its own weight
//! as it walks (bound to its body, a cloak pitched with its hunched torso
//! and crumpled up over its head). Each logs what it costs, and how much it
//! still moves.

use std::time::Instant;

use bevy::prelude::*;

use crate::{
    Args,
    camera::FlyCam,
    fabric::{Body, Fabric, Weave},
    rig::Rig,
    terrain::Streamer,
};

/// The sheets: points a side, how far apart (m); dropped again every so
/// often (s).
const SHEET: usize = 38;
const SPACING: f32 = 0.04;
const DROP: f32 = 8.0;

/// A sheet in the lab: its cloth, what it falls on, its table, where it
/// starts; since it was dropped (s); what it has cost since (s of stepping,
/// frames, the worst frame).
#[derive(Component)]
pub(super) struct Sheet {
    fabric: Fabric,
    bodies: Vec<Body>,
    floor: f32,
    start: Vec<Vec3>,
    since: f32,
    cost: (f32, u32, f32),
}

/// A plain matte cloth, seen from both sides.
fn cloth_material(materials: &mut Assets<StandardMaterial>, shade: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: shade, perceptual_roughness: 0.95, reflectance: 0.2, double_sided: true, cull_mode: None, ..default() })
}

/// The sheets: set up in front of you once the world round you has loaded;
/// stepped every frame, dropped again every `DROP` s.
#[allow(clippy::too_many_arguments)]
pub(super) fn sheets(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    streamer: Res<Streamer>,
    camera: Single<&Transform, With<FlyCam>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sheets: Query<(&mut Sheet, &Mesh3d, &mut Transform), Without<FlyCam>>,
    mut settled_for: Local<f32>,
    mut spawned: Local<bool>,
) {
    if !args.opt("clothlab") {
        return;
    }
    let dt = time.delta_secs().min(1.0 / 30.0);
    if !*spawned {
        *settled_for = if streamer.near { *settled_for + time.delta_secs() } else { 0.0 };
        if *settled_for < 1.0 {
            return;
        }
        *spawned = true;
        let eye = camera.translation;
        let ahead = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::NEG_Z);
        let across = Vec3::new(-ahead.z, 0.0, ahead.x);
        let centre = eye + ahead * 2.6;
        let top = eye.y - 0.75;
        let stone = materials.add(StandardMaterial { base_color: Color::srgb(0.08, 0.08, 0.09), perceptual_roughness: 0.8, ..default() });
        let turn = Quat::from_rotation_arc(Vec3::NEG_Z, ahead);
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(4.0, 0.1, 2.0))), MeshMaterial3d(stone.clone()), Transform::from_translation(Vec3::new(centre.x, top - 0.05, centre.z)).with_rotation(turn)));
        let ball = (centre - across * 0.95, 0.3);
        let block = (centre + across * 0.95, Vec3::new(0.3, 0.22, 0.3));
        commands.spawn((Mesh3d(meshes.add(Sphere::new(ball.1 - 0.005).mesh().uv(48, 24))), MeshMaterial3d(stone.clone()), Transform::from_translation(Vec3::new(ball.0.x, top + ball.1, ball.0.z))));
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(block.1 * 2.0 - 0.01))), MeshMaterial3d(stone), Transform::from_translation(Vec3::new(block.0.x, top + block.1.y, block.0.z)).with_rotation(turn)));
        let shade = cloth_material(&mut materials, Color::srgb(0.45, 0.16, 0.1));
        for (under, body) in [(Vec3::new(ball.0.x, top + ball.1, ball.0.z), Body::Capsule(Vec3::ZERO, Vec3::ZERO, ball.1)), (Vec3::new(block.0.x, top + block.1.y, block.0.z), Body::Box(Vec3::ZERO, turn, block.1))] {
            let body = match body {
                Body::Capsule(_, _, r) => Body::Capsule(under, under, r),
                Body::Box(_, q, h) => Body::Box(under, q, h),
            };
            // (Turned a little, held a little off level: not dropped dead
            // square on it.)
            let tilt = Quat::from_rotation_y(0.3) * Quat::from_rotation_x(0.08);
            let half = (SHEET - 1) as f32 * SPACING * 0.5;
            let at = |c: usize, r: usize| under + Vec3::Y * 0.75 + tilt * Vec3::new(c as f32 * SPACING - half, 0.0, r as f32 * SPACING - half);
            let fabric = Fabric::sheet(SHEET, SHEET, at, Weave { thickness: SPACING * 0.8, ..Weave::default() });
            let start: Vec<Vec3> = (0..fabric.len()).map(|i| fabric.point(i)).collect();
            let mesh = meshes.add(fabric.mesh());
            let fabric_origin = fabric.origin();
            commands.spawn((Sheet { fabric, bodies: vec![body], floor: top, start, since: 0.0, cost: (0.0, 0, 0.0) }, Mesh3d(mesh), MeshMaterial3d(shade.clone()), Transform::from_translation(fabric_origin), bevy::camera::visibility::NoFrustumCulling));
        }
        return;
    }
    for (mut sheet, mesh, mut place) in &mut sheets {
        let sheet = &mut *sheet;
        sheet.since += dt;
        if sheet.since > DROP {
            let (spent, frames, worst) = sheet.cost;
            info!("clothlab: a sheet of {} points: {:.3} ms a frame, the worst {:.3} ms; moving at {:.3} m/s on average", sheet.start.len(), spent * 1000.0 / frames.max(1) as f32, worst * 1000.0, sheet.fabric.stir());
            sheet.since = 0.0;
            sheet.cost = (0.0, 0, 0.0);
            let start = sheet.start.clone();
            sheet.fabric.set(|i| start[i]);
        }
        let begun = Instant::now();
        sheet.fabric.step(&[], &sheet.bodies, sheet.floor, dt);
        let took = begun.elapsed().as_secs_f32();
        sheet.cost = (sheet.cost.0 + took, sheet.cost.1 + 1, sheet.cost.2.max(took));
        place.translation = sheet.fabric.origin();
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            sheet.fabric.draw(&mut m);
        }
    }
}

/// The cape: points across it and down it; how long it is, down its back
/// from its collar (m; `--set cape_length`: short enough to show its legs
/// at their struggle, the user: 1.45 m; 1.575 first); how far
/// round the body it goes (radians, each side of its back: at its throat,
/// all but its clasp's gap at the front; at its hem, open wider); how much
/// further out than the body's pieces it is made, and how much wider at its
/// hem; how many rows it takes to widen from round its neck to round its
/// shoulders; how far its collar stands off the neck, and how far up it;
/// how far under its load is off the load (m); how long it hangs, still,
/// before it is shown (frames of 1/60 s).
const ACROSS: usize = 28;
const DOWN: usize = 22;
const LENGTH: f32 = 1.45;
const SPAN: (f32, f32) = (3.04, 2.3);
const LOOSE: f32 = 0.04;
const BELL: f32 = 0.5;
const YOKE: f32 = 3.0;
const COLLAR: f32 = 0.03;
const NAPE: f32 = 0.06;
const UNDER: f32 = 0.015;
const SETTLE: usize = 150;
/// How much bigger the body is than its bones (its pieces stand out from
/// them; times, and m).
const BULK: (f32, f32) = (1.5, 0.04);

/// What a held point of the cloak is held by.
#[derive(Clone, Copy)]
enum Hold {
    Load,
    Bone(usize),
}

/// A part's frame now: where, turned how.
fn frame(rig: &Rig, hold: Hold) -> (Vec3, Quat) {
    match hold {
        Hold::Load => rig.load().map_or((rig.root, Quat::IDENTITY), |(at, turn, _)| (at, turn)),
        Hold::Bone(i) => (rig.bones[i].a, rig.bones[i].rotation()),
    }
}

/// What the cloak hangs on: the body as it is seen (its bones made out to
/// its pieces), and the load (always as many, in the same order).
fn bodies(rig: &Rig) -> Vec<Body> {
    let n = 5 + 2 * rig.plan.legs.len() + 2 * rig.plan.arms.len();
    let mut out: Vec<Body> = rig.bones.iter().take(n).map(|b| Body::Capsule(b.a, b.b, b.radius * BULK.0 + BULK.1)).collect();
    if let Some((at, turn, size)) = rig.load() {
        out.push(Body::Box(at, turn, size * 0.5));
    }
    out
}

/// A porter's cloak: whose; its cloth; its held points (which, by what,
/// where in its frame); what it has cost (s of stepping, frames, the
/// worst), since when (s).
#[derive(Component)]
pub(super) struct Cloak {
    hunter: Entity,
    fabric: Fabric,
    held: Vec<(u32, Hold, Vec3)>,
    cost: (f32, u32, f32),
    since: f32,
}

/// The cape made on the porter as it stands, settled: a sheet fastened
/// round its throat, at the front (the user, with a reference: fastened
/// there, it covers the shoulders and the top of the chest), over its
/// shoulders, round its back and sides, open down its front, wider open
/// lower down, passing under its load (pinned there by its
/// weight), falling past its hips; cut flat. (A cape first, the user:
/// before it, a hooded cloak, whose hood was its own trouble. Held all along
/// its top round the shoulders, it stood off the body, a hollow half
/// cylinder; pinned where it started under the load, inside it, it went
/// through it.)
fn make(rig: &Rig, length: f32) -> Option<(Fabric, Vec<(u32, Hold, Vec3)>)> {
    let row = length / (DOWN - 1) as f32;
    let (load_at, load_turn, load_size) = rig.load()?;
    let b = &rig.bones;
    let (chest, rump) = (b[2].b, b[0].a);
    let down = (rump - chest).normalize_or(Vec3::NEG_Y);
    let ahead = Vec3::new(rig.heading.sin(), 0.0, rig.heading.cos());
    let front = b[2].side.cross(down).normalize_or(ahead);
    let front = if front.dot(ahead) < 0.0 { -front } else { front };
    let side = (-down).cross(front).normalize_or(Vec3::X);
    let girth = b[2].radius * BULK.0 + BULK.1 + LOOSE;
    let floor = rig.root.y + 0.05;
    // Its collar close round its neck, square to it, a little up it, its
    // clasp at its throat (made round its body's own bulk and square to
    // its torso, it was a wide flat ring the neck left forward out of: a
    // hole between its shoulders, the user).
    let neck = (b[3].b - b[3].a).normalize_or(-down);
    let collar = b[3].radius + COLLAR;
    let throat = (front - Vec3::Y - neck * (front - Vec3::Y).dot(neck)).normalize_or(front);
    let round = neck.cross(throat);
    let nape = b[3].a + neck * NAPE;
    // Row 0 its collar; over its first rows, out over its shoulders to
    // round its body; each row lower, wider.
    let at = |c: usize, r: usize| {
        let span = SPAN.0 + (SPAN.1 - SPAN.0) * r as f32 / (DOWN - 1) as f32;
        let a = std::f32::consts::PI + span * (2.0 * c as f32 / (ACROSS - 1) as f32 - 1.0);
        let radius = girth * (1.0 + BELL * r as f32 / DOWN as f32);
        let body = chest + down * r as f32 * row + front * a.cos() * radius + side * a.sin() * radius;
        let t = (r as f32 / YOKE).min(1.0);
        (nape + throat * a.cos() * collar + round * a.sin() * collar).lerp(body, t * (2.0 - t))
    };
    // Under its load: on the load's underside, between it and the back,
    // to its edges (and a row past its ends, to them: left free there, it
    // was pushed out the nearer face, the load's back, and lay over it).
    let half = load_size * 0.5;
    let footprint = |p: Vec3| {
        let local = load_turn.inverse() * (p - load_at);
        (local.x.abs() < half.x && local.y.abs() < half.y + row && local.z < half.z).then(|| load_at + load_turn * Vec3::new(local.x, local.y.clamp(-half.y, half.y), half.z + UNDER))
    };
    let points: Vec<Vec3> = (0..DOWN)
        .flat_map(|r| (0..ACROSS).map(move |c| (c, r)))
        .map(|(c, r)| {
            let p = at(c, r);
            let p = footprint(p).unwrap_or(p);
            Vec3::new(p.x, p.y.max(floor + 0.05), p.z)
        })
        .collect();
    let mut fabric = Fabric::sheet(ACROSS, DOWN, |c, r| points[r * ACROSS + c], Weave { thickness: 0.05, ..Weave::default() });
    // (Cut flat, as cloth is: made round, it kept itself a tube, the user.)
    fabric.cut_flat();
    // Held: round its throat (by its chest), and under its load.
    let held: Vec<(u32, Hold, Vec3)> = (0..fabric.len())
        .filter_map(|i| {
            let hold = if footprint(at(i % ACROSS, i / ACROSS)).is_some() {
                Hold::Load
            } else if i < ACROSS {
                Hold::Bone(2)
            } else {
                return None;
            };
            let (at, turn) = frame(rig, hold);
            Some((i as u32, hold, turn.inverse() * (fabric.point(i) - at)))
        })
        .collect();
    fabric.tether(&held.iter().map(|h| h.0).collect::<Vec<_>>(), 1.02);
    let targets = targets(rig, &held);
    let bodies = bodies(rig);
    for _ in 0..SETTLE {
        fabric.step(&targets, &bodies, rig.root.y, 1.0 / 60.0);
    }
    Some((fabric, held))
}

/// Where its held points are held now.
fn targets(rig: &Rig, held: &[(u32, Hold, Vec3)]) -> Vec<(u32, Vec3)> {
    held.iter()
        .map(|&(i, h, local)| {
            let (at, turn) = frame(rig, h);
            (i, at + turn * local)
        })
        .collect()
}

/// With `--opt cape`, each porter wears a cape (made as it first stands),
/// stepped with it every frame; a porter gone, its cape is gone.
#[allow(clippy::too_many_arguments)]
pub(super) fn cloaks(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    hunters: Query<(Entity, &super::hunter::Hunter)>,
    mut cloaks: Query<(Entity, &mut Cloak, &Mesh3d, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut worn: Local<Vec<Entity>>,
    mut shade: Local<Option<Handle<StandardMaterial>>>,
) {
    if !args.opt("cape") {
        return;
    }
    let dt = time.delta_secs().min(1.0 / 30.0);
    // (Worn, matte, a dark warm grey: a thing its bearer has used.)
    let shade = shade.get_or_insert_with(|| cloth_material(&mut materials, Color::srgb(0.24, 0.22, 0.2))).clone();
    for (entity, h) in &hunters {
        if worn.contains(&entity) || h.rig().load().is_none() || h.rig().bones.len() < 5 {
            continue;
        }
        worn.push(entity);
        if let Some((fabric, held)) = make(h.rig(), args.num("cape_length", LENGTH)) {
            let mesh = meshes.add(fabric.mesh());
            let origin = fabric.origin();
            commands.spawn((Cloak { hunter: entity, fabric, held, cost: (0.0, 0, 0.0), since: 0.0 }, Mesh3d(mesh), MeshMaterial3d(shade.clone()), Transform::from_translation(origin), bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster));
        }
    }
    for (entity, mut cloak, mesh, mut place) in &mut cloaks {
        let Ok((_, h)) = hunters.get(cloak.hunter) else {
            commands.entity(entity).despawn();
            continue;
        };
        let cloak = &mut *cloak;
        let rig = h.rig();
        let targets = targets(rig, &cloak.held);
        let begun = Instant::now();
        cloak.fabric.step(&targets, &bodies(rig), rig.root.y, dt);
        let took = begun.elapsed().as_secs_f32();
        cloak.cost = (cloak.cost.0 + took, cloak.cost.1 + 1, cloak.cost.2.max(took));
        cloak.since += dt;
        if cloak.since > 4.0 {
            let (spent, frames, worst) = cloak.cost;
            info!("cape: {} points: {:.3} ms a frame, the worst {:.3} ms; moving at {:.3} m/s on average", cloak.fabric.len(), spent * 1000.0 / frames.max(1) as f32, worst * 1000.0, cloak.fabric.stir());
            cloak.cost = (0.0, 0, 0.0);
            cloak.since = 0.0;
        }
        place.translation = cloak.fabric.origin();
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            cloak.fabric.draw(&mut m);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Plan;

    /// On a porter standing still, out in the world, its cape comes to
    /// rest, though its frames are never quite the same length (stepped by
    /// the frame, it never did: the user saw a wind).
    #[test]
    fn cloak_rests() {
        let rig = Rig::new(Plan::porter(), Vec3::new(1200.0, 30.0, 900.0), 0.4, 1, &|_, _| 30.0);
        let (mut fabric, held) = make(&rig, LENGTH).expect("the porter has a load");
        let (held, bodies) = (targets(&rig, &held), bodies(&rig));
        let mut moving = 0.0;
        for k in 0..1200 {
            // (Frames from 1/80 s to 1/48 s.)
            let dt = (1.0 / 60.0) * (0.75 + 0.5 * ((k as f32 * 12.9898).sin() * 43758.547).fract().abs());
            let was: Vec<Vec3> = (0..fabric.len()).map(|i| fabric.point(i)).collect();
            fabric.step(&held, &bodies, rig.root.y, dt);
            if k >= 900 {
                moving += (0..fabric.len()).map(|i| fabric.point(i).distance(was[i]) / dt).sum::<f32>() / fabric.len() as f32 / 300.0;
            }
        }
        assert!(moving < 0.01, "its cape still moving at {moving} m/s on average");
    }

    /// Its cape passes under its load, standing and walking: none of it
    /// stays in the load or over it (pinned where it started, it went
    /// through it, the user). (Short, its hem below the load flaps over the
    /// load's lower end at a step, and falls off it: for less than a
    /// second, not lying there; and where it folds round its edges, to 5
    /// cm, not at all.)
    #[test]
    fn under_load() {
        const EDGE: f32 = 0.05;
        let mut rig = Rig::new(Plan::porter(), Vec3::new(1200.0, 30.0, 900.0), 0.4, 1, &|_, _| 30.0);
        let (mut fabric, held) = make(&rig, LENGTH).expect("the porter has a load");
        let (mut over, mut longest) = (0, 0);
        for k in 0..900 {
            // (Standing 2 s, then walking at its pace.)
            if k >= 120 {
                let ahead = Vec3::new(rig.heading.sin(), 0.0, rig.heading.cos());
                let intent = crate::rig::Intent { velocity: ahead * 0.7, look: rig.root + ahead * 5.0 + Vec3::Y * 1.5, ..Default::default() };
                rig.update(1.0 / 60.0, intent, &|_, _| 30.0, &|from, step| from + step);
            }
            fabric.step(&targets(&rig, &held), &bodies(&rig), rig.root.y, 1.0 / 60.0);
            if k < 60 {
                continue;
            }
            let (at, turn, size) = rig.load().expect("the porter has a load");
            let through = (0..fabric.len())
                .map(|i| turn.inverse() * (fabric.point(i) - at))
                .filter(|l| l.x.abs() < size.x * 0.5 - EDGE && l.y.abs() < size.y * 0.5 - EDGE && l.z < size.z * 0.5 - EDGE)
                .count();
            over = if through > 0 { over + 1 } else { 0 };
            longest = longest.max(over);
        }
        assert!(longest < 60, "some of it in its load or over it for {longest} frames on end");
    }
}
