//! The bent air round the brute's beams (`field.rs`): what is behind it seen
//! bent, from the screen as drawn before it, by soft lumps running down it;
//! the bending fading out towards its outline and its ends, as heat haze
//! has no edge (the user, 2026-11-12: its borders showed, standing still).
//! Each column's own scroll and strength come in its `MeshTag` (see
//! `haze.wgsl`), so nothing is changed in it each frame (the user,
//! 2026-11-12: changed each frame, a brute cost the renderer dearly).

use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::combat) struct Haze {
    /// A tiling normal map of soft lumps.
    #[texture(0)]
    #[sampler(1)]
    pub lumps: Handle<Image>,
}

impl Material for Haze {
    fn fragment_shader() -> ShaderRef {
        "embedded://game/combat/impaled/haze.wgsl".into()
    }

    fn reads_view_transmission_texture(&self) -> bool {
        true
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    // (Only its far side: its near side, written into depth, hid the beam's
    // start inside it. Its tag read in the fragment.)
    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Front);
        descriptor.vertex.shader_defs.push("VERTEX_OUTPUT_INSTANCE_INDEX".into());
        if let Some(f) = descriptor.fragment.as_mut() {
            f.shader_defs.push("VERTEX_OUTPUT_INSTANCE_INDEX".into());
        }
        Ok(())
    }
}

/// A column's tag: how far its lumps have run down (of their tile, 0..1),
/// and how hard it strains (0..1.6).
pub(in crate::combat) fn tag(ran: f32, strain: f32) -> u32 {
    let ran = (ran.rem_euclid(1.0) * 65535.0) as u32 & 0xffff;
    let strain = ((strain.clamp(0.0, 1.6) * 1000.0) as u32) << 16;
    ran | strain
}
