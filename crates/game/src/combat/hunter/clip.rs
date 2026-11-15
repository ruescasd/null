//! Creatures moved by authored clips: the `clip` kind (the user, 2026-10-30:
//! enemies are the critical path, and no body animated procedurally
//! convinced; this tests a process instead of a creature). A rigged model,
//! its clips from a library, in `creatures/NAME.glb`, and beside it
//! `creatures/NAME.creature`, written by `tools/creature_clips.py`: how tall
//! it stands, which way it faces, its walking clips' ground speeds, when its
//! attack lands, its joints.
//!
//! The hunter under it is a biped's, unseen: its mind, its way through built
//! places, its body's place and pace (see `hunt`, its `clip` stances). On it
//! the model is set and its clips played: its walk at its own ground speed,
//! so its feet stay where they come down, walking on as it dashes (it
//! walks or dashes, never runs: a `run` line in its file is not read, see
//! `hunt`); its attack as it winds up, timed so the blow lands as the clip's
//! does. Over the clips its head turns to you; shot, it
//! leans with its hunter's reel and a limb struck flinches (see `aim`); its
//! hits are taken on its joints. Killed, it falls limp:
//! the stalker's ragdoll on its bones as drawn, its joints turned after it
//! (see `fall`). With `echo`, a second body beside it, a moment behind
//! (see `echo.rs`). On district ground (`--opt blink`), its shade, its
//! teleports' look (see `shade.rs`).

mod echo;
mod gait;
mod shade;

use std::path::PathBuf;

use bevy::{app::AnimationSystems, gltf::Gltf, prelude::*, transform::TransformSystems, world_serialization::{WorldAsset, WorldAssetRoot}};

use super::{Hunter, Stance};
use crate::camera::FlyCam;

/// Where the creatures are: `creatures/` in the working directory if it is
/// there, else beside this crate's workspace.
pub(crate) fn folder() -> PathBuf {
    let local = PathBuf::from("creatures");
    let dir = if local.is_dir() { local } else { PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../creatures")) };
    dir.canonicalize().unwrap_or(dir)
}

/// Its creature files, in name order: the order they are numbered in.
fn files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder()).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "creature")).collect()).unwrap_or_default();
    files.sort();
    files
}

/// The creatures' names (their files', `creatures/NAME.creature`), each
/// spawned by it (`spawn gladiator`), in the order they are numbered in.
pub(crate) fn names() -> &'static [String] {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| files().iter().map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()).collect())
}

/// A clip as the creature file names it, and its numbers (see `Spec`).
#[derive(Clone, Default, Debug)]
struct Clip {
    name: String,
    numbers: Vec<f32>,
}

/// What `tools/creature_clips.py` measured of a model, and what was chosen.
#[derive(Clone, Debug)]
pub(crate) struct Spec {
    pub(crate) name: String,
    model: String,
    /// How tall it stands (m); the model's own height and its lowest point
    /// (its units); how far its top is above its head joint (its units).
    height: f32,
    model_height: f32,
    model_floor: f32,
    head_length: f32,
    /// Which way it faces in the model (x, z).
    forward: Vec2,
    idle: Clip,
    /// Ground speed (model units/s), where its left foot comes down (of the
    /// clip), length (s).
    walk: Clip,
    /// When the blow lands (of the clip), length (s).
    attack: Clip,
    /// Length (s).
    hit: Clip,
    death: Clip,
    spine: Vec<String>,
    head: String,
    legs: [[String; 3]; 2],
    arms: [[String; 3]; 2],
    /// Torso, neck, head, leg, arm (m).
    radii: [f32; 5],
    /// The arm (0 left, 1 right) shards float off, if any (`shards = right`:
    /// its weapon arm, the user's, 2026-10-31).
    shards: Option<usize>,
    /// Whether it wields the mace in its right hand (`weapon = mace`; see
    /// `mace.rs`).
    mace: bool,
}

impl Spec {
    pub(crate) fn parse(name: &str, text: &str) -> Result<Spec, String> {
        let mut map = std::collections::HashMap::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(format!("line {}: not `key = value`: {line}", n + 1));
            };
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
        let get = |k: &str| map.get(k).cloned().ok_or_else(|| format!("no `{k} =` line"));
        let num = |k: &str| get(k)?.parse::<f32>().map_err(|_| format!("{k}: not a number"));
        let clip = |k: &str, count: usize, needed: bool| -> Result<Clip, String> {
            let line = map.get(k).cloned().unwrap_or_default();
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.is_empty() {
                return if needed { Err(format!("{k}: no clip named")) } else { Ok(Clip::default()) };
            }
            // (Its name all but the last `count` words: a clip's name may
            // have spaces in it.)
            if words.len() <= count {
                return Err(format!("{k}: {count} numbers after the clip's name (run tools/creature_clips.py again)"));
            }
            let (name, numbers) = words.split_at(words.len() - count);
            let numbers = numbers.iter().map(|w| w.parse::<f32>().map_err(|_| format!("{k}: {w:?} is not a number ({count} numbers after the clip's name)"))).collect::<Result<Vec<_>, _>>()?;
            Ok(Clip { name: name.join(" "), numbers })
        };
        let limbs = |k: &str| -> Result<[[String; 3]; 2], String> {
            let sides: Vec<Vec<String>> = get(k)?.split(',').map(|s| s.split_whitespace().map(String::from).collect()).collect();
            match sides.as_slice() {
                [a, b] if a.len() == 3 && b.len() == 3 => Ok([[a[0].clone(), a[1].clone(), a[2].clone()], [b[0].clone(), b[1].clone(), b[2].clone()]]),
                _ => Err(format!("{k}: three joints a side, left, then right, a comma between")),
            }
        };
        let numbers = |k: &str, count: usize| -> Result<Vec<f32>, String> {
            let n = get(k)?.split_whitespace().map(|w| w.parse::<f32>().map_err(|_| format!("{k}: {w:?} is not a number"))).collect::<Result<Vec<_>, _>>()?;
            if n.len() == count { Ok(n) } else { Err(format!("{k}: {count} numbers")) }
        };
        let forward = numbers("forward", 2)?;
        let radii = numbers("radii", 5)?;
        let spec = Spec {
            name: name.to_string(),
            model: get("model")?,
            height: num("height")?,
            model_height: num("model_height")?,
            model_floor: num("model_floor")?,
            head_length: num("head_length")?,
            forward: Vec2::new(forward[0], forward[1]).normalize_or(Vec2::Y),
            idle: clip("idle", 1, false)?,
            walk: clip("walk", 3, true)?,
            attack: clip("attack", 2, false)?,
            hit: clip("hit", 1, false)?,
            death: clip("death", 1, false)?,
            spine: get("spine")?.split_whitespace().map(String::from).collect(),
            head: get("head")?,
            legs: limbs("legs")?,
            arms: limbs("arms")?,
            radii: [radii[0], radii[1], radii[2], radii[3], radii[4]],
            shards: match map.get("shards").map(String::as_str) {
                None | Some("") => None,
                Some("left") => Some(0),
                Some("right") => Some(1),
                Some(other) => return Err(format!("shards: `left` or `right`, not {other:?}")),
            },
            mace: match map.get("weapon").map(String::as_str) {
                None | Some("") => false,
                Some("mace") => true,
                Some(other) => return Err(format!("weapon: `mace`, not {other:?}")),
            },
        };
        if spec.spine.len() < 2 {
            return Err("spine: from the hips up to the neck, two joints or more".into());
        }
        if spec.height <= 0.0 || spec.model_height <= 0.0 || spec.walk.numbers[0] <= 0.0 {
            return Err("height, model_height and the walk's speed must be more than 0".into());
        }
        Ok(spec)
    }

    /// Metres to the model's unit.
    fn scale(&self) -> f32 {
        self.height / self.model_height
    }

    /// Its walk's ground speed (m/s).
    fn pace(&self) -> f32 {
        self.walk.numbers[0] * self.scale()
    }

    /// The model's turn about up from its own facing to +z.
    fn facing(&self) -> f32 {
        self.forward.x.atan2(self.forward.y)
    }

    /// What its hunter needs of it (see `hunt`).
    fn moves(&self) -> Moves {
        let walk = self.pace();
        let (impact, attack) = match self.attack.numbers.as_slice() {
            [f, length] => (f * length, *length),
            _ => (0.3, 0.6),
        };
        Moves { walk, impact, attack, reach: self.height * 0.7, height: self.height }
    }
}

/// What a hunter with an authored body moves by: its walk's ground speed
/// (m/s), how long its attack takes to land and all of it
/// (s), how near it strikes from (m), and how tall it stands (m).
#[derive(Clone, Copy, Debug)]
pub(super) struct Moves {
    pub(super) walk: f32,
    pub(super) impact: f32,
    pub(super) attack: f32,
    pub(super) reach: f32,
    pub(super) height: f32,
}

/// The creatures in `creatures/`, each with its model and, once loaded, its
/// clips in a graph.
#[derive(Resource, Default)]
pub(crate) struct Kits(pub(crate) Vec<Kit>);

pub(crate) struct Kit {
    pub(crate) spec: Spec,
    gltf: Handle<Gltf>,
    scene: Handle<WorldAsset>,
    graph: Option<(Handle<AnimationGraph>, Nodes)>,
    /// Its walk as waves (`NAME.gait`, see `gait.rs`), if it has one.
    gait: Option<gait::Gait>,
}

#[derive(Clone, Copy)]
struct Nodes {
    idle: Option<AnimationNodeIndex>,
    walk: AnimationNodeIndex,
    attack: Option<AnimationNodeIndex>,
    hit: Option<AnimationNodeIndex>,
    death: Option<AnimationNodeIndex>,
}

/// Reads every `creatures/*.creature` (failing loudly on one it cannot
/// read: its file and what is wrong) and begins loading its model.
pub(super) fn load(mut commands: Commands, server: Res<AssetServer>) {
    let dir = folder();
    let mut kits = Vec::new();
    for (path, name) in files().into_iter().zip(names()) {
        assert!(!crate::console::KINDS.contains(&name.as_str()), "{}: a creature can't be named as a kind of creature already ({}): rename its files", path.display(), crate::console::KINDS.join(", "));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let spec = Spec::parse(&name, &text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(dir.join(&spec.model).is_file(), "{}: its model {} is not in {}", path.display(), spec.model, dir.display());
        // (Its walk as waves, if fitted: of the walk it names, else stale.)
        let waves = path.with_extension("gait");
        let gait = waves.is_file().then(|| {
            let text = std::fs::read_to_string(&waves).unwrap_or_else(|e| panic!("{}: {e}", waves.display()));
            let mut gait = gait::Gait::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", waves.display()));
            assert!(gait.clip == spec.walk.name, "{}: fitted to the clip {:?}, but its walk is {:?}: run tools/gait_fit.py {} --write 6 again", waves.display(), gait.clip, spec.walk.name, dir.join(&spec.model).display());
            let trunk: Vec<String> = spec.spine.iter().chain([&spec.head]).cloned().collect();
            let plan = gait.plan(&spec.legs, &spec.arms, &trunk, spec.walk.numbers[1]).unwrap_or_else(|e| panic!("{}: {e}", waves.display()));
            info!("creature {name}: its steps' shape changes at {:.2} and {:.2} of its stride", plan.lulls[0], plan.lulls[1]);
            gait.plan = Some(plan);
            gait
        });
        let at = format!("creatures://{}", spec.model);
        info!("creature {}: `spawn {name}`{}", kits.len(), if gait.is_some() { " (its walk as waves too: walk=1)" } else { "" });
        kits.push(Kit { gltf: server.load(at.clone()), scene: server.load(GltfAssetLabel::Scene(0).from_asset(at)), graph: None, spec, gait });
    }
    commands.insert_resource(Kits(kits));
}

/// Each creature's clips in a graph, once its model has loaded (failing
/// loudly on a clip it names that the model has not).
pub(super) fn prepare(server: Res<AssetServer>, gltfs: Res<Assets<Gltf>>, mut graphs: ResMut<Assets<AnimationGraph>>, mut kits: ResMut<Kits>) {
    for kit in kits.0.iter_mut().filter(|k| k.graph.is_none()) {
        if let Some(bevy::asset::LoadState::Failed(e)) = server.get_load_state(&kit.gltf) {
            panic!("creatures/{}: {e}", kit.spec.model);
        }
        let Some(gltf) = gltfs.get(&kit.gltf) else { continue };
        let mut graph = AnimationGraph::new();
        let root = graph.root;
        let mut add = |c: &Clip| -> Option<AnimationNodeIndex> {
            if c.name.is_empty() {
                return None;
            }
            let Some(clip) = gltf.named_animations.get(c.name.as_str()) else {
                let mut have: Vec<&str> = gltf.named_animations.keys().map(|k| k.as_ref()).collect();
                have.sort();
                panic!("creatures/{}.creature names a clip {:?} that {} has not; it has {have:?}", kit.spec.name, c.name, kit.spec.model);
            };
            Some(graph.add_clip(clip.clone(), 1.0, root))
        };
        let spec = &kit.spec;
        let nodes = Nodes { idle: add(&spec.idle), walk: add(&spec.walk).expect("a walk"), attack: add(&spec.attack), hit: add(&spec.hit), death: add(&spec.death) };
        kit.graph = Some((graphs.add(graph), nodes));
    }
}

/// A hunter to be given an authored body: which creature (its number in
/// `names`).
#[derive(Component)]
pub(super) struct Wants(pub(super) usize);

/// An authored body on a hunter (gone, its death played where it fell).
#[derive(Component)]
pub(crate) struct Body {
    hunter: Entity,
    kit: usize,
    /// Its clips' player, and its joints by role, once its model is in.
    player: Option<Entity>,
    joints: Option<Joints>,
    /// Through its walk (0..1); its steps so far (each drawing its pace).
    phase: f32,
    steps: u32,
    /// With `walk=1`, its walk played from its waves (see `gait.rs`): each
    /// joint of them and its track, once its model is in; and the walk's
    /// share of what is played (0..1).
    fitted: Option<Vec<(Entity, usize)>>,
    walking: f32,
    /// With `walk=1`, its steps' shapes (see `gait::Steps`).
    shaping: Option<gait::Steps>,
    /// How it holds itself (radians): its trunk stooped forward, its arms
    /// in toward its sides (the user, 2026-11-07: its clips' upright chest
    /// and flared arms, a swagger); and how much of what is played they
    /// hold for (not its attack's or a hit's share).
    hunch: f32,
    arms_in: f32,
    posing: f32,
    /// With `still`: the way it faces (its heading) and how far it is
    /// through its showing (s, see `STILL`).
    still: Option<(f32, f32)>,
    /// What it plays over its walking, and how long it has (s).
    act: Option<(Act, f32)>,
    /// How long since it died (s).
    dying: Option<f32>,
    /// Its head's turn to you.
    look: Quat,
    /// Each of its hunter's bones struck: turned (radians about an axis, in
    /// the world) and turning, sprung back (see `FLINCH`).
    flinch: [(Vec3, Vec3); 13],
    /// What `aim` last set each joint it turns to, and from what: a joint
    /// its clips do not move keeps what was set, and turned again from it,
    /// turned on and on.
    turned: std::collections::HashMap<Entity, (Quat, Quat)>,
    /// Fallen limp, its joints after its hunter's ragdoll (see `fall`).
    fallen: Option<Fallen>,
    /// What it wields, once in its hand, and how far its head is bloomed
    /// (0..1: thrust out as its blow lands).
    weapon: Option<Entity>,
    bloom: f32,
}

/// A body as it fell: its bones then, and each joint below its hips with
/// the bone it turns with and how it was turned in the world then.
struct Fallen {
    from: Vec<crate::rig::Bone>,
    joints: Vec<(Entity, usize, Quat)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    Attack,
    Hit,
}

struct Joints {
    spine: Vec<Entity>,
    head: Entity,
    legs: [[Entity; 3]; 2],
    arms: [[Entity; 3]; 2],
}

/// Hunters wanting a body get one, once its creature's clips are ready.
pub(super) fn dress(mut commands: Commands, args: Res<crate::Args>, kits: Res<Kits>, mut hunters: Query<(Entity, &mut Hunter, &Wants, Option<&mut crate::navmesh::district::Blink>)>) {
    for (entity, mut h, wants, blink) in &mut hunters {
        let Some(kit) = kits.0.get(wants.0) else {
            error!("creature {}: there are {} in {} (see the log at start)", wants.0, kits.0.len(), folder().display());
            commands.entity(entity).despawn();
            continue;
        };
        if kit.graph.is_none() {
            continue;
        }
        let spec = &kit.spec;
        // Its walk as waves, if fitted (`walk=0`, its clip's; asked for and
        // none fitted: said, not made).
        let fitted = h.traits.value("walk", if kit.gait.is_some() { 1.0 } else { 0.0 }) == 1.0;
        if fitted && kit.gait.is_none() {
            error!("{}: walk=1, but it has no walk as waves: run tools/gait_fit.py creatures/{} --write 6", spec.name, spec.model);
            commands.entity(entity).despawn();
            continue;
        }
        let t = |name: &str, or: f32| h.traits.value(name, or);
        let shaping = fitted.then(|| gait::Steps::new(gait::Amounts { reach: t("step_reach", 0.1), arms: t("step_arms", 0.25), lift: t("step_lift", 0.25), lean: t("step_lean", 3.0).to_radians() }, t("arm_swing", -1.0), t("torso", -1.0), t("sway", -1.0)));
        let (hunch, arms_in) = (t("hunch", 0.0).to_radians(), t("arms_in", 0.0).to_radians());
        h.clip = Some(spec.moves());
        commands.entity(entity).remove::<Wants>();
        let body = commands.spawn((
            Body { hunter: entity, kit: wants.0, player: None, joints: None, phase: 0.0, steps: 0, fitted: fitted.then(Vec::new), walking: 0.0, shaping, hunch, arms_in, posing: 0.0, still: None, act: None, dying: None, look: Quat::IDENTITY, flinch: [(Vec3::ZERO, Vec3::ZERO); 13], turned: Default::default(), fallen: None, weapon: None, bloom: 0.0 },
            WorldAssetRoot(kit.scene.clone()),
            placed(spec, h.rig.root, h.rig.heading),
            Visibility::default(),
        )).id();
        if args.opt("blink")
            && let Some(mut blink) = blink
        {
            blink.shaded = true;
            // (Its shade its teleport's look: so it teleports, 10 m but as
            // given.)
            blink.range = h.traits.value("blink_range", 10.0).min(crate::navmesh::district::FARTHEST as f32);
            // (No tell, standing, before it goes: its shade leaving it is
            // the tell; it paused, the user, 2026-11-04. Given, kept.)
            if !h.traits.has("blink_tell") {
                blink.tell = 0.0;
            }
            commands.spawn((shade::Shade::new(body, entity), WorldAssetRoot(kit.scene.clone()), placed(spec, h.rig.root, h.rig.heading), Visibility::Hidden));
        }
        if h.traits.has("echo") {
            commands.spawn((echo::Echo::new(body, h.traits.value("echo_lag", echo::LAG), h.traits.value("echo_off", echo::OFF)), WorldAssetRoot(kit.scene.clone()), placed(spec, h.rig.root, h.rig.heading), Visibility::default()));
        }
    }
}

/// Where its model stands: its lowest point on the ground at `root`, facing
/// along `heading`, its height as asked.
fn placed(spec: &Spec, root: Vec3, heading: f32) -> Transform {
    Transform::from_translation(root - Vec3::Y * spec.model_floor * spec.scale()).with_rotation(Quat::from_rotation_y(heading - spec.facing())).with_scale(Vec3::splat(spec.scale()))
}

/// A body whose model is in: its clips' player given its graph, its walking
/// begun, its joints found (failing loudly on a joint it names that the
/// model has not).
#[allow(clippy::too_many_arguments)]
pub(super) fn bind(mut commands: Commands, kits: Res<Kits>, mut bodies: Query<(Entity, &mut Body)>, children: Query<&Children>, names: Query<&Name>, mut players: Query<&mut AnimationPlayer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut shard: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>) {
    for (entity, mut body) in &mut bodies {
        if body.player.is_some() {
            continue;
        }
        let Some(player) = children.iter_descendants(entity).find(|&e| players.contains(e)) else { continue };
        let kit = &kits.0[body.kit];
        let Some((graph, nodes)) = &kit.graph else { continue };
        // (The first of a name, nearest the root: a joint's piece of mesh
        // may share its name, hung under it, as the robot's feet do: taken
        // for the joint, its feet were measured 22 cm up.)
        let mut named: std::collections::HashMap<&str, Entity> = std::collections::HashMap::new();
        for e in children.iter_descendants(entity) {
            if let Ok(n) = names.get(e) {
                named.entry(n.as_str()).or_insert(e);
            }
        }
        let joint = |n: &String| *named.get(n.as_str()).unwrap_or_else(|| panic!("creatures/{}.creature names a joint {n:?} that {} has not", kit.spec.name, kit.spec.model));
        let spec = &kit.spec;
        let limb = |l: &[String; 3]| [joint(&l[0]), joint(&l[1]), joint(&l[2])];
        body.joints = Some(Joints { spine: spec.spine.iter().map(joint).collect(), head: joint(&spec.head), legs: [limb(&spec.legs[0]), limb(&spec.legs[1])], arms: [limb(&spec.arms[0]), limb(&spec.arms[1])] });
        if body.fitted.is_some() {
            let gait = kit.gait.as_ref().expect("checked in dress");
            body.fitted = Some(gait.tracks.iter().enumerate().map(|(k, t)| (*named.get(t.name.as_str()).unwrap_or_else(|| panic!("creatures/{}.gait names a joint {:?} that {} has not", kit.spec.name, t.name, kit.spec.model)), k)).collect());
        }
        commands.entity(player).insert(AnimationGraphHandle(graph.clone()));
        let mut p = players.get_mut(player).expect("found above");
        for node in [nodes.idle, Some(nodes.walk)].into_iter().flatten() {
            p.play(node).repeat().set_weight(0.0);
        }
        body.player = Some(player);
        if let Some(arm) = spec.shards {
            let (mesh, material) = shard
                .get_or_insert_with(|| {
                    // (Faceted, a dark grey catching the light, as the pale
                    // facets of its body's surface: as dark as its body,
                    // against a dark sky they were lost but for a glint.)
                    let mesh = meshes.add(crate::player::tether::shard_mesh(&[(Vec3::Y, 1.0, 0.35), (Vec3::NEG_Y, 0.55, 0.35)]));
                    let material = materials.add(StandardMaterial { base_color: Color::srgb(0.22, 0.22, 0.23), metallic: 0.6, perceptual_roughness: 0.35, ..default() });
                    (mesh, material)
                })
                .clone();
            hang_shards(&mut commands, body.hunter, arm, spec.scale(), &mesh, &material);
        }
    }
}

/// How many shards float about its arm.
const SHARDS: usize = 14;

/// Shards floating about arm `arm` (0 left, 1 right): each on its own, at
/// its own distance all round the upper arm, forearm or hand (not into its
/// body), drifting and tumbling at its own pace (`Drift`); the shard bodies'
/// sprung pieces (`Part`), on its bones as drawn, softer, so they trail the
/// arm's swing and scatter when it is shot. (First laid in a line up off its
/// shoulder, graded by size, as the concept: in play a rigid thing with gaps
/// in it, sticking up, a glitch; the user, 2026-10-31.)
fn hang_shards(commands: &mut Commands, hunter: Entity, arm: usize, scale: f32, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>) {
    use worldgen::noise::hash01;
    let (upper, fore) = (9 + 2 * arm, 10 + 2 * arm);
    // (In a bone's frame: y down the arm, x to its body's left.)
    let out = if arm == 1 { -1.0 } else { 1.0 };
    for k in 0..SHARDS {
        let r = |j: i32| hash01(k as i32, j, arm as i32, 0x5a7) - 0.5;
        // On which bone and where along it (past a forearm's end: about its
        // hand).
        let (bone, along) = match k % 3 {
            0 => (upper, 0.2 + 0.8 * (r(0) + 0.5)),
            _ => (fore, 0.1 + 1.1 * (r(0) + 0.5)),
        };
        // All round it but its body's side, at its own distance: close,
        // just off its skin (the gladiator's arm 4 to 7 model cm about its
        // bones, measured; out to 35, they hovered, too far, the user).
        let angle = r(1) * 1.4 * std::f32::consts::PI;
        let reach = 0.07 + 0.06 * (r(2) + 0.5).powi(2);
        let offset = Vec3::new(out * angle.cos(), 0.0, angle.sin()) * reach;
        let size = 0.05 + 0.11 * (r(3) + 0.5);
        commands.spawn((
            super::Part { hunter, bone, along, offset: offset * scale, rotation: Quat::from_euler(EulerRot::XYZ, r(4) * 6.0, r(5) * 6.0, r(6) * 6.0), velocity: Vec3::ZERO, stiffness: SHARD_STIFFNESS },
            Drift {
                phase: Vec3::new(r(7), r(8), r(9)) * std::f32::consts::TAU,
                rate: Vec3::new(0.5 + r(10), 0.5 + r(11), 0.5 + r(12)) * 0.8 + Vec3::splat(0.2),
                reach: (0.01 + 0.015 * (r(13) + 0.5)) * scale,
                spin: Vec3::new(r(14), r(15), r(16)).normalize_or(Vec3::Y) * (0.4 + 1.2 * (r(17) + 0.5)),
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(Vec3::splat(size * scale / 1.55)),
        ));
    }
}

/// A floating piece's own wandering about where it hangs: its phase and
/// pace (cycles/s) on each axis, how far (m), and its tumbling (radians/s
/// about an axis): see `Drift::at`, and `flesh`, which moves it.
#[derive(Component)]
pub(crate) struct Drift {
    phase: Vec3,
    rate: Vec3,
    reach: f32,
    spin: Vec3,
}

impl Drift {
    /// Where it has wandered to (in its bone's frame) and how it has
    /// tumbled, at time `t` (s).
    pub(crate) fn at(&self, t: f32) -> (Vec3, Quat) {
        let w = self.phase + self.rate * t * std::f32::consts::TAU;
        (Vec3::new(w.x.sin(), w.y.sin(), w.z.sin()) * self.reach, Quat::from_scaled_axis(self.spin * t))
    }
}

/// How stiffly a floating shard follows (the shard bodies' pieces are 160).
const SHARD_STIFFNESS: f32 = 60.0;

/// A struck bone's flinch: how fast each shard turns it (radians/s), the
/// spring back and its damping (the shard bodies' pieces', `STIFFNESS` and
/// `DAMPING`), and how far at most (radians).
const FLINCH: (f32, f32, f32, f32) = (2.0, 160.0, 13.0, 0.8);

/// A weapon's head blooming: how long before its blow lands it opens, and
/// how long after it closes (s).
const BLOOM: (f32, f32) = (0.2, 0.5);

/// Where in its walk a foot leaves the ground (of the stride, from its left
/// foot coming down; the left's half a stride on): its right, a tenth of a
/// stride after its left comes down, as in a walk both are down a while.
const TOE_OFF: f32 = 0.1;

/// How long a clip's blend in and out takes (s).
const BLEND: f32 = 0.12;
/// How long a body lies once its death is played (s).
const LIES: f32 = 6.0;

/// With `still` (the user, 2026-11-07: to judge its animations without it
/// turning back and forth): how long it walks in place, then stands, then
/// (after its attack) stands again (s).
const STILL: (f32, f32, f32) = (6.0, 2.5, 1.5);

/// Each body set on its hunter and its clips played (see the module's
/// account); its hunter gone, its death.
#[allow(clippy::type_complexity)]
pub(super) fn animate(mut commands: Commands, time: Res<Time>, kits: Res<Kits>, mut hunters: Query<&mut Hunter>, mut bodies: Query<(Entity, &mut Body, &mut Transform), Without<FlyCam>>, mut players: Query<&mut AnimationPlayer>, camera: Single<&Transform, With<FlyCam>>) {
    let dt = time.delta_secs();
    for (entity, mut body, mut transform) in &mut bodies {
        let kit = &kits.0[body.kit];
        let spec = &kit.spec;
        let Some((_, nodes)) = kit.graph else { continue };
        let Some(mut player) = body.player.and_then(|p| players.get_mut(p).ok()) else { continue };
        let Ok(mut h) = hunters.get_mut(body.hunter) else {
            // (Fallen limp, its hunter gone at last: gone with it.)
            if body.fallen.is_some() {
                commands.entity(entity).despawn();
                continue;
            }
            // Dead: its death, where it fell, then lying there a while.
            let since = body.dying.map_or(0.0, |t| t + dt);
            if body.dying.is_none() {
                for (_, active) in player.playing_animations_mut() {
                    active.set_weight(0.0);
                }
                if let Some(death) = nodes.death {
                    player.start(death).set_weight(1.0);
                }
            }
            body.dying = Some(since);
            if since > spec.death.numbers.first().copied().unwrap_or(0.0) + LIES {
                commands.entity(entity).despawn();
            }
            continue;
        };
        // (Fallen limp: its clips stopped where they were, its joints turned
        // by `fall`, the body left where it stood.)
        if h.rig.ragdoll.is_some() {
            player.stop_all();
            continue;
        }
        // (Its shade away along a teleport's line: left behind, held still
        // where it was in its stride, till its shade is there; see
        // `shade.rs`.)
        if h.gliding {
            // (Its walk is where it is set, each frame; its idle stopped.)
            if let Some(idle) = nodes.idle
                && let Some(active) = player.animation_mut(idle)
            {
                active.set_speed(0.0);
            }
            h.push = false;
            continue;
        }
        // With `still`: in one place, facing one way (`still_turn` degrees
        // from you as it first stood, 90 side on), walking in place, then
        // standing, then its attack, over and over.
        let still = h.traits.has("still").then(|| {
            let (heading, clock) = *body.still.get_or_insert_with(|| {
                let to = camera.translation - h.rig.root;
                (to.x.atan2(to.z) + h.traits.value("still_turn", 90.0).to_radians(), 0.0)
            });
            let length = spec.attack.numbers.get(1).copied().unwrap_or(0.0);
            let clock = (clock + dt).rem_euclid(STILL.0 + STILL.1 + length + STILL.2);
            body.still = Some((heading, clock));
            (heading, clock < STILL.0, (STILL.0 + STILL.1..STILL.0 + STILL.1 + 0.1).contains(&clock))
        });
        *transform = placed(spec, h.rig.root, still.map_or(h.rig.heading, |s| s.0));
        // A limb, its neck or its head struck: turned by each shard the way
        // it flew, its bone's parent by half, sprung back (its trunk leans
        // with its hunter's reel instead, see `aim`).
        for &(b, dir) in &h.hits {
            let Some(bone) = h.hitbox.get(b).filter(|_| b >= 3) else { continue };
            let spin = (bone.b - bone.a).normalize_or_zero().cross(dir.normalize_or_zero()) * FLINCH.0;
            body.flinch[b].1 += spin;
            if let Some(up) = match b {
                4 => Some(3),
                6 | 8 | 10 | 12 => Some(b - 1),
                _ => None,
            } {
                body.flinch[up].1 += spin * 0.5;
            }
        }
        for (turn, spin) in &mut body.flinch {
            *spin += (-*turn * FLINCH.1 - *spin * FLINCH.2) * dt;
            *turn = (*turn + *spin * dt).clamp_length_max(FLINCH.3);
        }

        // Over its walking: its attack as it winds up, a hit as it is struck
        // (if it has one).
        let winding = matches!(h.stance, Stance::Windup) || still.is_some_and(|s| s.2);
        let act = match body.act {
            Some((Act::Attack, left)) if left > 0.0 => Some((Act::Attack, left - dt)),
            Some((Act::Hit, left)) if left > 0.0 && !winding => Some((Act::Hit, left - dt)),
            _ => None,
        };
        let act = match (act, winding, h.stun > 0.0) {
            (None | Some((Act::Hit, _)), true, _) if nodes.attack.is_some() => {
                player.start(nodes.attack.expect("checked")).set_weight(0.0);
                Some((Act::Attack, spec.attack.numbers[1]))
            }
            (None, false, true) if nodes.hit.is_some() => {
                player.start(nodes.hit.expect("checked")).set_weight(0.0);
                Some((Act::Hit, spec.hit.numbers[0]))
            }
            (act, ..) => act,
        };
        body.act = act;
        // Its weapon's head bloomed by its blow: out in the moment before
        // it lands, drawn in after.
        body.bloom = match act {
            Some((Act::Attack, left)) => {
                let (length, impact) = (spec.attack.numbers[1], spec.attack.numbers[0] * spec.attack.numbers[1]);
                let t = length - left - impact;
                if t < 0.0 { (1.0 + t / BLOOM.0).clamp(0.0, 1.0) } else { (1.0 - t / BLOOM.1).clamp(0.0, 1.0) }
            }
            _ => 0.0,
        };
        let acting = match act {
            Some((which, left)) => {
                let length = if which == Act::Attack { spec.attack.numbers[1] } else { spec.hit.numbers[0] };
                ((length - left) / BLEND).min(left / BLEND).clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        for (node, which) in [(nodes.attack, Act::Attack), (nodes.hit, Act::Hit)] {
            if let Some(node) = node
                && let Some(active) = player.animation_mut(node)
            {
                active.set_weight(if act.is_some_and(|a| a.0 == which) { acting } else { 0.0 });
            }
        }

        // Walking: its walk at its ground speed, so a foot down stays down;
        // idle as it stops. Dashing: its walk played on at its walking pace,
        // its feet sliding (held mid-stride first; the user, 2026-11-04: the
        // walk playing).
        let dashing = h.dashing();
        let walk = spec.pace();
        let speed = match still {
            Some((_, walking, _)) => if walking { walk * h.pace } else { 0.0 },
            None if dashing => walk,
            None => h.rig.speed(),
        };
        let ease = |t: f32| t * t * (3.0 - 2.0 * t);
        let moving = if dashing { 1.0 } else { ease((speed / (0.3 * walk)).clamp(0.0, 1.0)) };
        let stride = spec.walk.numbers[0] * spec.walk.numbers[2] * spec.scale();
        // (Its step reaching further, as drawn: the same ground covered in
        // fewer strides, so a planted foot stays planted.)
        let plan = kit.gait.as_ref().and_then(|g| g.plan.as_ref());
        let reach = match (&body.shaping, plan) {
            (Some(s), Some(plan)) => s.now(plan, body.phase).reach,
            _ => 1.0,
        };
        let was = body.phase;
        body.phase = (body.phase + dt * speed / (stride * reach).max(0.05)).fract();
        let now = body.phase;
        if let (Some(s), Some(plan)) = (&mut body.shaping, plan) {
            s.step(plan, was, now, dt, |n, k| worldgen::noise::hash01(entity.index_u32() as i32, n as i32, k, 0x5e9));
        }
        // A foot leaving the ground, walking: its push for a dash (see
        // `hunt`). (Each a little after the other comes down, at `TOE_OFF`.)
        let past = |at: f32| gait::passed(was, now, at);
        h.push = !dashing && moving > 0.5 && (past(TOE_OFF) || past(TOE_OFF + 0.5));
        // A foot down: the next step's pace drawn (the user, 2026-11-05: its
        // strides as regular as a machine's). Its walk goes at its ground
        // speed, so a foot down stays down whatever the pace; now and then
        // a short, slow step, as if it checked.
        if past(0.0) || past(0.5) {
            body.steps = body.steps.wrapping_add(1);
            let r = |k: i32| worldgen::noise::hash01(entity.index_u32() as i32, body.steps as i32, k, 0x6d2);
            let vary = h.traits.value("stride_vary", 0.15);
            h.pace = if r(0) < vary * 0.8 { 1.0 - 2.5 * vary } else { 1.0 + vary * (2.0 * r(1) - 1.0) };
        }
        let rest = 1.0 - acting;
        let walk_weight = moving * rest + 1e-4;
        if let Some(active) = player.animation_mut(nodes.walk) {
            active.set_weight(walk_weight).set_speed(0.0).set_seek_time(((body.phase + spec.walk.numbers[1]).fract()) * spec.walk.numbers[2]);
        }
        // (The walk's share of all that is played: its waves, with
        // `walk=1`, take that much of each joint, see `gait.rs`.)
        let others = if nodes.idle.is_some() { (1.0 - moving) * rest } else { 0.0 } + if act.is_some() { acting } else { 0.0 };
        body.walking = walk_weight / (walk_weight + others);
        body.posing = rest;
        if let Some(idle) = nodes.idle
            && let Some(active) = player.animation_mut(idle)
        {
            active.set_weight((1.0 - moving) * rest).set_speed(1.0);
        }
    }
}

/// Over its clips: its head turned to you (so far, easing); its trunk leant
/// as its hunter reels from a blast (`Rig::reeling`, the spring every
/// hunter's body has); each struck bone turned by its flinch; before its
/// joints are carried to the world.
#[allow(clippy::type_complexity)]
pub(super) fn aim(time: Res<Time>, kits: Res<Kits>, hunters: Query<&Hunter>, camera: Single<&Transform, With<FlyCam>>, mut bodies: Query<(&mut Body, &Transform), Without<FlyCam>>, mut joints: Query<&mut Transform, (Without<Body>, Without<FlyCam>)>, globals: Query<&GlobalTransform>, parents: Query<&ChildOf>) {
    let dt = time.delta_secs();
    for (mut body, placed) in &mut bodies {
        let Some(j) = &body.joints else { continue };
        if body.fallen.is_some() {
            continue;
        }
        let head = j.head;
        let (low, top) = (j.spine[1], j.spine[j.spine.len() - 1]);
        let ends = [top, j.head, j.legs[0][0], j.legs[0][1], j.legs[1][0], j.legs[1][1], j.arms[0][0], j.arms[0][1], j.arms[1][0], j.arms[1][1]];
        let stoop: Vec<Entity> = j.spine[1..j.spine.len() - 1].to_vec();
        let upper_arms = [j.arms[0][0], j.arms[1][0]];
        let spec = &kits.0[body.kit].spec;
        let alive = body.dying.is_none();
        let world_rot = |e: Entity| parents.get(e).ok().and_then(|p| globals.get(p.parent()).ok()).map(|g| g.to_scale_rotation_translation().1);
        // Its face's way: its body's, turned to you up to 70 degrees.
        let faces = placed.rotation * Vec3::new(spec.forward.x, 0.0, spec.forward.y);
        let want = match (alive && body.still.is_none(), globals.get(head)) {
            (true, Ok(g)) => {
                let to = (camera.translation - g.translation()).normalize_or(faces);
                let turn = Quat::from_rotation_arc(faces, to);
                let (axis, angle) = turn.to_axis_angle();
                Quat::from_axis_angle(axis, angle.min(70f32.to_radians()))
            }
            _ => Quat::IDENTITY,
        };
        body.look = body.look.slerp(want, 1.0 - (-dt * 6.0).exp());
        let mut turns = vec![(head, body.look)];
        // Its trunk leant from its lower spine as far as its hunter's chest
        // reels.
        if let (Ok(h), Ok(from), Ok(to)) = (hunters.get(body.hunter), globals.get(low), globals.get(top)) {
            let reel = h.rig.reeling().with_y(0.0);
            let lever = from.translation().distance(to.translation()).max(0.1);
            turns.push((low, Quat::from_axis_angle(Vec3::Y.cross(reel).normalize_or(Vec3::X), (reel.length() / lever).atan())));
        }
        // Walking, its trunk leant as this step's drawn: forward, sideways,
        // twisted (see `gait::Steps`).
        if let (Some(steps), Some(plan)) = (&body.shaping, kits.0[body.kit].gait.as_ref().and_then(|g| g.plan.as_ref()))
            && alive
        {
            let lean = steps.now(plan, body.phase).lean * body.walking;
            let ahead = faces.with_y(0.0).normalize_or(Vec3::Z);
            turns.push((low, Quat::from_scaled_axis(Vec3::Y.cross(ahead) * lean.x + ahead * lean.y + Vec3::Y * lean.z)));
        }
        // How it holds itself: its trunk stooped from its spine's joints
        // (each a share), its neck turned back by half so it looks ahead; its
        // arms turned in toward its sides about its forward.
        if alive && body.posing > 0.0 {
            let ahead = faces.with_y(0.0).normalize_or(Vec3::Z);
            let across = Vec3::Y.cross(ahead);
            for &joint in &stoop {
                turns.push((joint, Quat::from_axis_angle(across, body.hunch * body.posing / stoop.len() as f32)));
            }
            turns.push((top, Quat::from_axis_angle(across, -0.5 * body.hunch * body.posing)));
            if let Ok(chest) = globals.get(top) {
                for arm in upper_arms {
                    let Ok(shoulder) = globals.get(arm) else { continue };
                    let out = (shoulder.translation() - chest.translation()).dot(across).signum();
                    turns.push((arm, Quat::from_axis_angle(ahead, -out * body.arms_in * body.posing)));
                }
            }
        }
        for (k, &joint) in ends.iter().enumerate() {
            let turn = body.flinch[3 + k].0;
            if turn != Vec3::ZERO {
                turns.push((joint, Quat::from_scaled_axis(turn)));
            }
        }
        // (A joint turned twice, its head: the second over the first.)
        let mut each: Vec<(Entity, Quat)> = Vec::new();
        for (joint, turn) in turns {
            match each.iter_mut().find(|e| e.0 == joint) {
                Some(e) => e.1 = turn * e.1,
                None => each.push((joint, turn)),
            }
        }
        for (joint, turn) in each {
            let (Some(parent), Ok(mut t)) = (world_rot(joint), joints.get_mut(joint)) else { continue };
            let from = match body.turned.get(&joint) {
                Some(&(set, from)) if set == t.rotation => from,
                _ => t.rotation,
            };
            t.rotation = parent.inverse() * turn * parent * from;
            body.turned.insert(joint, (t.rotation, from));
        }
    }
}

/// Its weapon put in its right hand, once its hand is placed (its scale,
/// the model's, undone), and bloomed as its blow lands.
#[allow(clippy::too_many_arguments)]
pub(super) fn wield(mut commands: Commands, args: Res<crate::Args>, kits: Res<Kits>, mut bodies: Query<&mut Body>, globals: Query<&GlobalTransform>, mut maces: Query<&mut super::mace::Mace>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for mut body in &mut bodies {
        if !kits.0[body.kit].spec.mace {
            continue;
        }
        let Some(j) = &body.joints else { continue };
        let hand = j.arms[1][2];
        match body.weapon {
            Some(w) => {
                if let Ok(mut m) = maces.get_mut(w)
                    && m.bloom != body.bloom
                {
                    m.bloom = body.bloom;
                }
            }
            None => {
                let Ok(g) = globals.get(hand) else { continue };
                let scale = g.to_scale_rotation_translation().0.x;
                if scale <= 0.0 {
                    continue;
                }
                body.weapon = Some(super::mace::spawn(&mut commands, &mut meshes, &mut materials, hand, grip(&args, scale)));
            }
        }
    }
}

/// Where a weapon sits in its hand, the hand's scale `scale` (the model's,
/// undone). (In the hand's own frame: across its palm, head to its thumb,
/// held near its pommel; turned otherwise, the gladiator's hung it head
/// down, or out sideways. Its hand measured in the GLB, 2026-10-31: fingers
/// along +Y, spread along Z, its palm facing -X, its thumb to +Z; held at
/// the joint's X 0, it went through the hand, the user. `--set grip_x`,
/// `grip_y`, `grip_z` (degrees), `grip_out` (m up the hand) and `grip_palm`
/// (m out of the palm) to try others.)
fn grip(args: &crate::Args, scale: f32) -> Transform {
    let grip = Quat::from_euler(EulerRot::XYZ, args.num("grip_x", 90.0).to_radians(), args.num("grip_y", 0.0).to_radians(), args.num("grip_z", 0.0).to_radians());
    let fist = Vec3::new(-args.num("grip_palm", 0.09), args.num("grip_out", 0.145), 0.0);
    Transform::from_translation((fist + grip * Vec3::Y * super::mace::GRIP) / scale).with_rotation(grip).with_scale(Vec3::splat(1.0 / scale))
}

/// Fallen limp: each joint turned as its bone of the hunter's ragdoll has
/// turned since it fell (its trunk's as one; a limb's bone from where the
/// trunk has carried it, round to where it lies), its hips carried with it;
/// before its joints are carried to the world.
#[allow(clippy::type_complexity)]
pub(super) fn fall(hunters: Query<&Hunter>, mut bodies: Query<&mut Body>, mut joints: Query<&mut Transform, (Without<Body>, Without<FlyCam>)>, globals: Query<&GlobalTransform>, parents: Query<&ChildOf>, children: Query<&Children>) {
    for mut body in &mut bodies {
        let Ok(h) = hunters.get(body.hunter) else { continue };
        let (Some(doll), Some(j)) = (&h.rig.ragdoll, &body.joints) else { continue };
        let now = doll.bones();
        if body.fallen.is_none() {
            // Which bone each joint turns with: as given, else its parent's.
            let mut given: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
            for &e in &j.spine[..j.spine.len() - 1] {
                given.insert(e, 1);
            }
            given.insert(j.spine[j.spine.len() - 1], 3);
            given.insert(j.head, 4);
            for s in 0..2 {
                given.insert(j.legs[s][0], 5 + 2 * s);
                given.insert(j.legs[s][1], 6 + 2 * s);
                given.insert(j.arms[s][0], 9 + 2 * s);
                given.insert(j.arms[s][1], 10 + 2 * s);
            }
            let hips = j.spine[0];
            let mut list = vec![(hips, 1, globals.get(hips).map_or(Quat::IDENTITY, |g| g.to_scale_rotation_translation().1))];
            for e in children.iter_descendants(hips) {
                let bone = given.get(&e).copied().or_else(|| parents.get(e).ok().and_then(|p| list.iter().find(|x| x.0 == p.parent()).map(|x| x.1)));
                if let (Some(bone), Ok(g)) = (bone, globals.get(e)) {
                    list.push((e, bone, g.to_scale_rotation_translation().1));
                }
            }
            body.fallen = Some(Fallen { from: h.hitbox.clone(), joints: list });
        }
        let fallen = body.fallen.as_ref().expect("set above");
        if fallen.from.len() != now.len() {
            continue;
        }
        let trunk = |b: &[crate::rig::Bone]| {
            let y = (b[2].b - b[0].a).normalize_or(Vec3::Y);
            let across = b[7].a - b[5].a;
            let x = (across - y * across.dot(y)).normalize_or(y.any_orthonormal_vector());
            Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
        };
        let carried = trunk(&now) * trunk(&fallen.from).inverse();
        let turned: Vec<Quat> = (0..now.len())
            .map(|k| {
                if k < 3 {
                    return carried;
                }
                let was = carried * (fallen.from[k].b - fallen.from[k].a).normalize_or(Vec3::Y);
                Quat::from_rotation_arc(was, (now[k].b - now[k].a).normalize_or(was)) * carried
            })
            .collect();
        let world: Vec<Quat> = fallen.joints.iter().map(|&(_, bone, was)| turned[bone] * was).collect();
        for (i, &(e, ..)) in fallen.joints.iter().enumerate() {
            let Some(parent) = parents.get(e).ok().map(|p| p.parent()) else { continue };
            let Ok(mut t) = joints.get_mut(e) else { continue };
            if let Some(k) = fallen.joints.iter().position(|x| x.0 == parent) {
                t.rotation = world[k].inverse() * world[i];
            } else if let Ok(g) = globals.get(parent) {
                // Its hips: turned in what holds them, and carried.
                t.rotation = g.to_scale_rotation_translation().1.inverse() * world[i];
                t.translation = g.affine().inverse().transform_point3(now[0].a);
            }
        }
    }
}

/// Its hits taken on its joints as they are drawn: its hunter's bones (the
/// rig's thirteen, in its order: three of torso, neck, head, legs, arms)
/// laid along them (see `Hunter::ray`).
pub(super) fn hitbox(time: Res<Time>, kits: Res<Kits>, bodies: Query<(&Body, &Transform)>, globals: Query<&GlobalTransform>, mut hunters: Query<&mut Hunter>) {
    for (body, placed) in &bodies {
        let (Some(j), Ok(mut h)) = (&body.joints, hunters.get_mut(body.hunter)) else { continue };
        let spec = &kits.0[body.kit].spec;
        let at = |e: Entity| globals.get(e).map(|g| g.translation()).unwrap_or(h.rig.root);
        let side = placed.rotation * Vec3::new(spec.forward.y, 0.0, -spec.forward.x);
        let r = spec.radii;
        let bone = |a: Vec3, b: Vec3, radius: f32| crate::rig::Bone { a, b, radius, side };
        // The spine, hips to neck, in three of equal length.
        let spine: Vec<Vec3> = j.spine.iter().map(|&e| at(e)).collect();
        let total: f32 = spine.windows(2).map(|w| w[0].distance(w[1])).sum();
        let along = |d: f32| {
            let mut left = d;
            for w in spine.windows(2) {
                let l = w[0].distance(w[1]);
                if left <= l {
                    return w[0].lerp(w[1], left / l.max(1e-6));
                }
                left -= l;
            }
            spine[spine.len() - 1]
        };
        let thirds = [along(0.0), along(total / 3.0), along(total * 2.0 / 3.0), along(total)];
        let (neck, head) = (thirds[3], at(j.head));
        let top = head + (head - neck).normalize_or(Vec3::Y) * spec.head_length * spec.scale();
        let mut bones = vec![bone(thirds[0], thirds[1], r[0]), bone(thirds[1], thirds[2], r[0]), bone(thirds[2], thirds[3], r[0]), bone(neck, head, r[1]), bone(head, top, r[2])];
        for (limbs, radius) in [(&j.legs, r[3]), (&j.arms, r[4])] {
            for limb in limbs {
                bones.push(bone(at(limb[0]), at(limb[1]), radius));
                bones.push(bone(at(limb[1]), at(limb[2]), radius));
            }
        }
        // (Its `rigtrace` option: its feet as drawn, every frame, to measure
        // how far a foot down slides; and its hips' and head's heights as
        // drawn, to see how it falls, killed.)
        if h.traits.has("rigtrace") {
            let (l, r) = (at(j.legs[0][2]), at(j.legs[1][2]));
            let root = h.rig.root;
            super::trace(&format!("cliptrace: {:.4} {:.4} {:.4} {:.4} {:.3} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4} {:.4}", time.elapsed_secs(), root.x, root.y, root.z, h.rig.speed(), l.x, l.y, l.z, r.x, r.y, r.z, thirds[0].y, head.y));
        }
        h.hitbox = bones;
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<Kits>()
        .add_systems(Startup, (load, shade::setup))
        .add_systems(Update, (prepare, dress, bind, animate, wield).chain().after(super::apart).before(super::flesh).run_if(crate::loading::playing))
        // (Its waves first, then turned over them: unchained, the waves came
        // last and its head's turn to you, its flinches and its lean were
        // lost, walking; found 2026-11-05.)
        .add_systems(PostUpdate, ((gait::play, aim, fall).chain(), echo::follow, shade::follow).chain().after(AnimationSystems).before(TransformSystems::Propagate))
        .add_systems(PostUpdate, hitbox.after(TransformSystems::Propagate));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The creatures kept in `creatures/` are read as the game reads them
    /// (its file broken, the game would stop at start).
    #[test]
    fn creatures_read() {
        let dir = folder();
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("creatures/") {
            let path = entry.expect("an entry").path();
            if path.extension().is_some_and(|x| x == "creature") {
                let spec = Spec::parse("test", &std::fs::read_to_string(&path).expect("readable")).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                assert!(dir.join(&spec.model).is_file(), "{}: no {}", path.display(), spec.model);
                let walk = spec.pace();
                assert!(walk > 0.3, "{}: walk {walk} m/s", path.display());
                seen += 1;
            }
        }
        assert!(seen > 0, "no creatures in {}", dir.display());
    }

    /// A broken file is refused, saying what is wrong.
    #[test]
    fn refuses_a_broken_file() {
        let good = std::fs::read_to_string(folder().join("robot.creature")).expect("robot.creature");
        let broken = good.replace("walk = Walking 3.5122 0.950 0.958", "walk = Walking 3.5");
        assert!(Spec::parse("robot", &broken).unwrap_err().contains("walk: 3 numbers after the clip's name"));
        let broken = good.lines().filter(|l| !l.starts_with("legs")).collect::<Vec<_>>().join("\n");
        assert!(Spec::parse("robot", &broken).unwrap_err().contains("legs"));
    }
}
