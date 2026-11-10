//! The impaled brute's thrust as beams (`field.rs`): its own material
//! (`plume.wgsl`), white-hot at its source, fading to black, in curves or in
//! shards. The stalker's dark beam, which it was first, only darkens what is
//! behind it: the beams had no whites, and did not stand out from the
//! brute's black body (the user, 2026-11-10).

use bevy::{prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::combat) struct Plume {
    /// x: how much of it there is (0..1), y: how bright its streaks, z:
    /// its `Look`, w: from how far down it
    /// fades over the ground (of the way).
    #[uniform(0)]
    look: Vec4,
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
    /// In `look`, fading over the ground from `flare` of the way down.
    pub(in crate::combat) fn new(look: Look, flare: f32) -> Self {
        Self { look: Vec4::new(0.0, 0.0, look as u8 as f32, flare) }
    }

    /// How much of it there is now (0..1), and how bright its streaks.
    pub(in crate::combat) fn set(&mut self, on: f32, white: f32) {
        self.look.x = on;
        self.look.y = white;
    }
}
