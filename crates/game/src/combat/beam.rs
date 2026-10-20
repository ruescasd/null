//! The stalker's shot: from its muzzle along its aim, to the ground or as
//! far as it reaches, a ray of light (see `beam.wgsl`); a flash, held, gone.
//! Where it lands, on the ground or on what is built, a dome (`envelop.rs`):
//! aimed at what you stand on, it lands by you, or where you were. (Drawn as the comet
//! itself, a flat pale band at the sky's depth, it showed only against the
//! sky, and did not work, the user.)

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

use crate::Args;

/// How far a shot reaches (m), how wide it is (m), and how bright.
const RANGE: f32 = 300.0;
const WIDTH: f32 = 0.35;
const GLOW: f32 = 3.0;
/// A shot's flash (s): up, held (`--set ray_hold`), and gone in. (Whence
/// it came, after the fact: its searchlight back on you, see `halo.rs`; a
/// trail lingering after it was too like the beam, the user.)
pub(super) const FLASH: (f32, f32, f32) = (0.04, 0.4, 0.3);


#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct Ray {
    /// x: width (m), y: least width drawn (px), z: dark (see `dark`).
    #[uniform(0)]
    shape: Vec4,
    /// x: brightness.
    #[uniform(1)]
    light: Vec4,
}

impl Material for Ray {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/combat/beam.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/combat/beam.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        if self.shape.z > 0.5 { AlphaMode::Multiply } else { AlphaMode::Add }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// A shot's ribbon, a metre along +Z, in many pieces (each turns to the
/// camera its own way: seen end-on, a few would not make a wedge).
#[derive(Resource)]
pub(super) struct Kit(pub Handle<Mesh>);

impl Ray {
    /// A ray `width` m wide, `glow` bright (the searchlight's, `halo.rs`).
    pub fn new(width: f32, glow: f32) -> Self {
        Self { shape: Vec4::new(width, 1.5, 0.0, 0.0), light: Vec4::new(glow, 0.0, 0.0, 0.0) }
    }

    /// A dark ray, `width` m wide, darkening what is behind it by as much
    /// as `dark` (0..1): the searchlight's edge, so it shows on pale
    /// ground in the sun, where light added to white is lost.
    pub fn dark(width: f32, dark: f32) -> Self {
        Self { shape: Vec4::new(width, 1.5, 1.0, 0.0), light: Vec4::new(dark, 0.0, 0.0, 0.0) }
    }

}

/// A shot, its finish (its own, to fade), how long it has been, and whose
/// gun it comes from (its stalker, and how far along its bore its emitter):
/// drawn from it as it moves on (fixed, it shone on as the gun swung away,
/// the user).
#[derive(Component)]
pub(super) struct Shot {
    ray: Handle<Ray>,
    age: f32,
    gun: (Entity, f32),
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let n = 64;
    let (mut positions, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..=n {
        for side in [-1.0, 1.0] {
            positions.push([0.0, 0.0, k as f32 / n as f32]);
            uvs.push([side, 0.0]);
        }
        if k < n {
            let v = 2 * k;
            indices.extend_from_slice(&[v, v + 1, v + 3, v, v + 3, v + 2]);
        }
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices));
    commands.insert_resource(Kit(meshes.add(mesh)));
}

/// How far along `dir` (a unit) from `from` a ray first meets the ground or
/// what is built, within `RANGE` (the shot's, and the searchlight's,
/// `halo.rs`).
pub(super) fn reach(ground: &crate::ground::Ground, from: Vec3, dir: Vec3) -> Option<f32> {
    let step = 0.5;
    let terrain = (1..=(RANGE / step) as u32).map(|k| k as f32 * step).find(|&d| {
        let p = from + dir * d;
        p.y < ground.at(p)
    });
    match (terrain, ground.hit(from, dir, RANGE)) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Fires a shot from `from` along `dir` (a unit), to where it first meets
/// the ground or what is built, or `RANGE`. Where it lands, it strikes
/// (`strike.rs`), and a dome stands there (`envelop.rs`), or (`--opt
/// groundzone`, to judge against it) the ground there is electrified, and
/// (`--opt pillar`) a pillar of light stands there besides, stone hanging
/// in it.
/// (`--opt noray`: not drawn, only where it lands, to judge that alone.)
#[allow(clippy::too_many_arguments)]
pub(super) fn fire(
    commands: &mut Commands,
    rays: &mut Assets<Ray>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    kit: &Kit,
    strikes: &super::strike::Kit,
    layers: &super::envelop::Layers,
    args: &Args,
    ground: &crate::ground::Ground,
    from: Vec3,
    dir: Vec3,
    gun: (Entity, f32),
) {
    let length = reach(ground, from, dir);
    let landed = length.map(|d| from + dir * d);
    let length = length.unwrap_or(RANGE);
    if let Some(at) = landed {
        // (The ground's slope there, for its scar.)
        let height = |dx: f32, dz: f32| ground.at(at + Vec3::new(dx, 0.5, dz));
        let up = Vec3::new(height(-0.6, 0.0) - height(0.6, 0.0), 1.2, height(0.0, -0.6) - height(0.0, 0.6)).normalize_or(Vec3::Y);
        super::strike::strike(commands, meshes, materials, strikes, at, dir, up, args.num("ray_hold", FLASH.1));
        if args.opt("pillar") {
            super::envelop::electrify(commands, at);
            super::envelop::raise(commands, layers, at);
            super::strike::levitate(commands, strikes, ground, at, 3.0);
        } else if args.opt("groundzone") {
            super::envelop::electrify(commands, at);
        } else {
            super::envelop::land(commands, at);
        }
    }
    if args.opt("noray") {
        return;
    }
    let ray = rays.add(Ray { shape: Vec4::new(WIDTH, 1.5, 0.0, 0.0), light: Vec4::ZERO });
    commands.spawn((
        Shot { ray: ray.clone(), age: 0.0, gun },
        Mesh3d(kit.0.clone()),
        MeshMaterial3d(ray),
        Transform::from_translation(from).with_rotation(Quat::from_rotation_arc(Vec3::Z, dir)).with_scale(Vec3::new(1.0, 1.0, length)),
        Visibility::default(),
        // (Turned to the camera in its shader: its bounds would not hold.)
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Each shot flashes, holds, and goes.
pub(super) fn shots(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    hunters: Query<&super::hunter::Hunter>,
    mut shots: Query<(Entity, &mut Shot, &mut Transform)>,
    mut rays: ResMut<Assets<Ray>>,
) {
    let hold = args.num("ray_hold", FLASH.1);
    for (entity, mut shot, mut transform) in &mut shots {
        // From its gun as it is now, along its aim.
        if let Some(f) = hunters.get(shot.gun.0).ok().and_then(|h| h.rig().gun_frame) {
            let from = f.grip + f.up * crate::rig::GUN_BORE + f.axis * shot.gun.1;
            let long = reach(&ground, from, f.axis).unwrap_or(RANGE);
            *transform = Transform::from_translation(from).with_rotation(Quat::from_rotation_arc(Vec3::Z, f.axis)).with_scale(Vec3::new(1.0, 1.0, long));
        }
        shot.age += time.delta_secs();
        let a = shot.age;
        if a > hold + FLASH.2 {
            rays.remove(&shot.ray);
            commands.entity(entity).despawn();
            continue;
        }
        let strength = if a < FLASH.0 { a / FLASH.0 } else if a < hold { 1.0 } else { 1.0 - (a - hold) / FLASH.2 };
        if let Some(mut ray) = rays.get_mut(&shot.ray) {
            ray.light.x = GLOW * strength;
        }
    }
}
