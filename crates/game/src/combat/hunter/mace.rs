//! The gladiator's weapon (the user's concept, 2026-10-31): a dense, even
//! ball of long thin spikes about a crystal core, on a haft of loose shards
//! (its arm's own stuff, nothing like the hook's strands), a long crystal
//! for a pommel. As the hook's needle splays into its burr, the head
//! blooms: its spikes thrust out further as the blow lands.
//!
//! (Three heads made to differ from the hook's burr, an ordered mace,
//! blades among needles, a swept comet, read as a staff, a plant or a broom,
//! not a weapon: what made the concept one was its spikes even all round,
//! down and back too, the user.)
//!
//! In a hand: `spawn` (its haft one mesh, each spike its own piece, so a
//! bloom only stretches them), bloomed by `bloom`. At rest and in bloom, on
//! the arena's plaza: `--opt weaponlab --opt clean --do "cam
//! 1169,1201.1,1000,90,-4"`.

use bevy::prelude::*;
use worldgen::noise::hash01;

use crate::player::tether::shard_mesh;

/// The haft's length and the pommel's (m), and how many shards the haft is.
/// (The concept's: its head's spikes reaching a third of its length about
/// it. A metre of haft under a head a quarter of that was a magic staff, the
/// user; then a fifth too short overall, the user, 2026-10-31: the haft and
/// pommel lengthened, the head kept; and a tenth again (a fifth was too
/// much, the user), the haft lengthened at its head, so that its lower part, `GRIPPED` long with its shards as
/// tuned about the hand, is as it was, and `TOP_SHARDS` more above it.)
const HAFT: f32 = 0.97;
const POMMEL: f32 = 0.3;
const GRIPPED: f32 = 0.83;
const HAFT_SHARDS: usize = 102;
const TOP_SHARDS: usize = 17;
/// One haft shard in this many pale, against the dark (the user's, for
/// contrast), none within `PALE_CLEAR` (m) of its head's middle (pale ones
/// there sat in the head, the user). Two big ones were side by side at the
/// hand (67 at its edge, 77 in it, found by showing each alone): 77 is
/// dark, and 82, in the fist, is carried down below the hand, beside its
/// pommel (the user: one of them below the hand).
const PALE: usize = 5;
const PALE_CLEAR: f32 = 0.15 * HEAD;
const PALE_HIDDEN: usize = 77;
const PALE_BELOW: usize = 82;
/// And 67, left at the hand, half its size (too prominent, the user).
const PALE_SMALL: usize = 67;
/// Its spikes: how many, how long at rest (m, each a little more or less)
/// and how much longer in bloom, and their half-width at the base.
const SPIKES: usize = 26;
const SPIKE: (f32, f32, f32) = (0.3 * HEAD, 1.35, 0.011 * HEAD);
/// Its head's size against the one first made, kept in proportion to its
/// haft and pommel as they lengthen (the user: it had lost it).
const HEAD: f32 = (HAFT + POMMEL) / (0.65 + 0.24);
/// Its core's radius (m). (Thicker spikes about a larger core ran together
/// at rest into a black blob; the concept's ball is airy.)
const CORE: f32 = 0.04 * HEAD;

/// Its core as `shard_mesh` takes it: a faceted crystal, a little longer
/// along the haft.
fn core() -> Vec<(Vec3, f32, f32)> {
    [Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z].iter().map(|&d| (d, CORE * if d.y != 0.0 { 1.4 } else { 1.0 }, CORE * 0.8)).collect()
}

/// Its spikes at rest, each its way out, length and half-width at its base:
/// even all round (a Fibonacci sphere), each turned and sized a little
/// apart.
fn spikes() -> Vec<(Vec3, f32, f32)> {
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    (0..SPIKES)
        .map(|k| {
            let r = |j: i32| hash01(k as i32, j, 3, 0x7b4) - 0.5;
            let y = 1.0 - 2.0 * (k as f32 + 0.5) / SPIKES as f32;
            let a = golden * k as f32;
            let ring = (1.0 - y * y).sqrt();
            let dir = (Vec3::new(a.cos() * ring, y, a.sin() * ring) + Vec3::new(r(0), r(1), r(2)) * 0.15).normalize();
            (dir, SPIKE.0 * (1.0 + 0.25 * r(3)), SPIKE.2)
        })
        .collect()
}

/// A spike's length and half-width in `bloom` (0 at rest, 1 thrust out).
fn bloomed(length: f32, width: f32, bloom: f32) -> (f32, f32) {
    let b = bloom.clamp(0.0, 1.0);
    (length * (1.0 + (SPIKE.1 - 1.0) * b), width * (1.0 - 0.15 * b))
}

/// Its head as `shard_mesh` takes it, about its middle (the haft along -Y),
/// in `bloom`.
fn head(bloom: f32) -> Vec<(Vec3, f32, f32)> {
    let mut out = core();
    out.extend(spikes().into_iter().map(|(dir, l, w)| {
        let (l, w) = bloomed(l, w, bloom);
        (dir, l, w)
    }));
    out
}

/// Its haft and pommel as two meshes: its dark shards and its pale ones.
fn haft_meshes() -> [Mesh; 2] {
    let mut all: [Option<Mesh>; 2] = [None, None];
    for (shards, at, turn, pale) in haft() {
        let piece = shard_mesh(&shards).transformed_by(Transform::from_translation(at).with_rotation(turn));
        match all[pale as usize].as_mut() {
            Some(m) => m.merge(&piece).expect("shard meshes alike"),
            None => all[pale as usize] = Some(piece),
        }
    }
    all.map(|m| m.expect("a haft"))
}

/// Its materials: the dark one, and its pale shards'.
fn materials(materials: &mut Assets<StandardMaterial>) -> [Handle<StandardMaterial>; 2] {
    [
        materials.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.055), metallic: 0.6, perceptual_roughness: 0.3, ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.85, 0.82), metallic: 0.1, perceptual_roughness: 0.35, ..default() }),
    ]
}

/// A weapon in a hand: how far bloomed (set by its wielder), and its spikes
/// (each piece, and its length and half-width at rest).
#[derive(Component)]
pub(crate) struct Mace {
    pub(crate) bloom: f32,
    spikes: Vec<(Entity, f32, f32)>,
}

/// How far below its head it is held (m): near its pommel.
pub(crate) const GRIP: f32 = HAFT - 0.1;

/// A weapon under `parent`, placed by `at` there (its scale undoing its
/// parent's: a creature's joints are scaled with its model), its head at its
/// own origin.
pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, parent: Entity, at: Transform) -> Entity {
    let [material, pale] = self::materials(materials);
    let root = commands.spawn((at, Visibility::default(), ChildOf(parent))).id();
    let [dark, light] = haft_meshes();
    commands.spawn((Mesh3d(meshes.add(dark)), MeshMaterial3d(material.clone()), Transform::IDENTITY, ChildOf(root)));
    commands.spawn((Mesh3d(meshes.add(light)), MeshMaterial3d(pale), Transform::IDENTITY, ChildOf(root)));
    commands.spawn((Mesh3d(meshes.add(shard_mesh(&core()))), MeshMaterial3d(material.clone()), Transform::IDENTITY, ChildOf(root)));
    // (One spike, a unit long and a unit wide at its base, stretched to
    // each.)
    let unit = meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 1.0)]));
    let spikes = spikes()
        .into_iter()
        .map(|(dir, length, width)| {
            let e = commands.spawn((Mesh3d(unit.clone()), MeshMaterial3d(material.clone()), Transform::from_rotation(Quat::from_rotation_arc(Vec3::Y, dir)).with_scale(Vec3::new(width, length, width)), ChildOf(root))).id();
            (e, length, width)
        })
        .collect();
    commands.entity(root).insert(Mace { bloom: 0.0, spikes });
    root
}

/// Each weapon's spikes stretched to its bloom.
pub(crate) fn bloom(maces: Query<&Mace, Changed<Mace>>, mut pieces: Query<&mut Transform>) {
    for mace in &maces {
        for &(e, length, width) in &mace.spikes {
            if let Ok(mut t) = pieces.get_mut(e) {
                let (l, w) = bloomed(length, width, mace.bloom);
                t.scale = Vec3::new(w, l, w);
            }
        }
    }
}

/// Its haft's shards (meshes as `shard_mesh` takes them, with where each sits
/// and how it is turned): a chunky, broken column along -Y from the head,
/// many small shards of all sizes at all angles, close, a few loose of it;
/// and its pommel, a stout crystal. (One size, upright, in a thin line: a
/// string of beads; fewer and bigger, loose: chunks, not the concept's.)
/// Each says whether it is pale.
pub(crate) fn haft() -> Vec<(Vec<(Vec3, f32, f32)>, Vec3, Quat, bool)> {
    let mut out = Vec::new();
    for k in 0..HAFT_SHARDS + TOP_SHARDS {
        let r = |j: i32| hash01(k as i32, j, 2, 0x7b3) - 0.5;
        // (How far down from its head, and whether clear of it for a pale
        // one: below, as tuned before it was lengthened.)
        let (down, clear) = if k < HAFT_SHARDS {
            let along = ((k as f32 + 0.5 + r(0) * 1.2) / HAFT_SHARDS as f32).clamp(0.0, 1.0);
            (HAFT - GRIPPED + along * GRIPPED, along * GRIPPED > PALE_CLEAR)
        } else {
            let along = (((k - HAFT_SHARDS) as f32 + 0.5 + r(0) * 1.2) / TOP_SHARDS as f32).clamp(0.0, 1.0);
            let down = along * (HAFT - GRIPPED);
            (down, down > PALE_CLEAR)
        };
        let loose = if k % 9 == 4 { 2.2 } else { 1.0 };
        let a = r(1) * std::f32::consts::TAU;
        let mut at = Vec3::new(a.cos(), 0.0, a.sin()) * (0.01 + 0.025 * (r(2) + 0.5)) * loose - Vec3::Y * down;
        if k == PALE_BELOW {
            at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.04 - Vec3::Y * (HAFT + 0.05);
        }
        // (Of all sizes: most small, a few big.)
        let size = (0.03 + 0.06 * (r(3) + 0.5).powi(2)) * if k == PALE_SMALL { 0.5 } else { 1.0 };
        let turn = Quat::from_rotation_y(r(4) * 6.0) * Quat::from_rotation_x(r(5) * 1.4) * Quat::from_rotation_z(r(6) * 1.4);
        // (Uneven, as broken: its two points of different lengths and girths.)
        let girth = size * (0.35 + 0.3 * (r(7) + 0.5));
        out.push((vec![(Vec3::Y, size, girth), (Vec3::NEG_Y, size * (0.4 + 0.6 * (r(8) + 0.5)), girth * (0.7 + 0.5 * (r(9) + 0.5)))], at, turn, k % PALE == 2 && clear && k != PALE_HIDDEN));
    }
    out.push((vec![(Vec3::Y, POMMEL * 0.35, 0.07), (Vec3::NEG_Y, POMMEL * 0.65, 0.07)], -Vec3::Y * (HAFT + POMMEL * 0.35), Quat::IDENTITY, false));
    out
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, lab).add_systems(Update, bloom);
}

/// It at rest and in bloom, side by side on the plaza a few metres before
/// where you start, tilted as the concept is drawn (`--opt weaponlab`).
fn lab(args: Res<crate::Args>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    if !args.opt("weaponlab") {
        return;
    }
    let [material, pale] = self::materials(&mut materials);
    let haft: Vec<(Handle<Mesh>, Vec3, Quat, bool)> = haft().into_iter().map(|(s, at, turn, light)| (meshes.add(shard_mesh(&s)), at, turn, light)).collect();
    for (k, bloom) in [0.0, 1.0].into_iter().enumerate() {
        let top = Vec3::new(1166.5, 1201.3, 1000.8 - k as f32 * 1.6);
        let turn = Quat::from_rotation_x(-0.7);
        commands.spawn((Mesh3d(meshes.add(shard_mesh(&head(bloom)))), MeshMaterial3d(material.clone()), Transform::from_translation(top).with_rotation(turn)));
        for (mesh, at, t, light) in &haft {
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(if *light { pale.clone() } else { material.clone() }), Transform::from_translation(top + turn * *at).with_rotation(turn * *t)));
        }
    }
}

