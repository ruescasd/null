//! Tubes drawn on the GPU from their points: its cables, flung and dropped
//! (the user, 2026-11-12: one brute's attack took a fifth of the frame; its
//! cables' meshes were made anew each frame, and the renderer took each in
//! anew). A set of tubes is one mesh made once, its rings and their sides
//! (its points only which ring, which side), and a buffer of points, which
//! is all that changes: `tube.wgsl` lays each ring round its point.
//!
//! The buffer holds two vectors a point: where it is and how thick (m), and
//! which way its ring's first side faces (carried along its strand, so the
//! tube does not twist). Each strand is closed at both ends by a point of
//! no thickness where its end is: the two between strands lie flat, nothing
//! drawn. The buffer and the mesh keep their size (so the buffer is written
//! in place and nothing else is made again); they grow (to the next power
//! of two) only when its strands need more.

use bevy::{
    mesh::Indices,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
};

/// A ring's sides (as `crate::rope`'s); the fewest points a set of tubes
/// has room for.
const SIDES: usize = 5;
const LEAST: usize = 256;

pub(in crate::combat) type TubeMaterial = ExtendedMaterial<StandardMaterial, Tube>;

/// Its points (see the module).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(in crate::combat) struct Tube {
    #[storage(100, read_only)]
    points: Handle<ShaderBuffer>,
}

impl MaterialExtension for Tube {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/combat/impaled/tube.wgsl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://game/combat/impaled/tube.wgsl".into()
    }
}

/// A set of tubes: its points' buffer, its material (bound to a new buffer
/// when it grows), and how many points it has room for.
#[derive(Component)]
pub(in crate::combat) struct Tubes {
    points: Handle<ShaderBuffer>,
    material: Handle<TubeMaterial>,
    room: usize,
}

/// What a set of tubes is drawn with: its assets.
#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::combat) struct Kit<'w> {
    pub meshes: ResMut<'w, Assets<Mesh>>,
    buffers: ResMut<'w, Assets<ShaderBuffer>>,
    materials: ResMut<'w, Assets<TubeMaterial>>,
}

/// A set of tubes made, empty, as `look` (its colour, how it shines),
/// `strands` in it to start with: what to spawn. (Not culled by its mesh's
/// bounds: its points are not where it is.)
pub(in crate::combat) fn make<'a>(kit: &mut Kit, look: StandardMaterial, strands: impl IntoIterator<Item = (&'a [Vec3], f32)>) -> impl Bundle {
    let data = packed(strands);
    let room = (data.len() / 2).next_power_of_two().max(LEAST);
    let points = kit.buffers.add(ShaderBuffer::from(filled(data, room)));
    let material = kit.materials.add(ExtendedMaterial { base: look, extension: Tube { points: points.clone() } });
    (Tubes { points, material: material.clone(), room }, Mesh3d(kit.meshes.add(rings(room))), MeshMaterial3d(material), bevy::camera::visibility::NoFrustumCulling)
}

/// Its strands now (each its points and thickness, m).
pub(in crate::combat) fn write<'a>(kit: &mut Kit, tubes: &mut Tubes, mesh: &mut Mesh3d, strands: impl IntoIterator<Item = (&'a [Vec3], f32)>) {
    let data = packed(strands);
    if data.len() / 2 > tubes.room {
        // (A new buffer, bigger, and its material bound to it: not the old
        // one made bigger, which its material may be bound to again before
        // it is, the renderer not ordering the two.)
        tubes.room = (data.len() / 2).next_power_of_two();
        *mesh = Mesh3d(kit.meshes.add(rings(tubes.room)));
        tubes.points = kit.buffers.add(ShaderBuffer::from(filled(data, tubes.room)));
        if let Some(mut material) = kit.materials.get_mut(&tubes.material) {
            material.extension.points = tubes.points.clone();
        }
        return;
    }
    if let Some(mut buffer) = kit.buffers.get_mut(&tubes.points) {
        buffer.set_data(filled(data, tubes.room));
    }
}

/// Points as the buffer has them (see the module).
fn packed<'a>(strands: impl IntoIterator<Item = (&'a [Vec3], f32)>) -> Vec<Vec4> {
    let mut data = Vec::new();
    for (points, thick) in strands {
        let n = points.len();
        if n < 2 {
            continue;
        }
        let mut side = (points[1] - points[0]).normalize_or(Vec3::Y).any_orthonormal_vector();
        data.extend([points[0].extend(0.0), side.extend(0.0)]);
        for i in 0..n {
            let tangent = (points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)]).normalize_or(Vec3::Y);
            side = (side - tangent * side.dot(tangent)).normalize_or(tangent.any_orthonormal_vector());
            data.extend([points[i].extend(thick), side.extend(0.0)]);
        }
        data.extend([points[n - 1].extend(0.0), side.extend(0.0)]);
    }
    data
}

/// The points, and nothing in the rest of the room.
fn filled(mut data: Vec<Vec4>, room: usize) -> Vec<Vec4> {
    data.resize(room * 2, Vec4::ZERO);
    data
}

/// `room` rings of `SIDES` sides, each ring joined to the next; each corner
/// at (its ring, its side).
fn rings(room: usize) -> Mesh {
    let positions: Vec<[f32; 3]> = (0..room).flat_map(|i| (0..SIDES).map(move |k| [i as f32, k as f32, 0.0])).collect();
    let mut indices = Vec::with_capacity((room - 1) * SIDES * 6);
    for i in 0..room as u32 - 1 {
        for k in 0..SIDES as u32 {
            let (a, b) = (i * SIDES as u32 + k, i * SIDES as u32 + (k + 1) % SIDES as u32);
            let (c, d) = (a + SIDES as u32, b + SIDES as u32);
            // (Wound to face outwards, as `crate::rope`'s.)
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions).with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strands_closed_and_apart() {
        let a = [Vec3::ZERO, Vec3::X, Vec3::new(2.0, 1.0, 0.0)];
        let b = [Vec3::Z, Vec3::new(0.0, 0.0, 3.0)];
        let one: &[Vec3] = &[Vec3::Y];
        let data = packed([(&a[..], 0.1), (one, 0.2), (&b[..], 0.3)]);
        // (Each strand its points and an end each side; one point no strand.)
        assert_eq!(data.len(), 2 * (a.len() + 2 + b.len() + 2));
        let thick: Vec<f32> = data.iter().step_by(2).map(|p| p.w).collect();
        assert_eq!(thick, [0.0, 0.1, 0.1, 0.1, 0.0, 0.0, 0.3, 0.3, 0.0]);
        // (Its ends where its ends are.)
        assert_eq!(data[0].truncate(), a[0]);
        assert_eq!(data[8].truncate(), a[2]);
        assert_eq!(data[10].truncate(), b[0]);
        // (Each ring's first side across its strand.)
        for (i, p) in data.iter().step_by(2).enumerate().skip(1).take(3) {
            let along = a[(i).min(2)] - a[(i - 1).saturating_sub(1)];
            assert!(data[2 * i + 1].truncate().dot(along.normalize()).abs() < 1e-4, "{p}");
        }
    }

    #[test]
    fn room_rings() {
        let mesh = rings(4);
        assert_eq!(mesh.count_vertices(), 4 * SIDES);
        assert_eq!(mesh.indices().map(|i| i.len()), Some(3 * SIDES * 6));
    }
}
