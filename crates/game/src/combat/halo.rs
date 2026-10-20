//! The stalker's warning that it is about to fire (the user's idea): round
//! its head, centred where its eyes would be, an arch, more than a half
//! circle, open below, shimmering; clear of its head, not over it. Square to
//! where it looks, as a halo: aimed at you, you see it whole. The niche's
//! shimmer (`chasm::rooms::shimmer`), brighter: two layers of fine veins
//! drifting their own ways, wavering; gathering as it holds its aim, and
//! when full, it fires (see `beam.rs`), and the arch goes out.
//!
//! And its searchlight (the user's idea): hunting, its gun up, a ray along
//! its gun, which tracks you, not quite steady (see `hunter.rs`), on till it meets
//! the ground (ending at you, it felt off, the user). (A ring where it
//! landed served no purpose, the user.) `--set searchlight`: 0 pale, as
//! the trail that lingered after a shot did; 1 black. (Faint and pale with
//! a dark edge, it was hard to see, the user.) Dark at its lock (your cue to
//! move), back on after its shot: whence it came, after the fact.

use bevy::{asset::RenderAssetUsages, math::Affine2, mesh::PrimitiveTopology, prelude::*};

use super::hunter::Hunter;
use crate::{Args, rig::GUN_BORE};

/// The arch, in the head bone's frame (x right, y where it looks, z down):
/// its middle up from the neck's end (m, where its eyes would be), its
/// inner and outer radius (clear of the skull), and from and to which
/// angle (radians from its right, up: a little past a half circle).
const MIDDLE: f32 = 0.24;
const RADII: (f32, f32) = (0.2, 0.33);
const SPAN: (f32, f32) = (-0.35, std::f32::consts::PI + 0.35);
/// How bright its veins at full charge, and the metres its veins' tile
/// covers.
const GLOW: f32 = 18.0;
const TILE_M: f32 = 1.2;
/// How quickly it gathers as it holds its aim, and fades as it lowers it
/// (of full a second); how long before it fires its aim locks (s: where you
/// were then, it fires; move, and it lands behind you), its arch flaring
/// (how much brighter).
const GATHER: (f32, f32) = (0.6, 3.0);
const LOCK: (f32, f32) = (0.5, 1.8);
/// In close combat its lock that much shorter (s): near, it fires where you
/// are, much more accurate (the user).
const LOCK_CLOSE: f32 = 0.2;
/// Its shot's sound begins this long before it fires (s: a sound with a
/// charge before its bang, as long as its charge; `--set shot_lead`, to
/// tune it).
const SHOT_LEAD: f32 = 0.01;
/// Its searchlight: pale, how wide (m) and bright; black, how wide and how
/// dark (0..1); and how long after its shot it comes back on (s: as the beam is
/// gone). (Where on you: `hunter::WANDER`, its gun's too.)
const PALE: (f32, f32) = (0.35, 0.36);
const BLACK: (f32, f32) = (0.12, 0.85);
const BACK_ON: f32 = 0.7;
/// After it fires, how long before it gathers again, its aim held (s;
/// standing near, it fired once only, never let go of its aim, the user's
/// captures).
const AGAIN: f32 = 2.8;

/// The arch's mesh and its two layers' veins, built once.
#[derive(Resource, Clone)]
pub(super) struct Kit {
    arch: Handle<Mesh>,
    veins: [Handle<Image>; 2],
}

/// A stalker's arch: whose, its two layers' finishes, how charged it is,
/// whether it has fired since it took its aim, and a phase for its
/// wavering.
#[derive(Component)]
pub(super) struct Halo {
    hunter: Entity,
    layers: [Handle<StandardMaterial>; 2],
    charge: f32,
    fired: bool,
    since: f32,
    /// Its shot's sound begun, this gathering.
    sounded: bool,
    phase: f32,
    /// Its searchlight, made as it first hunts.
    light: Option<Entity>,
}

/// The arch: a band of quads round its middle, faded at its edges and ends
/// (by its vertex colours, which its finish multiplies).
pub(super) fn build(meshes: &mut Assets<Mesh>, images: &mut Assets<Image>) -> Kit {
    let n = 48;
    let (mut positions, mut uvs, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for k in 0..=n {
        let s = k as f32 / n as f32;
        let a = SPAN.0 + (SPAN.1 - SPAN.0) * s;
        // (Never below zero: at its far end the sine came out a hair under,
        // its power not a number, and the arch's last stretch drew black,
        // a stray bar beside its head against the comet's pale sky, the
        // user.)
        let ends = (s * std::f32::consts::PI).sin().max(0.0).powf(0.6);
        for (j, (r, fade)) in [(RADII.0, 0.0), ((RADII.0 + RADII.1) * 0.5, 1.0), (RADII.1, 0.0)].into_iter().enumerate() {
            let p = Vec3::new(a.cos() * r, 0.0, -MIDDLE - a.sin() * r);
            positions.push(p.to_array());
            uvs.push([p.x / TILE_M, p.z / TILE_M]);
            let c = fade * ends;
            colors.push([c, c, c, 1.0]);
            if k < n && j < 2 {
                let i = (k * 3 + j) as u32;
                indices.extend_from_slice(&[i, i + 3, i + 1, i + 1, i + 3, i + 4]);
            }
        }
    }
    let arch = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; positions.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(bevy::mesh::Indices::U32(indices));
    Kit { arch: meshes.add(arch), veins: [0x4a1, 0x4a2].map(|seed| images.add(crate::chasm::rooms::shimmer::veins(seed))) }
}

/// The arch's two layers on a stalker's head (bone 4), unlit, added to what
/// is behind them, dark till it charges.
pub(super) fn spawn(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, kit: &Kit, hunter: Entity, at: Vec3, part: impl Fn() -> super::hunter::Part) {
    let layers = kit.veins.clone().map(|veins| {
        materials.add(StandardMaterial {
            base_color: Color::BLACK,
            base_color_texture: Some(veins),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            double_sided: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        })
    });
    for layer in &layers {
        commands.spawn((part(), Mesh3d(kit.arch.clone()), MeshMaterial3d(layer.clone()), Transform::from_translation(at), bevy::light::NotShadowCaster));
    }
    commands.spawn(Halo { hunter, layers, charge: 0.0, fired: false, since: 0.0, sounded: false, phase: (hunter.index_u32() % 97) as f32, light: None });
}

/// The veins' light at time `t`, for a layer of phase `q`: swells and
/// quick shivers, quicker than the niche's (the arch's, and a dome's).
pub(super) fn waver(t: f32, q: f32) -> f32 {
    let swell = 0.6 + 0.4 * ((t * 1.3 + q).sin() * 0.6 + (t * 0.47 + q * 1.3).sin() * 0.4);
    let shiver = 1.0 + 0.4 * (t * 17.0 + q).sin() * (t * 0.9 + q).sin().max(0.0).powi(2);
    swell * shiver
}

/// Each arch gathering as its stalker holds its gun aimed, firing when
/// full, and going out; fading as it lowers it; its veins drifting and
/// wavering.
#[allow(clippy::too_many_arguments)]
pub(super) fn animate(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    shot: Res<super::beam::Kit>,
    strikes: Res<super::strike::Kit>,
    layers: Res<super::envelop::Layers>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rays: ResMut<Assets<super::beam::Ray>>,
    mut hunters: Query<&mut Hunter>,
    mut halos: Query<(Entity, &mut Halo)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Single<&Transform, With<crate::camera::FlyCam>>,
) {
    let (t, dt) = (time.elapsed_secs(), time.delta_secs().min(0.05));
    for (entity, mut halo) in &mut halos {
        let Ok(mut h) = hunters.get_mut(halo.hunter) else {
            if let Some(e) = halo.light {
                commands.entity(e).despawn();
            }
            commands.entity(entity).despawn();
            continue;
        };
        let (frame, hold, speed) = (h.rig().gun_frame, h.rig().gun, h.rig().speed());
        // (Gathering only as it stands, its aim held: walking on, it may
        // gather again when it stops.)
        let aimed = frame.is_some_and(|f| f.aim > 0.98) && speed < 0.15 && h.may_fire();
        if aimed && !halo.fired {
            halo.charge = (halo.charge + GATHER.0 * dt).min(1.0);
            // Its shot's sound, begun as long before it fires as its charge
            // runs before its bang.
            if !halo.sounded && (1.0 - halo.charge) / GATHER.0 <= args.num("shot_lead", SHOT_LEAD) {
                halo.sounded = true;
                let far = frame.map_or(0.0, |f| f.grip.distance(camera.translation));
                commands.spawn((AudioPlayer::new(strikes.shot.clone()), PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(super::strike::loudness(super::strike::LOUD.0, far)))));
            }
            // Nearly full, its aim locks; full, it fires, from its muzzle
            // along its aim.
            let lock = LOCK.0 + (LOCK_CLOSE - LOCK.0) * h.rig().combat();
            let locking = halo.charge >= 1.0 - lock * GATHER.0;
            if let (1.0, Some(f), Some(_)) = (halo.charge, frame, hold) {
                let emitter = super::lance::emitter(args.num("lance", 0.0) as u32);
                super::beam::fire(&mut commands, &mut rays, &mut meshes, &mut materials, &shot, &strikes, &layers, &args, &ground, f.grip + f.up * GUN_BORE + f.axis * emitter, f.axis, (halo.hunter, emitter));
                halo.fired = true;
                halo.since = 0.0;
                // (Its aim let go at once: it carries on as it was, its zone
                // still standing, the user; held till it moved on, it stood
                // aiming at nothing.)
                h.lock(false);
            } else {
                h.lock(locking);
            }
        } else {
            // (Fired, it gathers again after `AGAIN`, or once it lowers its
            // gun.)
            halo.charge = (halo.charge - GATHER.1 * dt).max(0.0);
            halo.since += dt;
            if !aimed || halo.since > AGAIN {
                halo.fired = false;
                halo.sounded = false;
                h.lock(false);
            }
        }
        // Its searchlight: from its gun onto where it aims, while it hunts,
        // its gun up; dark from its lock till its shot is gone.
        let emitter = super::lance::emitter(args.num("lance", 0.0) as u32);
        let on = frame.filter(|f| f.aim > 0.98 && h.hunting() && !h.locked() && !(halo.fired && halo.since < BACK_ON));
        let black = args.num("searchlight", 0.0) > 0.5;
        let ray = *halo.light.get_or_insert_with(|| {
            let finish = if black { super::beam::Ray::dark(BLACK.0, BLACK.1) } else { super::beam::Ray::new(PALE.0, PALE.1) };
            commands.spawn((Mesh3d(shot.0.clone()), MeshMaterial3d(rays.add(finish)), Transform::default(), Visibility::Hidden, bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster, bevy::light::NotShadowReceiver)).id()
        });
        let shown = if on.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if let Some(f) = on {
            let from = f.grip + f.up * GUN_BORE + f.axis * emitter;
            // (Along its gun, as it points: drawn to where it tracks you, it
            // pointed apart from its gun as the gun turned, heavy, the user.)
            let way = f.axis;
            let long = super::beam::reach(&ground, from, way).unwrap_or(300.0);
            commands.entity(ray).insert(Transform::from_translation(from).with_rotation(Quat::from_rotation_arc(Vec3::Z, way)).with_scale(Vec3::new(1.0, 1.0, long)));
        }
        commands.entity(ray).insert(shown);
        let p = halo.phase;
        for (k, (layer, drift)) in halo.layers.iter().zip([Vec2::new(0.11, 0.07), Vec2::new(-0.09, 0.12)]).enumerate() {
            let Some(mut m) = materials.get_mut(layer) else { continue };
            let q = p + k as f32 * 3.7;
            let flare = if h.locked() { LOCK.1 } else { 1.0 };
            m.base_color = Color::LinearRgba(LinearRgba::gray(GLOW * flare * halo.charge * halo.charge * waver(t, q)));
            m.uv_transform = Affine2::from_translation(drift * t);
        }
    }
}
