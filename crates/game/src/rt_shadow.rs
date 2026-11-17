//! SPIKE (the user, 2026-11-17, `--opt rt`): the suns' shadows on the
//! terrain traced by the GPU's ray queries instead of sampled from shadow
//! maps, to see whether it runs, what it costs and whether it is right
//! with no tuning (the shadow maps' problems, ~18 of them, came from
//! sampling, tolerances and the seams between their systems).
//!
//! Each terrain column drawn (but one fading out) has an acceleration
//! structure built from its mesh where Bevy keeps it (the terrain's vertex
//! layout is position, normal, a texture coordinate and colour: 48 bytes,
//! position first), once, as it comes; one scene structure over them all is
//! built every frame. Between the depth prepass and the main pass, a
//! compute pass traces a ray from each pixel's point towards each sun and
//! writes lit or not into `RT_MASK`, which the terrain's shader reads in
//! place of its far maps (terrain.wgsl). The terrain casts no shadow-map
//! shadows; the structures still do.

use std::{collections::HashMap, sync::Arc};

use bevy::{
    asset::{RenderAssetUsages, embedded_asset, uuid_handle},
    core_pipeline::{
        Core3d, Core3dSystems,
        core_3d::main_opaque_pass_3d,
        prepass::ViewPrepassTextures,
    },
    image::Image,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        mesh::{
            RenderMesh,
            allocator::{MeshAllocator, MeshAllocatorSettings, allocate_and_free_meshes},
        },
        render_asset::{ExtractedAssets, RenderAssets, prepare_assets},
        render_resource::{
            AccelerationStructureFlags, AccelerationStructureGeometryFlags, AccelerationStructureUpdateMode, BindGroupEntries,
            BindGroupLayoutDescriptor, BindGroupLayoutEntries, Blas, BlasBuildEntry, BlasGeometries, BlasGeometrySizeDescriptors,
            BlasTriangleGeometry, BlasTriangleGeometrySizeDescriptor, BufferInitDescriptor, BufferUsages, CachedComputePipelineId,
            Buffer, BufferDescriptor, CommandEncoderDescriptor, ComputePassDescriptor, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, ComputePipelineDescriptor, CreateBlasDescriptor, CreateTlasDescriptor,
            Extent3d, IndexFormat, PipelineCache, ShaderStages, StorageTextureAccess, TextureDimension, TextureFormat, TextureUsages,
            Tlas, TlasInstance, VertexFormat, binding_types,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        texture::GpuImage,
        view::{ViewUniform, ViewUniformOffset, ViewUniforms},
    },
    window::PrimaryWindow,
};

use crate::{
    Args,
    look::{Sky, sun_direction},
    terrain::{ColumnBlocks, PLANET_RADIUS, TerrainColumn},
};

/// What the terrain's shader reads: lit (1) or not by each sun, red and
/// green, a texel a pixel.
pub const RT_MASK: Handle<Image> = uuid_handle!("6b0e8f8e-3f7c-4f51-9a34-2f7d1f0c5a06");

/// The traced scene's origin is kept on a grid this coarse (m) near the
/// camera: in single precision, 16 km out, a point is only good to 2 mm,
/// a few kilometres out to a quarter of one (see rt_shadow.wgsl, `LIFT`).
const SHIFT: f32 = 256.0;

/// The mask's size (see `RtShadowPlugin`): the largest window traced.
const MASK: (u32, u32) = (3840, 2160);

/// Whether the suns' shadows on the terrain are traced (`--opt rt`).
pub fn enabled(args: &Args) -> bool {
    args.opt("rt")
}

/// A piece of the traced scene: a column's block (see worldgen's
/// `ColumnMesh::blocks`), its indices, where its column is, its level.
#[derive(Clone, Copy, PartialEq)]
struct Piece {
    mesh: AssetId<Mesh>,
    block: u8,
    first: u32,
    count: u32,
    at: Vec3,
    level: u32,
}

#[derive(Resource, Clone, Default, ExtractResource)]
struct RtFrame {
    on: bool,
    /// What is traced (see `collect`).
    columns: Arc<Vec<Piece>>,
    /// DEBUG (`--set rt_px`, `rt_py`): the pixel probed, on (z 1).
    probe: Vec4,
    /// SPIKE (`--set rt_keep 1`): the scene's structure built again only
    /// when what it holds changes; (`--opt rtlog`) its costs logged.
    keep: bool,
    log: bool,
    /// Where the traced scene's origin is in the world (see `SHIFT`).
    shift: Vec3,
    /// Towards each sun, and whether it is up.
    suns: [Vec4; 2],
    bend: f32,
}

pub struct RtShadowPlugin;

impl Plugin for RtShadowPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "rt_shadow.wgsl");
        // (One mask, never made again: as large as any window. Made again
        // as the window went full screen, the terrain's material went on
        // reading the first, frozen, the user's overlay with the gun cut
        // out of it; rebinding the materials did not take. 33 MB.)
        let mut mask = Image::new_fill(Extent3d { width: MASK.0, height: MASK.1, depth_or_array_layers: 1 }, TextureDimension::D2, &[255; 4], TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
        mask.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST;
        app.world_mut().resource_mut::<Assets<Image>>().insert(&RT_MASK, mask).expect("the traced shadows' mask");
        app.init_resource::<RtFrame>().add_plugins(ExtractResourcePlugin::<RtFrame>::default());
        if enabled(app.world().resource::<Args>()) {
            app.add_systems(PostUpdate, collect);
        }
    }

    fn finish(&self, app: &mut App) {
        if !enabled(app.world().resource::<Args>()) {
            return;
        }
        let Some(render) = app.get_sub_app_mut(RenderApp) else { return };
        let features = render.world().resource::<RenderDevice>().features();
        assert!(
            features.contains(bevy::render::settings::WgpuFeatures::EXPERIMENTAL_RAY_QUERY),
            "--opt rt: this GPU or its driver offers no ray queries (wgpu's EXPERIMENTAL_RAY_QUERY)"
        );
        render.world_mut().resource_mut::<MeshAllocatorSettings>().extra_buffer_usages |= BufferUsages::BLAS_INPUT;
        render
            .init_resource::<RtScene>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareAssets).after(allocate_and_free_meshes).before(prepare_assets::<RenderMesh>))
            .add_systems(Render, read_probe.in_set(RenderSystems::Cleanup))
            .add_systems(Core3d, trace.before(main_opaque_pass_3d).in_set(Core3dSystems::MainPass));
    }
}

/// What is traced, the suns, and the mask the size of the window. The
/// scene is one surface, each place once: a coarser column's blocks (see
/// worldgen's `ColumnMesh::blocks`) each left out where a finer column
/// drawn covers it, whatever the finer columns round it (left out whole
/// as any finer one came, its other places were in no scene while drawn,
/// their shadows gone till the rest came; kept whole, it lay buried under
/// the finer one, shading it from beneath).
#[allow(clippy::too_many_arguments)]
fn collect(
    sky: Res<Sky>,
    args: Res<Args>,
    columns: Query<(&Mesh3d, &GlobalTransform, &ColumnBlocks, &MeshMaterial3d<crate::terrain::TerrainMaterial>), With<TerrainColumn>>,
    levels: Res<crate::terrain::TerrainMaterialHandle>,
    window: Single<&Window, With<PrimaryWindow>>,
    anchor: Single<&Transform, With<crate::terrain::StreamAnchor>>,
    mut frame: ResMut<RtFrame>,
) {
    let lod = |m: &MeshMaterial3d<crate::terrain::TerrainMaterial>| levels.0.iter().position(|h| h.id() == m.id()).unwrap_or(0) as u32;
    // (Columns going are traced till gone: the finest version drawn is
    // traced, arriving or leaving, so both halves of a cross-fade take
    // their shadows from one surface, the coarser's pixels lifted onto it
    // (rt_shadow.wgsl). Left out as it began to go, the finer ground went
    // on being seen for the fade under the coarser's shadows, which came
    // and went: blinking, the user, gone with `--set lod_fade 0`.)
    let drawn: Vec<(AssetId<Mesh>, Vec3, u32, &ColumnBlocks)> = columns.iter().map(|c| (c.0.id(), c.1.translation(), lod(c.3), c.2)).collect();
    let finer_in = |level: u32, at: Vec2, size: f32| drawn.iter().any(|&(_, t, l, _)| l < level && t.x >= at.x - 0.5 && t.x < at.x + size - 0.5 && t.z >= at.y - 0.5 && t.z < at.y + size - 0.5);
    let mut list = Vec::new();
    for &(id, at, level, blocks) in &drawn {
        if level == 0 {
            if let Some(&(first, count)) = blocks.0.first() {
                list.push(Piece { mesh: id, block: 0, first, count, at, level });
            }
            continue;
        }
        let child = worldgen::column_size(level - 1);
        for (b, &(first, count)) in blocks.0.iter().enumerate() {
            let corner = at.xz() + Vec2::new((b % 4) as f32, (b / 4) as f32) * child;
            if count > 0 && !finer_in(level, corner, child) {
                list.push(Piece { mesh: id, block: b as u8, first, count, at, level });
            }
        }
    }
    // (`--set rt_only N`: only sun N traced, the other lit everywhere.)
    let only = args.num("rt_only", -1.0);
    let sun = |i: usize| {
        let d = sun_direction(i, sky.time);
        d.extend(if d.y > 0.0 && (only < 0.0 || only as usize == i) { 1.0 } else { 0.0 })
    };
    let probe = Vec4::new(args.num("rt_px", 0.0), args.num("rt_py", 0.0), if args.num("rt_px", -1.0) >= 0.0 { 1.0 } else { 0.0 }, 0.0);
    let shift = (anchor.translation / SHIFT).round() * SHIFT;
    *frame = RtFrame { on: true, columns: Arc::new(list), suns: [sun(0), sun(1)], bend: if args.opt("flat") { 0.0 } else { 0.5 / PLANET_RADIUS }, probe, shift, keep: args.num("rt_keep", 0.0) > 0.0, log: args.opt("rtlog") };
    assert!(
        window.physical_width() <= MASK.0 && window.physical_height() <= MASK.1,
        "--opt rt: a window of {}x{}, larger than the traced shadows' mask ({}x{}, rt_shadow.rs `MASK`)",
        window.physical_width(),
        window.physical_height(),
        MASK.0,
        MASK.1
    );
}

#[derive(Resource, Default)]
struct RtScene {
    blas: HashMap<(AssetId<Mesh>, u8), Blas>,
    tlas: Option<Tlas>,
    /// Each instance in the scene: where, its level.
    instances: Vec<(Vec3, u32)>,
    /// What the scene's structure was last built over (see `RtFrame::keep`).
    held: Vec<Piece>,
    /// DEBUG: the probe's results, on the GPU and to read.
    probe: Option<(Buffer, Buffer)>,
    /// Frames since the start (the probe read once, after 300).
    frames: u32,
    /// Columns' structures built, last frame and in all (for the log).
    built: (usize, usize),
}

#[derive(Resource)]
struct RtPipeline {
    layout: BindGroupLayoutDescriptor,
    pipeline: CachedComputePipelineId,
}

fn init_pipeline(mut commands: Commands, cache: Res<PipelineCache>, assets: Res<AssetServer>) {
    let layout = BindGroupLayoutDescriptor::new(
        "rt_shadow",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                binding_types::acceleration_structure(),
                binding_types::texture_depth_2d(),
                binding_types::texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::WriteOnly),
                binding_types::uniform_buffer::<ViewUniform>(true),
                binding_types::uniform_buffer_sized(false, None),
                binding_types::storage_buffer_sized(false, None),
            ),
        ),
    );
    let pipeline = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("rt_shadow".into()),
        layout: vec![layout.clone()],
        shader: assets.load("embedded://game/rt_shadow.wgsl"),
        entry_point: Some("trace".into()),
        ..default()
    });
    commands.insert_resource(RtPipeline { layout, pipeline });
}

/// Each new column's structure built from its mesh where Bevy keeps it,
/// the gone ones' dropped; the scene's built over all drawn.
#[allow(clippy::too_many_arguments)]
fn prepare(
    frame: Res<RtFrame>,
    mut scene: ResMut<RtScene>,
    extracted: Res<ExtractedAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    if !frame.on {
        return;
    }
    let start = std::time::Instant::now();
    // (Which sun is which, for the shadows' shader (sun_shadows.wgsl,
    // `far_for`): where the far maps keep their suns, map 0 the main's,
    // map 2 the second's, texel 5 of each; no far maps are drawn.)
    if let Some(data) = images.get(crate::far_shadow::FAR_DATA.id()) {
        for (map, sun) in [(0usize, frame.suns[0]), (2, frame.suns[1])] {
            let texel = map * crate::far_shadow::BLOCK + 5;
            let bytes: Vec<u8> = sun.truncate().extend(0.0).to_array().iter().flat_map(|f| f.to_le_bytes()).collect();
            queue.write_texture(
                TexelCopyTextureInfo { texture: &data.texture, mip_level: 0, origin: Origin3d { x: texel as u32, y: 0, z: 0 }, aspect: TextureAspect::All },
                &bytes,
                TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(16), rows_per_image: None },
                Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            );
        }
    }
    // (A mesh changed or gone: its structure built again or dropped.)
    for id in extracted.removed.iter().chain(extracted.modified.iter()).chain(extracted.added.iter()) {
        scene.blas.retain(|k, _| k.0 != *id);
    }
    let t_sources = start.elapsed().as_secs_f32() * 1000.0;
    // (Each new column's structure built from its mesh where Bevy keeps it
    // for drawing: its positions first in each 48-byte vertex (see
    // `to_bevy_mesh`). Copied into buffers of their own first, as a check
    // (the same shadows), it cost the render thread 6-9 ms a frame as
    // columns came, flying: the spike's slow frames.)
    let mut new: Vec<((AssetId<Mesh>, u8), Blas, BlasTriangleGeometrySizeDescriptor, _, _, u32)> = Vec::new();
    for piece in frame.columns.iter() {
        let key = (piece.mesh, piece.block);
        if scene.blas.contains_key(&key) || new.iter().any(|(n, ..)| *n == key) {
            continue;
        }
        let (Some(vertices), Some(indices)) = (allocator.mesh_vertex_slice(&piece.mesh), allocator.mesh_index_slice(&piece.mesh)) else { continue };
        let size = BlasTriangleGeometrySizeDescriptor {
            vertex_format: VertexFormat::Float32x3,
            vertex_count: vertices.range.len() as u32,
            index_format: Some(IndexFormat::Uint32),
            index_count: Some(piece.count),
            flags: AccelerationStructureGeometryFlags::OPAQUE,
        };
        let blas = device.wgpu_device().create_blas(
            &CreateBlasDescriptor { label: Some("rt_column"), flags: AccelerationStructureFlags::PREFER_FAST_TRACE, update_mode: AccelerationStructureUpdateMode::Build },
            BlasGeometrySizeDescriptors::Triangles { descriptors: vec![size.clone()] },
        );
        new.push((key, blas, size, vertices, indices, piece.first));
    }
    let t_create = start.elapsed().as_secs_f32() * 1000.0;
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("rt_build") });
    if !new.is_empty() {
        let entries: Vec<BlasBuildEntry> = new
            .iter()
            .map(|(_, blas, size, vertices, indices, first)| BlasBuildEntry {
                blas,
                geometry: BlasGeometries::TriangleGeometries(vec![BlasTriangleGeometry {
                    size,
                    vertex_buffer: vertices.buffer,
                    first_vertex: vertices.range.start,
                    vertex_stride: 48,
                    index_buffer: Some(indices.buffer),
                    first_index: Some(indices.range.start + first),
                    transform_buffer: None,
                    transform_buffer_offset: None,
                }]),
            })
            .collect();
        encoder.build_acceleration_structures(&entries, &[]);
    }
    scene.built = (new.len(), scene.built.1 + new.len());
    let t_encode = start.elapsed().as_secs_f32() * 1000.0;
    queue.submit([encoder.finish()]);
    let t_submit = start.elapsed().as_secs_f32() * 1000.0;
    if frame.log && t_submit > 1.0 {
        let tris: u32 = new.iter().map(|(_, _, size, ..)| size.index_count.unwrap_or(0) / 3).sum();
        info!("rt split: copying positions {t_sources:.2} ms, creating {:.2}, encoding {:.2}, submitting {:.2}; {} structures, {tris} triangles", t_create - t_sources, t_encode - t_create, t_submit - t_encode, new.len());
    }
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("rt_scene") });
    for (id, blas, ..) in new {
        scene.blas.insert(id, blas);
    }
    let built_now = scene.built.0;
    let after_blas = start.elapsed().as_secs_f32() * 1000.0;
    let held: Vec<Piece> = frame.columns.iter().filter(|p| scene.blas.contains_key(&(p.mesh, p.block))).map(|p| Piece { at: p.at - frame.shift, ..*p }).collect();
    if frame.keep && scene.tlas.is_some() && held == scene.held {
        queue.submit([encoder.finish()]);
        scene.frames += 1;
        if frame.log && after_blas > 1.0 {
            info!("rt frame {}: {:.2} ms, {built_now} columns' structures built, scene kept", scene.frames, after_blas);
        }
        return;
    }
    scene.held = held;
    let instances: Vec<(&Blas, Vec3, u32)> = frame.columns.iter().filter_map(|p| scene.blas.get(&(p.mesh, p.block)).map(|b| (b, p.at, p.level))).collect();
    let placed: Vec<(Vec3, u32)> = instances.iter().map(|i| (i.1, i.2)).collect();
    let mut tlas = device.wgpu_device().create_tlas(&CreateTlasDescriptor {
        label: Some("rt_scene"),
        flags: AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: AccelerationStructureUpdateMode::Build,
        max_instances: instances.len().max(1) as u32,
    });
    for (k, (blas, at, _)) in instances.iter().enumerate() {
        let at = *at - frame.shift;
        let transform = [1.0, 0.0, 0.0, at.x, 0.0, 1.0, 0.0, at.y, 0.0, 0.0, 1.0, at.z];
        *tlas.get_mut_single(k).expect("an instance's place") = Some(TlasInstance::new(blas, transform, k as u32, 0xFF));
    }
    encoder.build_acceleration_structures(&[], [&tlas]);
    queue.submit([encoder.finish()]);
    scene.tlas = Some(tlas);
    scene.instances = placed;
    scene.frames += 1;
    let all = start.elapsed().as_secs_f32() * 1000.0;
    if frame.log && (all > 1.0 || built_now > 0) {
        info!("rt frame {}: {:.2} ms ({:.2} before the scene), {built_now} columns' structures built, scene of {} built", scene.frames, all, after_blas, scene.instances.len());
    }
    if scene.probe.is_none() {
        let gpu = device.create_buffer(&BufferDescriptor { label: Some("rt_probe"), size: 128, usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, mapped_at_creation: false });
        let read = device.create_buffer(&BufferDescriptor { label: Some("rt_probe_read"), size: 128, usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST, mapped_at_creation: false });
        scene.probe = Some((gpu, read));
    }
}

/// DEBUG: the probed pixel's results, read once after 300 frames (the
/// frame waited for), logged.
fn read_probe(frame: Res<RtFrame>, scene: Res<RtScene>, device: Res<RenderDevice>) {
    if frame.probe.z < 0.5 || scene.frames < 300 || scene.frames % 50 != 0 {
        return;
    }
    let Some((_, read)) = &scene.probe else { return };
    let slice = read.slice(..);
    device.map_buffer(&slice, bevy::render::render_resource::MapMode::Read, |_| {});
    let _ = device.poll(bevy::render::render_resource::PollType::wait_indefinitely());
    let data: Vec<f32> = slice.get_mapped_range().chunks(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    read.unmap();
    let v = |k: usize| [data[4 * k], data[4 * k + 1], data[4 * k + 2], data[4 * k + 3]];
    let instance = v(3)[1] as usize;
    let column = scene.instances.get(instance).copied();
    info!(
        "rt probe (frame {}) at {:?}: depth {:?}; point {:?}; origin {:?}; main sun hit t {} instance {} primitive {} kind {}; barycentrics/front {:?}; that instance (translation, level) {:?}; hit point {:?}; hit instance's own translation and custom index {:?}; first-hit trace (main: lit, t; second: lit, t) {:?}",
        scene.frames, [frame.probe.x, frame.probe.y], v(0), v(1), v(2), v(3)[0], v(3)[1], v(3)[2], v(3)[3], v(4), column, v(5), v(6), v(7)
    );
}

fn trace(
    view: ViewQuery<(&ViewPrepassTextures, &ViewUniformOffset)>,
    pipeline: Option<Res<RtPipeline>>,
    cache: Res<PipelineCache>,
    scene: Res<RtScene>,
    frame: Res<RtFrame>,
    view_uniforms: Res<ViewUniforms>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (prepass, offset) = view.into_inner();
    let (Some(pipeline), Some(tlas), Some(depth), Some(mask), Some(view_binding)) =
        (pipeline, scene.tlas.as_ref(), prepass.depth_view(), images.get(RT_MASK.id()), view_uniforms.uniforms.binding())
    else {
        return;
    };
    let Some(compute) = cache.get_compute_pipeline(pipeline.pipeline) else { return };
    let Some((probe, probe_read)) = &scene.probe else { return };
    let mut params = Vec::with_capacity(64);
    for v in [frame.suns[0], frame.suns[1], Vec4::new(frame.bend, 0.0, 0.0, 0.0), frame.probe, frame.shift.extend(0.0)] {
        for f in v.to_array() {
            params.extend_from_slice(&f.to_le_bytes());
        }
    }
    let params = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("rt_params"), contents: &params, usage: BufferUsages::UNIFORM });
    let group = device.create_bind_group(
        "rt_shadow",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((tlas.as_binding(), depth, &mask.texture_view, view_binding, params.as_entire_binding(), probe.as_entire_binding())),
    );
    let encoder = ctx.command_encoder();
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("rt_shadow"), timestamp_writes: None });
        pass.set_pipeline(compute);
        pass.set_bind_group(0, &group, &[offset.offset]);
        // (Over the view, not the whole mask: see `MASK`.)
        pass.dispatch_workgroups(prepass.size.width.div_ceil(8), prepass.size.height.div_ceil(8), 1);
    }
    if frame.probe.z > 0.5 && scene.frames >= 299 && scene.frames % 50 == 49 {
        encoder.copy_buffer_to_buffer(probe, 0, probe_read, 0, 128);
    }
}
