//! The swarm assembling into a hunter. When enough free swarmers bunch up
//! they fly together (with a grinding you can hear from afar) and, unless
//! enough of them are broken first, become a creature: a tall biped with
//! long arms, or a beast the size of a horse built like a big cat (`--opt
//! biped`, `--opt beast` to choose). Its body moves on a procedural rig (see
//! `rig.rs`): feet that plant and step, a spine that leads and follows, a
//! head that tracks you, a tail. What it is made of, dark shards and a few
//! glowing cores, hangs on the rig's bones by springs, so it lags, sways and
//! settles. Its head is one faceted skull, a muzzle and a hinged jaw that
//! opens as it crouches and strikes. Glow is kept for rare, powerful
//! creatures: these are dark with thin pale markings (stripes across the
//! torso, a brow over the eyes, bands above the paws), and their pale eyes
//! flare only when they look straight at you, like eyeshine. It stalks and attacks
//! after a crouch you can see coming: the
//! biped dashes, the beast pounces. It takes hits as a whole: each shard
//! jolts the part it strikes and staggers the body, and when its health is
//! gone the whole body bursts at once.

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};

use super::*;
use crate::rig::{Intent, Plan, Rig};

pub(super) mod cables;
pub(super) use cables::run as cables;

/// How many free swarmers weave themselves into a hunter, how long it takes,
/// and how near you it forms at most. Break one and the weave fails.
const GATHER_COUNT: usize = 3;
const GATHER_TIME: f32 = 3.5;
const GATHER_NEAREST: f32 = 6.0;
/// Seconds between assemblies.
const GATHER_COOLDOWN: f32 = 8.0;

const RECOVER: f32 = 1.1;
/// How far it sees you (m), with nothing built between its eyes and yours,
/// and hears the gun.
const SIGHT: f32 = 50.0;
const HEAR: f32 = 45.0;
/// For how long after it loses sight of you it still knows where you are
/// (s): it saw which way you went, so it follows you round the corner, not
/// only to it.
const GRACE: f32 = 1.0;
/// How long it casts about where it lost you before it gives up (s).
const SEARCH: f32 = 10.0;
/// How close a harrow is to you to snap rather than pounce (m).
const SNAP: f32 = 2.5;
/// The sprawl's flanking: how far round from where you look (radians, 120
/// degrees: at your back, a little to one side), how far off (m), how fast
/// it circles and darts (m/s; 10% up, the user: a bit slow).
const FLANK_ANGLE: f32 = 2.1;
const FLANK_OFF: f32 = 9.0;
const FLANK_SPEED: f32 = 9.9;
const DART_SPEED: f32 = 17.6;
/// How long it hangs back with no way round you before it comes at you
/// anyway, seen or not (s).
const HUNG: f32 = 2.5;
/// How near it hauls itself onto you from (m), and how fast (m/s).
const HAUL_FROM: f32 = 6.0;
const HAUL: f32 = 24.0;
/// How many of a blast's shards stagger it, and how hard each shoves it
/// (m/s, a harrow's weight; a biped, lighter, further).
const STAGGER: f32 = 4.0;
const SHOVE: f32 = 0.9;
const ATTACK: f32 = 30.0;
/// What it takes to break (two or three good shots at close range).
const HUNTER_HEALTH: f32 = 45.0;
/// How many of a blast's shards tear a beast's leg off (of 16: about point
/// blank). Not what a leg has taken over a fight: aimed at the body, a
/// shotgun's shards all but miss the legs (a few in each by its death).
const SEVER: u32 = 10;
/// The body's pieces: how stiffly they follow their bones (limbs stiffer,
/// so they stay limbs), and how they settle.
const STIFFNESS: f32 = 160.0;
const LIMB_STIFFNESS: f32 = 600.0;
/// No spring at all: fixed to its bone.
const RIGID: f32 = 0.0;
const DAMPING: f32 = 13.0;
/// How much of its bone's movement a piece is carried along with each frame
/// before its spring acts (the rest it trails behind).
const CARRY: f32 = 0.9;

#[derive(Component)]
pub(super) struct Assembly {
    centre: Vec3,
    time: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Biped,
    Beast,
}

#[derive(Clone, Copy, PartialEq)]
enum Stance {
    Stalk,
    Windup,
    /// Dashing (the biped) or in a pounce (the beast).
    Attack(Vec3),
    Recover,
    /// The beast: still, low, head locked on you.
    Freeze,
    /// The beast: building to a gallop before it pounces (or hauls).
    Charge,
    /// The beast's haul: stopped dead, rearing, jaws open; its forelimbs
    /// shooting out to spike into the ground either side of you; and its
    /// body hauled along them onto you, faster than it runs.
    Rear,
    Reach,
    Haul(Vec3),
    /// Thrown back by a blast, its run at you broken.
    Stagger,
}

/// What a hunter knows of you.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Aware {
    /// It has not seen you: it prowls your way (it has your scent, not
    /// where you are).
    Unaware,
    /// It sees you (or did a moment ago): it comes for you.
    Chasing,
    /// It lost sight of you (or heard you, or was hit): it runs to where you
    /// were.
    Pursuing,
    /// It got there and you are not: it casts about, seconds left.
    Searching(f32),
}

/// How many of a broken hunter's parts burst, flinging pieces and ichor.
const BURSTS: usize = 12;

#[derive(Component)]
pub(crate) struct Hunter {
    kind: Kind,
    rig: Rig,
    /// Its bones last frame (the pieces on them are carried along with
    /// them; see `flesh`).
    last_bones: Vec<crate::rig::Bone>,
    stance: Stance,
    timer: f32,
    pub(crate) health: f32,
    /// In a built place, its way to you (see `nav::steer`), and when to
    /// find it afresh.
    way: Vec<Vec3>,
    way_age: f32,
    /// What it knows of you: whether it hunts you, where it thinks you are
    /// and which way you were going, how long since it saw (or heard) you,
    /// and, searching, the spot it is looking in and how long it has been
    /// heading there.
    aware: Aware,
    known: Vec3,
    went: Vec3,
    unseen: f32,
    search: Option<(Vec3, f32)>,
    /// A sprawl: which side of you it flanks round (-1 left, 1 right); the
    /// spot it runs to, and how long until it looks for another.
    flank: f32,
    flank_spot: Vec3,
    flank_age: f32,
    /// How long it has hung back with no way round to your back (a flight,
    /// a narrow way): past a while it comes at you anyway.
    hung: f32,
    /// Hit since it last thought.
    alarmed: bool,
    /// This frame's shards in it: how many, and their directions summed.
    shards: u32,
    blast: Vec3,
    stun: f32,
    /// Hits this frame: the bone struck and the shot's direction (the pieces
    /// on that bone are jolted).
    hits: Vec<(usize, Vec3)>,
    struck: bool,
    /// How far its jaw is open (radians).
    jaw: f32,
    /// What it is doing beyond walking, and when it next does something.
    gesture: Gesture,
    gesture_in: f32,
}

/// Gestures that break up a prowl.
#[derive(Clone, Copy, PartialEq)]
enum Gesture {
    None,
    /// Stopped, staring, the head slowly tilting over (radians).
    Tilt(f32, f32),
    /// The head snapping aside and back (-1 left, 1 right).
    Twitch(f32, f32),
    /// Very low and slow.
    Creep(f32),
    /// A short trot.
    Burst(f32),
}

impl Heads {
    /// The kinds of load (see `slab.rs`).
    pub(super) fn loads(&self) -> &[super::slab::Kind] {
        &self.load
    }
}

impl Hunter {
    /// Its rig (for what it wears: see `clothlab.rs`).
    pub(super) fn rig(&self) -> &Rig {
        &self.rig
    }

    /// The biped candidates (the stalker, the porter) make no sound till
    /// they have their own: the hunters' coloured how they read (the user).
    fn silent(&self) -> bool {
        matches!(self.rig.plan.style, crate::rig::Style::Stilt | crate::rig::Style::Porter)
    }

    /// A shard struck bone `bone`, flying along `dir`.
    pub(super) fn hurt(&mut self, bone: usize, dir: Vec3, damage: f32) {
        self.health -= damage;
        self.shards += 1;
        self.blast += dir;
        self.stun = self.stun.max(0.12);
        self.hits.push((bone, dir));
        self.alarmed = true;
    }

    /// The nearest bone a ray hits within `max`, and how far.
    pub(super) fn ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, usize)> {
        self.rig
            .bones
            .iter()
            .enumerate()
            // (Not a leg it has lost.)
            .filter(|&(i, _)| self.rig.leg_of(i).is_none_or(|l| !self.rig.is_lost(l)))
            .filter_map(|(i, b)| b.ray(origin, dir, max).map(|d| (d, i)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }
}

/// A piece of a hunter's body, hung on one of its bones.
#[derive(Component)]
pub(super) struct Part {
    hunter: Entity,
    bone: usize,
    /// Where along the bone (0..1), and out from it in the bone's frame.
    along: f32,
    offset: Vec3,
    rotation: Quat,
    velocity: Vec3,
    stiffness: f32,
}

/// `spawn KIND [N]` at the console (see `console::Spawns`): hunters and buds
/// a metre before you (several, side by side), in a built place on the floor
/// nearest there.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    nav: Option<Res<crate::chasm::nav::Nav>>,
    assets: Res<Assets3>,
    heads: Res<Heads>,
    kit: Res<forms::Kit>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawns: ResMut<crate::console::Spawns>,
    camera: Single<&Transform, With<FlyCam>>,
) {
    let forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::Z);
    let across = Vec3::new(-forward.z, 0.0, forward.x);
    for (kind, count) in std::mem::take(&mut spawns.creatures) {
        // (Bishops are the bishops' to make: see `bishop::arrive`.)
        if kind == "bishop" {
            spawns.creatures.push((kind, count));
            continue;
        }
        for n in 0..count {
            let aside = (n as f32 - (count - 1) as f32 * 0.5) * 1.6;
            let mut at = camera.translation + forward * AHEAD + across * aside;
            at = Vec3::new(at.x, ground.at(at), at.z);
            if let Some(nav) = nav.as_deref()
                && let Some(j) = nav.nearest(camera.translation + forward * AHEAD + across * aside, 8)
            {
                at = nav.nodes[j as usize];
            }
            let seed = (time.elapsed_secs() * 1000.0) as u32 + n * 7919;
            if kind == "bud" {
                super::spawn_bud(&mut commands, &kit, &mut materials, at + Vec3::Y * 1.6, seed);
            } else if kind == "grove" {
                // Seven asleep, scattered over a few metres a little further
                // on, each on the floor under it.
                for k in 0..7 {
                    let r = |j: i32| hash01(seed as i32, k, j, 0x5a4) - 0.5;
                    let p = at + forward * 4.0 + Vec3::new(r(0), 0.0, r(1)) * 7.0;
                    let p = nav.as_deref().and_then(|nav| nav.nearest(p, 8).map(|j| nav.nodes[j as usize])).unwrap_or(Vec3::new(p.x, ground.at(p), p.z));
                    super::spawn_sleeper(&mut commands, &kit, &mut materials, p, seed.wrapping_mul(31).wrapping_add(k as u32));
                }
            } else {
                spawn_hunter(&mut commands, &args, &ground, &assets, &heads, &mut materials, seed, at, camera.translation, Some(&kind));
            }
        }
    }
}

/// How far before you `spawn` puts what it makes (m): right before you,
/// whole (at 1 m a harrow's body was round the camera; at 2 its jaws still
/// in your face, it being long).
pub(super) const AHEAD: f32 = 3.0;

/// A shard stuck in hunter `hunter` where it struck (`at`, on bone `bone`,
/// flying along `dir`), white hot a moment (see `Needle`): a piece of it,
/// carried with its bone, falling with a leg torn off.
#[allow(clippy::too_many_arguments)]
pub(super) fn embed(commands: &mut Commands, assets: &Assets3, hunter: Entity, h: &Hunter, bone: usize, at: Vec3, dir: Vec3, delay: f32) {
    let Some(b) = h.rig.bones.get(bone) else { return };
    let axis = b.b - b.a;
    let along = ((at - b.a).dot(axis) / axis.length_squared().max(1e-6)).clamp(0.0, 1.0);
    let rotation = b.rotation();
    let base = at - dir * NEEDLE_OUT;
    commands.spawn((
        Part { hunter, bone, along, offset: rotation.inverse() * (base - b.a.lerp(b.b, along)), rotation: rotation.inverse() * Quat::from_rotation_arc(Vec3::Y, dir), velocity: Vec3::ZERO, stiffness: RIGID },
        Needle { delay, life: NEEDLE_LIFE, size: NEEDLE_SIZE },
        Mesh3d(assets.shard.clone()),
        MeshMaterial3d(assets.bright.clone()),
        Transform::from_translation(base).with_scale(Vec3::ZERO),
    ));
}

/// A hunter's voice, kept at its head.
#[derive(Component)]
pub(super) struct Voice {
    hunter: Entity,
    /// Seconds until it may growl again.
    next: f32,
    count: u32,
}

/// A hunter's lower jaw: it opens about its hinge.
#[derive(Component)]
pub(super) struct Jaw;

/// The skulls and jaws, built once.
#[derive(Resource)]
pub(super) struct Heads {
    /// The beasts' heads: the harrow's, the angler's.
    beast: Vec<(Handle<Mesh>, Handle<Mesh>)>,
    biped: (Handle<Mesh>, Handle<Mesh>),
    /// A beast's leg segments: the upper, and the lower tapering to a prong.
    legs: (Handle<Mesh>, Handle<Mesh>),
    /// The porter's loads, each kind its parts (`--set slab`: see
    /// `slab.rs`).
    load: Vec<super::slab::Kind>,
    /// The porter's body pieces, other than shards (`--set piece=4`): rods
    /// (the user keeps shards or rods for it, till one is settled on, or two
    /// kinds of porter), a blade the size of a shard and a knot. (Blocks,
    /// tiles, pebbles, wedges and spheres, tried for it, are in the notes on
    /// pieces.)
    rods: (Handle<Mesh>, Handle<Mesh>),
}

/// A porter's load, following its back (see `loads`).
#[derive(Component)]
pub(super) struct Load(Entity);

/// Loads follow their porters' backs; a porter gone, its load is gone.
pub(super) fn loads(mut commands: Commands, hunters: Query<&Hunter>, mut loads: Query<(Entity, &Load, &mut Transform)>) {
    for (entity, load, mut transform) in &mut loads {
        let Ok(h) = hunters.get(load.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        if let Some((at, turn, _)) = h.rig.load() {
            *transform = Transform::from_translation(at).with_rotation(turn);
        }
    }
}

/// Where a beast head's eyes and brow sit: out from the bone and up, in
/// head radii, and along it (0..1).
const HEAD_EYES: [(f32, f32, f32); 2] = [(0.62, 0.32, 0.5), (0.86, 0.9, 0.3)];

/// A faceted solid from rings along +Y (y, half width, half height, how far
/// up): flat shaded, closed at both ends. "Up" is -Z, as in a bone's frame
/// on a level head.
fn rings_mesh(rings: &[(f32, f32, f32, f32)]) -> Mesh {
    const SIDES: usize = 8;
    let ring = |&(y, w, h, up): &(f32, f32, f32, f32)| -> Vec<Vec3> {
        (0..SIDES)
            .map(|k| {
                let a = (k as f32 + 0.5) / SIDES as f32 * std::f32::consts::TAU;
                Vec3::new(a.cos() * w, y, -(a.sin() * h + up))
            })
            .collect()
    };
    let pts: Vec<Vec<Vec3>> = rings.iter().map(ring).collect();
    let (mut positions, mut normals): (Vec<[f32; 3]>, Vec<[f32; 3]>) = (Vec::new(), Vec::new());
    let centre = |r: &[Vec3]| r.iter().copied().sum::<Vec3>() / r.len() as f32;
    let mut tri = |a: Vec3, b: Vec3, c: Vec3, inside: Vec3| {
        let mut n = (b - a).cross(c - a).normalize_or(Vec3::Y);
        let (b, c) = if n.dot((a + b + c) / 3.0 - inside) < 0.0 {
            n = -n;
            (c, b)
        } else {
            (b, c)
        };
        for p in [a, b, c] {
            positions.push(p.to_array());
            normals.push(n.to_array());
        }
    };
    for r in 0..pts.len() - 1 {
        let inside = (centre(&pts[r]) + centre(&pts[r + 1])) * 0.5;
        for k in 0..SIDES {
            let k2 = (k + 1) % SIDES;
            tri(pts[r][k], pts[r][k2], pts[r + 1][k2], inside);
            tri(pts[r][k], pts[r + 1][k2], pts[r + 1][k], inside);
        }
    }
    for r in [0, pts.len() - 1] {
        let c = centre(&pts[r]);
        let inside = c + Vec3::Y * if r == 0 { 1.0 } else { -1.0 };
        for k in 0..SIDES {
            tri(c, pts[r][k], pts[r][(k + 1) % SIDES], inside);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // A long, narrow, angular head: a ridge of cranium behind, the brow low
    // over the eyes, a long muzzle; nothing round about it.
    let beast = rings_mesh(&[(0.0, 0.5, 0.65, 0.1), (0.2, 0.68, 0.8, 0.22), (0.42, 0.55, 0.5, 0.06), (0.72, 0.38, 0.34, -0.04), (1.0, 0.16, 0.18, -0.1)]);
    let beast_jaw = rings_mesh(&[(0.0, 0.5, 0.2, 0.0), (0.55, 0.36, 0.15, 0.0), (1.0, 0.14, 0.08, 0.0)]);
    // A shorter, rounder head with a jutting brow.
    let biped = rings_mesh(&[(0.0, 0.75, 0.8, 0.0), (0.3, 1.0, 1.0, 0.1), (0.6, 0.85, 0.75, 0.05), (0.85, 0.55, 0.5, -0.1), (1.0, 0.3, 0.3, -0.15)]);
    let biped_jaw = rings_mesh(&[(0.0, 0.6, 0.25, 0.0), (0.6, 0.45, 0.2, 0.0), (1.0, 0.25, 0.12, 0.0)]);
    // An angler fish's: a broad flat skull, its front the upper lip, raised
    // over a gaping mouth; under it a huge jaw jutting past the snout and
    // curling up at the front.
    let angler = rings_mesh(&[(0.0, 0.85, 0.75, 0.15), (0.3, 1.05, 0.8, 0.35), (0.6, 1.0, 0.55, 0.3), (0.85, 0.85, 0.35, 0.22), (1.0, 0.7, 0.2, 0.18)]);
    // (The jaw is deep: a heavy faceted chin and throat bulging below the
    // teeth, reaching back under the skull.)
    let angler_jaw = rings_mesh(&[(-0.2, 0.7, 0.55, -0.35), (0.3, 1.0, 0.78, -0.48), (0.75, 1.1, 0.62, -0.32), (1.05, 1.05, 0.42, -0.08), (1.25, 0.9, 0.25, 0.25)]);
    let beast_jaw = meshes.add(beast_jaw);
    // (Rods: thin and straight, taut, the shard's footprint, from just below
    // its root to its length along +Y, so a shard's scaling fits them.)
    let rods = (meshes.add(Cylinder::new(0.012, 0.75).mesh().resolution(6).build().translated_by(Vec3::Y * 0.325)), meshes.add(Cylinder::new(0.035, 0.5).mesh().resolution(6).build()));
    let load = super::slab::build(&mut commands, &mut meshes, &mut materials);
    commands.insert_resource(Heads {
        rods,
        load,
        beast: vec![(meshes.add(beast), beast_jaw), (meshes.add(angler), meshes.add(angler_jaw))],
        biped: (meshes.add(biped), meshes.add(biped_jaw)),
        // Hard and faceted, swelling just below the joint and narrowing
        // down it; the lower down to a point that plants in the ground.
        legs: (
            meshes.add(rings_mesh(&[(0.0, 0.8, 0.8, 0.0), (0.12, 1.1, 1.25, 0.15), (0.55, 0.85, 0.95, 0.05), (1.0, 0.55, 0.6, 0.0)])),
            meshes.add(rings_mesh(&[(0.0, 0.7, 0.7, 0.0), (0.15, 0.9, 1.05, 0.1), (0.65, 0.55, 0.6, 0.0), (0.92, 0.25, 0.25, 0.0), (1.0, 0.02, 0.02, 0.0)])),
        ),
    });
}

/// One of a hunter's eyes (each hunter's pair shares a material, which
/// flares as it looks at you).
#[derive(Component)]
pub(super) struct Eye {
    hunter: Entity,
    /// The bone it is on: its head (4), or a tripod's body (2: its eyes
    /// round it, each seeing out its own way).
    bone: usize,
    /// Where on the head: across and up (head radii), and along it (0..1).
    at: Vec3,
    material: Handle<StandardMaterial>,
}

/// Starts assemblies, draws their members together, and turns a finished
/// one into a hunter (or lets it fall apart if too many were broken).
#[allow(clippy::too_many_arguments)]
pub(super) fn gather(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    nav: Option<Res<crate::chasm::nav::Nav>>,
    assets: Res<Assets3>,
    heads: Res<Heads>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut director: ResMut<Director>,
    player: Single<&Transform, (With<Player>, Without<Swarmer>)>,
    mut assemblies: Query<(Entity, &mut Assembly)>,
    hunters: Query<(), With<Hunter>>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    if director.alive <= 0.0 {
        return;
    }
    director.next_gather -= dt;

    // `--opt hunters`: no swarm, no assembly; a hunter, already built,
    // appears a little way off whenever there is none (`--set crowd=N`: N of
    // them at once, round you, for measuring their cost; `--set
    // hunters_at=M`, M metres off, 25 else).
    // `--opt specimen`: one hunter standing still in front of you, to look
    // at (`--set specimen_at` metres ahead, `specimen_yaw` degrees turned
    // from facing you, `jaw` radians open, `stretch` 0..1).
    if args.opt("specimen") {
        if hunters.is_empty() && director.next_gather <= 0.0 {
            let ahead = player.forward();
            let at = player.translation + Vec3::new(ahead.x, 0.0, ahead.z).normalize_or(Vec3::Z) * args.num("specimen_at", 3.5);
            let face = at + Quat::from_rotation_y(args.num("specimen_yaw", 0.0).to_radians()) * (player.translation - at);
            spawn_hunter(&mut commands, &args, &ground, &assets, &heads, &mut materials, 7, at, face, None);
            director.next_gather = f32::MAX;
        }
        return;
    }
    if args.opt("hunters") {
        if hunters.is_empty() && director.next_gather <= 0.0 {
            let crowd = args.num("crowd", 1.0).max(1.0) as u32;
            for n in 0..crowd {
                let k = (time.elapsed_secs() * 1000.0) as u32 + n * 7919;
                let a = hash01(k as i32, 1, 0, 0x6b8) * std::f32::consts::TAU;
                let mut at = player.translation + Vec3::new(a.cos(), 0.0, a.sin()) * args.num("hunters_at", 25.0);
                // (In a built place, on its floors, 15 to 40 m off.)
                if let Some(nav) = nav.as_deref()
                    && let Some(q) = nav.spot(player.translation, (15.0, 40.0), &|_| true, &|j| hash01(k as i32, j as i32, 2, 0x6b9), 400)
                {
                    at = q;
                }
                // (`--opt chained`: a porter kept on a chain by a stalker,
                // beside it to start, behind it once they walk: see
                // `chain.rs`.)
                if args.opt("chained") {
                    let porter = spawn_hunter(&mut commands, &args, &ground, &assets, &heads, &mut materials, k, at, player.translation, Some("porter"));
                    let ahead = (player.translation - at).with_y(0.0).normalize_or(Vec3::Z).cross(Vec3::Y);
                    let stalker = spawn_hunter(&mut commands, &args, &ground, &assets, &heads, &mut materials, k + 1, at + ahead * 2.0, player.translation, Some("stalker"));
                    commands.entity(stalker).insert(super::chain::Keeps(porter));
                } else {
                    spawn_hunter(&mut commands, &args, &ground, &assets, &heads, &mut materials, k, at, player.translation, None);
                }
            }
            director.next_gather = 4.0;
        }
        return;
    }

    // Start one: enough free swarmers, wherever they are. (`--opt swarm`:
    // never; the swarm alone.)
    let allowed = 1 + (director.alive / 90.0) as usize;
    if !args.opt("swarm") && assemblies.is_empty() && hunters.iter().count() < allowed && director.next_gather <= 0.0 {
        let near: Vec<Entity> = swarm
            .iter()
            .filter(|(_, _, s)| s.mode == Mode::Free)
            .map(|(e, ..)| e)
            .take(GATHER_COUNT)
            .collect();
        if near.len() >= GATHER_COUNT {
            let mut centre = near.iter().filter_map(|&e| swarm.get(e).ok()).map(|(_, t, _)| t.translation).sum::<Vec3>() / near.len() as f32;
            // Not on top of the player: a little way off.
            let away = Vec3::new(centre.x - player.translation.x, 0.0, centre.z - player.translation.z);
            if away.length() < GATHER_NEAREST {
                centre = player.translation + away.normalize_or(Vec3::X) * GATHER_NEAREST;
            }
            centre.y = ground.at(centre) + 2.0;
            // (In a built place, over the floor nearest it: their middle can
            // be in the rock, where a hunter would form in a wall.)
            if let Some(nav) = nav.as_deref()
                && let Some(j) = nav.nearest(centre, 8)
            {
                centre = nav.nodes[j as usize] + Vec3::Y * 2.0;
            }
            let assembly = commands.spawn((Assembly { centre, time: 0.0 }, Transform::from_translation(centre))).id();
            commands.spawn((
                AudioPlayer::new(assets.assemble.clone()),
                PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.6)),
                Transform::from_translation(centre),
            ));
            for &e in &near {
                if let Ok((_, _, mut s)) = swarm.get_mut(e) {
                    s.mode = Mode::Gather(assembly);
                }
            }
            director.next_gather = GATHER_COOLDOWN;
            info!("the swarm assembles ({} of them)", near.len());
        }
    }

    // Members are drawn straight in by their cables (see `web.rs`).
    let t = time.elapsed_secs();
    for (_, mut transform, mut s) in &mut swarm {
        let Mode::Gather(a) = s.mode else { continue };
        // (An assembly started this frame exists from the next.)
        let Ok((_, assembly)) = assemblies.get(a) else { continue };
        let to = assembly.centre - transform.translation;
        // (Reeled in: slowly, then faster, the web tightening over the whole
        // gathering.)
        let k = assembly.time / GATHER_TIME;
        let want = to * (0.3 + 2.5 * k * k) + Vec3::new((t * 7.0 + s.phase).sin(), 0.0, (t * 6.0 + s.phase).cos());
        let change = (want - s.velocity).clamp_length_max(40.0 * dt);
        s.velocity += change;
        // (Swept: what is built stops it, as in its flight.)
        transform.translation = ground.sweep(&Collider::sphere(0.35), transform.translation, s.velocity * dt);
        transform.rotate_local_y(dt * 9.0);
    }

    // Finished, or too many broken.
    for (entity, mut assembly) in &mut assemblies {
        assembly.time += dt;
        let members: Vec<Entity> = swarm.iter().filter(|(_, _, s)| s.mode == Mode::Gather(entity)).map(|(e, ..)| e).collect();
        if members.len() < GATHER_COUNT {
            for &e in &members {
                if let Ok((_, _, mut s)) = swarm.get_mut(e) {
                    s.mode = Mode::Free;
                }
            }
            info!("the assembly falls apart");
            commands.entity(entity).despawn();
            continue;
        }
        if assembly.time < GATHER_TIME {
            continue;
        }
        // The members are taken into the body.
        for &e in &members {
            commands.entity(e).despawn();
        }
        commands.entity(entity).despawn();
        spawn_hunter(&mut commands, &args, &ground, &assets, &heads, &mut materials, entity.index_u32(), assembly.centre, player.translation, None);
    }
}

/// A hunter, built, standing at `centre` and facing the player.
#[allow(clippy::too_many_arguments)]
fn spawn_hunter(
    commands: &mut Commands,
    args: &Args,
    ground: &crate::ground::Ground,
    assets: &Assets3,
    heads: &Heads,
    materials: &mut Assets<StandardMaterial>,
    seed: u32,
    centre: Vec3,
    player: Vec3,
    want: Option<&str>,
) -> Entity {
    // (`want`: a kind asked for, at the console: harrow, angler, biped,
    // sprawl; or for a pair: stalker, porter.)
    // A biped, or a beast: a harrow (a narrow head, two rows of
    // needles) or an angler (a huge underbite jaw, uneven needles leaning
    // in). `--opt biped`, `harrow`, `angler`, or `beast` (either beast).
    let kind = if let Some(want) = want {
        if matches!(want, "biped" | "stalker" | "porter") { Kind::Biped } else { Kind::Beast }
    } else if args.opt("beast") || args.opt("harrow") || args.opt("angler") {
        Kind::Beast
    } else if args.opt("biped") || hash01(seed as i32, 0, 0, 0x6b1) < 1.0 / 3.0 {
        Kind::Biped
    } else {
        Kind::Beast
    };
    let angler = kind == Kind::Beast && want.map_or(args.opt("angler") || (!args.opt("harrow") && hash01(seed as i32, 1, 0, 0x6b1) < 0.5), |w| w == "angler");
    let feet = Vec3::new(centre.x, ground.at(centre), centre.z);
    let to = player - feet;
    let floor = |x: f32, z: f32| ground.at(Vec3::new(x, centre.y, z));
    // (The angler's body is a size larger than the harrow's; its head is the
    // same size, so smaller for the body.)
    let plan = if angler {
        let head = Plan::beast().head;
        Plan { head, ..Plan::beast().scaled(1.15) }
    } else if kind == Kind::Beast {
        Plan::beast()
    } else {
        Plan::biped()
    };
    // (`--opt sprawl`: the beast moving another way; `--opt stalker`, the
    // biped on stilts.)
    let plan = if kind == Kind::Beast && want.map_or(args.opt("sprawl"), |w| w == "sprawl") { Plan::sprawl() } else { plan };
    let plan = if kind == Kind::Biped && want.map_or(args.opt("stalker"), |w| w == "stalker") { Plan::stalker() } else { plan };
    let plan = if kind == Kind::Biped && want.map_or(args.opt("porter"), |w| w == "porter") { Plan::porter() } else { plan };
    // Bones: three of torso, the neck, the head, then two per leg and per
    // arm (upper, lower), then the tail.
    let limbs = 5..5 + 2 * (plan.legs.len() + plan.arms.len());
    let sprawl = plan.style == crate::rig::Style::Sprawl;
    let radial = plan.radial;
    // (The biped candidates: no voice; see `Hunter::silent`.)
    let silent = matches!(plan.style, crate::rig::Style::Stilt | crate::rig::Style::Porter);
    let porter = plan.style == crate::rig::Style::Porter;
    let mut rig = Rig::new(plan, feet, to.x.atan2(to.z), seed, &floor);
    rig.lost = vec![false; rig.plan.legs.len()];
    let bones = rig.bones.clone();
    let health = HUNTER_HEALTH * if args.opt("tough") { 10.0 } else { 1.0 };
    let hunter = commands
        .spawn((
            Hunter { kind, rig, last_bones: Vec::new(), stance: Stance::Stalk, timer: 1.0, health, stun: 0.0, hits: Vec::new(), struck: false, jaw: 0.0, gesture: Gesture::None, gesture_in: 2.0, way: Vec::new(), way_age: 0.0, aware: Aware::Unaware, known: Vec3::ZERO, went: Vec3::ZERO, unseen: f32::MAX, search: None, alarmed: false, shards: 0, blast: Vec3::ZERO, flank: if hash01(seed as i32, 3, 0, 0x6c7) < 0.5 { -1.0 } else { 1.0 }, flank_spot: Vec3::ZERO, flank_age: 0.0, hung: 0.0 },
            Transform::from_translation(feet + Vec3::Y * 1.5),
            Visibility::default(),
        ))
        .id();
    // The porter's load: a plain slab on its back (see `loads`).
    if porter {
        let k = args.num("slab", 0.0) as usize;
        let parts = heads.load.iter().find(|(id, _)| *id == k).map(|(_, parts)| parts).unwrap_or_else(|| panic!("--set slab={k}: the porter's loads are {:?}", heads.load.iter().map(|(id, _)| *id).collect::<Vec<_>>()));
        let load = commands.spawn((Load(hunter), Transform::from_translation(feet + Vec3::Y * 2.0), Visibility::default())).id();
        for (mesh, material) in parts {
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), ChildOf(load)));
        }
    }
    // Its voice, at its head: growls now and then (see `voice`).
    if !silent {
        commands.spawn((Voice { hunter, next: 1.0, count: 0 }, Transform::from_translation(centre), Visibility::default()));
    }
    info!("{} forms", if angler { "an angler" } else if sprawl { "a sprawl" } else if kind == Kind::Beast { "a harrow" } else { "a biped" });

    // Its body: shards along every bone, thicker bones more and bigger,
    // with pale markings.
    let seed = seed;
    // (`--set piece=4`: the porter built of rods instead of shards: the
    // shape of its pieces as its own (the user). Its markings stay shards.)
    let piece = if porter { args.num("piece", 0.0) as i32 } else { 0 };
    let (blade, knot) = match piece {
        0 => (assets.shard.clone(), assets.swarmer.clone()),
        4 => heads.rods.clone(),
        k => panic!("--set piece={k}: the porter's pieces are 0 (shards) or 4 (rods); the others were taken out (their recipes are in the notes)"),
    };
    let pale = |commands: &mut Commands, bone: usize, along: f32, offset: Vec3, dir: Vec3, length: f32, width: f32| {
        commands.spawn((
            Part { hunter, bone, along, offset, rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::Y)), velocity: Vec3::ZERO, stiffness: if bone == 4 { RIGID } else { STIFFNESS * 1.5 } },
            Mesh3d(assets.shard.clone()),
            MeshMaterial3d(assets.pale.clone()),
            Transform::from_scale(Vec3::new(width / 0.025, length / 0.75, width / 0.025)),
        ));
    };
    for (i, bone) in bones.iter().enumerate() {
        // (Woven cable instead; see `cables.rs`.)
        if cables::replaces(args, i) {
            continue;
        }
        // (A tripod: no neck nor head; its eyes are round its body.)
        if radial && (i == 3 || i == 4) {
            continue;
        }
        let length = bone.a.distance(bone.b);
        if limbs.contains(&i) && kind == Kind::Beast {
            // A beast's leg: hard, a spider's, one faceted piece a segment,
            // the lower down to a prong that plants; a spur out past each
            // knee, a pale band above the prong.
            let lower = (i - limbs.start) % 2 == 1;
            let rr = bone.radius;
            let mesh = if lower { &heads.legs.1 } else { &heads.legs.0 };
            commands.spawn((
                Part { hunter, bone: i, along: 0.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(assets.dark.clone()),
                Transform::from_translation(bone.a).with_scale(Vec3::new(rr, length, rr)),
            ));
            if lower {
                let spur = 0.2 + length * 0.18;
                commands.spawn((
                    Part { hunter, bone: i, along: 0.0, offset: Vec3::ZERO, rotation: Quat::from_rotation_arc(Vec3::Y, Vec3::NEG_Y), velocity: Vec3::ZERO, stiffness: RIGID },
                    Mesh3d(assets.shard.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.a).with_scale(Vec3::new(rr * 0.8 / 0.025, spur / 0.75, rr * 0.8 / 0.025)),
                ));
                for k in 0..3 {
                    let a = k as f32 / 3.0 * std::f32::consts::TAU;
                    let (c, sn) = (a.cos(), a.sin());
                    pale(commands, i, 0.8, Vec3::new(c, 0.0, sn) * rr * 0.6, Vec3::new(-sn, 0.0, c), rr * 1.3, 0.015);
                }
            }
            continue;
        }
        if limbs.contains(&i) {
            // A limb: long blades laid along it, overlapping, so it reads
            // as one solid faceted limb; and a heavy paw at a foot.
            let lower = (i - limbs.start) % 2 == 1;
            let count = ((length / 0.18).round() as usize).clamp(3, 8);
            for k in 0..count {
                let r = |j: i32| hash01(seed as i32, (i * 97 + k) as i32, j, 0x6b4) - 0.5;
                let along = (k as f32 + 0.5) / count as f32 + r(0) * 0.1;
                let a = r(1) * std::f32::consts::TAU;
                let offset = Vec3::new(a.cos(), 0.0, a.sin()) * bone.radius * 0.3;
                let rotation = Quat::from_euler(EulerRot::XYZ, r(2) * 0.3, r(3) * 6.0, r(4) * 0.3);
                let width = bone.radius * (0.9 + 0.4 * (r(5) + 0.5)) / 0.025;
                let scale = Vec3::new(width, length * 0.6 / 0.75, width);
                commands.spawn((
                    Part { hunter, bone: i, along: along - 0.3 / count as f32, offset, rotation, velocity: Vec3::ZERO, stiffness: LIMB_STIFFNESS },
                    Mesh3d(blade.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.a.lerp(bone.b, along)).with_scale(scale),
                ));
            }
            if lower {
                // A pale band above the paw.
                for k in 0..3 {
                    let a = k as f32 / 3.0 * std::f32::consts::TAU;
                    let (c, sn) = (a.cos(), a.sin());
                    pale(commands, i, 0.72, Vec3::new(c, 0.0, sn) * bone.radius * 1.1, Vec3::new(-sn, 0.0, c), bone.radius * 1.3, 0.02);
                }
                commands.spawn((
                    Part { hunter, bone: i, along: 1.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: LIMB_STIFFNESS },
                    Mesh3d(knot.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.b).with_scale(Vec3::splat(bone.radius * 3.2)),
                ));
            }
            continue;
        }
        if i == 4 {
            // The head: one skull, a hinged jaw beneath, a thin pale brow.
            let (skull, jaw) = if kind == Kind::Beast { &heads.beast[angler as usize] } else { &heads.biped };
            let (across, up, _) = HEAD_EYES[angler as usize];
            let rr = bone.radius;
            commands.spawn((
                Part { hunter, bone: i, along: 0.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
                Mesh3d(skull.clone()),
                MeshMaterial3d(assets.dark.clone()),
                Transform::from_translation(bone.a).with_scale(Vec3::new(rr, length, rr)),
            ));
            commands.spawn((
                Jaw,
                Part { hunter, bone: i, along: 0.3, offset: Vec3::new(0.0, 0.0, rr * 0.3), rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
                Mesh3d(jaw.clone()),
                MeshMaterial3d(assets.dark.clone()),
                Transform::from_translation(bone.a).with_scale(Vec3::new(rr, length * 0.66, rr)),
            ));
            for side in [-1.0, 1.0] {
                if !angler {
                    pale(commands, i, 0.4, Vec3::new(side * rr * across, 0.0, -rr * (up + 0.34)), Vec3::new(-side * 0.35, 1.0, 0.15), length * 0.3, 0.012);
                }
            }
            if kind == Kind::Beast {
                // Teeth, bared when the jaw opens. Each: where along the
                // head, out from the middle (head radii), how long (head
                // radii), on the skull or the jaw, a lean.
                let mut teeth: Vec<(f32, f32, f32, bool, f32)> = Vec::new();
                if angler {
                    // Round the whole rim, of very uneven lengths, leaning
                    // in. Upper ones hang from the lip; lower ones stand on
                    // the jaw out to its upturned front (past the snout).
                    let teeth_rnd = |k: i32, j: i32| hash01(seed as i32, k, j, 0x7d1);
                    for k in 0..6 {
                        let t = 0.5 + 0.09 * k as f32 + 0.05 * (teeth_rnd(k, 2) - 0.5);
                        let l = 0.4 + 1.5 * teeth_rnd(k, 0).powi(2);
                        teeth.push((t, 0.95 - 0.35 * (k as f32 / 5.0) + 0.1 * (teeth_rnd(k, 3) - 0.5), l, true, -0.6 * teeth_rnd(k, 4)));
                    }
                    for k in 0..7 {
                        // (Along the jaw: past 1.0 it reaches beyond the skull.)
                        let t = 0.55 + 0.1 * k as f32 + 0.06 * (teeth_rnd(k, 5) - 0.5);
                        let l = 0.5 + 1.9 * teeth_rnd(k, 1).powi(2);
                        teeth.push((t, 0.95 - 0.5 * (k as f32 / 6.0) + 0.1 * (teeth_rnd(k, 6) - 0.5), l, false, -0.7 * teeth_rnd(k, 7)));
                    }
                } else {
                    // Two rows, the inner one shorter and set back between
                    // the outer teeth.
                    for k in 0..18 {
                        let inner = k >= 10;
                        let j = if inner { k - 10 } else { k };
                        let t = if inner { 0.46 + 0.065 * j as f32 } else { 0.42 + 0.056 * j as f32 };
                        let w = (0.42 - 0.25 * (t - 0.42)) * if inner { 0.62 } else { 1.0 };
                        let l = (0.6 - 0.3 * (t - 0.42)) * if inner { 0.7 } else { 1.0 };
                        teeth.push((t, w, l, true, 0.0));
                        teeth.push((t, w * 0.85, l * 0.8, false, 0.0));
                    }
                }
                for (k, &(t, w, l, upper, lean)) in teeth.iter().enumerate() {
                    for side in [-1.0, 1.0] {
                        // Crooked: each side leans its own way.
                        // Crooked: each side leans its own way; the angler's all
                        // lean in, towards the middle of the mouth.
                        let lean = if angler { lean * side * (0.6 + 0.4 * hash01(seed as i32, k as i32, 2, 0x7d2)) } else { lean * side * if k % 2 == 0 { 1.0 } else { -0.6 } };
                        // The angler's rake (forward or back) and thickness vary
                        // tooth by tooth.
                        let q = |j: i32| hash01(seed as i32, k as i32 * 2 + (side > 0.0) as i32, j, 0x7d3);
                        let (rake, thick) = if angler { (-0.15 + 0.7 * q(0), 0.45 + 0.7 * q(1)) } else { (0.25, 0.9) };
                        let (along, offset, dir) = if upper {
                            (t, Vec3::new(side * w * rr, 0.0, rr * 0.22), Vec3::new(lean, rake, 1.0))
                        } else {
                            (0.3, Vec3::new(side * w * rr, length * 0.66 * (t - 0.3) / 0.7, rr * 0.22), Vec3::new(lean, rake, -1.0))
                        };
                        let mut tooth = commands.spawn((
                            Part { hunter, bone: i, along, offset, rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize()), velocity: Vec3::ZERO, stiffness: RIGID },
                            Mesh3d(assets.shard.clone()),
                            MeshMaterial3d(assets.pale.clone()),
                            Transform::from_scale(Vec3::new(thick, l * rr / 0.75, thick)),
                        ));
                        if !upper {
                            tooth.insert(Jaw);
                        }
                    }
                }
            }
            continue;
        }
        if args.opt("wiry") {
            // Fibres: thin, long, bundled along the bone and splaying from
            // it, gaps between them; now and then a spine bristling out.
            let count = ((length / 0.06) * (bone.radius / 0.12).clamp(0.6, 2.0)).round().clamp(4.0, 40.0) as usize;
            for k in 0..count {
                let r = |j: i32| hash01(seed as i32, (i * 97 + k) as i32, j, 0x6b9) - 0.5;
                let along = k as f32 / count as f32 + r(0) / count as f32;
                let a = r(1) * std::f32::consts::TAU;
                let out = bone.radius * (0.2 + 0.85 * (r(2) + 0.5));
                let mut offset = Vec3::new(a.cos() * out, 0.0, a.sin() * out);
                if kind == Kind::Beast && i < 3 && offset.z > 0.0 {
                    offset.z *= 0.45;
                }
                let spine = r(3) > 0.4;
                let dir = if spine {
                    Vec3::new(a.cos() * 0.8, -0.6, a.sin() * 0.8)
                } else {
                    Vec3::new(r(4) * 0.5, 1.0, r(5) * 0.5)
                };
                let long = if spine { bone.radius * (1.0 + 1.2 * (r(6) + 0.5)) } else { length * (0.5 + 0.7 * (r(6) + 0.5)) };
                let thin = if spine { 0.6 } else { 0.35 + 0.35 * (r(7) + 0.5) };
                commands.spawn((
                    Part { hunter, bone: i, along, offset, rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize()), velocity: Vec3::ZERO, stiffness: STIFFNESS },
                    Mesh3d(assets.shard.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.a).with_scale(Vec3::new(thin, long / 0.75, thin)),
                ));
            }
            continue;
        }
        let count = ((length / 0.11) * (bone.radius / 0.12).clamp(0.6, 2.2)).round().clamp(3.0, 40.0) as usize;
        for k in 0..count {
            let r = |j: i32| hash01(seed as i32, (i * 97 + k) as i32, j, 0x6b2) - 0.5;
            let along = k as f32 / count as f32 + r(0) / count as f32;
            let a = r(1) * std::f32::consts::TAU;
            let out = bone.radius * (0.35 + 0.6 * (r(2) + 0.5));
            let mut offset = Vec3::new(a.cos() * out, 0.0, a.sin() * out);
            // A beast's belly is lean: what hangs under the torso (+Z in
            // its frame) sits close in.
            if kind == Kind::Beast && !radial && i < 3 && offset.z > 0.0 {
                offset.z *= 0.45;
            }
            // Knots, and now and then a long blade swept back.
            let long = r(3) > 0.3;
            let rotation = if long {
                Quat::from_rotation_arc(Vec3::Y, Vec3::new(a.cos() * 0.6, -0.8, a.sin() * 0.6).normalize())
            } else {
                Quat::from_euler(EulerRot::XYZ, r(4) * 6.0, r(5) * 6.0, r(6) * 6.0)
            };
            // The neck thick at its base, tapering to the head.
            let taper = if i == 3 { 1.25 - 0.6 * along } else { 1.0 };
            let size = bone.radius * (0.9 + 0.8 * (r(7) + 0.5)) * taper;
            let p = bone.a.lerp(bone.b, along) + bone.rotation() * offset;
            let (mesh, scale) = if long { (blade.clone(), Vec3::new(size * 3.0, size * 1.4, size * 3.0)) } else { (knot.clone(), Vec3::splat(size * 1.6)) };
            commands.spawn((
                Part { hunter, bone: i, along, offset, rotation, velocity: Vec3::ZERO, stiffness: STIFFNESS },
                Mesh3d(mesh),
                MeshMaterial3d(assets.dark.clone()),
                Transform::from_translation(p).with_scale(scale),
            ));
        }
        // In a bone's frame Y runs along it, X to the body's right, and
        // -Z is the back (up on a beast's level torso).
        if kind == Kind::Beast && !radial && (i == 2 || i == 3) {
            // Muscle over the shoulders and up the back of the neck: a
            // heavy hump of mass on top, where its strength is.
            let (from, to) = if i == 2 { (0.45, 1.0) } else { (0.0, 0.45) };
            for k in 0..10 {
                let r = |j: i32| hash01(seed as i32, (i * 53 + k) as i32, j, 0x6b7) - 0.5;
                let along = from + (to - from) * (k as f32 + 0.5 + r(0) * 0.5) / 10.0;
                let offset = Vec3::new(r(1) * bone.radius * 0.9, 0.0, -bone.radius * (0.55 + 0.3 * (r(2) + 0.5)));
                let size = bone.radius * (1.1 + 0.5 * (r(3) + 0.5));
                commands.spawn((
                    Part { hunter, bone: i, along, offset, rotation: Quat::from_euler(EulerRot::XYZ, r(4) * 6.0, r(5) * 6.0, r(6) * 6.0), velocity: Vec3::ZERO, stiffness: STIFFNESS },
                    Mesh3d(assets.swarmer.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.a).with_scale(Vec3::splat(size * 1.6)),
                ));
            }
        }
        if i < 3 {
            // Thin stripes lying across the back.
            for k in 0..2 {
                let r = |j: i32| hash01(seed as i32, (i * 31 + k) as i32, j, 0x6b6) - 0.5;
                let along = (k as f32 + 0.5 + r(0) * 0.4) / 2.0;
                for m in 0..3 {
                    let a = (m as f32 - 1.0) * 0.5 + r(1) * 0.3;
                    let (c, sn) = (a.cos(), a.sin());
                    let out = Vec3::new(sn, 0.0, -c) * bone.radius * 1.02;
                    pale(commands, i, along, out, Vec3::new(c, r(2) * 0.4, sn), bone.radius * 0.5, 0.015);
                }
            }
        }
    }
    // Pale pinprick eyes, flaring when it looks at you (see `flesh`).
    // (Not hazed, so eyeshine shows in the swarm's darkness.)
    let eyes = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.8),
        perceptual_roughness: 0.4,
        fog_enabled: false,
        ..default()
    });
    let (across, up, along) = HEAD_EYES[angler as usize];
    if radial {
        // A tripod: three round its body, high up, a third of a turn apart.
        for k in 0..3 {
            let a = k as f32 / 3.0 * std::f32::consts::TAU;
            let own = materials.add(materials.get(&eyes).cloned().unwrap_or_default());
            commands.spawn((
                Eye { hunter, bone: 2, at: Vec3::new(a.cos() * 0.95, a.sin() * 0.95, 0.75), material: own.clone() },
                Mesh3d(assets.core.clone()),
                MeshMaterial3d(own),
                Transform::from_translation(feet).with_scale(Vec3::splat(0.17)),
            ));
        }
    } else {
        for side in [-1.0f32, 1.0] {
            commands.spawn((
                Eye { hunter, bone: 4, at: Vec3::new(side * across, up, along), material: eyes.clone() },
                Mesh3d(assets.core.clone()),
                MeshMaterial3d(eyes.clone()),
                Transform::from_translation(feet).with_scale(Vec3::splat(0.13)),
            ));
        }
    }
    hunter
}

/// Hunters stalk and attack; a broken one bursts.
#[allow(clippy::too_many_arguments)]
pub(super) fn hunt(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    nav: Option<Res<crate::chasm::nav::Nav>>,
    assets: Res<Assets3>,
    mut director: ResMut<Director>,
    ichor: Res<ichor::Ichor>,
    mut player: Single<(&Transform, &mut Player), Without<Hunter>>,
    mut hunters: Query<(Entity, &mut Hunter, &mut Transform, Option<&super::chain::Keeps>), Without<Player>>,
    parts: Query<(Entity, &Part, &Transform), (Without<Hunter>, Without<Player>)>,
    eyes: Query<(Entity, &Eye)>,
    mut gun: ResMut<Gun>,
    mut said: Local<f32>,
    mut heard: Local<(u32, Option<(Vec3, Vec3)>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let target = player.0.translation;
    // Which way you look, across the ground (a sprawl keeps out of it).
    let ahead = Vec3::new(player.0.forward().x, 0.0, player.0.forward().z).normalize_or(Vec3::Z);
    // A shot this frame (heard as far as `HEAR`); which way you are going
    // (your pace, smoothed; a jump, as on dying, ignored).
    let shot = gun.shots != heard.0;
    heard.0 = gun.shots;
    let (last, pace) = heard.1.unwrap_or((target, Vec3::ZERO));
    let now = ((target - last) / dt.max(1e-4)).with_y(0.0);
    let pace = if now.length() < 15.0 { pace.lerp(now, 1.0 - (-dt * 6.0).exp()) } else { pace };
    heard.1 = Some((target, pace));
    let went = pace.normalize_or(Vec3::ZERO);
    // (`--opt huntlog`: where each is, and how far it has to go to you, every
    // second: to see them find their way.)
    *said -= dt;
    let log = args.opt("huntlog") && *said <= 0.0;
    if log {
        *said = 1.0;
    }
    // (Where each is, and which way it faces: a chained pair's each walks
    // by where the other is.)
    let spots: std::collections::HashMap<Entity, (Vec3, f32)> = hunters.iter().map(|(e, h, ..)| (e, (h.rig.root, h.rig.heading))).collect();
    for (entity, mut h, mut transform, keeps) in &mut hunters {
        if h.health <= 0.0 {
            // Broken: the whole body bursts at once.
            let at = h.rig.chest();
            commands.spawn((
                AudioPlayer::new(assets.death.clone()),
                PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(if h.silent() { 0.0 } else { 2.0 })),
                Transform::from_translation(at),
            ));
            let knock = h.rig.knock + h.blast.with_y(0.0) * SHOVE;
            // (A dozen parts burst, spread over the body, each spraying
            // more: a third of them all, a hound's some 80 bursts and 1700
            // pieces flung in one frame, held a frame up twice as long as
            // the others, a hitch at every kill.)
            let mine: Vec<(Entity, Vec3)> = parts.iter().filter(|(_, part, _)| part.hunter == entity).map(|(e, _, t)| (e, t.translation)).collect();
            let every = (mine.len() / BURSTS).max(1);
            for (k, &(e, p)) in mine.iter().enumerate() {
                if k % every == 0 && k / every < BURSTS {
                    let out = (p - at).normalize_or(Vec3::Y) * 9.0 + knock;
                    shatter_quiet(&mut commands, &assets, p, out, e.index_u32());
                    ichor::spray(&mut commands, &ichor, p, out.normalize_or(Vec3::Y), 3.0, e.index_u32());
                }
                commands.entity(e).despawn();
            }
            for (e, eye) in &eyes {
                if eye.hunter == entity {
                    commands.entity(e).despawn();
                }
            }
            director.kills += 1;
            info!("a hunter is broken");
            commands.entity(entity).despawn();
            continue;
        }

        let beast = h.kind == Kind::Beast;
        // (The sprawl, a flanker: see its stances below.)
        let sprawl = beast && h.rig.plan.style == crate::rig::Style::Sprawl;
        // A blast of `SEVER` shards or more tears off a beast's leg: the one
        // nearest where they struck (one leg a blast).
        let mut torn = None;
        if beast && h.shards >= SEVER && !h.hits.is_empty() {
            let struck = h.hits.iter().map(|&(b, _)| (h.rig.bones[b].a + h.rig.bones[b].b) * 0.5).sum::<Vec3>() / h.hits.len() as f32;
            torn = (0..h.rig.plan.legs.len()).filter(|&l| !h.rig.is_lost(l)).min_by(|&a, &b| {
                let at = |l: usize| h.rig.bones[5 + 2 * l].b.distance(struck);
                at(a).total_cmp(&at(b))
            });
        }
        // Shot: the blast's shards together shove it, the more of them the
        // harder: its run at you stopped, and it thrown back, its body
        // reeling; enough of them stagger it, and knock it out of a pounce.
        if h.shards > 0 {
            let n = std::mem::take(&mut h.shards) as f32;
            let away = std::mem::take(&mut h.blast).with_y(0.0).normalize_or(Vec3::ZERO);
            let shove = (n * SHOVE / if beast { 1.0 } else { 0.8 }).min(12.0);
            let into = h.rig.velocity.dot(-away);
            if into > 0.0 {
                h.rig.velocity += away * into * (n / 6.0).min(1.0);
            }
            if let Some(vy) = h.rig.airborne {
                h.rig.velocity += away * shove;
                h.rig.airborne = Some(vy.min(1.0));
            } else {
                h.rig.knock = (h.rig.knock + away * shove).clamp_length_max(12.0);
            }
            h.rig.reel(away * (n / 12.0).min(1.0));
            if args.opt("huntlog") {
                info!("huntlog: {} shot, {n} shards: shoved {shove:.1} m/s{}{}", entity.index_u32(), if h.rig.airborne.is_some() { ", out of the air" } else { "" }, if n >= STAGGER { ", staggered" } else { "" });
            }
            if n >= STAGGER {
                h.stance = Stance::Stagger;
                h.timer = 0.25 + 0.04 * n;
                h.gesture = Gesture::None;
            }
        }
        // The leg torn off bursts at the joint, its hard pieces fall away,
        // and the body sags at that corner.
        if let Some(leg) = torn {
            h.rig.lost[leg] = true;
            let (upper, lower) = (5 + 2 * leg, 6 + 2 * leg);
            let joint = h.rig.bones[upper].a;
            let out = (joint - h.rig.chest()).with_y(0.0).normalize_or(Vec3::X);
            shatter(&mut commands, &assets, joint, h.rig.knock + out * 3.0, entity.index_u32() * 13 + leg as u32);
            ichor::spray(&mut commands, &ichor, joint, (out + Vec3::Y).normalize(), 2.5, entity.index_u32() * 17 + leg as u32);
            for (e, part, t) in &parts {
                if part.hunter == entity && (part.bone == upper || part.bone == lower) {
                    let r = |j: i32| hash01(e.index_u32() as i32, j, 0, 0x6c5) - 0.5;
                    commands.entity(e).remove::<Part>().insert(Debris {
                        velocity: out * 3.0 + Vec3::Y * 3.5 + h.rig.knock * 0.5 + Vec3::new(r(0), r(1), r(2)) * 2.0,
                        spin: Vec3::new(r(3), r(4), r(5)) * 6.0,
                        life: 4.0,
                        total: 4.0,
                        size: t.scale,
                    });
                }
            }
            if args.opt("huntlog") {
                info!("huntlog: {} lost a {} leg", entity.index_u32(), if h.rig.plan.legs[leg].root == crate::rig::Root::Chest { "fore" } else { "hind" });
            }
        }
        let root = h.rig.root;
        // What it knows of you. It sees you: in reach, nothing built between
        // its eyes and yours. It hears the gun, and knows where it was hit
        // from. Out of sight a moment it still knows where you went; then
        // it runs to where that was, and casts about there, and in the end
        // gives up and prowls (it has your scent, roughly).
        let eye = h.rig.head().0;
        let sees = eye.distance(target) < SIGHT && ground.clear(eye, target);
        let alarmed = std::mem::take(&mut h.alarmed) || (shot && root.distance(target) < HEAR);
        h.unseen = if sees || alarmed { 0.0 } else { h.unseen + dt };
        if h.unseen < GRACE {
            h.known = target;
            h.went = went;
        }
        let was = h.aware;
        h.aware = match h.aware {
            _ if sees => Aware::Chasing,
            Aware::Chasing if h.unseen < GRACE => Aware::Chasing,
            _ if alarmed => Aware::Pursuing,
            Aware::Chasing => Aware::Pursuing,
            Aware::Searching(t) if t <= dt => Aware::Unaware,
            Aware::Searching(t) => Aware::Searching(t - dt),
            a => a,
        };
        // Roused (it sees or hears you, from prowling or searching): a beast
        // stops dead a moment, staring, before it runs.
        let roused = matches!(was, Aware::Unaware | Aware::Searching(_)) && matches!(h.aware, Aware::Chasing | Aware::Pursuing);
        if roused && beast && matches!(h.stance, Stance::Stalk | Stance::Freeze) {
            h.stance = Stance::Freeze;
            h.timer = 0.35;
            h.gesture = Gesture::None;
        }
        if let Some(s) = h.search.as_mut() {
            s.1 += dt;
        }
        // Where it heads: for you, seen (or prowling, by scent); for where
        // you were; for the spot it is searching.
        let goal = match h.aware {
            // A sprawl hunting you, circling: to a spot at your back, to one
            // side or the other, 9 m off (sweeping round as you turn).
            // (Only near you, you in its sight: running from it, the spot at
            // your back was where it stood, and it held there, at the head of
            // a flight above all, you out of its sight below, the user. Else
            // it follows you along the floors.)
            Aware::Chasing if sprawl && sees && Vec2::new(target.x - root.x, target.z - root.z).length() < FLANK_OFF + 3.0 && matches!(h.stance, Stance::Stalk | Stance::Freeze | Stance::Recover) => {
                h.flank_age -= dt;
                if h.flank_age <= 0.0 {
                    h.flank_age = 0.25;
                    // The first of a few spots at your back, its side first,
                    // on a floor near you (9 m behind you, in a corridor, was
                    // as often in the rock: it stood still, the way there
                    // none); failing all, hanging back where it is, 8 m off,
                    // for you to look away.
                    let on_floor = |spot: Vec3| match nav.as_deref() {
                        Some(n) => n.nearest(spot, 6).map(|j| n.nodes[j as usize]),
                        None => Some(spot),
                    };
                    let fits = |q: Vec3| {
                        let far = Vec2::new(q.x - target.x, q.z - target.z).length();
                        (4.0..14.0).contains(&far) && (q.y - target.y).abs() < 4.5
                    };
                    let tries = [h.flank, -h.flank].into_iter().flat_map(|side| [(FLANK_ANGLE, FLANK_OFF), (2.6, FLANK_OFF), (1.6, FLANK_OFF), (FLANK_ANGLE, 6.0), (2.6, 6.0)].map(move |(a, d)| (side, a, d)));
                    let found = tries.filter_map(|(side, a, d)| on_floor(target + Quat::from_rotation_y(side * a) * ahead * d)).find(|&q| fits(q));
                    // (Hanging back, it creeps in, 8 m off to 4: it stood
                    // still on a flight, you watching it, waiting forever,
                    // the user.)
                    h.hung = if found.is_some() { 0.0 } else { h.hung + 0.25 };
                    let back = on_floor(target + (root - target).with_y(0.0).normalize_or(-ahead) * (8.0 - h.hung * 1.6).max(4.0)).unwrap_or(root);
                    h.flank_spot = found.unwrap_or(back);
                }
                h.flank_spot
            }
            Aware::Chasing | Aware::Unaware => target,
            Aware::Pursuing => h.known,
            Aware::Searching(_) => h.search.map_or(h.known, |s| s.0),
        };
        let chasing = h.aware == Aware::Chasing;
        let flat = Vec3::new(goal.x - root.x, 0.0, goal.z - root.z);
        let mut distance = flat.length();
        let mut toward = flat.normalize_or(Vec3::Z);
        // (A straight run at you, a leap or a dash: only with you in sight
        // and the way to you about straight; in a built place, a leap
        // through a wall, or off a ledge, it fell into the void.)
        let mut straight = chasing;
        let mut lost = false;
        // In a built place: there by the way along its floors (the furthest
        // spot on along it in sight), and as far as that way is.
        if let Some(nav) = nav.as_deref() {
            let mut way = std::mem::take(&mut h.way);
            let mut age = h.way_age;
            // (A new goal, a new way at once.)
            if way.last().is_some_and(|&end| end.distance(goal) > 4.0) {
                age = 0.0;
            }
            // (Heading for a spot its body gets to, not only one it sees: a
            // line of sight past a corner aimed it into the corner, and it
            // stuck there, pressed head on into the wall, the user.)
            let body = if beast { 0.4 } else { 0.3 };
            if let Some((ahead, left)) = nav.steer(root, goal, &mut way, &mut age, dt, &|a, b| ground.passes(a + Vec3::Y * 0.6, b + Vec3::Y * 0.6, body)) {
                toward = Vec3::new(ahead.x - root.x, 0.0, ahead.z - root.z).normalize_or(toward);
                straight = straight && left < flat.length() * 1.2 + 1.5;
                distance = left;
            } else {
                lost = true;
                // Off the floors' spots (knocked into a corner, say): back
                // onto the nearest first, not pressing at you through what
                // is in the way.
                if let Some(j) = nav.nearest(root, 8) {
                    let q = nav.nodes[j as usize];
                    toward = Vec3::new(q.x - root.x, 0.0, q.z - root.z).normalize_or(toward);
                }
            }
            straight = straight && ground.clear(root + Vec3::Y * 1.0, target);
            h.way = way;
            h.way_age = age;
        }
        // There, and you are not (or there is no way there): it searches.
        if h.aware == Aware::Pursuing && (distance < 2.0 || lost) {
            h.aware = Aware::Searching(SEARCH);
            h.search = None;
        }
        // Searching: a spot near where it lost you, mostly on the way you
        // were going, one it cannot see into from where it stands; another
        // when it gets there (or cannot, or has been on its way too long).
        if matches!(h.aware, Aware::Searching(_)) && (lost || h.search.is_none_or(|(s, age)| Vec2::new(s.x - root.x, s.z - root.z).length() < 1.5 || age > 5.0)) {
            let r = |k: i32| hash01(entity.index_u32() as i32, (time.elapsed_secs() * 13.0) as i32, k, 0x6c1);
            let spot = (0..12).find_map(|k| {
                let a = r(k * 2) * std::f32::consts::TAU;
                let mut dir = Vec3::new(a.cos(), 0.0, a.sin());
                if k < 8 && dir.dot(h.went) < 0.0 {
                    dir = -dir;
                }
                let mut q = h.known + dir * (3.0 + 10.0 * r(k * 2 + 1));
                if let Some(nav) = nav.as_deref() {
                    q = nav.nodes[nav.nearest(q, 4)? as usize];
                }
                (k >= 8 || !ground.clear(eye, q + Vec3::Y * 1.2)).then_some(q)
            });
            h.search = Some((spot.unwrap_or(h.known), 0.0));
        }
        if args.opt("huntlog") && std::mem::discriminant(&was) != std::mem::discriminant(&h.aware) {
            info!("huntlog: {} {:?} -> {:?} (you {:.1} m off, {}; {:.1} m on to where it was going{})", entity.index_u32(), was, h.aware, root.distance(target), if sees { "in sight" } else if alarmed { "heard" } else { "unseen" }, distance, if lost { ", no way there" } else { "" });
        }
        if log {
            info!("huntlog: {} at {:.1}, {:?}, {:.1} m to go{}, {} at {:.1} m/s", entity.index_u32(), h.rig.root, h.aware, distance, if h.way.is_empty() { " (no way)" } else { "" }, match h.stance { Stance::Stalk => "stalking", Stance::Freeze => "still", Stance::Charge => "charging", Stance::Windup => "gathering", Stance::Attack(_) => "attacking", Stance::Recover => "recovering", Stance::Stagger => "staggered", Stance::Rear => "rearing", Stance::Reach => "reaching", Stance::Haul(_) => "hauling" }, h.rig.speed());
        }
        // Seeing you, it looks at you; else where it is going.
        let look = if sees { target } else { root + toward * 6.0 + Vec3::Y * 1.3 };
        // Hunting you, a beast does not prowl: it runs (and, the hunt over,
        // it slows to a prowl).
        if beast && !sprawl && matches!(h.aware, Aware::Chasing | Aware::Pursuing) && h.stance == Stance::Stalk {
            h.stance = Stance::Charge;
            h.timer = 4.0;
            h.gesture = Gesture::None;
        }
        if beast && !sprawl && matches!(h.aware, Aware::Unaware | Aware::Searching(_)) && h.stance == Stance::Charge {
            h.stance = Stance::Stalk;
            h.timer = 1.0;
        }
        h.timer -= dt;
        let stunned = h.stun > 0.0;
        h.stun -= dt;
        let around = Vec3::new(-toward.z, 0.0, toward.x);
        let r = |k: i32| hash01(entity.index_u32() as i32, (time.elapsed_secs() * 7.0) as i32, k, 0x6b5);
        h.rig.rear = if matches!(h.stance, Stance::Rear | Stance::Reach) { 1.0 } else { 0.0 };
        // (`porter_step`: the porter's steps that many times as long, 2
        // else: slower, and so further, at the same pace; live, at the
        // console too.)
        h.rig.step_scale = args.num("porter_step", 2.0);
        // (`stilt_vault`, `stilt_creep`, `stilt_beat`: the stalker's walk,
        // live: see `Rig::stilt_tune`.)
        h.rig.stilt_tune = (args.num("stilt_vault", crate::rig::STILT_VAULT), args.num("stilt_creep", crate::rig::STILT_GATHER.0), args.num("stilt_beat", crate::rig::STILT_BEAT_SCALE));
        if !matches!(h.stance, Stance::Reach | Stance::Haul(_)) {
            h.rig.grab = None;
        }
        let mut intent = Intent { velocity: Vec3::ZERO, look, crouch: 0.0, paw: None, tilt: 0.0, head_rate: 0.0, stretch: 0.0, still: false };
        if args.opt("specimen") {
            // Straight ahead from its body (not its head, or each glance
            // would turn it), and dead still.
            intent.look = h.rig.root + Vec3::new(h.rig.heading.sin(), 1.4, h.rig.heading.cos()) * 8.0;
            intent.stretch = args.num("stretch", 0.0);
            intent.still = true;
        } else if stunned {
        } else if h.stance == Stance::Stagger {
            // Low, sliding on with the shove; once down (and the time up), on
            // again.
            intent.crouch = 0.35;
            if h.timer <= 0.0 && h.rig.airborne.is_none() {
                h.stance = Stance::Stalk;
                h.timer = 0.4;
            }
        } else if args.opt("tame") || matches!(h.rig.plan.style, crate::rig::Style::Stilt | crate::rig::Style::Porter) {
            // (The stalker and the porter never hunt: their only motion, for
            // now, what has been tuned in tame mode, the user.)
            // (`--opt tame`, for captures: it circles at a distance, side on,
            // its pace changing every 8 s: prowl, trot, gallop.)
            let keep = args.num("tame_at", 14.0);
            let fixed = args.num("tame_pace", 0.0);
            // (The porter at its own walking pace, the user's: 0.7 m/s; the
            // stalker at its own, 1 m/s, upright: at the hounds' paces,
            // crouched, its gait was wrong, the user.)
            let porter = h.rig.plan.style == crate::rig::Style::Porter;
            let stalker = h.rig.plan.style == crate::rig::Style::Stilt;
            let pace = if fixed > 0.0 { fixed } else if porter { 0.7 } else if stalker { 1.0 } else { [2.5, 6.0, 12.0][(time.elapsed_secs() / 8.0) as usize % 3] };
            intent.velocity = (around + toward * ((distance - keep) / 3.0).clamp(-1.0, 1.0)).normalize_or(around) * pace;
            // (`--opt tame_line`: back and forth along a straight line across
            // in front of you instead (along x, `tame_at` off in z), `tame_span`
            // m long (16), turning
            // at each end: to judge a walk straight on.)
            if args.opt("tame_line") {
                let span = args.num("tame_span", 16.0);
                let leg = (time.elapsed_secs() * pace / span).rem_euclid(2.0);
                let way = Vec3::X * if leg < 1.0 { 1.0 } else { -1.0 };
                let off = Vec3::Z * ((target.z + keep - root.z) / 3.0).clamp(-1.0, 1.0);
                intent.velocity = (way + off).normalize_or(way) * pace;
            }
            intent.crouch = if pace < 3.0 && !stalker { 0.35 } else { 0.0 };
        } else if sprawl {
            // The flanker: it never comes at you from before you. Hunting you,
            // it runs low and fast round to your back and keeps there as you
            // turn; out of your sight, near, the way clear, it darts in and
            // snaps; seen as it comes (more than 5 m off), it breaks off and
            // circles the other way; after a bite, it circles the other way.
            let near = Vec2::new(target.x - root.x, target.z - root.z).length();
            let seen_by_you = (root - target).with_y(0.0).normalize_or(Vec3::Z).dot(ahead);
            match h.stance {
                Stance::Charge => {
                    intent.velocity = toward * DART_SPEED;
                    intent.crouch = 0.6;
                    if seen_by_you > 25f32.to_radians().cos() && near > 5.0 && h.hung < HUNG {
                        h.stance = Stance::Stalk;
                        h.flank = -h.flank;
                        h.timer = 1.0;
                        if args.opt("huntlog") {
                            info!("huntlog: {} seen as it darts: breaks off", entity.index_u32());
                        }
                    } else if distance < SNAP && straight {
                        h.stance = Stance::Windup;
                        h.timer = 0.12;
                    } else if h.timer <= 0.0 || !chasing {
                        h.stance = Stance::Stalk;
                        h.timer = 1.0;
                    }
                }
                Stance::Windup => {
                    intent.velocity = toward * DART_SPEED * 0.5;
                    intent.crouch = 1.0;
                    intent.stretch = 1.0;
                    if h.timer <= 0.0 {
                        // (Carrying the dart's speed: at 7 m/s a runner left it
                        // behind, never bitten.)
                        h.rig.leap(toward, 14.3, 1.5);
                        h.stance = Stance::Attack(toward);
                        h.timer = 1.0;
                    }
                }
                Stance::Attack(_) => {
                    intent.stretch = 1.0;
                    if h.rig.airborne.is_none() && h.timer < 0.9 {
                        h.stance = Stance::Recover;
                        h.timer = RECOVER * 0.6;
                        h.struck = false;
                        h.flank = -h.flank;
                        h.hung = 0.0;
                    }
                }
                Stance::Recover => {
                    intent.velocity = toward * FLANK_SPEED * 0.6;
                    intent.crouch = 0.4;
                    if h.timer <= 0.0 {
                        // Round to your other side a while before it darts
                        // again (every 1.5 s, a bite at a time, it was
                        // relentless).
                        h.stance = Stance::Stalk;
                        h.timer = 2.5 + 1.5 * r(8);
                    }
                }
                // Circling (or prowling, or casting about for you).
                _ => {
                    let pace = if matches!(h.aware, Aware::Chasing | Aware::Pursuing) { FLANK_SPEED } else { 3.5 };
                    intent.velocity = toward * pace * (distance / 1.0).min(1.0);
                    intent.crouch = 0.35;
                    if h.stance == Stance::Freeze && h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                    }
                    // (Out of your sight; or, no way round you, having hung
                    // back a while, even in it.)
                    if chasing && h.stance == Stance::Stalk && h.timer <= 0.0 && (seen_by_you < 55f32.to_radians().cos() || h.hung >= HUNG) && near < 14.0 && ground.clear(root + Vec3::Y, target) {
                        h.stance = Stance::Charge;
                        h.timer = 2.5;
                        if args.opt("huntlog") {
                            info!("huntlog: {} darts in from {:.1} m, {:.0} degrees off your look", entity.index_u32(), near, seen_by_you.clamp(-1.0, 1.0).acos().to_degrees());
                        }
                    }
                }
            }
        } else if beast {
            match h.stance {
                // Prowling: low and slow, closing in; now and then it freezes,
                // or breaks into a charge.
                Stance::Stalk => {
                    intent.crouch = 0.35;
                    // Never quite straight, never quite steady: the path
                    // wanders either side, the pace comes and goes.
                    let t = time.elapsed_secs() + entity.index_u32() as f32 * 1.7;
                    let wander = (t * 0.55).sin() * 0.45 + (t * 1.3).sin() * 0.15;
                    let pace = 0.65 + 0.5 * (0.5 + 0.5 * (t * 0.37).sin());
                    let searching = matches!(h.aware, Aware::Searching(_));
                    let base = if distance > 18.0 || searching { toward } else { (toward * 0.4 + around * 0.8).normalize_or(toward) };
                    let dir = Quat::from_rotation_y(if searching { wander * 0.3 } else { wander });
                    let dir = dir * base;
                    intent.velocity = dir * if searching { 7.5 } else if distance > 30.0 { 6.5 } else if distance > 18.0 { 4.5 } else { 3.0 } * pace;
                    if searching {
                        intent.crouch = 0.2;
                    }
                    // Now and then a gesture breaks the prowl.
                    h.gesture_in -= dt;
                    if h.gesture == Gesture::None && h.gesture_in <= 0.0 {
                        let pick = r(5);
                        let side = if r(7) < 0.5 { -1.0 } else { 1.0 };
                        h.gesture = if pick < 0.3 {
                            Gesture::Tilt(1.4 + 1.2 * r(6), side * (0.5 + 0.4 * r(9)))
                        } else if pick < 0.6 {
                            Gesture::Twitch(0.25, side)
                        } else if pick < 0.85 {
                            Gesture::Creep(1.5 + 1.5 * r(6))
                        } else {
                            Gesture::Burst(0.8 + 0.6 * r(6))
                        };
                        h.gesture_in = 1.5 + 3.0 * r(8);
                    }
                    let forward = Vec3::new(h.rig.heading.sin(), 0.0, h.rig.heading.cos());
                    h.gesture = match h.gesture {
                        Gesture::Tilt(t, angle) => {
                            // Stopped dead, staring, the head rolling over.
                            intent.velocity = Vec3::ZERO;
                            intent.crouch = 0.3;
                            intent.tilt = angle;
                            intent.still = true;
                            if t > dt { Gesture::Tilt(t - dt, angle) } else { Gesture::None }
                        }
                        Gesture::Twitch(t, side) => {
                            // A sudden snap of the head aside, and back.
                            let aside = Vec3::new(-forward.z, 0.0, forward.x) * side * 3.0 + forward;
                            intent.look = if t > 0.12 { root + aside + Vec3::Y * 1.0 } else { look };
                            intent.head_rate = 45.0;
                            intent.velocity *= 0.3;
                            if t > dt { Gesture::Twitch(t - dt, side) } else { Gesture::None }
                        }
                        Gesture::Creep(t) => {
                            intent.velocity *= 0.45;
                            intent.crouch = 0.65;
                            if t > dt { Gesture::Creep(t - dt) } else { Gesture::None }
                        }
                        Gesture::Burst(t) => {
                            intent.velocity = intent.velocity.normalize_or(toward) * 9.0;
                            intent.crouch = 0.1;
                            if t > dt { Gesture::Burst(t - dt) } else { Gesture::None }
                        }
                        Gesture::None => Gesture::None,
                    };
                    // Now and then it stops dead: listening, searching or
                    // far off; longer, prowling near.
                    if h.timer <= 0.0 {
                        h.stance = Stance::Freeze;
                        h.timer = if searching || distance > 30.0 { 0.6 + 0.8 * r(1) } else { 2.0 + 2.5 * r(1) };
                    }
                }
                // Still, low, head locked on you.
                Stance::Freeze => {
                    intent.crouch = 0.5;
                    intent.still = true;
                    if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 1.2 + 2.5 * r(2);
                    }
                }
                // Building to a gallop, then a pounce out of the run, keeping
                // its momentum.
                Stance::Charge => {
                    intent.velocity = toward * if chasing { 18.0 } else { 14.0 };
                    intent.crouch = 0.1;
                    let speed = h.rig.speed();
                    // (`--opt pounce`: the hound's leap, kept; the user: it
                    // sort of works for the harrow too.)
                    let pounce = args.opt("pounce");
                    if pounce && speed > 7.0 && distance < 5.0 + speed * 0.35 && straight {
                        // A quick gather, still running.
                        h.stance = Stance::Windup;
                        h.timer = 0.13;
                    } else if pounce && distance < SNAP && straight {
                        // Up against you, too slow to pounce: a gather, jaws
                        // open, and a snap (it pushed at you, never biting).
                        h.stance = Stance::Windup;
                        h.timer = 0.25;
                    } else if !pounce && distance < HAUL_FROM && straight {
                        // Near: it stops dead and rears (up against you, only
                        // a moment).
                        h.stance = Stance::Rear;
                        h.timer = if distance < SNAP { 0.15 } else { 0.35 };
                    } else if h.timer <= 0.0 {
                        h.timer = 4.0;
                    }
                }
                Stance::Rear => {
                    h.rig.velocity *= (-dt * 10.0).exp();
                    intent.stretch = 0.4;
                    if h.timer <= 0.0 {
                        // Its forelimbs shoot out and spike into the ground
                        // either side of you, a little past.
                        let dir = (target - root).with_y(0.0).normalize_or(toward);
                        let left = Vec3::new(-dir.z, 0.0, dir.x);
                        let at = |s: f32| {
                            let p = Vec3::new(target.x, root.y, target.z) + dir * 0.6 + left * s * 0.9;
                            Vec3::new(p.x, ground.at(p), p.z)
                        };
                        h.rig.grab = Some([at(1.0), at(-1.0)]);
                        commands.spawn((
                            AudioPlayer::new(assets.pounce.clone()),
                            PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(if h.silent() { 0.0 } else { 2.0 })),
                            Transform::from_translation(h.rig.head().0),
                        ));
                        h.stance = Stance::Reach;
                        h.timer = 0.12;
                    }
                }
                Stance::Reach => {
                    h.rig.velocity *= (-dt * 10.0).exp();
                    intent.stretch = 1.0;
                    if h.timer <= 0.0 {
                        h.stance = Stance::Haul((target - root).with_y(0.0).normalize_or(toward));
                        h.timer = 0.4;
                    }
                }
                // Hauled along its arms onto you, faster than it runs (each
                // forelimb lost, slower; none, a lunge to bite); stopped by
                // you (your body stops it), it bites.
                Stance::Haul(dir) => {
                    let arms = (0..h.rig.plan.legs.len()).filter(|&i| h.rig.is_lost(i) && h.rig.plan.legs[i].root == crate::rig::Root::Chest).count();
                    let pace = HAUL * 0.6f32.powi(arms as i32);
                    h.rig.velocity = dir * pace;
                    intent.velocity = dir * pace;
                    intent.stretch = 1.0;
                    if h.timer <= 0.0 || h.struck {
                        h.stance = Stance::Recover;
                        h.timer = RECOVER;
                        h.struck = false;
                    }
                }
                Stance::Windup => {
                    intent.velocity = toward * 18.0;
                    intent.crouch = 1.0;
                    intent.stretch = 1.0;
                    if h.timer <= 0.0 {
                        // Fast and flat, aimed to land on you: about a third
                        // of a second in the air.
                        let up = 4.5;
                        let flight = 2.0 * up / 25.0;
                        // (Close, a short low lunge into you.)
                        if distance < SNAP {
                            h.rig.leap(toward, 6.0, 2.0);
                        } else {
                            h.rig.leap(toward, (distance / flight).clamp(12.0, 34.0), up);
                        }
                        commands.spawn((
                            AudioPlayer::new(assets.pounce.clone()),
                            PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(if h.silent() { 0.0 } else { 2.0 })),
                            Transform::from_translation(h.rig.head().0),
                        ));
                        h.stance = Stance::Attack(toward);
                        h.timer = 2.0;
                    }
                }
                Stance::Attack(_) => {
                    intent.stretch = 1.0;
                    if h.rig.airborne.is_none() && h.timer < 1.9 {
                        h.stance = Stance::Recover;
                        h.timer = RECOVER;
                        h.struck = false;
                    }
                }
                // Landed: it skids to a stop (its deceleration), low.
                Stance::Stagger => {}
                Stance::Recover => {
                    intent.crouch = 0.4 * (h.timer / RECOVER).max(0.0);
                    if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 1.0 + 1.5 * r(3);
                    }
                }
            }
        } else {
            match h.stance {
                Stance::Rear | Stance::Reach | Stance::Haul(_) => {}
                Stance::Stalk | Stance::Freeze | Stance::Charge => {
                    let (pace, stop) = match h.aware {
                        Aware::Chasing => (8.0, 2.5),
                        Aware::Pursuing => (8.0, 0.5),
                        Aware::Searching(_) => (5.0, 0.5),
                        Aware::Unaware => (3.5, 2.5),
                    };
                    if distance > stop {
                        intent.velocity = toward * pace * ((distance - stop) / 3.0).min(1.0);
                    }
                    if distance < 9.0 && h.timer <= 0.0 && straight {
                        h.stance = Stance::Windup;
                        h.timer = 0.7;
                    }
                }
                Stance::Windup => {
                    intent.crouch = 1.0;
                    if h.timer <= 0.0 {
                        h.stance = Stance::Attack(toward);
                        h.timer = 0.45;
                    }
                }
                // A dash, low and reaching.
                Stance::Attack(dir) => {
                    intent.velocity = dir * 20.0;
                    intent.crouch = 0.5;
                    h.rig.velocity = dir * 20.0;
                    if h.timer <= 0.0 {
                        h.stance = Stance::Recover;
                        h.timer = RECOVER;
                        h.struck = false;
                    }
                }
                Stance::Stagger => {}
                Stance::Recover => {
                    intent.crouch = 0.3 * (h.timer / RECOVER).max(0.0);
                    if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 1.2;
                    }
                }
            }
        }
        // The jaw: nearly shut, open in a crouch, gaping in an attack.
        let open = match h.stance {
            Stance::Windup => if beast { 1.4 } else { 0.55 },
            Stance::Attack(_) => if beast { 1.9 } else { 0.8 },
            Stance::Stagger => if beast { 1.0 } else { 0.5 },
            Stance::Rear => 1.2,
            Stance::Reach => 1.6,
            Stance::Haul(_) => 1.9,
            Stance::Charge => 0.3,
            Stance::Freeze => 0.15,
            _ => 0.06 + 0.04 * (time.elapsed_secs() * 1.3).sin(),
        };
        let open = if args.opt("specimen") { args.num("jaw", 0.1) } else { open };
        h.jaw += (open - h.jaw) * (1.0 - (-dt * 10.0).exp());
        // An attack that reaches the player strikes once.
        if matches!(h.stance, Stance::Attack(_) | Stance::Haul(_)) && !h.struck && h.rig.chest().distance(target) < 2.2 {
            h.struck = true;
            // (A sprawl's bite lighter: it comes often, from where you are not
            // looking.)
            player.1.health -= if sprawl { ATTACK * 0.5 } else { ATTACK };
            // It shoves you along its line, off your feet a little, harder
            // the faster it came (a pounce, a dash, more than a snap); the
            // view snaps back, rolled away from its side.
            let along = (target - h.rig.root).with_y(0.0).normalize_or(toward);
            let force = (3.0 + h.rig.speed() * 0.4).min(10.0);
            player.1.velocity += along * force + Vec3::Y * (1.5 + force * 0.15);
            gun.jolt += Vec2::new(0.05 + force * 0.008, -along.dot(*player.0.right()) * 0.12);
            if args.opt("huntlog") {
                info!("huntlog: {} bit you ({:.0} left)", entity.index_u32(), player.1.health);
            }
            director.hurt = 1.0;
            commands.spawn((AudioPlayer::new(assets.bite.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(if h.silent() { 0.0 } else { 1.0 }))));
            // You cry out.
            commands.spawn((AudioPlayer::new(assets.player_pain.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
        }
        // (Its ground under where it stands: in a built place, the floor it
        // is on, not one above or below.)
        let y = h.rig.root.y;
        // Its body: a capsule sized to it, its foot half a metre over its
        // feet (the steps of a flight under it, not against it), swept
        // across, what is built stopping it.
        let (r, length) = if beast { (0.45, 0.6) } else { (0.35, 1.0) };
        let shape = Collider::capsule(r, length);
        let lift = Vec3::Y * (0.5 + r + length * 0.5);
        // (On its feet in a built place, never on where there is no floor a
        // step below: at a gallop it ran off a ledge before it could stop. A
        // look a metre and a half ahead for the drop took a flight down for
        // one, and stopped it there.)
        let footed = h.rig.airborne.is_none() && nav.is_some();
        let sweep = |from: Vec3, step: Vec3| {
            let to = ground.sweep(&shape, from + lift, Vec3::new(step.x, 0.0, step.z)) - lift + Vec3::Y * step.y;
            if footed && ground.under(to).is_none_or(|g| g < from.y - 1.0) { from } else { to }
        };
        let before = h.rig.root;
        // (`huntpace` at the console: every hunter faster or slower.) Its
        // legs lost slow it: each forelimb to 0.6 of its pace, each hind leg
        // to 0.8 (both forelimbs lost, it drags itself on at a third).
        let crippled: f32 = (0..h.rig.plan.legs.len()).filter(|&i| h.rig.is_lost(i)).map(|i| if h.rig.plan.legs[i].root == crate::rig::Root::Chest { 0.6 } else { 0.8 }).product();
        // On a chain (see `chain.rs`): the porter goes its own way ahead;
        // its stalker keeps behind it on its track, `BEHIND` m back
        // (quicker the further it is from there, easing in), watching it.
        if let Some(&(porter, heading)) = keeps.and_then(|k| spots.get(&k.0)) {
            let spot = porter - Vec3::new(heading.sin(), 0.0, heading.cos()) * super::chain::BEHIND;
            let to = (spot - h.rig.root).with_y(0.0);
            let d = to.length();
            intent.velocity = if d < 0.25 { Vec3::ZERO } else { to / d * (d * 1.2).min(1.6) };
            intent.look = porter + Vec3::Y * 1.6;
            intent.crouch = 0.0;
        }
        intent.velocity *= args.num("huntpace", 1.0) * crippled;
        // (`--opt still`: its body frozen as it first stands, not a sway,
        // to see what moves of its own on it: its cloak.)
        if !(args.opt("still") && !h.rig.bones.is_empty()) {
            h.rig.update(dt, intent, &|x: f32, z: f32| ground.at(Vec3::new(x, y, z)), &sweep);
        }
        if args.opt("huntlog") && !ground.clear(before + lift, h.rig.root + lift) {
            info!("huntlog: {} went through something built, from {:.1} to {:.1}", entity.index_u32(), before, h.rig.root);
        }
        transform.translation = h.rig.chest();
    }
}

/// The body's pieces follow their bones by springs (lagging, swaying,
/// settling); those on a bone just struck are jolted along the shot. The
/// eyes sit on the head.
#[allow(clippy::type_complexity)]
pub(super) fn flesh(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Single<&Transform, (With<FlyCam>, Without<Part>, Without<Eye>)>,
    mut hunters: Query<&mut Hunter>,
    mut parts: Query<(&mut Part, &mut Transform, Has<Jaw>), (Without<Eye>, Without<FlyCam>)>,
    mut eyes: Query<(&Eye, &mut Transform), (Without<Part>, Without<FlyCam>)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut part, mut transform, jaw) in &mut parts {
        let Ok(h) = hunters.get(part.hunter) else { continue };
        let Some(bone) = h.rig.bones.get(part.bone) else { continue };
        // (A jaw turns about its hinge, opening downwards.)
        let rotation = bone.rotation() * if jaw { Quat::from_rotation_x(h.jaw) } else { Quat::IDENTITY };
        let target = bone.a.lerp(bone.b, part.along) + rotation * part.offset;
        // Carried along with its bone first (all but a little), so the
        // springs give jiggle and a little lag, not a body trailing a metre
        // behind its head at a run.
        if let Some(last) = h.last_bones.get(part.bone) {
            let last_rotation = last.rotation() * if jaw { Quat::from_rotation_x(h.jaw) } else { Quat::IDENTITY };
            let last_target = last.a.lerp(last.b, part.along) + last_rotation * part.offset;
            transform.translation += (target - last_target) * CARRY;
        }
        for &(b, dir) in &h.hits {
            if b == part.bone {
                part.velocity += dir * 7.0;
            }
        }
        let to = target - transform.translation;
        if part.stiffness == RIGID {
            // The skull and what is on it move as one with the head.
            transform.translation = target;
            transform.rotation = rotation * part.rotation;
            continue;
        }
        if to.length() > 4.0 {
            transform.translation = target;
            part.velocity = Vec3::ZERO;
        } else {
            let damping = DAMPING * (part.stiffness / STIFFNESS).sqrt();
            let accel = to * part.stiffness - part.velocity * damping;
            part.velocity += accel * dt;
            transform.translation += part.velocity * dt;
        }
        transform.rotation = transform.rotation.slerp(rotation * part.rotation, (dt * 12.0).min(1.0));
    }
    for (eye, mut transform) in &mut eyes {
        let Ok(h) = hunters.get(eye.hunter) else { continue };
        // In the skull's sides, under the brow (or round a tripod's body).
        let Some(&head) = h.rig.bones.get(eye.bone) else { continue };
        let rotation = head.rotation();
        let out = rotation * Vec3::new(eye.at.x * head.radius, 0.0, -eye.at.y * head.radius);
        transform.translation = head.a.lerp(head.b, eye.at.z) + out;
        transform.rotation = rotation;
        // Eyeshine: bright only when it looks straight at you (a tripod's
        // each its own way out, broadly: two of the three see you).
        let facing = if eye.bone == 4 {
            let (tip, dir) = h.rig.head();
            dir.dot((camera.translation - tip).normalize_or(Vec3::Y)).max(0.0).powi(8)
        } else {
            out.normalize_or(Vec3::X).dot((camera.translation - transform.translation).normalize_or(Vec3::Y)).max(0.0).powi(2)
        };
        if let Some(mut m) = materials.get_mut(&eye.material) {
            m.emissive = LinearRgba::rgb(1.0, 1.0, 1.0) * 6000.0 * facing;
        }
    }
    for mut h in &mut hunters {
        h.hits.clear();
        h.last_bones = h.rig.bones.clone();
    }
}

/// The voice follows the head and growls now and then: a phrase picked at
/// random from the recordings, a little higher or lower each time, with a
/// pause after it. Prowling, a growl every few seconds; creeping, quieter and
/// rarer; charging, one after another, louder and higher; stopped and
/// staring, or in the air, nothing (it lets the current phrase finish). A
/// voice whose hunter is gone goes with it.
pub(super) fn voice(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<Assets3>,
    hunters: Query<&Hunter>,
    mut voices: Query<(Entity, &mut Voice, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut voice, mut transform) in &mut voices {
        let Ok(h) = hunters.get(voice.hunter) else {
            commands.entity(e).despawn();
            continue;
        };
        transform.translation = h.rig.head().0;
        voice.next -= dt;
        // (volume, pitch, pause after a phrase), or silence.
        let style = match (h.stance, h.gesture) {
            (_, Gesture::Tilt(..)) | (Stance::Freeze | Stance::Attack(_) | Stance::Haul(_) | Stance::Stagger, _) => None,
            (Stance::Charge | Stance::Windup | Stance::Rear | Stance::Reach, _) => Some((1.6, 1.08, 0.0..0.25)),
            (Stance::Recover, _) => Some((1.0, 0.92, 0.3..1.0)),
            (_, Gesture::Creep(_)) => Some((0.55, 0.9, 2.0..4.5)),
            _ => Some((1.0, 1.0, 1.0..3.5)),
        };
        let Some((volume, pitch, pause)) = style else { continue };
        if voice.next > 0.0 {
            continue;
        }
        voice.count += 1;
        let r = |k: i32| hash01(e.index_u32() as i32, voice.count as i32, k, 0x7c1);
        let (sound, length) = assets.growls[(r(0) * assets.growls.len() as f32) as usize % assets.growls.len()].clone();
        let speed = pitch * (0.9 + 0.18 * r(1));
        commands.entity(e).with_child((
            AudioPlayer::new(sound),
            PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(volume * 1.6)).with_speed(speed),
            Transform::default(),
        ));
        voice.next = length / speed + pause.start + (pause.end - pause.start) * r(2);
    }
}

/// `--opt bones` (to see how a body moves, its pieces hiding its limbs):
/// its pieces hidden, its bones drawn as lines, its joints as small balls.
pub(super) fn skeleton(args: Res<Args>, hunters: Query<&Hunter>, mut parts: Query<&mut Visibility, With<Part>>, mut gizmos: Gizmos) {
    if !args.opt("bones") {
        return;
    }
    for mut v in &mut parts {
        *v = Visibility::Hidden;
    }
    let colour = Color::srgb(1.0, 0.25, 0.05);
    for h in &hunters {
        for b in &h.rig.bones {
            gizmos.line(b.a, b.b, colour);
            gizmos.sphere(Isometry3d::from_translation(b.b), 0.06, colour);
        }
    }
}

/// `--opt watch` (for captures): the view turns to the nearest hunter.
pub(super) fn watch(
    args: Res<Args>,
    hunters: Query<&Transform, (With<Hunter>, Without<FlyCam>)>,
    bishops: Query<&super::bishop::Bishop>,
    mut camera: Single<(&Transform, &mut FlyCam)>,
) {
    if !args.opt("watch") {
        return;
    }
    let eye = camera.0.translation;
    // (A hunter's chest, or a bishop's.)
    let Some(at) = hunters.iter().map(|t| t.translation).chain(bishops.iter().filter_map(|b| b.chest())).min_by(|a, b| a.distance(eye).total_cmp(&b.distance(eye))) else { return };
    let to = at - eye;
    camera.1.yaw = (-to.x).atan2(-to.z);
    camera.1.pitch = (to.y / Vec2::new(to.x, to.z).length().max(0.1)).atan().clamp(-0.6, 0.6);
}
