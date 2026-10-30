//! A harrow's body from an authored mesh (its `mesh=1` option; the user,
//! 2026-10-29: none of the bodies built piece by piece convinced, their
//! outline never designed; a whole creature, generated as a mesh, brought
//! in as the stalker's head was). The shard beast: an arched spine, a crest
//! of blades tallest over its shoulders, an open ribcage, legs of stacked
//! shards, a burst of spikes for a head.
//!
//! Rigged to the mesh: its build is the mesh's, measured (`plan`), and it
//! moves as the harrow does (its gaits, its strides to its legs' length).
//! The mesh is cut into one rigid piece per bone; each piece rides its bone,
//! moved at build from where the mesh has its bone onto where the rig stands
//! it. (Fitted onto the harrow's own build first, its forelegs stretched into
//! slabs, its thighs left on its rump, the user.) Baked by
//! `tools/bake_beast.py` into `data/harrow_mesh.bin`.

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

use super::hunter::{Part, RIGID};
use crate::rig::{Bone, Gait, LegPlan, Plan, Rig, Root, Style};

/// The baked body: its build (the plan's numbers, m), each bone's segment
/// as the mesh has it, and each bone's triangles (corners and normals); in
/// the body's frame, standing (x right, y up from the ground, z forward).
struct Baked {
    build: [f32; 21],
    segments: [(Vec3, Vec3); 13],
    pieces: Vec<Vec<[(Vec3, Vec3); 3]>>,
}

const BAD: &str = "data/harrow_mesh.bin is not a baked body: bake it again with tools/bake_beast.py";

/// Reading the baked body, word by word.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn word(&mut self) -> [u8; 4] {
        assert!(self.bytes.len() >= self.at + 4, "{BAD}");
        self.at += 4;
        self.bytes[self.at - 4..self.at].try_into().expect("four bytes")
    }

    fn float(&mut self) -> f32 {
        f32::from_le_bytes(self.word())
    }

    fn vec3(&mut self) -> Vec3 {
        Vec3::new(self.float(), self.float(), self.float())
    }
}

fn decode(bytes: &[u8]) -> Baked {
    assert!(bytes.len() >= 8 && &bytes[..8] == b"BEAST3\0\0", "{BAD}");
    let mut r = Reader { bytes, at: 8 };
    let build = std::array::from_fn(|_| r.float());
    let segments = std::array::from_fn(|_| (r.vec3(), r.vec3()));
    let pieces = (0..13)
        .map(|_| {
            let count = u32::from_le_bytes(r.word()) as usize;
            (0..count).map(|_| std::array::from_fn(|_| (r.vec3(), r.vec3()))).collect()
        })
        .collect();
    assert_eq!(r.at, bytes.len(), "{BAD}");
    Baked { build, segments, pieces }
}

fn baked() -> Baked {
    decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/harrow_mesh.bin")))
}

/// Its build: the mesh's, its forelegs' joints folding forward and its hind
/// legs' back; moving as
/// a mass (`Style::Mass`, the user's choice). (Moving as the harrow does,
/// its strides cut to its legs, it stepped three times as fast and shook,
/// the user; crouched into the harrow's stance, its legs could not reach.)
/// Its gaits: from what pace (m/s), and how much of its stride a foot is in
/// the air: a walk, a trot's pace, a run, and flat out. Each stride as long
/// as its foreleg, a straight strut, sweeps from leaning forward to leaning
/// back as far (`rig::mass_lean`, more the faster) while its foot is down.
const STRIDES: [(f32, f32); 4] = [(0.0, 0.2), (3.0, 0.5), (8.0, 0.68), (14.0, 0.72)];
/// How tall it stands on its legs, of their length: nearly straight, struts
/// it vaults over (see `rig::MASS_RIDE`). (Stooped, its hips at four fifths
/// of its legs' length, its knees were at a right angle whatever it did,
/// the user.)
const STAND: f32 = 0.98;
/// Its hips drawn in under it, of how far out on its flanks the mesh's legs
/// meet its body; its feet set straight under them, its knees straight
/// back. (Out on its flanks, its feet splayed further as the mesh's are,
/// its feet came down wide of its body, its legs a centipede's, not under
/// it carrying it, the user.)
const UNDER: f32 = 0.7;

fn plan(baked: &Baked) -> Plan {
    let [hips_f, _, chest_f, _, rump, torso, neck, neck_f, neck_u, head, head_r, fore @ .., hind_side, hind_upper, hind_lower, _, fore_drop, hind_drop] = baked.build;
    let [fore_side, fore_upper, fore_lower, _] = fore;
    // (A foreleg's joint folds forward, its foot tucked back under it as it
    // swings, a hind leg's back, as a quadruped's do: both back, as the
    // mesh stands, its forelegs folded as a bird's, the shin thrown forward
    // at every step, the user.)
    let leg = |root, side: f32, drop, upper, lower| LegPlan { root, side: side * UNDER, forward: 0.0, drop, upper, lower, knee: if root == Root::Chest { 1.0 } else { -1.0 }, knee_up: 0.0, knee_out: 0.0, spread: 0.0, radius: (0.13, 0.1) };
    let beast = Plan::beast();
    let fore_strut = (fore_upper + fore_lower) * STAND;
    Plan {
        hips: (hips_f, (hind_upper + hind_lower) * STAND + hind_drop),
        chest: (chest_f, (fore_upper + fore_lower) * STAND + fore_drop),
        torso_radius: torso,
        torso_profile: [1.0; 3],
        rump,
        neck_radius: torso * 0.7,
        neck: (neck, neck_f, neck_u),
        head: (head, head_r),
        legs: vec![
            leg(Root::Chest, -fore_side, fore_drop, fore_upper, fore_lower),
            leg(Root::Chest, fore_side, fore_drop, fore_upper, fore_lower),
            leg(Root::Hips, -hind_side, hind_drop, hind_upper, hind_lower),
            leg(Root::Hips, hind_side, hind_drop, hind_upper, hind_lower),
        ],
        // One foot at a time, round the body: hind left, fore left, hind
        // right, fore right; walking, each down four fifths of its stride,
        // running, a third and less, the body carried between; each stride
        // its legs' sweep (`STRIDES`).
        gaits: STRIDES.iter().map(|&(speed, swing)| Gait { speed, offsets: vec![0.25, 0.75, 0.0, 0.5], swing, stride: 2.0 * fore_strut * crate::rig::mass_lean(speed).sin() / (1.0 - swing) }).collect(),
        lift: 0.3,
        accel: 12.0,
        decel: 14.0,
        turn: 2.2,
        lean: 0.04,
        shoulder: 0.0,
        style: Style::Mass,
        ..beast
    }
}

/// Its build, and its pieces, each on its bone.
pub(super) struct Kit {
    pub(super) plan: Plan,
    pieces: Vec<(usize, Handle<Mesh>)>,
}

/// Each piece moved from its bone's segment in the mesh onto the bone as the
/// rig stands (turned onto it, stretched along it to its length), in the
/// bone's frame.
fn fitted(baked: &Baked, rest: &[Bone]) -> Vec<(usize, Vec<[(Vec3, Vec3); 3]>)> {
    (0..13)
        .map(|bone| {
            let (a, b) = baked.segments[bone];
            let to = &rest[bone];
            let (from, onto) = ((b - a).normalize(), (to.b - to.a).normalize());
            let stretch = to.a.distance(to.b) / a.distance(b);
            let turn = to.rotation().inverse() * Quat::from_rotation_arc(from, onto);
            let tris = baked.pieces[bone]
                .iter()
                .map(|tri| {
                    tri.map(|(p, n)| {
                        let q = p - a;
                        let along = q.dot(from);
                        (turn * (q + from * along * (stretch - 1.0)), turn * n)
                    })
                })
                .collect();
            (bone, tris)
        })
        .collect()
}

pub(super) fn build(meshes: &mut Assets<Mesh>) -> Kit {
    let baked = baked();
    let plan = plan(&baked);
    let rest = Rig::new(plan.clone(), Vec3::ZERO, 0.0, 0, &|_, _| 0.0).bones;
    let pieces = fitted(&baked, &rest)
        .into_iter()
        .map(|(bone, tris)| {
            let positions: Vec<[f32; 3]> = tris.iter().flatten().map(|c| c.0.to_array()).collect();
            let normals: Vec<[f32; 3]> = tris.iter().flatten().map(|c| c.1.to_array()).collect();
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions).with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            (bone, meshes.add(mesh))
        })
        .collect();
    Kit { plan, pieces }
}

/// The body on `hunter`, each piece on its bone.
pub(super) fn spawn(commands: &mut Commands, kit: &Kit, material: &Handle<StandardMaterial>, hunter: Entity) {
    for (bone, mesh) in &kit.pieces {
        commands.spawn((
            Part { hunter, bone: *bone, along: 0.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::default(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Intent;

    /// Each piece, moved onto its bone as the rig stands, lies along it (a
    /// piece fitted wrong, or to another bone, lies far off it).
    #[test]
    fn every_bone_dressed() {
        let baked = baked();
        let rest = Rig::new(plan(&baked), Vec3::ZERO, 0.0, 0, &|_, _| 0.0).bones;
        for (bone, tris) in fitted(&baked, &rest) {
            assert!(!tris.is_empty(), "bone {bone} has no piece");
            let middle = tris.iter().flat_map(|t| t.iter().map(|c| c.0)).sum::<Vec3>() / (tris.len() * 3) as f32;
            let length = rest[bone].a.distance(rest[bone].b);
            assert!(middle.x.abs() < 0.6 && middle.z.abs() < 1.2 && (-0.5..length + 0.5).contains(&middle.y), "bone {bone}'s piece lies off it, its middle at {middle}");
        }
    }

    /// How far off where it is planted a foot gets at worst, at `speed`.
    fn worst(plan: &Plan, speed: f32) -> f32 {
        let mut rig = Rig::new(plan.clone(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
        let mut worst = 0.0f32;
        for frame in 0..600 {
            let intent = Intent { velocity: Vec3::Z * speed, look: rig.root + Vec3::new(0.0, 1.5, 20.0), ..Default::default() };
            rig.update(1.0 / 60.0, intent, &|_, _| 0.0, &|from, step| from + step);
            if frame < 120 {
                continue;
            }
            // A foot down: its leg's end where it is planted (bounding, not in
            // the air).
            for (i, foot) in rig.feet.iter().enumerate().filter(|(_, f)| !f.swinging && rig.airborne.is_none()) {
                worst = worst.max(rig.bones[6 + 2 * i].b.distance(foot.planted));
            }
        }
        worst
    }

    /// Its feet reach the ground standing, walking and running (its legs
    /// are the mesh's, shorter than the harrow's its gait was made for), as
    /// the harrow's do (hauled past its forefeet at a run, they hung in the
    /// air behind it).
    #[test]
    fn feet_reach_the_ground() {
        for (name, plan) in [("the shard beast", plan(&baked())), ("the harrow", Plan::beast())] {
            for speed in [0.0, 1.0, 2.0, 4.0, 6.0, 10.0] {
                let worst = worst(&plan, speed);
                assert!(worst < 0.05, "{name} at {speed} m/s: a planted foot is {worst:.2} m off the ground");
            }
        }
    }

    /// It moves as in play, starting, stopping, turning hard, stalking
    /// crouched, charging, without its legs whipping round in a frame, a foot
    /// off the ground, or a foot left in the air (all three seen in play,
    /// the user: its legs glitched, it floated, it stretched its hind legs
    /// out behind it).
    #[test]
    fn moves_in_one_piece() {
        type Drive = fn(f32) -> (Vec3, f32);
        let drives: [(&str, Drive); 5] = [
            ("charging", |_| (Vec3::Z * 18.0, 0.1)),
            ("starting and stopping", |t| (if t % 4.0 < 2.0 { Vec3::Z * 12.0 } else { Vec3::ZERO }, 0.0)),
            ("turning hard", |t| (Quat::from_rotation_y((t / 1.5).floor() * 1.57) * Vec3::Z * 6.0, 0.0)),
            ("circling", |t| (Quat::from_rotation_y(t * 0.5) * Vec3::Z * 6.0, 0.0)),
            ("stalking crouched", |t| (if t % 3.0 < 1.5 { Vec3::Z * 3.0 } else { Vec3::ZERO }, 0.5)),
        ];
        let plan = plan(&baked());
        for (name, drive) in drives {
            let mut rig = Rig::new(plan.clone(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
            let mut last: Vec<Vec3> = vec![];
            let mut aloft = [0; 4];
            for frame in 0..900 {
                let (velocity, crouch) = drive(frame as f32 / 60.0);
                let intent = Intent { velocity, crouch, look: rig.root + velocity.normalize_or(Vec3::Z) * 20.0 + Vec3::Y * 1.5, ..Default::default() };
                rig.update(1.0 / 60.0, intent, &|_, _| 0.0, &|from, step| from + step);
                let now: Vec<Vec3> = rig.bones.iter().map(|b| (b.b - b.a).normalize()).collect();
                if frame >= 60 {
                    // (Flat out, a planted leg sweeps past as fast as the
                    // body goes over it: at 18 m/s, 0.3 m a frame, up to 37
                    // degrees; a whip is beyond what its pace sweeps.)
                    let limit = 35.0 * (rig.speed() / 10.0).max(1.0);
                    for bone in 5..13 {
                        let turn = last[bone].angle_between(now[bone]).to_degrees();
                        assert!(turn < limit, "{name}: bone {bone} turned {turn:.0} degrees in a frame");
                    }
                    for (i, foot) in rig.feet.iter().enumerate() {
                        aloft[i] = if foot.swinging { aloft[i] + 1 } else { 0 };
                        assert!(aloft[i] < 60, "{name}: foot {i} a second in the air");
                        let off = if foot.swinging || rig.airborne.is_some() { 0.0 } else { rig.bones[6 + 2 * i].b.distance(foot.planted) };
                        assert!(off < 0.06, "{name}: foot {i} planted {off:.2} m off the ground");
                    }
                }
                last = now;
            }
        }
    }
    /// It runs: as fast as a hunter asks, on long strides, never hopping
    /// (bounding instead, it hopped like a frog; run as a walk sped up, its
    /// legs flickered, the user), its feet on the ground when planted.
    #[test]
    fn runs() {
        let plan = plan(&baked());
        for speed in [4.0, 8.0, 12.0, 18.0] {
            let mut rig = Rig::new(plan.clone(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
            let (mut from, mut lifts, mut was) = (Vec3::ZERO, 0, [false; 4]);
            for frame in 0..900 {
                let intent = Intent { velocity: Vec3::Z * speed, look: rig.root + Vec3::new(0.0, 1.5, 20.0), ..Default::default() };
                rig.update(1.0 / 60.0, intent, &|_, _| 0.0, &|from, step| from + step);
                assert!(rig.airborne.is_none(), "at {speed} m/s it left the ground");
                if frame == 300 {
                    from = rig.root;
                }
                if frame >= 300 {
                    for i in 0..4 {
                        lifts += (rig.feet[i].swinging && !was[i]) as u32;
                        let off = if rig.feet[i].swinging { 0.0 } else { rig.bones[6 + 2 * i].b.distance(rig.feet[i].planted) };
                        assert!(off < 0.05, "at {speed} m/s foot {i} planted {off:.2} m off the ground");
                    }
                }
                was = std::array::from_fn(|i| rig.feet[i].swinging);
            }
            let went = (rig.root - from).length() / 10.0;
            let cadence = lifts as f32 / 4.0 / 10.0;
            assert!(went > speed * 0.95, "asked {speed} m/s, it went {went:.1}");
            assert!(cadence < 2.8, "at {speed} m/s, {cadence:.2} strides a second");
            // (Its legs straight as often as they are: they bear it all but
            // straight, as a leopard's, reaching far ahead to pull the ground
            // back, the user; stuck straight out behind, out of reach, its
            // foot in the air, as they once were, is caught above.)
        }
    }

    /// Its body carried smoothly, at the game's frame rate, in the world:
    /// rising and falling with its steps, never kinked in a frame, on ground
    /// that wavers by 2 cm from spot to spot as a built floor's does when
    /// felt (eased after targets that jumped as each foot came down, it
    /// jerked; set to the ground's height each frame, it shook with it: the
    /// user's vibration).
    #[test]
    fn body_without_kinks() {
        let plan = plan(&baked());
        let ground = |x: f32, z: f32| ((x * 12.9898 + z * 78.233).sin() * 43758.547).fract().abs() * 0.02;
        for speed in [0.7, 1.5, 4.0, 10.0] {
            let mut rig = Rig::new(plan.clone(), Vec3::ZERO, 0.0, 5, &ground);
            let mut ends: Vec<[Vec3; 2]> = vec![];
            for frame in 0..1440 {
                let intent = Intent { velocity: Vec3::Z * speed, look: rig.root + Vec3::new(0.0, 1.5, 20.0), ..Default::default() };
                rig.update(1.0 / 144.0, intent, &ground, &|from, step| from + step);
                if frame >= 288 {
                    ends.push([rig.bones[0].a, rig.bones[2].b]);
                }
            }
            for end in 0..2 {
                let kink = ends.windows(3).map(|w| (w[0][end] - 2.0 * w[1][end] + w[2][end]).length()).fold(0.0f32, f32::max);
                assert!(kink < 0.002, "at {speed} m/s its {} kinked {:.1} mm in a frame", ["hips", "chest"][end], kink * 1000.0);
            }
        }
    }

    /// Gathering itself to pounce and settling after, it starts hard and
    /// stops, and each foot steps once in its turn, a whole step (its turn
    /// to step growing as it sped up, a foot just down stepped again and
    /// again, slivers of steps: its legs bobbed at 7 Hz for half a second
    /// before and after each attack, the user's vibration).
    #[test]
    fn starts_and_stops_stepping_once() {
        let plan = plan(&baked());
        let mut rig = Rig::new(plan.clone(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
        let (mut lifted, mut was) = ([f32::MIN; 4], [false; 4]);
        for frame in 0..144 * 8 {
            let t = frame as f32 / 144.0;
            // Standing a second, then asked for 18 m/s for half a second.
            let velocity = if t % 1.5 > 1.0 { Vec3::Z * 18.0 } else { Vec3::ZERO };
            let intent = Intent { velocity, crouch: if velocity == Vec3::ZERO { 0.4 } else { 1.0 }, look: rig.root + Vec3::new(0.0, 1.5, 20.0), ..Default::default() };
            rig.update(1.0 / 144.0, intent, &|_, _| 0.0, &|from, step| from + step);
            for i in 0..4 {
                let swinging = rig.feet[i].swinging;
                if swinging && !was[i] {
                    assert!(t - lifted[i] > 0.15, "foot {i} lifted again {:.3} s after it last did, at {t:.2} s", t - lifted[i]);
                    lifted[i] = t;
                }
                if !swinging && was[i] {
                    assert!(t - lifted[i] > 0.05, "foot {i} swung for only {:.3} s, at {t:.2} s", t - lifted[i]);
                }
                was[i] = swinging;
            }
        }
    }
}
