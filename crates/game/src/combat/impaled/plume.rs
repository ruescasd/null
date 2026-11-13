//! The impaled brute's thrust as beams (`field.rs`): its own material
//! (`plume.wgsl`), white-hot at its source, fading to black, in curves or in
//! shards. The stalker's dark beam, which it was first, only darkens what is
//! behind it: the beams had no whites, and did not stand out from the
//! brute's black body (the user, 2026-11-10).

use bevy::{prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::combat) struct Plume {
    /// x: how bright its streaks, z: its `Look`, w: from how far down it
    /// flares and fades over the ground (of the way). How much of it there
    /// is, each beam's own, comes in its `MeshTag` (see `plume.wgsl`).
    #[uniform(0)]
    look: Vec4,
    /// How wide at its source and at the ground (m), how much wider it
    /// flares, how far down it grows to its width (m).
    #[uniform(1)]
    shape: Vec4,
}
impl Material for Plume {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/combat/impaled/plume.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/combat/impaled/plume.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// White-hot at its source, fading to black outwards and downwards, bent
/// all along, in curves or in hard straight-edged shards (`plume=1|2`; the
/// user, 2026-11-10: kept both).
#[derive(Clone, Copy)]
pub(in crate::combat) enum Look {
    Curves = 1,
    Shards = 2,
}

impl Plume {
    /// In `look`: streaks `white` bright; `wide` at its source and at the
    /// ground (m), flaring `flare.1` times wider from `flare.0` of the way
    /// down, fading there; grown to its width over `grow` m.
    pub(in crate::combat) fn new(look: Look, white: f32, wide: (f32, f32), flare: (f32, f32), grow: f32) -> Self {
        Self { look: Vec4::new(white, 0.0, look as u8 as f32, flare.0), shape: Vec4::new(wide.0, wide.1, flare.1, grow) }
    }
}
