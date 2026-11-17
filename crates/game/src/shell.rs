//! PROBE (the user, 2026-11-17): the world a shell, its floor the top of
//! vaults on pillars (worldgen's `Vaults`, built in the terrain's columns),
//! seen where the voids cut it. Below the deck, nothing is lit but through
//! its holes: the shell's mask, read by terrain.wgsl's `shell_light`.

use bevy::{asset::RenderAssetUsages, image::Image, prelude::*};

use crate::terrain::WorldGen;

pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        // (Open everywhere until made: no shell's shadow.)
        let open = Image::new_fill(
            bevy::render::render_resource::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            bevy::render::render_resource::TextureDimension::D2,
            &[255],
            bevy::render::render_resource::TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        app.world_mut().resource_mut::<Assets<Image>>().insert(&SHELL, open).expect("the shell's mask");
        app.add_systems(PostStartup, shell);
    }
}

/// PROBE: the shell's mask, the whole world 16 m a texel, open (1) where
/// voids are, solid (0) elsewhere: a point below the deck is lit by a sun
/// only if its way to it reaches the deck's level over a void
/// (terrain.wgsl, `shell_light`). (Pillars deep under the land were lit as
/// if the world were not above them: the far maps reach only the
/// terrain's own heights, the user's "sun underneath".)
pub const SHELL: Handle<Image> = bevy::asset::uuid_handle!("6b0e8f8e-3f7c-4f51-9a34-2f7d1f0c5a05");

/// The shell's mask's texels on a side.
const SHELL_SIZE: u32 = 1024;

fn shell(world: Res<WorldGen>, mut images: ResMut<Assets<Image>>) {
    if world.0.vaults.is_none() {
        return;
    }
    let size = world.0.size();
    let step = size / SHELL_SIZE as f32;
    let n = SHELL_SIZE as usize;
    let mut data = vec![0u8; n * n];
    let threads = std::thread::available_parallelism().map_or(8, |t| t.get());
    std::thread::scope(|scope| {
        for (band, rows) in data.chunks_mut(n * n.div_ceil(threads)).enumerate() {
            let world = &world.0;
            scope.spawn(move || {
                let first = band * n.div_ceil(threads);
                for (i, texel) in rows.iter_mut().enumerate() {
                    let (x, z) = ((i % n) as f32 + 0.5, ((first + i / n) as f32) + 0.5);
                    *texel = if world.is_void(x * step, z * step) { 255 } else { 0 };
                }
            });
        }
    });
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d { width: SHELL_SIZE, height: SHELL_SIZE, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::linear();
    images.insert(&SHELL, image).expect("the shell's mask");
}
