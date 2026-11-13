//! A body cut from a generated mesh into rigid pieces, one on each bone,
//! moved by the creature's own walk: no rigging, no clips. The stalker's
//! (the user, 2026-11-06: its walk on a new mesh; `spawn stalker mesh=1`),
//! from `creatures/stalker.pieces`, and the harrow's (the user, 2026-11-12:
//! the haul, the walk that worked, on a new mesh; `spawn harrow mesh=1`),
//! from `creatures/harrow.pieces`: the model and its joints, measured. Its
//! build is fitted to the mesh (its limbs, torso and neck as long as the
//! mesh's; the rest of it, and how it moves, the creature's own), so no
//! piece is stretched, but the harrow's forelegs: the haul needs them long
//! (a short-legged mesh on it stepped five times a second, a vibration), so
//! they are the harrow's, the mesh's stretched along them and thickened.
//! Each triangle goes to the bone it is nearest (the stalker's head: all
//! from its neck's top up), keeping the model's own surface (its textures
//! and normal map); each piece is turned and set from where the mesh has
//! its bone onto where the creature stands it, and rides it rigidly. (The
//! shard beast's way, `tools/bake_beast.py`, done here as it loads: the
//! model stays the one source, its joints the one thing measured.)

use std::{collections::HashMap, path::PathBuf, sync::OnceLock};

use bevy::{
    asset::RenderAssetUsages,
    gltf::{Gltf, GltfMesh, GltfNode},
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

use super::hunter::{Hunter, Part, RIGID};
use crate::rig::{ArmPlan, Bone, Gait, LegPlan, Plan, Rig};

/// Whose body: each its own file of joints in `creatures/`, its own build.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Kind {
    Stalker,
    Harrow,
}

impl Kind {
    const ALL: [Kind; 2] = [Kind::Stalker, Kind::Harrow];

    fn name(self) -> &'static str {
        match self {
            Kind::Stalker => "stalker",
            Kind::Harrow => "harrow",
        }
    }

    fn file(self) -> PathBuf {
        super::creatures_folder().join(format!("{}.pieces", self.name()))
    }

    /// Its legs and arms, as its rig has them.
    fn limbs(self) -> (usize, usize) {
        match self {
            Kind::Stalker => (2, 2),
            Kind::Harrow => (4, 0),
        }
    }
}

/// A model's joints as measured (m: the model scaled to `height`, its
/// lowest point at y 0, x and z as the model has them; x to its left, z
/// forward). Legs and arms in the rig's order, each pair the one at -x
/// first; each hip (shoulder), knee (elbow), foot (palm).
#[derive(Clone, Debug)]
pub(crate) struct Joints {
    pub(crate) model: String,
    kind: Kind,
    height: f32,
    /// A stretched piece thickens by its stretch to this power (0: not).
    thicken: f32,
    /// Its torso, rump to chest, this many times the mesh's (its pieces
    /// stretched to it, not thickened: the harrow's, the user, 2026-11-13,
    /// its body a bit short).
    spine: f32,
    /// If given, the half of the model kept (-1: at -x, 1: at +x), mirrored
    /// onto the other, its limbs too (the harrow's: its mesh posed
    /// mid-stride, its legs unlike side to side, the user, 2026-11-13).
    mirror: Option<f32>,
    /// Its surface within this of a joint (m; shoulders, elbows, hips,
    /// knees, its neck's foot and top) marked, in the creatures' pale (the
    /// harrow's: the user, 2026-11-13, its composition made visible); 0 none.
    marks: f32,
    /// Areas of its surface marked in the same pale (see `Area`).
    highlights: Vec<Area>,
    /// How level its body stands: 0 its rump low, as the harrow's; 1 its
    /// hips as high as its chest (the harrow's: the user, 2026-11-13, its
    /// slant made its hind part look an afterthought).
    level: f32,
    /// Its body's hind end, if it runs on behind its hips (the harrow's).
    rump: Option<Vec3>,
    hips: Vec3,
    chest: Vec3,
    head: Vec3,
    /// The way its head points from `head`, if not straight up (the
    /// stalker's goes up to its crown).
    tip: Option<Vec3>,
    legs: Vec<[Vec3; 3]>,
    arms: Vec<[Vec3; 3]>,
}

/// Its joints, read once (failing loudly: the file and what is wrong).
pub(super) fn joints(kind: Kind) -> &'static Joints {
    static READ: [OnceLock<Joints>; 2] = [OnceLock::new(), OnceLock::new()];
    READ[kind as usize].get_or_init(|| {
        let path = kind.file();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e} (mesh=1 needs it)", path.display()));
        Joints::parse(&text, kind).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    })
}

impl Joints {
    pub(crate) fn parse(text: &str, kind: Kind) -> Result<Joints, String> {
        let (mut mirror, mut marks, mut highlights, mut level) = (None, 0.0, Vec::new(), 0.0);
        let (mut model, mut height, mut thicken, mut spine,mut rump, mut hips, mut chest, mut head, mut tip, mut legs, mut arms) = (None, None, 0.0, 1.0, None, None, None, None, None, Vec::new(), Vec::new());
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let fail = |what: &str| format!("line {}: {what}: {line}", n + 1);
            let (key, value) = line.split_once('=').ok_or_else(|| fail("not `name = value`"))?;
            let (key, value) = (key.trim(), value.trim());
            if key == "model" {
                model = Some(value.to_string());
                continue;
            }
            if key == "mirror" {
                mirror = Some(match value {
                    "-x" => -1.0,
                    "+x" => 1.0,
                    _ => return Err(fail("mirror is -x or +x: the half kept")),
                });
                continue;
            }
            let numbers: Vec<f32> = value.split_whitespace().map(str::parse).collect::<Result<_, _>>().map_err(|_| fail("not numbers"))?;
            let points = |count: usize| -> Result<Vec<Vec3>, String> {
                if numbers.len() != 3 * count {
                    return Err(fail(&format!("{} numbers wanted, {count} points", 3 * count)));
                }
                Ok(numbers.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect())
            };
            match key {
                "height" if numbers.len() == 1 && numbers[0] > 0.0 => height = Some(numbers[0]),
                "thicken" if numbers.len() == 1 && (0.0..=1.0).contains(&numbers[0]) => thicken = numbers[0],
                "level" if numbers.len() == 1 && (0.0..=1.0).contains(&numbers[0]) => level = numbers[0],
                "marks" if numbers.len() == 1 && numbers[0] >= 0.0 => marks = numbers[0],
                "highlight" if numbers.len() % 3 == 0 => highlights.push(Area::new(&points(numbers.len() / 3)?).map_err(|e| fail(&e))?),
                "spine" if numbers.len() == 1 && numbers[0] >= 1.0 => spine = numbers[0],
                "rump" => rump = Some(points(1)?[0]),
                "hips" => hips = Some(points(1)?[0]),
                "chest" => chest = Some(points(1)?[0]),
                "head" => head = Some(points(1)?[0]),
                "tip" => tip = Some(points(1)?[0]),
                "leg" => legs.push(<[Vec3; 3]>::try_from(points(3)?).expect("three")),
                "arm" => arms.push(<[Vec3; 3]>::try_from(points(3)?).expect("three")),
                _ => return Err(fail("not model, mirror (-x or +x), height (a length), thicken (0 to 1), spine (1 or more), marks (a length), highlight (its corners), level (0 to 1), rump, hips, chest, head, tip, leg or arm")),
            }
        }
        let some = |v: Option<Vec3>, name: &str| v.ok_or(format!("no `{name} =` line"));
        let (want_legs, want_arms) = kind.limbs();
        if legs.len() != want_legs || arms.len() != want_arms {
            return Err(format!("{} `leg =` and {} `arm =` lines; the {} has {want_legs} and {want_arms}", legs.len(), arms.len(), kind.name()));
        }
        let mut joints = Joints { model: model.ok_or("no `model =` line")?, kind, height: height.ok_or("no `height =` line, a length")?, thicken, spine, mirror, marks, highlights, level, rump, hips: some(hips, "hips")?, chest: some(chest, "chest")?, head: some(head, "head")?, tip, legs, arms };
        if !joints.legs.chunks(2).chain(joints.arms.chunks(2)).all(|pair| pair[0][0].x < pair[1][0].x) {
            return Err("of each pair of legs (arms), the one at -x comes first".into());
        }
        // Mirrored: of each pair, the kept half's limb and its mirror image
        // (its middle joints on the mirror, x 0).
        if let Some(keep) = mirror {
            if [Some(joints.hips), Some(joints.chest), Some(joints.head), joints.rump, joints.tip].into_iter().flatten().any(|p| p.x.abs() > 0.05) {
                return Err("mirrored, its middle joints (rump, hips, chest, head, tip) must lie on its middle, x 0".into());
            }
            let flip = |j: [Vec3; 3]| j.map(|p| p * Vec3::new(-1.0, 1.0, 1.0));
            for pair in joints.legs.chunks_mut(2).chain(joints.arms.chunks_mut(2)) {
                let kept = if keep < 0.0 { pair[0] } else { pair[1] };
                pair[(keep < 0.0) as usize] = flip(kept);
            }
        }
        if joints.head.y >= joints.height {
            return Err("its head must be under its height".into());
        }
        match kind {
            Kind::Stalker if joints.level != 0.0 => Err("`level` is the harrow's (the stalker stands upright)".into()),
            Kind::Stalker if joints.spine != 1.0 => Err("`spine` is the harrow's (the stalker's torso is fitted to its legs)".into()),
            Kind::Stalker if !(joints.hips.y < joints.chest.y && joints.chest.y < joints.head.y) => Err("hips, chest and head must rise in that order".into()),
            Kind::Harrow if joints.tip.is_none() => Err("no `tip =` line: the way its head points".into()),
            Kind::Harrow if joints.legs[0][0].z < joints.legs[2][0].z || joints.legs[1][0].z < joints.legs[3][0].z => Err("its forelegs first, then its hind legs".into()),
            _ => Ok(joints),
        }
    }

    /// Its creature's build fitted to these (see the module's account).
    pub(crate) fn plan(&self) -> Plan {
        match self.kind {
            Kind::Stalker => self.stalker(),
            Kind::Harrow => self.harrow(),
        }
    }

    /// The stalker's build fitted to these: its legs, arms, torso and neck
    /// as long as the mesh's, stood as the stalker stands (its hips as high
    /// for its legs, its chest as far before them); the rest its own.
    fn stalker(&self) -> Plan {
        let base = Plan::stalker();
        let stance = base.hips.1 / (base.legs[0].upper + base.legs[0].lower);
        let legs: Vec<LegPlan> = base.legs.iter().zip(&self.legs).map(|(l, j)| LegPlan { side: j[0].x - self.hips.x, upper: j[0].distance(j[1]), lower: j[1].distance(j[2]), ..*l }).collect();
        let arms: Vec<ArmPlan> = base.arms.iter().zip(&self.arms).map(|(a, j)| ArmPlan { side: j[0].x - self.chest.x, upper: j[0].distance(j[1]), lower: j[1].distance(j[2]), ..*a }).collect();
        let hips = legs.iter().map(|l| l.upper + l.lower).sum::<f32>() / legs.len() as f32 * stance;
        let torso = self.hips.distance(self.chest);
        let chest = (base.chest.0, hips + (torso * torso - base.chest.0 * base.chest.0).max(0.0).sqrt());
        Plan { hips: (base.hips.0, hips), chest, neck: (self.chest.distance(self.head), base.neck.1, base.neck.2), legs, arms, ..base }
    }

    /// The harrow's build fitted to these: scaled so its hind legs are as
    /// long as the mesh's, and stood as the harrow stands (its chest high,
    /// its rump low); its torso and rump the mesh's times `spine`, its
    /// neck, head and hind legs the mesh's lengths, each leg rooted where the mesh has it; its forelegs as much
    /// longer than its hind legs as the harrow's, split at the knee as the
    /// mesh's are (the haul is made for them: see the module's account).
    fn harrow(&self) -> Plan {
        let length = |j: &[Vec3; 3]| j[0].distance(j[1]) + j[1].distance(j[2]);
        let beast = Plan::beast();
        let hind = beast.legs[2].upper + beast.legs[2].lower;
        let base = beast.scaled((length(&self.legs[2]) + length(&self.legs[3])) / 2.0 / hind);
        let fore = base.legs[0].upper + base.legs[0].lower;
        let legs: Vec<LegPlan> = base
            .legs
            .iter()
            .zip(&self.legs)
            .enumerate()
            .map(|(i, (l, j))| {
                let (root, k) = if i < 2 { (self.chest, fore / length(j)) } else { (self.hips, 1.0) };
                // (Its hind knees back, as the mesh has them: the harrow's,
                // forward and out, stood its hind legs under its body, ahead
                // of its hind end, the user, 2026-11-13. Only the way they
                // fold; its feet set down as the haul sets them.)
                let l = if i < 2 { *l } else { LegPlan { knee: -0.6, knee_up: 0.3, knee_out: 0.2, ..*l } };
                LegPlan { side: j[0].x - root.x, forward: j[0].z - root.z, upper: j[0].distance(j[1]) * k, lower: j[1].distance(j[2]) * k, ..l }
            })
            .collect();
        // Its torso, rump to chest, as long as the mesh's (times `spine`, all
        // of it in the waist: its rump the mesh's, so its hind legs stay at
        // its hind end), at the harrow's heights, about the same middle (the
        // rig's rump behind its hips and rising a little: see `Rig::solve`).
        let rump = self.rump.map_or(0.0, |r| r.distance(self.hips));
        let torso = self.rump.unwrap_or(self.hips).distance(self.chest) * self.spine;
        // (Its hips raised towards its chest's height by `level`.)
        let hips = base.hips.1 + (base.chest.1 - base.hips.1) * self.level;
        let rise = base.chest.1 - hips - rump * 0.35;
        let run = (torso * torso - rise * rise).max(0.0).sqrt() - rump;
        let mid = (base.hips.0 + base.chest.0) / 2.0;
        let tip = self.tip.expect("a harrow's tip (parse checks it)");
        // Its forelegs in turn at every pace, not together as the harrow's
        // reach when fast (the user, 2026-11-13), its hind legs between;
        // the haul's surge after each (`Rig::hauls`).
        let gaits = base.gaits.iter().map(|g| Gait { offsets: vec![0.0, 0.5, 0.25, 0.75], ..g.clone() }).collect();
        Plan {
            gaits,
            hips: (mid - run / 2.0, hips),
            chest: (mid + run / 2.0, base.chest.1),
            rump,
            neck: (self.chest.distance(self.head), base.neck.1, base.neck.2),
            head: (self.head.distance(tip), base.head.1),
            legs,
            ..base
        }
    }

    /// Where the mesh has each bone (in its rig's order: three of torso,
    /// from its rump, neck, head, each leg's two, each arm's two): its head
    /// from its neck's top to its tip (the stalker's: up to its crown).
    fn cuts(&self) -> [(Vec3, Vec3); 13] {
        let rump = self.rump.unwrap_or(self.hips);
        let third = |k: f32| rump.lerp(self.chest, k / 3.0);
        let tip = self.tip.unwrap_or(self.head.with_y(self.height));
        let bones: Vec<(Vec3, Vec3)> = [(third(0.0), third(1.0)), (third(1.0), third(2.0)), (third(2.0), third(3.0)), (self.chest, self.head), (self.head, tip)]
            .into_iter()
            .chain(self.legs.iter().chain(&self.arms).flat_map(|j| [(j[0], j[1]), (j[1], j[2])]))
            .collect();
        bones.try_into().expect("thirteen bones (parse checks the limbs)")
    }

    /// Its joints marked (`marks`): its neck's foot and top, each limb's
    /// root and middle joint.
    fn marked(&self) -> Vec<Vec3> {
        [self.chest, self.head].into_iter().chain(self.legs.iter().chain(&self.arms).flat_map(|j| [j[0], j[1]])).collect()
    }

    /// Which bone a point of the mesh is cut to: the nearest.
    fn bone_of(cuts: &[(Vec3, Vec3); 13], p: Vec3) -> usize {
        let away = |&(a, b): &(Vec3, Vec3)| {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-8)).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        };
        (0..13).min_by(|&i, &j| away(&cuts[i]).total_cmp(&away(&cuts[j]))).expect("thirteen")
    }

    /// How each bone's piece is carried from the mesh into its bone's own
    /// frame at rest (`rest`, the rig of `plan` standing): from where the
    /// mesh has its bone's foot, turned from its way to the bone's (the
    /// stalker's head upright: its up to the head's), and stretched along
    /// it if it is longer (fitted, it is not, but the harrow's forelegs).
    fn carry(&self, rest: &[Bone]) -> [(Vec3, Quat, Vec3, f32); 13] {
        let cuts = self.cuts();
        std::array::from_fn(|k| {
            let (a, b) = cuts[k];
            let to = &rest[k];
            let from = (b - a).normalize();
            let (onto, stretch) = if k == 4 && self.tip.is_none() { (to.rotation() * Vec3::NEG_Z, 1.0) } else { ((to.b - to.a).normalize(), to.a.distance(to.b) / a.distance(b)) };
            (a, to.rotation().inverse() * Quat::from_rotation_arc(from, onto), from, stretch)
        })
    }
}

/// A creature waiting for its pieces: whose (`kind`); its own head or (a
/// stalker's `head=1`) its cage, the mesh's neck and head left out; on its
/// build fitted to the mesh (`mesh=1`) or, `own`, its creature's own, the
/// pieces stretched along their bones to it (the stalker's `mesh=2`: the
/// user, 2026-11-06, its proportions and height were better); in `look`,
/// if given, not the model's own material (the harrow's: the creatures'
/// black; its model has none).
#[derive(Component)]
pub(super) struct Wants {
    pub(super) kind: Kind,
    pub(super) head: bool,
    pub(super) own: bool,
    pub(super) look: Option<Handle<StandardMaterial>>,
    /// Its marks' material (`marks` in its joints), if marked.
    pub(super) marks: Option<Handle<StandardMaterial>>,
}

/// Each model loading, and once loaded its pieces (cut once for each build):
/// each bone's, with the model's own material.
#[derive(Resource, Default)]
pub(super) struct Kit {
    gltf: HashMap<Kind, Handle<Gltf>>,
    pieces: HashMap<(Kind, bool), Pieces>,
}

/// Each piece: its bone, its mesh (in the bone's frame), its material, and
/// whether it is its bone's marks (see `Joints::marks`).
type Pieces = Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>, bool)>;

fn load(mut kit: ResMut<Kit>, server: Res<AssetServer>) {
    for kind in Kind::ALL.into_iter().filter(|k| k.file().is_file()) {
        let handle = server.load(format!("creatures://{}", joints(kind).model));
        kit.gltf.insert(kind, handle);
    }
}

/// A primitive's vertices as one attribute's numbers, if it has them.
fn floats<const N: usize>(mesh: &Mesh, attribute: bevy::mesh::MeshVertexAttribute) -> Option<Vec<[f32; N]>> {
    match mesh.attribute(attribute)? {
        VertexAttributeValues::Float32x2(v) if N == 2 => Some(v.iter().map(|x| std::array::from_fn(|i| x[i])).collect()),
        VertexAttributeValues::Float32x3(v) if N == 3 => Some(v.iter().map(|x| std::array::from_fn(|i| x[i])).collect()),
        VertexAttributeValues::Float32x4(v) if N == 4 => Some(v.iter().map(|x| std::array::from_fn(|i| x[i])).collect()),
        _ => None,
    }
}

/// A primitive (with its node's place, put into it) halved: the triangles
/// whose middles are on the kept side (`keep`: -1 at -x, 1 at +x) and their
/// mirror images (turned the other way round, to face out); but those
/// `whole` (by their middles: its head's) kept as they are, both sides, no
/// images (the user, 2026-11-13: the harrow's blade, off its middle,
/// mirrored, was two heads).
fn mirrored((mesh, t, _): &(Mesh, Transform, Handle<StandardMaterial>), keep: f32, whole: impl Fn(Vec3) -> bool, name: &str) -> Mesh {
    let place: Vec<Vec3> = floats::<3>(mesh, Mesh::ATTRIBUTE_POSITION).unwrap_or_else(|| panic!("{name}: a mesh without positions")).into_iter().map(|p| t.transform_point(Vec3::from(p))).collect();
    let normal: Vec<Vec3> = floats::<3>(mesh, Mesh::ATTRIBUTE_NORMAL).unwrap_or_else(|| panic!("{name}: a mesh without normals")).into_iter().map(|n| t.rotation * Vec3::from(n)).collect();
    let uv = floats::<2>(mesh, Mesh::ATTRIBUTE_UV_0);
    let tangent: Option<Vec<[f32; 4]>> = floats::<4>(mesh, Mesh::ATTRIBUTE_TANGENT).map(|v| v.into_iter().map(|v| { let d = t.rotation * Vec3::new(v[0], v[1], v[2]); [d.x, d.y, d.z, v[3]] }).collect());
    let corners: Vec<usize> = match mesh.indices() {
        Some(i) => i.iter().collect(),
        None => (0..place.len()).collect(),
    };
    let flip = Vec3::new(-1.0, 1.0, 1.0);
    let (mut p, mut n, mut u, mut g) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for tri in corners.as_chunks::<3>().0 {
        let kept = whole(tri.iter().map(|&i| place[i]).sum::<Vec3>() / 3.0);
        if !kept && tri.iter().map(|&i| place[i].x).sum::<f32>() * keep < 0.0 {
            continue;
        }
        let images: &[(Vec3, [usize; 3])] = if kept { &[(Vec3::ONE, [0, 1, 2])] } else { &[(Vec3::ONE, [0, 1, 2]), (flip, [0, 2, 1])] };
        for &(side, order) in images {
            for k in order {
                let i = tri[k];
                p.push((place[i] * side).to_array());
                n.push((normal[i] * side).to_array());
                if let Some(uv) = &uv {
                    u.push(uv[i]);
                }
                if let Some(tangent) = &tangent {
                    let v = tangent[i];
                    g.push([v[0] * side.x, v[1], v[2], v[3] * side.x]);
                }
            }
        }
    }
    let mut out = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p).with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, n);
    if uv.is_some() {
        out.insert_attribute(Mesh::ATTRIBUTE_UV_0, u);
    }
    if tangent.is_some() {
        out.insert_attribute(Mesh::ATTRIBUTE_TANGENT, g);
    }
    out
}

/// How far either side of a highlight's corners the faces it is laid on lie
/// (m), and how far off them it is laid.
const DEEP: f32 = 0.3;
const LIFT: f32 = 0.003;

/// A highlight's area: its corners (m, stood), flat and convex; laid on
/// every face of its bone under it, seen square on to it, either side (the
/// harrow's blade: the user, 2026-11-13, drew where; stripes laid there
/// were too literal, and its faces filled out to their creases were the
/// whole of one side of the head).
#[derive(Debug, Clone)]
pub(crate) struct Area {
    at: Vec3,
    facing: Vec3,
    u: Vec3,
    w: Vec3,
    /// Its corners square on to it (along `u`, `w`), anticlockwise.
    outline: Vec<Vec2>,
}

impl Area {
    fn new(corners: &[Vec3]) -> Result<Area, String> {
        if corners.len() < 3 {
            return Err("a highlight is three corners or more".into());
        }
        // (Facing the way its corners turn anticlockwise.)
        let turned: Vec3 = corners.iter().zip(corners.iter().cycle().skip(1)).map(|(p, q)| p.cross(*q)).sum();
        let facing = turned.normalize_or_zero();
        if facing == Vec3::ZERO {
            return Err("a highlight's corners in a line".into());
        }
        let at = corners[0];
        if let Some(p) = corners.iter().find(|&&p| (p - at).dot(facing).abs() > 0.02) {
            return Err(format!("a highlight's corners not flat: {p} lies {:.3} m off their plane", (*p - at).dot(facing).abs()));
        }
        let u = (corners[1] - at).reject_from_normalized(facing).normalize();
        let w = facing.cross(u);
        let outline: Vec<Vec2> = corners.iter().map(|&p| Vec2::new((p - at).dot(u), (p - at).dot(w))).collect();
        let n = outline.len();
        if (0..n).any(|k| (outline[(k + 1) % n] - outline[k]).perp_dot(outline[(k + 2) % n] - outline[(k + 1) % n]) < -1e-6) {
            return Err("a highlight's corners not convex, in order round it".into());
        }
        Ok(Area { at, facing, u, w, outline })
    }

    /// The bone it lies on: the one nearest its middle.
    fn bone(&self, cuts: &[(Vec3, Vec3); 13]) -> usize {
        let middle = self.outline.iter().sum::<Vec2>() / self.outline.len() as f32;
        Joints::bone_of(cuts, self.at + self.u * middle.x + self.w * middle.y)
    }

    /// Its part over the triangle `tri` (facing the way of `out`): clipped
    /// to it, laid `LIFT` off it, wound to face out; none if it is not under
    /// it, faces across it, or lies further than `DEEP` from it.
    fn over(&self, tri: [Vec3; 3], out: Vec3) -> Vec<Vec3> {
        let [p, q, r] = tri;
        let n = (q - p).cross(r - p).normalize_or_zero();
        let n = if n.dot(out) < 0.0 { -n } else { n };
        if n.dot(self.facing).abs() < 0.5 || tri.iter().all(|&x| (x - self.at).dot(self.facing).abs() > DEEP) {
            return Vec::new();
        }
        let flat = |x: Vec3| Vec2::new((x - self.at).dot(self.u), (x - self.at).dot(self.w));
        let mut shape: Vec<Vec2> = tri.map(flat).to_vec();
        let corners = self.outline.len();
        for k in 0..corners {
            let (a, b) = (self.outline[k], self.outline[(k + 1) % corners]);
            let inside = |x: Vec2| (b - a).perp_dot(x - a);
            let mut kept = Vec::new();
            for j in 0..shape.len() {
                let (x, y) = (shape[j], shape[(j + 1) % shape.len()]);
                let (sx, sy) = (inside(x), inside(y));
                if sx >= 0.0 {
                    kept.push(x);
                }
                if (sx >= 0.0) != (sy >= 0.0) {
                    kept.push(x + (y - x) * (sx / (sx - sy)));
                }
            }
            shape = kept;
            if shape.len() < 3 {
                return Vec::new();
            }
        }
        // Onto the triangle along `facing`, then off it.
        let mut laid: Vec<Vec3> = shape
            .iter()
            .map(|v| {
                let x = self.at + self.u * v.x + self.w * v.y;
                x + self.facing * ((p - x).dot(n) / self.facing.dot(n)) + n * LIFT
            })
            .collect();
        let turned: Vec3 = laid.iter().zip(laid.iter().cycle().skip(1)).map(|(a, b)| a.cross(*b)).sum();
        if turned.length() < 1e-8 {
            return Vec::new();
        }
        if turned.dot(n) < 0.0 {
            laid.reverse();
        }
        laid
    }
}

/// The model cut (see the module's account): each primitive's triangles to
/// their bones, carried into each bone's frame at rest.
fn cut(joints: &Joints, plan: Plan, gltf: &Gltf, nodes: &Assets<GltfNode>, gltf_meshes: &Assets<GltfMesh>, meshes: &mut Assets<Mesh>, server: &AssetServer) -> Pieces {
    let name = &joints.model;
    // Every primitive, with its node's place (a flat model, as generated).
    let mut parts: Vec<(Mesh, Transform, Handle<StandardMaterial>)> = Vec::new();
    for node in gltf.nodes.iter().filter_map(|n| nodes.get(n)) {
        let Some(m) = node.mesh.as_ref().and_then(|m| gltf_meshes.get(m)) else { continue };
        for p in &m.primitives {
            let mesh = meshes.get(&p.mesh).unwrap_or_else(|| panic!("{name}: a mesh not loaded"));
            // (Its material as the game draws it: the standard material the
            // loader keeps beside each of the model's, `<label>/std`.)
            let material = p.material.as_ref().and_then(|m| server.get_path(m)).map(|path| path.into_owned()).map(|path| {
                let label = path.label().unwrap_or_else(|| panic!("{name}: a material without a label")).to_string();
                server.load::<StandardMaterial>(path.clone().with_label(format!("{label}/std")))
            });
            parts.push((mesh.clone(), node.transform, material.unwrap_or_default()));
        }
    }
    assert!(!parts.is_empty(), "{name}: no meshes in it");
    let positions = |(mesh, t, _): &(Mesh, Transform, _)| floats::<3>(mesh, Mesh::ATTRIBUTE_POSITION).unwrap_or_else(|| panic!("{name}: a mesh without positions")).into_iter().map(|p| t.transform_point(Vec3::from(p))).collect::<Vec<_>>();
    // Stood at its height, its lowest point at 0.
    let (low, high) = parts.iter().flat_map(positions).fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p.y), h.max(p.y)));
    let scale = joints.height / (high - low);
    let stood = |p: Vec3| (p - Vec3::Y * low) * scale;
    // (Mirrored: measured whole, as its joints were, then halved; its head
    // kept whole, one.)
    let cuts = joints.cuts();
    let parts = match joints.mirror {
        Some(keep) => parts.iter().map(|part| (mirrored(part, keep, |m| Joints::bone_of(&cuts, stood(m)) == 4, name), Transform::IDENTITY, part.2.clone())).collect(),
        None => parts,
    };
    let rest = Rig::new(plan, Vec3::ZERO, 0.0, 0, &|_, _| 0.0).bones;
    let carry = joints.carry(&rest);
    let marked = joints.marked();
    // The torso, rump to chest (its three bones in a line): its rump, to its
    // hips, as long as the mesh's, the rest (the waist) stretched to the
    // rig's (`spine`; the user, 2026-11-13: stretched evenly, the rump grew
    // too and its hind legs were left under the middle of it). A point's
    // distance along it, so placed; and how much it is stretched there.
    let (start, axis) = (cuts[0].0, (cuts[2].1 - cuts[0].0).normalize());
    let length = start.distance(cuts[2].1);
    let hips_at = (joints.hips - start).dot(axis).clamp(0.0, length * 0.9);
    let waist = (rest[0].a.distance(rest[2].b) - hips_at) / (length - hips_at);
    // (Without a rump, the stalker's, all of it evenly.)
    let rear = if joints.rump.is_some() { 1.0 } else { waist };
    let torso = |s: f32| if s <= hips_at { (s * rear, rear) } else { (hips_at + (s - hips_at) * waist, waist) };
    // A bone's: how thick it is made; a point (stood), in its frame (before
    // turning), and how much it is stretched along it there; a normal there.
    // (Stretched, thickened by `thicken`: the harrow's forelegs, stretched
    // only, were needles, the user's look. The torso only longer, not
    // thickened.)
    let thick = |bone: usize| if bone < 3 { 1.0 } else { carry[bone].3.max(1.0).powf(joints.thicken) };
    let local = |bone: usize, p: Vec3| -> (Vec3, f32) {
        let (from, _, along, stretch) = carry[bone];
        if bone < 3 {
            let s = (p - start).dot(axis);
            let (at, k) = torso(s);
            (axis * (at - rest[0].a.distance(rest[bone].a)) + (p - start - axis * s), k)
        } else {
            let v = p - from;
            (along * v.dot(along) * stretch + (v - along * v.dot(along)) * thick(bone), stretch)
        }
    };
    // (By the inverse of that: turned only, a stretched piece's faces were
    // lit as if unstretched, pale.)
    let normal_in = |bone: usize, p: Vec3, n: Vec3| {
        let (_, turn, along, _) = carry[bone];
        let k = local(bone, p).1;
        (turn * (along * n.dot(along) / k + (n - along * n.dot(along)) / thick(bone))).normalize_or_zero()
    };
    let mut out = Vec::new();
    let mut laid_each = vec![0; joints.highlights.len()];
    for part in &parts {
        let (mesh, t, material) = part;
        let place = positions(part);
        let normal = floats::<3>(mesh, Mesh::ATTRIBUTE_NORMAL).unwrap_or_else(|| panic!("{name}: a mesh without normals"));
        let uv = floats::<2>(mesh, Mesh::ATTRIBUTE_UV_0);
        let tangent = floats::<4>(mesh, Mesh::ATTRIBUTE_TANGENT);
        let corners: Vec<usize> = match mesh.indices() {
            Some(i) => i.iter().collect(),
            None => (0..place.len()).collect(),
        };
        assert!(mesh.primitive_topology() == PrimitiveTopology::TriangleList, "{name}: not triangles");
        // Its highlights, laid over its triangles under them, each its
        // bone's (flat, as cut: no texture to follow).
        for (k, area) in joints.highlights.iter().enumerate() {
            let bone = area.bone(&cuts);
            let mut laid: Vec<(Vec3, Vec3)> = Vec::new();
            for tri in corners.as_chunks::<3>().0 {
                let at = tri.map(|i| stood(place[i]));
                if Joints::bone_of(&cuts, at.iter().sum::<Vec3>() / 3.0) != bone {
                    continue;
                }
                let outward = tri.iter().map(|&i| t.rotation * Vec3::from(normal[i])).sum::<Vec3>();
                let shape = area.over(at, outward);
                if let [first, rest @ ..] = shape.as_slice() {
                    let n = shape.iter().zip(shape.iter().cycle().skip(1)).map(|(a, b)| a.cross(*b)).sum::<Vec3>().normalize();
                    for pair in rest.windows(2) {
                        laid.extend([*first, pair[0], pair[1]].map(|x| (x, n)));
                    }
                }
            }
            laid_each[k] += laid.len() / 3;
            if !laid.is_empty() {
                let (_, turn, _, _) = carry[bone];
                let piece = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, laid.iter().map(|&(x, _)| (turn * local(bone, x).0).to_array()).collect::<Vec<_>>())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, laid.iter().map(|&(x, n)| normal_in(bone, x, n).to_array()).collect::<Vec<_>>());
                out.push((bone, meshes.add(piece), material.clone(), true));
            }
        }
        // Each triangle to its bone, and to its marks or not; each bone's
        // corners (each half's), its own.
        let mut each: Vec<(HashMap<usize, u32>, Vec<u32>)> = (0..26).map(|_| Default::default()).collect();
        for tri in corners.as_chunks::<3>().0 {
            let middle = tri.iter().map(|&i| stood(place[i])).sum::<Vec3>() / 3.0;
            let mark = joints.marks > 0.0 && marked.iter().any(|j| j.distance(middle) < joints.marks);
            let (seen, list) = &mut each[Joints::bone_of(&cuts, middle) * 2 + mark as usize];
            for &i in tri {
                let next = seen.len() as u32;
                list.push(*seen.entry(i).or_insert(next));
            }
        }
        for (k, (seen, list)) in each.into_iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let (bone, mark) = (k / 2, k % 2 == 1);
            let (_, turn, along, _) = carry[bone];
            let mut order: Vec<(usize, u32)> = seen.into_iter().collect();
            order.sort_by_key(|&(_, k)| k);
            let at = |i: usize| turn * local(bone, stood(place[i])).0;
            let normal_of = |i: usize| normal_in(bone, stood(place[i]), t.rotation * Vec3::from(normal[i]));
            let scaled = |v: Vec3, k: f32| along * v.dot(along) * k + (v - along * v.dot(along)) * thick(bone);
            let mut piece = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, order.iter().map(|&(i, _)| at(i).to_array()).collect::<Vec<_>>())
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, order.iter().map(|&(i, _)| normal_of(i).to_array()).collect::<Vec<_>>());
            if let Some(uv) = &uv {
                piece.insert_attribute(Mesh::ATTRIBUTE_UV_0, order.iter().map(|&(i, _)| uv[i]).collect::<Vec<_>>());
            }
            if let Some(tangent) = &tangent {
                piece.insert_attribute(Mesh::ATTRIBUTE_TANGENT, order.iter().map(|&(i, _)| { let v = tangent[i]; let d = (turn * scaled(t.rotation * Vec3::new(v[0], v[1], v[2]), local(bone, stood(place[i])).1)).normalize_or_zero(); [d.x, d.y, d.z, v[3]] }).collect::<Vec<_>>());
            }
            piece.insert_indices(Indices::U32(list));
            out.push((bone, meshes.add(piece), material.clone(), mark));
        }
    }
    for (k, laid) in laid_each.iter().enumerate() {
        assert!(*laid > 0, "{name}: highlight {} lies over none of its surface (within {DEEP} m, its corners' plane)", k + 1);
    }
    let dressed: std::collections::BTreeSet<usize> = out.iter().map(|p| p.0).collect();
    info!("the {}'s body cut from {name}: {} pieces on {} of its 13 bones", joints.kind.name(), out.len(), dressed.len());
    out
}

/// Creatures waiting given their pieces, once their model is in (cut once).
#[allow(clippy::too_many_arguments)]
fn dress(mut commands: Commands, mut kit: ResMut<Kit>, gltfs: Res<Assets<Gltf>>, nodes: Res<Assets<GltfNode>>, gltf_meshes: Res<Assets<GltfMesh>>, mut meshes: ResMut<Assets<Mesh>>, server: Res<AssetServer>, waiting: Query<(Entity, &Wants), With<Hunter>>) {
    for (hunter, wants) in &waiting {
        let kind = wants.kind;
        let Some(handle) = kit.gltf.get(&kind).cloned() else {
            panic!("mesh=1: no {} (see combat/pieces.rs)", kind.file().display());
        };
        if let Some(bevy::asset::LoadState::Failed(e)) = server.get_load_state(&handle) {
            panic!("{}: {e}", joints(kind).model);
        }
        if !server.is_loaded_with_dependencies(&handle) {
            continue;
        }
        let Some(gltf) = gltfs.get(&handle) else { continue };
        assert!(!wants.own || kind == Kind::Stalker, "only the stalker has its own build to stretch its pieces to");
        let pieces = kit.pieces.entry((kind, wants.own)).or_insert_with(|| cut(joints(kind), if wants.own { Plan::stalker() } else { joints(kind).plan() }, gltf, &nodes, &gltf_meshes, &mut meshes, &server));
        for (bone, mesh, material, mark) in pieces.iter().filter(|p| wants.head || !(p.0 == 3 || p.0 == 4)) {
            let look = wants.look.clone().unwrap_or_else(|| material.clone());
            commands.spawn((
                Part { hunter, bone: *bone, along: 0.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(if *mark { wants.marks.clone().unwrap_or(look) } else { look }),
                Transform::default(),
            ));
        }
        commands.entity(hunter).remove::<Wants>();
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Kit>().add_systems(Startup, load).add_systems(Update, dress);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(kind: Kind) -> Joints {
        Joints::parse(&std::fs::read_to_string(kind.file()).unwrap(), kind).unwrap()
    }

    #[test]
    fn the_files_read() {
        for kind in Kind::ALL {
            let j = read(kind);
            assert!(super::super::creatures_folder().join(&j.model).is_file(), "{}: not in creatures/", j.model);
        }
    }

    /// Its build fitted: each bone at rest as long as the mesh has it, so
    /// no piece is stretched (the stalker's head kept as it is); but the
    /// harrow's forelegs (bones 5 to 8), the harrow's long ones.
    #[test]
    fn pieces_unstretched() {
        for kind in Kind::ALL {
            let j = read(kind);
            let rest = Rig::new(j.plan(), Vec3::ZERO, 0.0, 0, &|_, _| 0.0).bones;
            for (k, (_, _, _, stretch)) in j.carry(&rest).iter().enumerate() {
                if kind == Kind::Harrow && (5..9).contains(&k) {
                    assert!(*stretch > 1.5, "{kind:?}: foreleg bone {k} stretched only {stretch:.3}");
                } else if k < 3 {
                    assert!((stretch / j.spine - 1.0).abs() < 0.02, "{kind:?}: torso bone {k} stretched {stretch:.3}, not its spine {}", j.spine);
                } else {
                    assert!((stretch - 1.0).abs() < 0.02, "{kind:?}: bone {k} stretched {stretch:.3}");
                }
            }
        }
    }

    /// Mirrored, a model is the kept half and its image, facing out; and its
    /// limbs pairs of images.
    #[test]
    fn mirrored_halves() {
        let tri = |x: f32| [[x, 0.0, 0.0], [x, 1.0, 0.0], [x, 0.0, 1.0]];
        let place: Vec<[f32; 3]> = tri(-1.0).into_iter().chain(tri(2.0)).collect();
        let normal = vec![[-1.0, 0.0, 0.0]; 3].into_iter().chain(vec![[1.0, 0.0, 0.0]; 3]).collect::<Vec<_>>();
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, place).with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal);
        let part = (mesh, Transform::IDENTITY, Handle::default());
        // Kept whole (its head), the triangle on the other side stays, one.
        let whole = mirrored(&part, -1.0, |m| m.x > 0.0, "test");
        assert_eq!(floats::<3>(&whole, Mesh::ATTRIBUTE_POSITION).unwrap().len(), 9, "the kept triangle, its image, and the whole one alone");
        let half = mirrored(&part, -1.0, |_| false, "test");
        let p = floats::<3>(&half, Mesh::ATTRIBUTE_POSITION).unwrap();
        let n = floats::<3>(&half, Mesh::ATTRIBUTE_NORMAL).unwrap();
        assert_eq!(p.len(), 6, "the kept triangle and its image, not the other");
        assert!(p[..3].iter().all(|v| v[0] == -1.0) && p[3..].iter().all(|v| v[0] == 1.0));
        assert!(n[..3].iter().all(|v| v[0] == -1.0) && n[3..].iter().all(|v| v[0] == 1.0), "each facing out");
        // Its winding turned: the image's face (by its corners) the way its normal is.
        let [a, b, c] = [Vec3::from(p[3]), Vec3::from(p[4]), Vec3::from(p[5])];
        let [a0, b0, c0] = [Vec3::from(p[0]), Vec3::from(p[1]), Vec3::from(p[2])];
        assert!((b - a).cross(c - a).x.signum() == -(b0 - a0).cross(c0 - a0).x.signum(), "its winding not turned");
        let j = read(Kind::Harrow);
        if j.mirror.is_some() {
            for pair in j.legs.chunks(2) {
                for (p, q) in pair[0].iter().zip(&pair[1]) {
                    assert!((*p * Vec3::new(-1.0, 1.0, 1.0)).distance(*q) < 1e-6, "its legs not images: {p} {q}");
                }
            }
        }
    }

    /// A highlight over a thin plate lies on both its faces, all of it on
    /// each, wound to face out, and on none of its edges; its corners must
    /// be flat and convex.
    #[test]
    fn highlights_lie_on_both_faces() {
        let corners = [Vec3::new(-0.2, 0.02, -0.1), Vec3::new(-0.2, 0.02, 0.1), Vec3::new(0.2, 0.02, 0.1), Vec3::new(0.2, 0.02, -0.1)];
        let area = Area::new(&corners).unwrap();
        let plate = Mesh::from(Cuboid::new(1.0, 0.04, 0.5));
        let place: Vec<Vec3> = floats::<3>(&plate, Mesh::ATTRIBUTE_POSITION).unwrap().into_iter().map(Vec3::from).collect();
        let normal: Vec<Vec3> = floats::<3>(&plate, Mesh::ATTRIBUTE_NORMAL).unwrap().into_iter().map(Vec3::from).collect();
        let (mut up, mut down) = (0.0, 0.0);
        for tri in plate.indices().unwrap().iter().collect::<Vec<_>>().as_chunks::<3>().0 {
            let shape = area.over(tri.map(|i| place[i]), tri.iter().map(|&i| normal[i]).sum());
            if shape.is_empty() {
                continue;
            }
            let turned: Vec3 = shape.iter().zip(shape.iter().cycle().skip(1)).map(|(a, b)| a.cross(*b)).sum();
            let face = normal[tri[0]];
            assert!(face.y.abs() > 0.9, "laid on an edge, facing {face}");
            assert!(turned.dot(face) > 0.0, "wound to face in");
            assert!(shape.iter().all(|p| (p.y.abs() - (0.02 + LIFT)).abs() < 1e-5), "not laid just off its face: {shape:?}");
            *(if face.y > 0.0 { &mut up } else { &mut down }) += turned.length() / 2.0;
        }
        assert!((up - 0.08_f32).abs() < 1e-4 && (down - 0.08_f32).abs() < 1e-4, "its area on each face 0.08: up {up}, down {down}");
        assert!(Area::new(&[Vec3::ZERO, Vec3::X, Vec3::new(0.2, 0.2, 0.0), Vec3::Y]).is_err(), "not convex, taken");
        assert!(Area::new(&[Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.1), Vec3::Y]).is_err(), "not flat, taken");
    }

    /// The harrow on its mesh hauls on each foreleg in turn, twice a cycle;
    /// the harrow's own still on both together, once.
    #[test]
    fn hauls_in_turn() {
        let rig = |plan: Plan| Rig::new(plan, Vec3::ZERO, 0.0, 0, &|_, _| 0.0);
        assert_eq!(rig(read(Kind::Harrow).plan()).hauls(), 2.0);
        assert_eq!(rig(Plan::beast()).hauls(), 1.0);
        // Fast, its forelegs all but never in the air together (now and then
        // a foot lifts early, its leg unable to hold it); the harrow's mostly are.
        for (plan, alternate) in [(read(Kind::Harrow).plan(), true), (Plan::beast(), false)] {
            let mut r = rig(plan);
            let (mut both, mut either) = (0, 0);
            for frame in 0..900 {
                let intent = crate::rig::Intent { velocity: Vec3::Z * 6.0, look: r.root + Vec3::new(0.0, 1.5, 20.0), ..Default::default() };
                r.update(1.0 / 60.0, intent, &|_, _| 0.0, &|from, step| from + step);
                if frame >= 180 {
                    both += (r.feet[0].swinging && r.feet[1].swinging) as u32;
                    either += (r.feet[0].swinging || r.feet[1].swinging) as u32;
                }
            }
            println!("alternate {alternate}: both up {both} of {either} frames with one up");
            assert!(either > 100, "it never stepped");
            assert_eq!(both * 50 < either, alternate, "forelegs up together {both} frames of {either}");
        }
    }

    /// The harrow's hind legs at its hind end, however long its spine: as
    /// far before its rump as the mesh has them, their knees back (the
    /// user, 2026-11-13: the spine lengthened evenly, they were left under
    /// the middle of a longer rump; and folded forward, under its body).
    #[test]
    fn hind_legs_at_its_hind_end() {
        let j = read(Kind::Harrow);
        assert!(j.spine > 1.0, "test it lengthened");
        let rest = Rig::new(j.plan(), Vec3::ZERO, 0.0, 0, &|_, _| 0.0).bones;
        let rump = j.rump.expect("a harrow's rump");
        for (bone, leg) in [(9, 2), (11, 3)] {
            let (rig, mesh) = (rest[bone].a.z - rest[0].a.z, j.legs[leg][0].z - rump.z);
            assert!((rig - mesh).abs() < 0.02, "hind leg {leg}: {rig:.3} m before its rump, the mesh's {mesh:.3}");
            // Its knee back, behind its hip, as the mesh has it.
            assert!(rest[bone].b.z < rest[bone].a.z - 0.2, "hind leg {leg}: its knee not behind its hip");
        }
    }

    /// On its fitted build it walks with its feet down, as its creature
    /// does.
    #[test]
    fn walks_on_its_legs() {
        // (The harrow's varied too, as it is spawned: `vary_time`, `vary_length`.)
        let varied = Plan { wander: (0.6, 0.6), ..read(Kind::Harrow).plan() };
        for (kind, plan, speeds) in [(Kind::Stalker, read(Kind::Stalker).plan(), [0.0, 0.8, 1.6]), (Kind::Harrow, read(Kind::Harrow).plan(), [0.0, 2.0, 6.0]), (Kind::Harrow, varied, [0.0, 2.0, 6.0])] {
            for speed in speeds {
                let mut rig = Rig::new(plan.clone(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
                let mut worst = 0.0f32;
                for frame in 0..900 {
                    let intent = crate::rig::Intent { velocity: Vec3::Z * speed, look: rig.root + Vec3::new(0.0, 1.5, 20.0), ..Default::default() };
                    rig.update(1.0 / 60.0, intent, &|_, _| 0.0, &|from, step| from + step);
                    if frame >= 120 {
                        for (i, foot) in rig.feet.iter().enumerate().filter(|(_, f)| !f.swinging) {
                            worst = worst.max(rig.bones[6 + 2 * i].b.distance(foot.planted));
                        }
                    }
                }
                assert!(worst < 0.05, "{kind:?}: at {speed} m/s a planted foot was {worst:.3} m off where it was put down");
            }
        }
    }
}
