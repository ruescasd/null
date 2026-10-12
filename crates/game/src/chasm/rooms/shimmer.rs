//! A shimmer behind a wall (the user's intrusion: a niche upside down, and
//! from behind its back something trying to break through): on the niche's
//! back, two faint layers of fine glowing veins, laid over the stone, not
//! instead of it, each drifting its own way, so where they cross they
//! brighten and fade; their light wavers, unevenly. Only the niche shows
//! it: past its outline the layers are buried in the wall.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    math::Affine2,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use worldgen::noise::perlin2;

/// Pixels on a side of the veins' tile, and metres it covers.
/// (Larger than any niche: a tile repeated across one showed.)
const TILE: u32 = 512;
const TILE_M: f32 = 4.0;
/// How bright the veins are at most (the lab's lamps' boxes are 60).
const GLOW: f32 = 0.1;

/// A shimmer on a wall, in metres from the room's corner: its middle (just
/// off the wall), which way it faces, its size, its seed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shimmer {
    pub at: Vec3,
    pub facing: Vec3,
    pub size: Vec2,
    pub seed: u32,
}

/// A layer of a shimmer: its material, its size (for its veins' scale),
/// which way and how fast it drifts, and a phase for its wavering.
#[derive(Component)]
pub(crate) struct Layer {
    material: Handle<StandardMaterial>,
    size: Vec2,
    drift: Vec2,
    phase: f32,
}

pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, shimmers: &[Shimmer]) {
    for s in shimmers {
        let quad = meshes.add(Rectangle::new(s.size.x, s.size.y));
        let turn = Quat::from_rotation_y(s.facing.x.atan2(s.facing.z));
        for (k, drift) in [(0u32, Vec2::new(0.031, 0.017)), (1, Vec2::new(-0.023, 0.029))] {
            let material = materials.add(StandardMaterial {
                base_color: Color::BLACK,
                base_color_texture: Some(images.add(veins(s.seed * 2 + k))),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            });
            // (The second a hair before the first.)
            let at = s.at + s.facing * 0.005 * k as f32;
            commands.spawn((
                Layer { material: material.clone(), size: s.size, drift, phase: (s.seed % 97) as f32 + k as f32 * 3.7 },
                Mesh3d(quad.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(at).with_rotation(turn),
                bevy::light::NotShadowCaster,
            ));
        }
    }
}

/// The layers drift and waver.
pub(crate) fn animate(time: Res<Time>, layers: Query<&Layer>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let t = time.elapsed_secs();
    for l in &layers {
        let Some(mut m) = materials.get_mut(&l.material) else { continue };
        let p = l.phase;
        // (Slow swells, and now and then a quick shiver.)
        let swell = 0.55 + 0.45 * ((t * 0.37 + p).sin() * 0.6 + (t * 0.113 + p * 1.3).sin() * 0.4);
        let shiver = 1.0 + 0.25 * (t * 7.3 + p).sin() * (t * 0.21 + p).sin().max(0.0).powi(4);
        m.base_color = Color::LinearRgba(LinearRgba::gray(GLOW * swell * shiver));
        let scale = l.size / TILE_M;
        m.uv_transform = Affine2::from_scale_angle_translation(scale, 0.0, l.drift * t);
    }
}

/// A tile of fine veins: ridged noise, each octave's ridges thinned to
/// lines, summed, over a bent plane; tiling, so it can drift without a
/// seam.
fn veins(seed: u32) -> Image {
    let n = TILE as usize;
    let mut data = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / TILE as f32, y as f32 / TILE as f32);
            // (Bent by a coarser noise first: unbent, the ridges ran
            // straight along the lattice's cells here and there, lines
            // ruled across the wall.)
            let (u, v) = (u + 0.06 * perlin2(u * 3.0, v * 3.0, 3, seed ^ 0x5a5a), v + 0.06 * perlin2(u * 3.0, v * 3.0, 3, seed ^ 0xa5a5));
            let mut sum = 0.0;
            for (o, w) in [(0, 0.7f32), (1, 0.35), (2, 0.15)] {
                let period = 4 << o;
                let r = 1.0 - perlin2(u * period as f32, v * period as f32, period, seed.wrapping_add(o as u32 * 7919)).abs();
                sum += w * r.clamp(0.0, 1.0).powi(28);
            }
            let g = (sum.min(1.0) * 255.0) as u8;
            data.extend_from_slice(&[g, g, g, 255]);
        }
    }
    let mut image = Image::new(Extent3d { width: TILE, height: TILE, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}
