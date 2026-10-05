//! An experiment: a creature made of the world's fractal language. A
//! procedural skeleton walks on the real terrain; each bone's volume is
//! filled by a fractal fill of small fragments (cubes, wedges and shards,
//! from the same rule as the structures), and every fragment springs towards
//! its place on its bone instead of being fixed to it, so the body is held
//! together rather than solid.
//!
//! Four anatomies share the same skeleton. The default is human: exactly a
//! human's proportions (the eight-heads canon) at about 2.4 m, upright, arms
//! hanging, a calm walk, heavy-limbed and fairly solid (fewer, larger
//! fragments), glowing only in its chest. With `--opt luminous`, the same shape slimmer
//! and finer-grained, lit all through: strange only in its substance.
//! With `--opt hunched`, a menacing humanoid (hunched, head pushed forward
//! and low, shoulders raised, arms forward with elbows out); with `--opt
//! creature`, a feral creature (long low neck and elongated skull, long
//! clawed arms, digitigrade legs). Every part tapers, and each is filled as
//! a dense core with irregular gaps plus a sparse outer layer of fragments,
//! so the outline frays.
//!
//! A core of faintly glowing shards sits deep in the chest and skull, with
//! a small light among them: the body is lit from inside, through its own
//! gaps, so it reads against the black sky from any side, and it pulses.
//! The human has glowing fragments threaded through every part, not only
//! the chest, as if lit all through.
//!
//! The walk is procedural: a foot stays planted until it is too far from
//! where it should be, then steps there along an arc; legs bend with
//! two-bone IK. For now the figure walks towards the player and stops a few
//! metres away (`--opt nofigures`).

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use worldgen::{
    ifs::{self, Block, Keep, Rule},
    noise::hash01,
};

use crate::{Args, camera::FlyCam, terrain::WorldGen};

pub struct FigurePlugin;

impl Plugin for FigurePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn)
            .add_systems(Update, (walk, follow, pulse).chain().after(crate::terrain::StreamSet));
    }
}

const WALK_SPEED: f32 = 1.8;
/// A foot steps when it is this far from where it should be.
const STEP_TRIGGER: f32 = 0.8;
const STEP_TIME: f32 = 0.4;
const STEP_HEIGHT: f32 = 0.35;
/// Brightness of the glowing core (cd/m², before the pulse) and the light
/// output of the lamp inside the chest (lumens).
const GLOW: f32 = 20000.0;
const CORE_LIGHT: f32 = 60000.0;

/// Which body plan the figure has.
#[derive(Clone, Copy, PartialEq)]
enum Anatomy {
    /// A human's proportions, taller, upright, heavy-limbed and fairly
    /// solid, glowing only in the chest.
    Human,
    /// The same shape, slimmer and finer-grained, lit all through.
    Luminous,
    /// Human proportions with a menacing, hunched, forward stance.
    Humanoid,
    /// Long low neck, elongated skull, long clawed arms, digitigrade legs.
    Creature,
}

/// Limb lengths and widths of an anatomy, in metres.
struct Build {
    pelvis_height: f32,
    hip_width: f32,
    thigh: f32,
    shin: f32,
    /// The foot: for the creature the raised section from ankle to toe.
    foot: f32,
    shoulder_width: f32,
    upper_arm: f32,
    forearm: f32,
}

impl Anatomy {
    /// Whether it has a human's shape and stance.
    fn human(self) -> bool {
        matches!(self, Anatomy::Human | Anatomy::Luminous)
    }

    fn build(self) -> Build {
        match self {
            // The eight-heads canon at 2.4 m: hip joints at 0.53 of the
            // height, thigh and shin a quarter each, the arm to mid-thigh.
            Anatomy::Human | Anatomy::Luminous => Build {
                pelvis_height: 1.27,
                hip_width: if self == Anatomy::Human { 0.13 } else { 0.12 },
                thigh: 0.59,
                shin: 0.59,
                foot: 0.27,
                shoulder_width: if self == Anatomy::Human { 0.29 } else { 0.27 },
                upper_arm: 0.45,
                forearm: 0.36,
            },
            Anatomy::Humanoid => Build {
                pelvis_height: 1.58,
                hip_width: 0.19,
                thigh: 0.94,
                shin: 0.92,
                foot: 0.24,
                shoulder_width: 0.38,
                upper_arm: 0.74,
                forearm: 0.7,
            },
            Anatomy::Creature => Build {
                pelvis_height: 1.5,
                hip_width: 0.2,
                thigh: 0.78,
                shin: 0.82,
                foot: 0.55,
                shoulder_width: 0.4,
                upper_arm: 0.8,
                forearm: 0.8,
            },
        }
    }
}
/// How close the figure comes before stopping.
const KEEP_AWAY: f32 = 5.0;
/// Spring holding each fragment to its place: stiffness and damping.
const STIFFNESS: f32 = 220.0;
/// The human's skull, larger than life (with its eyes).
const HEAD: f32 = 1.15;
const DAMPING: f32 = 18.0;

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

impl Side {
    fn sign(self) -> f32 {
        if self == Side::Left { -1.0 } else { 1.0 }
    }
}

#[derive(Clone, Copy)]
enum Bone {
    Pelvis,
    Waist,
    Chest,
    Neck,
    Skull,
    UpperArm(Side),
    Forearm(Side),
    Claw(Side),
    Thigh(Side),
    Shin(Side),
    Metatarsal(Side),
    Toe(Side),
}

const BONES: [Bone; 19] = [
    Bone::Pelvis,
    Bone::Waist,
    Bone::Chest,
    Bone::Neck,
    Bone::Skull,
    Bone::UpperArm(Side::Left),
    Bone::UpperArm(Side::Right),
    Bone::Forearm(Side::Left),
    Bone::Forearm(Side::Right),
    Bone::Claw(Side::Left),
    Bone::Claw(Side::Right),
    Bone::Thigh(Side::Left),
    Bone::Thigh(Side::Right),
    Bone::Shin(Side::Left),
    Bone::Shin(Side::Right),
    Bone::Metatarsal(Side::Left),
    Bone::Metatarsal(Side::Right),
    Bone::Toe(Side::Left),
    Bone::Toe(Side::Right),
];

/// A part's volume: width and depth at its start and at its end (it tapers
/// between them), its nominal length, and how finely it is split.
struct Part {
    start: (f32, f32),
    end: (f32, f32),
    length: f32,
    levels: u32,
}

impl Bone {
    fn part(self, anatomy: Anatomy) -> Part {
        let p = |start, end, length, levels| Part { start, end, length, levels };
        let b = anatomy.build();
        if anatomy.human() {
            // A human's widths, scaled up with the height; the bulky one with
            // limbs a third thicker, the trunk as it is.
            let bulk = |part: Part, k: f32| Part { start: (part.start.0 * k, part.start.1 * k), end: (part.end.0 * k, part.end.1 * k), ..part };
            let (trunk, limb) = if anatomy == Anatomy::Human { (1.0, 1.35) } else { (1.0, 1.0) };
            let part = match self {
                // Narrow hips and waist under broad shoulders: lean, not
                // heavy-bottomed.
                Bone::Pelvis => p((0.4, 0.25), (0.38, 0.25), 0.26, 2),
                Bone::Waist => p((0.36, 0.25), (0.36, 0.26), 0.35, 2),
                Bone::Chest => p((0.36, 0.26), (0.55, 0.3), 0.4, 2),
                // Flared at the base, which starts inside the chest: the
                // trapezius sloping from the neck down to the shoulders.
                Bone::Neck => p((0.34, 0.2), (0.15, 0.16), 0.13, 1),
                Bone::Skull => p((0.2, 0.26), (0.17, 0.22), 0.31, 2),
                Bone::UpperArm(_) => p((0.14, 0.14), (0.1, 0.1), b.upper_arm, 2),
                Bone::Forearm(_) => p((0.1, 0.1), (0.07, 0.06), b.forearm, 1),
                // A hand, not a claw.
                Bone::Claw(_) => p((0.12, 0.04), (0.08, 0.03), 0.26, 1),
                // Slim at the top, so with the bulk the hips stay inside
                // the shoulders.
                Bone::Thigh(_) => p((0.19, 0.2), (0.14, 0.14), b.thigh, 2),
                Bone::Shin(_) => p((0.15, 0.16), (0.09, 0.09), b.shin, 1),
                Bone::Metatarsal(_) => p((0.12, 0.08), (0.11, 0.05), b.foot, 1),
                Bone::Toe(_) => p((0.11, 0.04), (0.08, 0.03), 0.09, 1),
            };
            let k = if matches!(self, Bone::Pelvis | Bone::Waist | Bone::Chest | Bone::Neck) { trunk } else { limb };
            return bulk(part, if matches!(self, Bone::Skull) { 1.0 } else { k });
        }
        if anatomy == Anatomy::Humanoid {
            return match self {
                Bone::Pelvis => p((0.46, 0.32), (0.42, 0.3), 0.26, 2),
                Bone::Waist => p((0.38, 0.28), (0.32, 0.26), 0.34, 2),
                // A broad V of a torso, shoulders wide and high.
                Bone::Chest => p((0.4, 0.3), (0.82, 0.46), 0.6, 2),
                Bone::Neck => p((0.17, 0.18), (0.14, 0.16), 0.22, 1),
                Bone::Skull => p((0.24, 0.28), (0.18, 0.22), 0.34, 2),
                Bone::UpperArm(_) => p((0.21, 0.21), (0.12, 0.12), b.upper_arm, 2),
                Bone::Forearm(_) => p((0.13, 0.13), (0.08, 0.09), b.forearm, 1),
                // Hands narrowing to clawed fingers.
                Bone::Claw(_) => p((0.12, 0.05), (0.03, 0.02), 0.3, 1),
                Bone::Thigh(_) => p((0.3, 0.32), (0.15, 0.16), b.thigh, 2),
                Bone::Shin(_) => p((0.14, 0.16), (0.09, 0.11), b.shin, 1),
                Bone::Metatarsal(_) => p((0.1, 0.08), (0.09, 0.06), b.foot, 1),
                Bone::Toe(_) => p((0.09, 0.05), (0.03, 0.02), 0.12, 1),
            };
        }
        match self {
            Bone::Pelvis => p((0.44, 0.34), (0.4, 0.3), 0.26, 2),
            Bone::Waist => p((0.36, 0.3), (0.28, 0.26), 0.36, 2),
            // A barrel ribcage, widest at the shoulders, hunched.
            Bone::Chest => p((0.34, 0.3), (0.86, 0.56), 0.62, 2),
            Bone::Neck => p((0.2, 0.22), (0.13, 0.15), 0.42, 1),
            // An elongated skull tapering to a point.
            Bone::Skull => p((0.26, 0.32), (0.05, 0.07), 0.6, 2),
            Bone::UpperArm(_) => p((0.24, 0.24), (0.12, 0.12), b.upper_arm, 2),
            Bone::Forearm(_) => p((0.15, 0.15), (0.08, 0.09), b.forearm, 1),
            Bone::Claw(_) => p((0.13, 0.05), (0.02, 0.02), 0.42, 1),
            Bone::Thigh(_) => p((0.36, 0.38), (0.15, 0.16), b.thigh, 2),
            Bone::Shin(_) => p((0.13, 0.15), (0.08, 0.1), b.shin, 1),
            Bone::Metatarsal(_) => p((0.08, 0.1), (0.07, 0.12), b.foot, 1),
            Bone::Toe(_) => p((0.1, 0.06), (0.02, 0.02), 0.22, 1),
        }
    }
}

/// A bone's pose: where its part starts, the part's length, and its frame
/// (local +y runs along the bone, +x across it).
#[derive(Clone, Copy, Default)]
struct Pose {
    start: Vec3,
    rotation: Quat,
    length: f32,
}

#[derive(Clone, Copy)]
struct Foot {
    /// Where the toe touches the ground.
    planted: Vec3,
    from: Vec3,
    to: Vec3,
    /// Progress of a step in 0..1, or None while planted.
    step: Option<f32>,
}

#[derive(Component)]
struct Figure {
    anatomy: Anatomy,
    position: Vec3,
    heading: f32,
    speed: f32,
    feet: [Foot; 2],
    poses: Vec<Pose>,
}

/// The glowing core's material (pulsed) and the lamp inside the chest.
#[derive(Resource)]
struct Glow(Handle<StandardMaterial>);

#[derive(Component)]
struct CoreLight;

/// One fragment of the body, held to a place on a bone by a spring.
#[derive(Component)]
struct Element {
    bone: usize,
    /// Centre in the bone's frame, with y as a fraction of the bone's
    /// length so parts follow the skeleton.
    offset: Vec3,
    rotation: Quat,
    velocity: Vec3,
    /// A per-fragment phase for the shiver.
    phase: f32,
    /// Hard parts and eyes don't shiver: they move as one with their bone,
    /// so the eyes stay on the face instead of dipping in and out of it.
    rigid: bool,
}

fn spawn(
    mut commands: Commands,
    args: Res<Args>,
    world: Res<WorldGen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if args.opt("nofigures") {
        return;
    }
    // About 22 m ahead of the default spawn point, facing it.
    let (x, z) = (1214.0, 883.0);
    let position = Vec3::new(x, world.ground_height(x, z), z);
    let heading = 140f32.to_radians();
    let forward = Vec3::new(heading.sin(), 0.0, heading.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let anatomy = if args.opt("creature") {
        Anatomy::Creature
    } else if args.opt("hunched") {
        Anatomy::Humanoid
    } else if args.opt("luminous") {
        Anatomy::Luminous
    } else {
        Anatomy::Human
    };
    let human = anatomy.human();
    // The bulky human is fairly solid: fewer, larger fragments, packed
    // tighter, hardly any fray.
    let solid = anatomy == Anatomy::Human;
    let hip_width = anatomy.build().hip_width;
    let foot = |side: f32| {
        let p = position + right * side * hip_width + forward * 0.2;
        let p = Vec3::new(p.x, world.ground_height(p.x, p.z), p.z);
        Foot { planted: p, from: p, to: p, step: None }
    };

    // Fragments are cubes, wedges (triangular prisms) and pointed shards,
    // all spanning -1..1 so a fragment's scale is its half extents.
    let shapes = [
        meshes.add(Cuboid::new(2.0, 2.0, 2.0)),
        meshes.add(faceted(
            &[[-1., -1., -1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.], [1., -1., 1.], [-1., 1., 1.]],
            &[&[0, 2, 1], &[3, 4, 5], &[0, 1, 4, 3], &[0, 3, 5, 2], &[1, 2, 5, 4]],
        )),
        meshes.add(faceted(
            &[[-1., -1., -1.], [1., -1., -0.6], [-0.2, -1., 1.], [0.3, 1., 0.1]],
            &[&[0, 1, 2], &[0, 3, 1], &[1, 3, 2], &[2, 3, 0]],
        )),
        // A faceted skull, and a flat rhombus (for eyes).
        meshes.add(skull()),
        meshes.add(faceted(
            &[[1., 0., -1.], [0., 1., -1.], [-1., 0., -1.], [0., -1., -1.], [1., 0., 1.], [0., 1., 1.], [-1., 0., 1.], [0., -1., 1.]],
            &[&[0, 1, 2, 3], &[4, 5, 6, 7], &[0, 1, 5, 4], &[1, 2, 6, 5], &[2, 3, 7, 6], &[3, 0, 4, 7]],
        )),
    ];
    // Three finishes, from pale to dark, so the body reads against both the
    // pale ground and the black sky (all-dark fragments vanish against it).
    // With line art the fragments are pale and matte, so the black lines
    // read as ink on paper.
    // Line art is on for the human by default (`--opt nooutline` turns it
    // off), and for the others with `--opt outline`.
    let outlined = !args.opt("nooutline") && (args.opt("outline") || anatomy == Anatomy::Human);
    let tones = if outlined { [(0.3, 0.75, 0.35), (0.23, 0.8, 0.35), (0.16, 0.75, 0.35)] } else { [(0.22, 0.35, 0.7), (0.09, 0.55, 0.5), (0.03, 0.2, 0.9)] };
    let finishes = tones.map(|(tone, rough, refl)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(tone, tone, tone),
            perceptual_roughness: rough,
            reflectance: refl,
            ..default()
        })
    });

    // The glowing core: emissive shards, pulsed by `pulse`.
    let glow = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(GLOW, GLOW, GLOW),
        ..default()
    });
    commands.insert_resource(Glow(glow.clone()));
    commands.spawn((
        CoreLight,
        PointLight { intensity: CORE_LIGHT, range: 5.0, shadow_maps_enabled: false, ..default() },
        Transform::from_translation(position + Vec3::Y * 2.0),
    ));

    commands.spawn((
        Figure {
            anatomy,
            position,
            heading,
            speed: 0.0,
            feet: [foot(-1.0), foot(1.0)],
            poses: vec![Pose::default(); BONES.len()],
        },
        Transform::default(),
        Visibility::default(),
    ));

    // Hard parts (the human's hands and feet): dark and glossy, smoother
    // than the fragments.
    let hard_tone = if outlined { 0.2 } else { 0.05 };
    let hard = materials.add(StandardMaterial {
        base_color: Color::srgb(hard_tone, hard_tone, hard_tone),
        perceptual_roughness: 0.25,
        reflectance: 0.8,
        ..default()
    });
    // `--opt outline`: line art on the figure alone. Each piece gets a
    // slightly larger black copy that shows only its back faces, so a line
    // of constant width runs round every piece and nowhere else.
    // The head darker still, so its eyes stand out; the eyes glow with the
    // chest.
    // Matte, so no facet catches the sun and drowns the eyes.
    let head_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.018, 0.018, 0.018),
        perceptual_roughness: 0.9,
        reflectance: 0.1,
        ..default()
    });
    // The eyes: a steady light, not the chest's pulse (an unwavering stare);
    // `--opt eyepulse` has them pulse with it.
    let eye_light = if args.opt("eyepulse") {
        glow.clone()
    } else {
        materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(GLOW * 0.8, GLOW * 0.8, GLOW * 0.8), ..default() })
    };
    let outline = outlined.then(|| {
        materials.add(StandardMaterial {
            base_color: Color::BLACK,
            unlit: true,
            cull_mode: Some(bevy::render::render_resource::Face::Front),
            ..default()
        })
    });
    let put = |commands: &mut Commands, element: Element, shape: usize, material: Handle<StandardMaterial>, half: Vec3| {
        let mut e = commands.spawn((
            element,
            Mesh3d(shapes[shape].clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * 2.0).with_scale(half),
        ));
        if let Some(line) = &outline {
            let width = 0.012;
            e.with_child((
                Mesh3d(shapes[shape].clone()),
                MeshMaterial3d(line.clone()),
                Transform::from_scale(Vec3::ONE + Vec3::splat(width) / half.max(Vec3::splat(0.005))),
            ));
        }
    };

    let mut count = 0;
    for (index, bone) in BONES.iter().enumerate() {
        let part = bone.part(anatomy);
        // The human's hands and feet are hard, solid pieces rather than
        // fragments: a palm, jointed fingers and a thumb; a foot, a heel and
        // a toe cap. Offsets in metres along the bone, converted below.
        // `--set head=N`: 0 fragments; 1 a hard human head (the default); 2 a long narrow
        // wedge of a head reaching forward; 3 a tall thin crest; 4 a wide
        // flat disc. `--set hand=N`: 0 hard human hands; 1 long three-digit
        // hands; 2 a single blade; 3 two razor prongs (the default).
        let head = args.num("head", 1.0) as i32;
        let hand = if args.opt("longhands") { 1 } else { args.num("hand", 3.0) as i32 };
        if solid && matches!(bone, Bone::Skull) && head > 0 {
            // The skull's frame: x across, y up, z forward.
            let l = part.length;
            let mut pieces: Vec<(Vec3, Vec3, Quat, usize)> = Vec::new();
            match head {
                1 => {
                    // A skull, not a box: wide at the cranium and cheekbones,
                    // narrowing to a chin set forward, a ridge down the face.
                    pieces.push((Vec3::new(0.0, l * 0.5, 0.0), Vec3::new(0.105 * HEAD, l * 0.5, 0.125 * HEAD), Quat::IDENTITY, 3));
                }
                5 => {
                    // The first hard head: a box and a jaw.
                    pieces.push((Vec3::new(0.0, l * 0.55, -0.01), Vec3::new(0.095, l * 0.42, 0.12), Quat::IDENTITY, 0));
                    pieces.push((Vec3::new(0.0, l * 0.14, 0.03), Vec3::new(0.072, l * 0.13, 0.09), Quat::IDENTITY, 0));
                }
                2 => {
                    // Long and narrow, reaching forward to a point, a fin behind.
                    pieces.push((Vec3::new(0.0, l * 0.5, 0.05), Vec3::new(0.06, l * 0.32, 0.2), Quat::IDENTITY, 0));
                    pieces.push((Vec3::new(0.0, l * 0.45, 0.32), Vec3::new(0.08, 0.05, 0.12), Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), 1));
                    pieces.push((Vec3::new(0.0, l * 0.75, -0.2), Vec3::new(0.012, l * 0.35, 0.09), Quat::IDENTITY, 0));
                }
                3 => {
                    // A tall thin blade of a head.
                    pieces.push((Vec3::new(0.0, l * 0.95, 0.0), Vec3::new(0.035, l * 0.95, 0.15), Quat::IDENTITY, 0));
                    pieces.push((Vec3::new(0.0, l * 0.2, 0.02), Vec3::new(0.06, l * 0.2, 0.09), Quat::IDENTITY, 0));
                }
                _ => {
                    // A wide flat disc on a short stalk.
                    pieces.push((Vec3::new(0.0, l * 0.25, 0.0), Vec3::new(0.055, l * 0.25, 0.055), Quat::IDENTITY, 0));
                    pieces.push((Vec3::new(0.0, l * 0.62, 0.02), Vec3::new(0.27, 0.045, 0.23), Quat::IDENTITY, 0));
                    pieces.push((Vec3::new(0.0, l * 0.62 + 0.06, 0.0), Vec3::new(0.12, 0.03, 0.1), Quat::IDENTITY, 0));
                }
            }
            // Eyes, `--set eyes=N`: 0 narrow slits; 1 flat rhombuses
            // slanting up and out; 2 upright rhombuses. On the skull they lie
            // on the facets either side of the ridge.
            let style = args.num("eyes", 1.0) as i32;
            let (eye_shape, eye_half, slant) = match style {
                0 => (0, Vec3::new(0.022, 0.008, 0.005), 0.0),
                2 => (4, Vec3::new(0.012, 0.022, 0.004), 0.0),
                _ => (4, Vec3::new(0.027, 0.011, 0.004), 0.22),
            };
            let eyes: Vec<(Vec3, Vec3, Quat, usize)> = match head {
                1 => [-1.0f32, 1.0]
                    .map(|sx| (Vec3::new(sx * 0.04 * HEAD, l * 0.62, 0.105 * HEAD), eye_half * HEAD, Quat::from_rotation_y(-sx * 0.43) * Quat::from_rotation_z(sx * slant), eye_shape))
                    .to_vec(),
                5 => [-1.0f32, 1.0].map(|sx| (Vec3::new(sx * 0.042, l * 0.62, 0.116), eye_half, Quat::from_rotation_z(sx * slant), eye_shape)).to_vec(),
                2 => [-1.0f32, 1.0].map(|sx| (Vec3::new(sx * 0.064, l * 0.55, 0.22), Vec3::new(0.004, 0.008, 0.03), Quat::IDENTITY, 0)).to_vec(),
                _ => Vec::new(),
            };
            for (n, (offset, half, rotation, shape)) in pieces.into_iter().enumerate() {
                let offset = Vec3::new(offset.x, offset.y / l.max(0.01), offset.z);
                let phase = hash01(index as i32, n as i32, 9, 0xf18) * 100.0;
                put(&mut commands, Element { bone: index, offset, rotation, velocity: Vec3::ZERO, phase, rigid: true }, shape, head_dark.clone(), half);
                count += 1;
            }
            for (n, (offset, half, rotation, shape)) in eyes.into_iter().enumerate() {
                let offset = Vec3::new(offset.x, offset.y / l.max(0.01), offset.z);
                let phase = hash01(index as i32, n as i32 + 50, 9, 0xf18) * 100.0;
                // No outline round the eyes: just the light.
                commands.spawn((
                    Element { bone: index, offset, rotation, velocity: Vec3::ZERO, phase, rigid: true },
                    Mesh3d(shapes[shape].clone()),
                    MeshMaterial3d(eye_light.clone()),
                    Transform::from_translation(position + Vec3::Y * 2.0).with_scale(half),
                ));
                count += 1;
            }
            continue;
        }
        if solid && matches!(bone, Bone::Claw(_) | Bone::Metatarsal(_) | Bone::Toe(_)) {
            let k = part.start.0 / if matches!(bone, Bone::Claw(_)) { 0.12 } else { 0.12 };
            let mut pieces: Vec<(Vec3, Vec3, Quat)> = Vec::new();
            // Blades: the wedge shape's long straight edge on one side, a
            // point at the far end, broad across (x) and thin front to back,
            // so they show their shape from the front; `outer` turns the
            // straight edge to the other side.
            let mut blades: Vec<(Vec3, Vec3, Quat)> = Vec::new();
            let flat = |outer: bool| if outer { Quat::IDENTITY } else { Quat::from_rotation_y(std::f32::consts::PI) };
            match bone {
                Bone::Claw(_) if hand == 2 => {
                    // A single blade, no hand at all.
                    blades.push((Vec3::new(0.0, 0.03 + 0.28, 0.0), Vec3::new(0.065, 0.28, 0.012), flat(true)));
                }
                Bone::Claw(_) if hand == 3 => {
                    // Two razor prongs, a little apart, edges outward.
                    for (x, fan, outer) in [(-0.035f32, 0.12f32, true), (0.035, -0.12, false)] {
                        // Fanned apart across (about z), straight edges out.
                        let dir = Vec3::new(-fan.sin(), fan.cos(), 0.0);
                        blades.push((Vec3::new(x, 0.02, 0.0) + dir * 0.26, Vec3::new(0.035, 0.25, 0.01), Quat::from_rotation_z(fan) * flat(outer)));
                    }
                }
                Bone::Claw(_) if hand == 1 => {
                    // Not a human hand: a narrow palm and three very long,
                    // slender digits of three joints each, fanned a little.
                    pieces.push((Vec3::new(0.0, 0.04, 0.0), Vec3::new(0.02 * k, 0.04, 0.04 * k), Quat::IDENTITY));
                    for (z, fan) in [(-0.025, -0.14f32), (0.0, 0.0), (0.025, 0.14)] {
                        let dir = Vec3::new(0.0, fan.cos(), fan.sin());
                        let segment = 0.15;
                        for j in 0..3 {
                            let along = 0.08 + segment * (j as f32 + 0.5);
                            let thin = 0.013 * (1.0 - 0.22 * j as f32) * k;
                            pieces.push((Vec3::new(0.004 * j as f32, 0.0, z * k) + dir * along, Vec3::new(thin, segment * 0.48, thin), Quat::from_rotation_x(fan)));
                        }
                    }
                }
                Bone::Claw(_) => {
                    // The palm lies in the plane of the arm's swing (palms to
                    // the thighs): thin across (x), wide front to back (z).
                    pieces.push((Vec3::new(0.0, 0.055, 0.0), Vec3::new(0.022 * k, 0.055, 0.068 * k), Quat::IDENTITY));
                    for (z, l) in [(-0.051, 0.045), (-0.017, 0.053), (0.017, 0.05), (0.051, 0.04)] {
                        let z = z * k;
                        pieces.push((Vec3::new(0.0, 0.11 + l * 0.5, z), Vec3::new(0.016 * k, l * 0.5, 0.014 * k), Quat::IDENTITY));
                        // The second joint, bent a little towards the palm.
                        pieces.push((Vec3::new(0.006, 0.11 + l * 1.42, z), Vec3::new(0.014 * k, l * 0.42, 0.013 * k), Quat::from_rotation_z(0.18)));
                    }
                    // The thumb, at the front, angled out.
                    pieces.push((Vec3::new(0.008, 0.055, -0.085 * k), Vec3::new(0.016 * k, 0.036, 0.015 * k), Quat::from_rotation_x(0.5)));
                }
                Bone::Metatarsal(_) => {
                    // Along the foot (y) from the ankle; z points down.
                    pieces.push((Vec3::new(0.0, part.length * 0.5, 0.0), Vec3::new(0.06 * k, part.length * 0.5, 0.042), Quat::IDENTITY));
                    pieces.push((Vec3::new(0.0, -0.02, 0.035), Vec3::new(0.05 * k, 0.05, 0.04), Quat::IDENTITY));
                }
                _ => {
                    pieces.push((Vec3::new(0.0, part.length * 0.5, 0.01), Vec3::new(0.056 * k, part.length * 0.5, 0.026), Quat::IDENTITY));
                }
            }
            let shaped = pieces.into_iter().map(|p| (p, 0)).chain(blades.into_iter().map(|p| (p, 1)));
            for (n, ((offset, half, rotation), shape)) in shaped.enumerate() {
                let offset = Vec3::new(offset.x, offset.y / part.length.max(0.01), offset.z);
                let phase = hash01(index as i32, n as i32, 9, 0xf17) * 100.0;
                put(&mut commands, Element { bone: index, offset, rotation, velocity: Vec3::ZERO, phase, rigid: true }, shape, hard.clone(), half);
                count += 1;
            }
            continue;
        }
        let (wide, deep) = (part.start.0.max(part.end.0), part.start.1.max(part.end.1));
        // Generated one unit long at the part's widest, then tapered.
        let root = Block {
            center: Vec3::Y * 0.5,
            rotation: Quat::IDENTITY,
            half: Vec3::new(wide * 0.5, 0.5, deep * 0.5),
            level: 0,
        };
        // A dense core, then a sparse outer layer pushed out from the
        // surface, so the outline frays.
        let core = Rule {
            divisions: [2, 4, 2],
            keep: Keep::Random(if solid { 0.96 } else { 0.9 }),
            depth: if solid { part.levels.max(2) } else { part.levels + 1 },
            gap: 1.0,
            twist: Quat::IDENTITY,
            stop_chance: 0.3,
            lift: 0.0,
            min_size: 0.0,
        };
        // The human frays less, so its outline stays a person's.
        let fray = Rule {
            divisions: [3, 6, 3],
            keep: Keep::Random(if solid { 0.06 } else if human { 0.16 } else { 0.3 }),
            depth: 1,
            stop_chance: 0.0,
            ..core
        };
        for (layer, rule) in [core, fray].iter().enumerate() {
            let leaves = ifs::generate(rule, root, 41 + index as u32 * 7 + layer as u32, 1200, &[]);
            for (n, leaf) in leaves.iter().enumerate() {
                let b = leaf.block;
                let r = |k: i32| hash01(index as i32 * 2 + layer as i32, n as i32, k, 0xf16);
                // Taper: width and depth follow the part along its length.
                let t = b.center.y.clamp(0.0, 1.0);
                let fx = (part.start.0 + (part.end.0 - part.start.0) * t) / wide;
                let fz = (part.start.1 + (part.end.1 - part.start.1) * t) / deep;
                // Irregular gaps: each fragment shrunk by its own amount.
                let (least, spread) = if solid { (0.78, 0.2) } else { (0.5, 0.45) };
                let shrink = Vec3::new(least + spread * r(1), least + spread * r(2), least + spread * r(3));
                let mut half = b.half * shrink * Vec3::new(fx, part.length, fz);
                let mut offset = Vec3::new(b.center.x * fx, b.center.y, b.center.z * fz);
                if layer == 1 {
                    // Pushed out beyond the surface, and smaller.
                    let out = if solid { 1.04 + 0.08 * r(4) } else if human { 1.1 + 0.2 * r(4) } else { 1.25 + 0.45 * r(4) };
                    offset.x *= out;
                    offset.z *= out;
                    half *= 0.55;
                }
                let tilt = Quat::from_euler(EulerRot::YXZ, (r(5) - 0.5) * 0.7, (r(6) - 0.5) * 0.5, (r(7) - 0.5) * 0.5);
                let shape = match r(8) {
                    x if x < 0.5 => 0,
                    x if x < 0.78 => 1,
                    _ => 2,
                };
                // Shards inside the chest, along the spine and in the skull
                // glow, close enough to the surface to show through the gaps.
                let deep_inside = (b.center.x / (wide * 0.5)).abs() < 0.75 && (b.center.z / (deep * 0.5)).abs() < 0.75;
                let core_bone = matches!(bone, Bone::Chest | Bone::Waist | Bone::Neck | Bone::Skull);
                // The human glows all through: threads of lit fragments in
                // every part, more in the chest and skull.
                let chance = match (human, core_bone) {
                    (true, true) => 0.32,
                    (true, false) => 0.14,
                    (false, true) => 0.3,
                    (false, false) => 0.0,
                };
                // The solid human glows only in its chest: an even core in
                // the middle of it (by place, not by chance alone, so it
                // doesn't gather to one side).
                let glows = if solid {
                    let (cx, cy, cz) = (b.center.x / (wide * 0.5), b.center.y, b.center.z / (deep * 0.5));
                    let core = (cx / 0.6).powi(2) + ((cy - 0.36) / 0.28).powi(2) + (cz / 0.7).powi(2) < 1.0;
                    layer == 0 && matches!(bone, Bone::Chest) && core && r(11) < 0.75
                } else {
                    layer == 0 && deep_inside && r(11) < chance
                };
                let material = if glows {
                    glow.clone()
                } else {
                    finishes[(r(10) * 3.0) as usize % 3].clone()
                };
                put(
                    &mut commands,
                    Element { bone: index, offset, rotation: b.rotation * tilt, velocity: Vec3::ZERO, phase: r(9) * 100.0, rigid: false },
                    shape,
                    material,
                    half,
                );
                count += 1;
            }
        }
    }
    info!("figure: {count} fragments");
}

/// Rotation whose +y runs along `along` and whose +x is as close to `right`
/// as possible.
fn frame(along: Vec3, right: Vec3) -> Quat {
    let y = along.normalize_or(Vec3::Y);
    let x = (right - y * right.dot(y)).normalize_or(Vec3::X);
    let z = x.cross(y);
    Quat::from_mat3(&Mat3::from_cols(x, y, z))
}

/// Two-bone IK: the middle joint between `root` and `end`, bending towards
/// `pole`.
fn middle_joint(root: Vec3, end: Vec3, a: f32, b: f32, pole: Vec3) -> Vec3 {
    let to_end = end - root;
    let d = to_end.length().clamp(0.05, a + b - 1e-3);
    let dir = to_end.normalize_or(Vec3::NEG_Y);
    let cos = ((a * a + d * d - b * b) / (2.0 * a * d)).clamp(-1.0, 1.0);
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Z);
    root + dir * a * cos + bend * a * (1.0 - cos * cos).sqrt()
}

/// `dir` turned by `angle` radians towards `towards` (both unit, roughly
/// perpendicular).
fn tip(dir: Vec3, towards: Vec3, angle: f32) -> Vec3 {
    (dir * angle.cos() + towards * angle.sin()).normalize()
}

fn walk(
    time: Res<Time>,
    args: Res<Args>,
    world: Res<WorldGen>,
    camera: Single<&Transform, (With<FlyCam>, Without<Figure>)>,
    mut figures: Query<&mut Figure>,
) {
    let dt = time.delta_secs().min(0.05);
    let ground = |p: Vec3| Vec3::new(p.x, world.ground_height(p.x, p.z), p.z);
    for mut f in &mut figures {
        // Head for the player; stop a few metres away.
        let to_player = camera.translation - f.position;
        let flat = Vec3::new(to_player.x, 0.0, to_player.z);
        // `--opt statue` keeps it still where it spawned, for looking at.
        let distance = if args.opt("statue") { 0.0 } else { flat.length() };
        if distance > 0.1 {
            let target = flat.x.atan2(flat.z);
            let turn = (target - f.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            f.heading += turn.clamp(-1.4 * dt, 1.4 * dt);
        }
        let speed = if f.anatomy.human() { 1.4 } else { WALK_SPEED };
        let wanted = if distance > KEEP_AWAY && distance < 120.0 { speed } else { 0.0 };
        f.speed += (wanted - f.speed).clamp(-2.0 * dt, 2.0 * dt);
        let forward = Vec3::new(f.heading.sin(), 0.0, f.heading.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        f.position = ground(f.position + forward * f.speed * dt);
        let body = f.anatomy.build();
        let creature = f.anatomy == Anatomy::Creature;
        let human = f.anatomy.human();

        // Feet: step when too far from where they belong, one at a time.
        for i in 0..2 {
            let other_stepping = f.feet[1 - i].step.is_some();
            let side = if i == 0 { -1.0 } else { 1.0 };
            let home = ground(f.position + right * side * body.hip_width + forward * (0.2 + f.speed * 0.4));
            let foot = &mut f.feet[i];
            match foot.step {
                Some(t) => {
                    let t = (t + dt / STEP_TIME).min(1.0);
                    let arc = (t * std::f32::consts::PI).sin() * STEP_HEIGHT;
                    foot.planted = foot.from.lerp(foot.to, t) + Vec3::Y * arc;
                    foot.step = (t < 1.0).then_some(t);
                }
                None if !other_stepping && foot.planted.distance(home) > STEP_TRIGGER => {
                    foot.from = foot.planted;
                    foot.to = home;
                    foot.step = Some(0.0);
                }
                None => {}
            }
        }

        // Pose: crouched, hunched, head low and forward.
        let lift = f.feet.iter().map(|ft| ft.step.map_or(0.0, |t| (t * std::f32::consts::PI).sin())).sum::<f32>();
        let pace = f.speed / WALK_SPEED;
        let bob = if human { -0.03 } else { -0.07 } * (1.0 - lift.min(1.0)) * pace;
        let pelvis = f.position + Vec3::Y * (body.pelvis_height + bob);
        let hunch = if human { 0.03 } else { 0.15 } * pace;
        // The creature folds far forward; the humanoid leans, hunched, with
        // its head pushed forward and down; the human stands upright.
        let (waist_lean, chest_lean) = if creature {
            (0.45, 1.0)
        } else if human {
            (0.02, 0.0)
        } else {
            (0.25, 0.55)
        };
        let waist = pelvis + tip(Vec3::Y, forward, waist_lean + hunch) * 0.35;
        let shoulders = waist + tip(Vec3::Y, forward, chest_lean + hunch) * if human { 0.4 } else { 0.6 };
        let neck_end = if creature {
            shoulders + tip(forward, Vec3::Y, 0.25) * 0.42
        } else if human {
            shoulders + tip(Vec3::Y, forward, 0.08) * 0.091
        } else {
            shoulders + tip(Vec3::Y, forward, 1.0) * 0.22
        };
        let skull_dir = if creature {
            tip(forward, Vec3::NEG_Y, 0.3)
        } else if human {
            Vec3::Y
        } else {
            tip(Vec3::Y, forward, 0.75)
        };
        let swing = |s: f32| {
            let leg = if s < 0.0 { 0 } else { 1 };
            -(f.feet[leg].planted - f.position).dot(forward) * 0.7
        };

        let mut poses = Vec::with_capacity(BONES.len());
        for bone in BONES {
            let pose = |a: Vec3, b: Vec3| Pose { start: a, rotation: frame(b - a, right), length: a.distance(b) };
            let p = match bone {
                Bone::Pelvis => pose(pelvis - Vec3::Y * 0.13, pelvis + Vec3::Y * 0.13),
                Bone::Waist => pose(pelvis, waist),
                Bone::Chest => pose(waist, shoulders),
                Bone::Neck if human => pose(shoulders - forward * 0.05 - Vec3::Y * 0.05, neck_end),
                Bone::Neck => pose(shoulders - forward * 0.05, neck_end),
                Bone::Skull if creature => pose(neck_end - skull_dir * 0.08, neck_end + skull_dir * 0.52),
                // Sunk into the neck, so it sits down on the shoulders
                // rather than perched on them.
                Bone::Skull if human => pose(neck_end - skull_dir * 0.07, neck_end + skull_dir * (0.33 * HEAD - 0.07)),
                Bone::Skull => pose(neck_end - skull_dir * 0.04, neck_end + skull_dir * 0.32),
                Bone::UpperArm(side) | Bone::Forearm(side) | Bone::Claw(side) => {
                    let s = side.sign();
                    // Shoulders raised on the humanoid, as if braced.
                    let raise = if creature { -0.05 } else if human { -0.04 } else { 0.07 };
                    let shoulder = shoulders + right * s * body.shoulder_width + Vec3::Y * raise;
                    // Arms forward from the hunched shoulders, elbows out;
                    // the human's hang at its sides, swinging a little.
                    let (reach, out, bent) = if creature {
                        (0.45, 0.18, 0.9)
                    } else if human {
                        (0.04, 0.07, 0.97)
                    } else {
                        (0.32, 0.18, 0.86)
                    };
                    let hang = (Vec3::NEG_Y + forward * (reach + swing(s) * if human { 0.5 } else { 1.0 }) + right * s * out).normalize();
                    let hand = shoulder + hang * (body.upper_arm + body.forearm) * bent;
                    let pole = if human { -forward } else { -forward + right * s * 0.7 };
                    let elbow = middle_joint(shoulder, hand, body.upper_arm, body.forearm, pole);
                    match bone {
                        Bone::UpperArm(_) => pose(shoulder, elbow),
                        Bone::Forearm(_) => pose(elbow, hand),
                        _ => {
                            let along = (hand - elbow).normalize();
                            let claw = if creature { 0.42 } else if human { 0.26 } else { 0.3 };
                            let curl = if human { 0.1 } else { 0.4 };
                            pose(hand, hand + tip(along, Vec3::NEG_Y, curl) * claw)
                        }
                    }
                }
                Bone::Thigh(side) | Bone::Shin(side) | Bone::Metatarsal(side) | Bone::Toe(side) => {
                    let s = side.sign();
                    let leg = if s < 0.0 { 0 } else { 1 };
                    let hip = pelvis + right * s * body.hip_width;
                    let toe = f.feet[leg].planted;
                    // The creature walks on its toes with the heel raised
                    // high behind; the humanoid's heel is on the ground.
                    let ankle = if creature {
                        toe + tip(Vec3::Y, -forward, 0.55) * body.foot
                    } else {
                        toe - forward * body.foot + Vec3::Y * 0.1
                    };
                    let knee = middle_joint(hip, ankle, body.thigh, body.shin, forward + right * s * 0.2);
                    let toe_length = if creature { 0.22 } else if human { 0.09 } else { 0.12 };
                    match bone {
                        Bone::Thigh(_) => pose(hip, knee),
                        Bone::Shin(_) => pose(knee, ankle),
                        Bone::Metatarsal(_) => pose(ankle, toe),
                        _ => pose(toe, toe + forward * toe_length - Vec3::Y * 0.02),
                    }
                }
            };
            poses.push(p);
        }
        f.poses = poses;
    }
}

/// Moves every fragment towards its place on its bone through a spring, with
/// a faint shiver.
fn follow(
    time: Res<Time>,
    figure: Single<&Figure>,
    mut elements: Query<(&mut Element, &mut Transform), Without<CoreLight>>,
    mut light: Single<&mut Transform, With<CoreLight>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    // The lamp sits in the middle of the chest.
    let chest = figure.poses[2];
    // The lamp sits in the chest's glowing core (lower for the human).
    let at = if figure.anatomy == Anatomy::Human { 0.38 } else { 0.55 };
    light.translation = chest.start + chest.rotation * Vec3::Y * chest.length * at;
    for (mut e, mut transform) in &mut elements {
        let pose = figure.poses[e.bone];
        if pose.length <= 0.0 {
            continue;
        }
        let local = Vec3::new(e.offset.x, e.offset.y * pose.length, e.offset.z);
        let shiver = if e.rigid {
            Vec3::ZERO
        } else {
            Vec3::new((t * 7.0 + e.phase).sin(), (t * 5.3 + e.phase * 1.7).sin(), (t * 6.1 + e.phase * 0.6).sin()) * 0.004
        };
        let target = pose.start + pose.rotation * local + shiver;
        let to = target - transform.translation;
        // Far away (first frame, or teleported): snap.
        if to.length() > 3.0 {
            transform.translation = target;
            e.velocity = Vec3::ZERO;
        } else {
            let accel = to * STIFFNESS - e.velocity * DAMPING;
            e.velocity += accel * dt;
            transform.translation += e.velocity * dt;
        }
        let rotation = pose.rotation * e.rotation;
        transform.rotation = transform.rotation.slerp(rotation, (dt * 14.0).min(1.0));
    }
}

/// A faceted skull spanning -1..1: octagonal rings from the chin (narrow,
/// set forward) through the cheekbones (widest) to the crown (bevelled),
/// with a corner rather than a face at the front, so a ridge runs down it.
fn skull() -> Mesh {
    // Each ring: height, width, depth, how far forward.
    let rings = [(-1.0f32, 0.32f32, 0.4f32, 0.28f32), (-0.45, 0.64, 0.72, 0.15), (0.15, 1.0, 0.92, 0.0), (0.7, 0.9, 1.0, -0.06), (1.0, 0.48, 0.64, -0.1)];
    let n = 8;
    let mut points = Vec::new();
    for &(y, w, d, f) in &rings {
        for i in 0..n {
            let a = std::f32::consts::TAU * i as f32 / n as f32 + std::f32::consts::FRAC_PI_2;
            points.push([a.cos() * w, y, a.sin() * d + f]);
        }
    }
    let mut faces: Vec<Vec<usize>> = vec![(0..n).collect(), ((rings.len() - 1) * n..rings.len() * n).collect()];
    for r in 0..rings.len() - 1 {
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![r * n + i, r * n + j, (r + 1) * n + j, (r + 1) * n + i]);
        }
    }
    let faces: Vec<&[usize]> = faces.iter().map(|f| f.as_slice()).collect();
    faceted(&points, &faces)
}

/// A flat-shaded convex mesh from corner points and faces (polygons of
/// indices); each face is turned to point away from the shape's centre.
fn faceted(points: &[[f32; 3]], faces: &[&[usize]]) -> Mesh {
    let center = points.iter().map(|&p| Vec3::from(p)).sum::<Vec3>() / points.len() as f32;
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for face in faces {
        let mut corners: Vec<Vec3> = face.iter().map(|&i| Vec3::from(points[i])).collect();
        let mut normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
        let middle = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        if normal.dot(middle - center) < 0.0 {
            corners.reverse();
            normal = -normal;
        }
        let base = positions.len() as u32;
        for c in &corners {
            positions.push(c.to_array());
            normals.push(normal.to_array());
        }
        for k in 1..corners.len() as u32 - 1 {
            indices.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}

/// A slow, uneven pulse of the glowing core and the lamp inside it.
fn pulse(
    time: Res<Time>,
    glow: Option<Res<Glow>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut light: Single<&mut PointLight, With<CoreLight>>,
) {
    let Some(glow) = glow else { return };
    let t = time.elapsed_secs();
    let beat = 0.55 + 0.45 * ((t * 1.6).sin() * 0.7 + (t * 0.53).sin() * 0.3).max(-1.0);
    if let Some(mut material) = materials.get_mut(&glow.0) {
        let g = GLOW * beat;
        material.emissive = LinearRgba::rgb(g, g, g);
    }
    light.intensity = CORE_LIGHT * beat;
}
