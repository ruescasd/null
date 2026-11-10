//! The brute's attacks (the user, 2026-11-10): its crash down onto its spot
//! a blow to the ground round it, and, landed, lashes of the cables wound
//! about its waist (its arms are cables, wound round it there: the
//! original concept).
//!
//! The crash: it hangs a moment over its spot (the tell), lets go, and
//! where it lands the ground is struck: a ring sweeps out, and if you
//! stand within `QUAKE` you are hurt, more the nearer, and thrown off your
//! feet; in the air (a jump), you are spared.
//!
//! A lash: landed, you within its reach and in its sight, a bundle of
//! cables uncoils from its waist band on the side towards you (the sides
//! in turn) and curls back behind it (the tell), whips out to where you
//! were as it let fly, holds a moment and reels back in, sagging. Struck,
//! you are hurt, shoved along its way, and cry out; out of its reach, it
//! misses.

use bevy::prelude::*;

use super::Impaled;
use crate::rope::tubes;

/// The crash: how far round it the ground is struck (m), how much it hurts
/// at its middle (falling to none at its edge), how hard it throws you
/// (m/s, out and up), how long its ring takes to sweep out (s).
pub(super) const QUAKE: f32 = 6.0;
const QUAKE_HARM: f32 = 25.0;
const QUAKE_THROW: (f32, f32) = (6.0, 3.0);
const RING_TIME: f32 = 0.35;
/// A lash: how far it reaches (m, from its waist); how long it winds up,
/// strikes, holds, reels back, and rests before the next (s); how much it
/// hurts; how hard it shoves you (m/s, along it and up); its cables (how
/// many, how thick at its waist and at their tips, m; how dark).
pub(super) const REACH: f32 = 8.0;
const WIND: f32 = 0.45;
const STRIKE: f32 = 0.22;
const HOLD: f32 = 0.12;
const REEL: f32 = 0.45;
const REST: f32 = 0.6;
const LASH_HARM: f32 = 20.0;
const LASH_SHOVE: (f32, f32) = (5.0, 1.5);
const CABLES: usize = 3;
const THICK: (f32, f32) = (0.07, 0.025);
const SHADE: f32 = 0.05;
/// Where on its waist band the cables come from (m, its own frame at its
/// height, +x its left; the side towards you), and where they are cocked
/// before they strike.
const WAIST: Vec3 = Vec3::new(0.30, 0.16, 0.38);
const COCKED: Vec3 = Vec3::new(1.1, 0.7, -0.6);
/// You, struck: from below your eye to above it (m), and how near a cable
/// must come (m).
const BODY: (f32, f32, f32) = (1.3, 0.15, 0.45);

/// A lash: what it is doing, how long it has, which side, where it aims,
/// whether it has struck.
#[derive(Default)]
pub(super) struct Lash {
    pub phase: Phase,
    pub time: f32,
    side: f32,
    aim: Vec3,
    struck: bool,
    /// Its tip last frame (what it swept through since).
    tip: Option<Vec3>,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub(super) enum Phase {
    #[default]
    Rest,
    Wind,
    Strike,
    Hold,
    Reel,
}

/// What a landed brute's lash does this frame: begins when it can (you in
/// reach and sight, rested), runs through its phases; Some(the way it
/// struck you along) the frame it does.
pub(super) fn lash(b: &mut Impaled, you: Vec3, seen: bool, dt: f32) -> Option<Vec3> {
    let (pos, rot) = b.flight.drawn();
    let l = &mut b.lash;
    l.time += dt;
    let reach = Vec2::new(you.x - pos.x, you.z - pos.z).length() < REACH;
    match l.phase {
        Phase::Rest if l.time > REST && reach && seen => {
            l.phase = Phase::Wind;
            l.time = 0.0;
            l.side = if l.side > 0.0 { -1.0 } else { 1.0 };
            l.struck = false;
            l.tip = None;
        }
        Phase::Wind if l.time > WIND => {
            // (It lets fly at where you are now: out of reach, as far as it
            // reaches towards you.)
            let from = waist(pos, rot, l.side);
            let chest = you - Vec3::Y * 0.5;
            l.aim = from + (chest - from).clamp_length_max(REACH);
            l.phase = Phase::Strike;
            l.time = 0.0;
        }
        Phase::Strike if l.time > STRIKE => {
            l.phase = Phase::Hold;
            l.time = 0.0;
        }
        Phase::Hold if l.time > HOLD => {
            l.phase = Phase::Reel;
            l.time = 0.0;
        }
        Phase::Reel if l.time > REEL => {
            l.phase = Phase::Rest;
            l.time = 0.0;
        }
        _ => {}
    }
    // (Striking or held out: what its cable swept through since last frame
    // touches you, once.)
    if matches!(l.phase, Phase::Strike | Phase::Hold) && !l.struck {
        let points = cable(pos, rot, l, 0);
        let tip = *points.last().unwrap_or(&pos);
        let (feet, head) = (you - Vec3::Y * BODY.0, you + Vec3::Y * BODY.1);
        let swept = l.tip.is_some_and(|t| crate::combat::bishop::between(t, tip, feet, head) < BODY.2);
        let along = points.windows(2).any(|w| crate::combat::bishop::between(w[0], w[1], feet, head) < BODY.2);
        l.tip = Some(tip);
        if swept || along {
            l.struck = true;
            return Some((tip - waist(pos, rot, l.side)).normalize_or(Vec3::Z));
        }
    }
    None
}

/// Its lash put away (it took off, or died).
pub(super) fn stow(b: &mut Impaled) {
    b.lash = Lash { side: b.lash.side, ..default() };
}

/// Where its cables come from, on `side` of its waist band.
fn waist(pos: Vec3, rot: Quat, side: f32) -> Vec3 {
    pos + rot * Vec3::new(WAIST.x * side, WAIST.y, WAIST.z)
}

/// The `k`th cable of a lash as it is now: from its waist to its tip along
/// a bowed curve, whipping as it strikes, sagging as it reels in.
fn cable(pos: Vec3, rot: Quat, l: &Lash, k: usize) -> Vec<Vec3> {
    let from = waist(pos, rot, l.side) + rot * Vec3::new(0.0, (k as f32 - 1.0) * 0.07, 0.0);
    let cocked = pos + rot * Vec3::new(COCKED.x * l.side, COCKED.y, COCKED.z);
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    // (Its tip, and how far out its middle bows, sideways and down.)
    let (tip, bow, sag) = match l.phase {
        Phase::Rest => return Vec::new(),
        Phase::Wind => {
            let t = ease((l.time / WIND).min(1.0));
            (from.lerp(cocked, t), 0.25 * t, 0.0)
        }
        Phase::Strike => {
            // (Faster and faster: a whip.)
            let t = (l.time / STRIKE).min(1.0);
            (cocked.lerp(l.aim, t * t), 0.35 * (1.0 - t * t), 0.0)
        }
        Phase::Hold => (l.aim, 0.0, 0.15),
        Phase::Reel => {
            let t = ease((l.time / REEL).min(1.0));
            (l.aim.lerp(from, t), 0.0, 0.6 * (1.0 - t) * t.sqrt() + 0.15 * (1.0 - t))
        }
    };
    let span = tip - from;
    let length = span.length().max(0.05);
    let across = rot * Vec3::X * l.side;
    let n = 18;
    (0..=n)
        .map(|i| {
            let s = i as f32 / n as f32;
            let hump = 4.0 * s * (1.0 - s);
            // (Each cable a little apart, and a wave running out along it.)
            let wave = (s * 9.0 - l.time * 30.0 + k as f32 * 1.7).sin() * 0.06 * hump;
            from + span * s + across * (bow * length * hump + wave) - Vec3::Y * (sag * length * hump) + rot * Vec3::new(0.0, (k as f32 - 1.0) * 0.05 * (1.0 - s), 0.0)
        })
        .collect()
}

/// A brute's lash, drawn: one mesh of its cables.
#[derive(Component)]
pub(in crate::combat) struct Cables {
    brute: Entity,
    mesh: Handle<Mesh>,
}

/// Its cables' mesh and look, made with it.
pub(super) fn make(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, brute: Entity) {
    let mesh = meshes.add(super::field::placeholder());
    let look = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.3, reflectance: 0.6, ..default() });
    commands.spawn((Cables { brute, mesh: mesh.clone() }, Mesh3d(mesh), MeshMaterial3d(look), bevy::camera::visibility::NoFrustumCulling, Transform::default(), Visibility::Hidden));
}

/// The lashes drawn this frame; the cables of brutes gone, gone.
pub(in crate::combat) fn draw(mut commands: Commands, brutes: Query<&Impaled>, mut cables: Query<(Entity, &Cables, &mut Visibility)>, mut meshes: ResMut<Assets<Mesh>>) {
    for (entity, c, mut visible) in &mut cables {
        let Ok(b) = brutes.get(c.brute) else {
            commands.entity(entity).despawn();
            continue;
        };
        // (Put away, hidden, its mesh left as it was: none to write.)
        if b.lash.phase == Phase::Rest {
            *visible = Visibility::Hidden;
            continue;
        }
        *visible = Visibility::Inherited;
        let (pos, rot) = b.flight.drawn();
        let strands: Vec<Vec<Vec3>> = (0..CABLES).map(|k| cable(pos, rot, &b.lash, k)).collect();
        if let Some(mut mesh) = meshes.get_mut(&c.mesh) {
            tubes(&mut mesh, strands.iter().map(|p| (&p[..], THICK, SHADE)));
        }
    }
}

/// The crash's ring sweeping out over the ground, and fading.
#[derive(Component)]
pub(in crate::combat) struct Ring {
    age: f32,
}

/// The crash strikes the ground at `at`: its ring; and you, standing within
/// it on the ground, hurt by how near, thrown; Some(how much) if so.
pub(super) fn quake(commands: &mut Commands, ring: Handle<Mesh>, materials: &mut Assets<StandardMaterial>, at: Vec3, you: Vec3, grounded: bool) -> Option<(f32, Vec3)> {
    let look = materials.add(StandardMaterial { base_color: Color::srgba(0.85, 0.88, 0.95, 0.8), unlit: true, alpha_mode: AlphaMode::Add, depth_bias: 2.0, ..default() });
    commands.spawn((Ring { age: 0.0 }, Mesh3d(ring), MeshMaterial3d(look), bevy::light::NotShadowCaster, Transform::from_translation(at + Vec3::Y * 0.04).with_scale(Vec3::splat(0.5))));
    let off = Vec2::new(you.x - at.x, you.z - at.z);
    let near = 1.0 - off.length() / QUAKE;
    if !grounded || near <= 0.0 {
        return None;
    }
    let out = Vec3::new(off.x, 0.0, off.y).normalize_or(Vec3::X);
    Some((QUAKE_HARM * near.min(1.0), out * (QUAKE_THROW.0 * near) + Vec3::Y * QUAKE_THROW.1))
}

/// Rings swept out and faded.
pub(in crate::combat) fn rings(mut commands: Commands, time: Res<Time>, mut rings: Query<(Entity, &mut Ring, &mut Transform, &MeshMaterial3d<StandardMaterial>)>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for (entity, mut r, mut t, m) in &mut rings {
        r.age += time.delta_secs();
        let k = r.age / RING_TIME;
        if k >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // (Its mesh one metre across: out to the crash's edge.)
        let wide = 0.5 + (2.0 * QUAKE - 0.5) * (1.0 - (1.0 - k) * (1.0 - k));
        t.scale = Vec3::new(wide, 1.0, wide);
        if let Some(mut m) = materials.get_mut(&m.0) {
            m.base_color = Color::srgba(0.85, 0.88, 0.95, 0.8 * (1.0 - k));
        }
    }
}

/// How much a lash or the crash hurts, and how it shoves you (for the
/// shared reckoning in `fly`).
pub(super) fn lash_blow(along: Vec3) -> (f32, Vec3) {
    (LASH_HARM, along.with_y(0.0).normalize_or(Vec3::Z) * LASH_SHOVE.0 + Vec3::Y * LASH_SHOVE.1)
}
