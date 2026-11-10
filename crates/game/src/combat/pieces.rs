//! A body cut from a generated mesh into rigid pieces, one on each bone,
//! moved by the creature's own walk: no rigging, no clips (the user,
//! 2026-11-06: the stalker's walk on a new mesh). The stalker's (`spawn
//! stalker mesh=1`), from `creatures/stalker.pieces`: the model and its
//! joints, measured. Its build is fitted to the mesh (its legs, arms, torso
//! and neck as long as the mesh's; the rest of it, and how it moves, the
//! stalker's), so no piece is stretched. Each triangle goes to the bone it
//! is nearest (its head: all from its neck's top up), keeping the model's
//! own surface (its textures and normal map); each piece is turned and set
//! from where the mesh has its bone onto where the stalker stands it, and
//! rides it rigidly. (The shard beast's way, `tools/bake_beast.py`, done
//! here as it loads: the model stays the one source, its joints the one
//! thing measured.)

use std::{collections::HashMap, path::PathBuf, sync::OnceLock};

use bevy::{
    asset::RenderAssetUsages,
    gltf::{Gltf, GltfMesh, GltfNode},
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

use super::hunter::{Hunter, Part, RIGID};
use crate::rig::{ArmPlan, Bone, LegPlan, Plan, Rig};

/// A model's joints as measured (m: the model scaled to `height`, its
/// lowest point at y 0, x and z as the model has them; x to its left, z
/// forward). Legs and arms: the one at -x first, as the rig has them; each
/// hip (shoulder), knee (elbow), foot (palm).
#[derive(Clone, Debug)]
pub(crate) struct Joints {
    pub(crate) model: String,
    height: f32,
    hips: Vec3,
    chest: Vec3,
    head: Vec3,
    legs: [[Vec3; 3]; 2],
    arms: [[Vec3; 3]; 2],
}

fn file() -> PathBuf {
    super::creatures_folder().join("stalker.pieces")
}

/// The stalker's, read once (failing loudly: the file and what is wrong).
pub(super) fn joints() -> &'static Joints {
    static READ: OnceLock<Joints> = OnceLock::new();
    READ.get_or_init(|| {
        let path = file();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e} (mesh=1 needs it)", path.display()));
        Joints::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    })
}

impl Joints {
    pub(crate) fn parse(text: &str) -> Result<Joints, String> {
        let (mut model, mut height, mut hips, mut chest, mut head, mut legs, mut arms) = (None, None, None, None, None, Vec::new(), Vec::new());
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
            let numbers: Vec<f32> = value.split_whitespace().map(str::parse).collect::<Result<_, _>>().map_err(|_| fail("not numbers"))?;
            let points = |count: usize| -> Result<Vec<Vec3>, String> {
                if numbers.len() != 3 * count {
                    return Err(fail(&format!("{} numbers wanted, {count} points", 3 * count)));
                }
                Ok(numbers.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect())
            };
            match key {
                "height" if numbers.len() == 1 && numbers[0] > 0.0 => height = Some(numbers[0]),
                "hips" => hips = Some(points(1)?[0]),
                "chest" => chest = Some(points(1)?[0]),
                "head" => head = Some(points(1)?[0]),
                "leg" => legs.push(<[Vec3; 3]>::try_from(points(3)?).expect("three")),
                "arm" => arms.push(<[Vec3; 3]>::try_from(points(3)?).expect("three")),
                _ => return Err(fail("not model, height, hips, chest, head, leg or arm")),
            }
        }
        let some = |v: Option<Vec3>, name: &str| v.ok_or(format!("no `{name} =` line"));
        let pair = |v: Vec<[Vec3; 3]>, name: &str| <[[Vec3; 3]; 2]>::try_from(v).map_err(|v| format!("{} `{name} =` lines, two wanted", v.len()));
        let joints = Joints { model: model.ok_or("no `model =` line")?, height: height.ok_or("no `height =` line, a length")?, hips: some(hips, "hips")?, chest: some(chest, "chest")?, head: some(head, "head")?, legs: pair(legs, "leg")?, arms: pair(arms, "arm")? };
        if !(joints.legs[0][0].x < joints.legs[1][0].x && joints.arms[0][0].x < joints.arms[1][0].x) {
            return Err("the leg (arm) at -x comes first".into());
        }
        if !(joints.hips.y < joints.chest.y && joints.chest.y < joints.head.y && joints.head.y < joints.height) {
            return Err("hips, chest and head must rise in that order, under its height".into());
        }
        Ok(joints)
    }

    /// The stalker's build fitted to these: its legs, arms, torso and neck
    /// as long as the mesh's, stood as the stalker stands (its hips as high
    /// for its legs, its chest as far before them); the rest its own.
    pub(crate) fn plan(&self) -> Plan {
        let base = Plan::stalker();
        let stance = base.hips.1 / (base.legs[0].upper + base.legs[0].lower);
        let legs: Vec<LegPlan> = base.legs.iter().zip(&self.legs).map(|(l, j)| LegPlan { side: j[0].x - self.hips.x, upper: j[0].distance(j[1]), lower: j[1].distance(j[2]), ..*l }).collect();
        let arms: Vec<ArmPlan> = base.arms.iter().zip(&self.arms).map(|(a, j)| ArmPlan { side: j[0].x - self.chest.x, upper: j[0].distance(j[1]), lower: j[1].distance(j[2]), ..*a }).collect();
        let hips = legs.iter().map(|l| l.upper + l.lower).sum::<f32>() / legs.len() as f32 * stance;
        let torso = self.hips.distance(self.chest);
        let chest = (base.chest.0, hips + (torso * torso - base.chest.0 * base.chest.0).max(0.0).sqrt());
        Plan { hips: (base.hips.0, hips), chest, neck: (self.chest.distance(self.head), base.neck.1, base.neck.2), legs, arms, ..base }
    }

    /// Where the mesh has each bone (in its rig's order: three of torso,
    /// neck, head, each leg's two, each arm's two): its head from its
    /// neck's top straight up to its crown.
    fn cuts(&self) -> [(Vec3, Vec3); 13] {
        let third = |k: f32| self.hips.lerp(self.chest, k / 3.0);
        let [l, r] = self.legs;
        let [la, ra] = self.arms;
        [
            (third(0.0), third(1.0)),
            (third(1.0), third(2.0)),
            (third(2.0), third(3.0)),
            (self.chest, self.head),
            (self.head, self.head.with_y(self.height)),
            (l[0], l[1]),
            (l[1], l[2]),
            (r[0], r[1]),
            (r[1], r[2]),
            (la[0], la[1]),
            (la[1], la[2]),
            (ra[0], ra[1]),
            (ra[1], ra[2]),
        ]
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
    /// mesh has its bone's foot, turned from its way to the bone's (its
    /// head upright: its up to the head's), and stretched along it if it is
    /// longer (it is not, fitted: see `plan`).
    fn carry(&self, rest: &[Bone]) -> [(Vec3, Quat, Vec3, f32); 13] {
        let cuts = self.cuts();
        std::array::from_fn(|k| {
            let (a, b) = cuts[k];
            let to = &rest[k];
            let from = (b - a).normalize();
            let (onto, stretch) = if k == 4 { (to.rotation() * Vec3::NEG_Z, 1.0) } else { ((to.b - to.a).normalize(), to.a.distance(to.b) / a.distance(b)) };
            (a, to.rotation().inverse() * Quat::from_rotation_arc(from, onto), from, stretch)
        })
    }
}

/// A stalker waiting for its pieces (its own head or, `head=1`, its cage:
/// the mesh's neck and head left out); on its build fitted to the mesh
/// (`mesh=1`) or, `own`, the stalker's own, the pieces stretched along their
/// bones to it (`mesh=2`: the user, 2026-11-06, its proportions and height
/// were better).
#[derive(Component)]
pub(super) struct Wants {
    pub(super) head: bool,
    pub(super) own: bool,
}

/// The stalker's model loading, and once loaded its pieces: each bone's,
/// with the model's own material.
#[derive(Resource, Default)]
pub(super) struct Kit {
    gltf: Option<Handle<Gltf>>,
    pieces: [Option<Pieces>; 2],
}

/// Each piece: its bone, its mesh (in the bone's frame), its material.
type Pieces = Vec<(usize, Handle<Mesh>, Handle<StandardMaterial>)>;

fn load(mut kit: ResMut<Kit>, server: Res<AssetServer>) {
    if file().is_file() {
        let joints = joints();
        kit.gltf = Some(server.load(format!("creatures://{}", joints.model)));
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
    let rest = Rig::new(plan, Vec3::ZERO, 0.0, 0, &|_, _| 0.0).bones;
    let cuts = joints.cuts();
    let carry = joints.carry(&rest);
    let mut out = Vec::new();
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
        // Each triangle to its bone; each bone's corners, its own.
        let mut each: Vec<(HashMap<usize, u32>, Vec<u32>)> = (0..13).map(|_| Default::default()).collect();
        for tri in corners.as_chunks::<3>().0 {
            let middle = tri.iter().map(|&i| stood(place[i])).sum::<Vec3>() / 3.0;
            let (seen, list) = &mut each[Joints::bone_of(&cuts, middle)];
            for &i in tri {
                let next = seen.len() as u32;
                list.push(*seen.entry(i).or_insert(next));
            }
        }
        for (bone, (seen, list)) in each.into_iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let (from, turn, along, stretch) = carry[bone];
            let mut order: Vec<(usize, u32)> = seen.into_iter().collect();
            order.sort_by_key(|&(_, k)| k);
            let spin = turn * t.rotation;
            let at = |i: usize| {
                let q = stood(place[i]) - from;
                turn * (q + along * q.dot(along) * (stretch - 1.0))
            };
            let mut piece = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, order.iter().map(|&(i, _)| at(i).to_array()).collect::<Vec<_>>())
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, order.iter().map(|&(i, _)| (spin * Vec3::from(normal[i])).normalize_or_zero().to_array()).collect::<Vec<_>>());
            if let Some(uv) = &uv {
                piece.insert_attribute(Mesh::ATTRIBUTE_UV_0, order.iter().map(|&(i, _)| uv[i]).collect::<Vec<_>>());
            }
            if let Some(tangent) = &tangent {
                piece.insert_attribute(Mesh::ATTRIBUTE_TANGENT, order.iter().map(|&(i, _)| { let v = tangent[i]; let d = spin * Vec3::new(v[0], v[1], v[2]); [d.x, d.y, d.z, v[3]] }).collect::<Vec<_>>());
            }
            piece.insert_indices(Indices::U32(list));
            out.push((bone, meshes.add(piece), material.clone()));
        }
    }
    let dressed: std::collections::BTreeSet<usize> = out.iter().map(|p| p.0).collect();
    info!("the stalker's body cut from {name}: {} pieces on {} of its 13 bones", out.len(), dressed.len());
    out
}

/// Stalkers waiting given their pieces, once the model is in (cut once).
#[allow(clippy::too_many_arguments)]
fn dress(mut commands: Commands, mut kit: ResMut<Kit>, gltfs: Res<Assets<Gltf>>, nodes: Res<Assets<GltfNode>>, gltf_meshes: Res<Assets<GltfMesh>>, mut meshes: ResMut<Assets<Mesh>>, server: Res<AssetServer>, waiting: Query<(Entity, &Wants), With<Hunter>>) {
    if waiting.is_empty() {
        return;
    }
    let Some(handle) = kit.gltf.clone() else {
        panic!("mesh=1: no {} (see combat/pieces.rs)", file().display());
    };
    if let Some(bevy::asset::LoadState::Failed(e)) = server.get_load_state(&handle) {
        panic!("{}: {e}", joints().model);
    }
    if !server.is_loaded_with_dependencies(&handle) {
        return;
    }
    let Some(gltf) = gltfs.get(&handle) else { return };
    for (hunter, wants) in &waiting {
        let pieces = kit.pieces[wants.own as usize].get_or_insert_with(|| cut(joints(), if wants.own { Plan::stalker() } else { joints().plan() }, gltf, &nodes, &gltf_meshes, &mut meshes, &server));
        for (bone, mesh, material) in pieces.iter().filter(|p| wants.head || !(p.0 == 3 || p.0 == 4)) {
            commands.spawn((
                Part { hunter, bone: *bone, along: 0.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
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

    #[test]
    fn the_file_reads() {
        let j = Joints::parse(&std::fs::read_to_string(file()).unwrap()).unwrap();
        assert!(super::super::creatures_folder().join(&j.model).is_file(), "{}: not in creatures/", j.model);
    }

    /// Its build fitted: each bone at rest as long as the mesh has it, so
    /// no piece is stretched (its head kept as it is).
    #[test]
    fn pieces_unstretched() {
        let j = Joints::parse(&std::fs::read_to_string(file()).unwrap()).unwrap();
        let rest = Rig::new(j.plan(), Vec3::ZERO, 0.0, 0, &|_, _| 0.0).bones;
        for (k, (_, _, _, stretch)) in j.carry(&rest).iter().enumerate() {
            assert!((stretch - 1.0).abs() < 0.02, "bone {k} stretched {stretch:.3}");
        }
    }

    /// On its fitted build it walks with its feet down, as the stalker does.
    #[test]
    fn walks_on_its_legs() {
        let j = Joints::parse(&std::fs::read_to_string(file()).unwrap()).unwrap();
        for speed in [0.0, 0.8, 1.6] {
            let mut rig = Rig::new(j.plan(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
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
            assert!(worst < 0.05, "at {speed} m/s a planted foot was {worst:.3} m off where it was put down");
        }
    }
}
