//! The far terrain's sun shadows, from cached maps (the user, 2026-11-17:
//! the far view's pop-in fixed by drawing the far levels nearly as the
//! nearest, whose cost was their live shadows, not their triangles).
//!
//! The terrain's coarser levels (`FarCaster`: every column but the nearest
//! level's) cast no live shadow. They are drawn into depth maps of their
//! own instead, each square round the camera as seen from its sun (see
//! `MAPS`): the main sun's inner and outer, the second sun's one. Each is
//! drawn again only as its sun moves (`--set far_sun_step`, degrees), the
//! camera travels (`far_move` for the inner ones, m; the outer, three
//! times as far) or a column in its reach changes, one map a frame, with
//! only the columns whose shadow can fall in it: the suns' paths are fixed
//! and slow (the main one 0.4 degrees a second), so far off their shadows
//! are nearly still. The terrain's shader (terrain.wgsl, `far_shadow`)
//! reads them, and the shadows shader (sun_shadows.wgsl) takes the darker
//! of a sun's and its live cascades, which still draw the nearest level and
//! the structures. `--opt nofarcache`: as before, every level casting live.
//!
//! Each map's matrix, the bend it was drawn with and so on are written into
//! a small texture of their own (`FAR_DATA`) in the render world as the map
//! is drawn: the material is never changed (each change of it, Bevy checks
//! every mesh drawn with it again, see `terrain::update_material`).

use std::{collections::HashMap, sync::Arc};

use bevy::{
    asset::{RenderAssetUsages, embedded_asset, uuid_handle},
    camera::primitives::Aabb,
    image::Image,
    mesh::VertexBufferLayout,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_resource::{
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId,
            CompareFunction, DepthBiasState, DepthStencilState, Extent3d, LoadOp, MultisampleState, Operations, Origin3d, PipelineCache,
            PrimitiveState, RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, StencilState,
            StoreOp, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDimension, TextureFormat, TextureUsages,
            VertexState, binding_types,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};

use crate::{
    Args,
    look::{Sky, sun_direction},
    terrain::{PLANET_RADIUS, StreamAnchor},
};

/// A far map: its sun, its texels on a side, and how far the camera may
/// travel before it is drawn again (in `far_move`s). How far round the
/// camera it reaches: `radius`.
struct Map {
    sun: usize,
    size: u32,
    moves: f32,
}

/// How far round the camera a map reaches (m): `--set far_inner`,
/// `far_radius` (the outer), `far_second`.
fn radius(args: &Args, k: usize) -> f32 {
    match k {
        0 => args.num("far_inner", 1800.0),
        1 => args.num("far_radius", 5000.0),
        _ => args.num("far_second", 2200.0),
    }
}

/// The main sun's inner and outer maps (one to 5 km, its texels 3 by 1.3
/// m, lost the steps' shadows between plates, half of all the far
/// terrain's in the stacks), and the second sun's: soft shadows, its live
/// ones ending at 1.2 km.
const MAPS: [Map; 3] = [
    Map { sun: 0, size: 4096, moves: 1.0 },
    Map { sun: 0, size: 4096, moves: 3.0 },
    Map { sun: 1, size: 2048, moves: 1.0 },
];

/// The maps' images, as `MAPS`.
pub const FAR_MAPS: [Handle<Image>; 3] = [
    uuid_handle!("6b0e8f8e-3f7c-4f51-9a34-2f7d1f0c5a01"),
    uuid_handle!("6b0e8f8e-3f7c-4f51-9a34-2f7d1f0c5a03"),
    uuid_handle!("6b0e8f8e-3f7c-4f51-9a34-2f7d1f0c5a04"),
];
/// What they were drawn with (see `MapFrame::data`), read by terrain.wgsl.
pub const FAR_DATA: Handle<Image> = uuid_handle!("6b0e8f8e-3f7c-4f51-9a34-2f7d1f0c5a02");

/// Texels in `FAR_DATA` for each map.
const BLOCK: usize = 8;

/// A terrain column drawn into the far maps, casting no live shadow.
#[derive(Component)]
pub struct FarCaster;

/// Whether the far terrain's shadows come from the cached maps.
pub fn enabled(args: &Args) -> bool {
    !args.opt("nofarcache")
}

/// What a far map is to be drawn with, for the render world.
#[derive(Clone, Default)]
struct MapFrame {
    /// Bumped at each redraw asked for.
    generation: u64,
    /// Its texels in `FAR_DATA`: 0-3 its clip-from-world matrix, by
    /// column; 4 the camera's x and z it was drawn bent round, the
    /// curvature (1 / 2R), on (1) or not (0); 5 towards its sun; 6 its x
    /// axis; 7 its texel's size (m, along x and y), its depth's range (m),
    /// a constant bias (m).
    data: [Vec4; BLOCK],
    /// Each column drawn: its mesh and translation.
    casters: Arc<Vec<(AssetId<Mesh>, Vec3)>>,
}

#[derive(Resource, Clone, ExtractResource)]
struct FarFrame {
    shader: Handle<Shader>,
    maps: [MapFrame; 3],
}

pub struct FarShadowPlugin;

impl Plugin for FarShadowPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "far_shadow.wgsl");
        let size = |w, h| Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        let data = Image::new_fill(size((BLOCK * MAPS.len()) as u32, 1), TextureDimension::D2, &[0; 16], TextureFormat::Rgba32Float, RenderAssetUsages::RENDER_WORLD);
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        for (handle, map) in FAR_MAPS.iter().zip(&MAPS) {
            let mut image = Image::new_uninit(size(map.size, map.size), TextureDimension::D2, TextureFormat::Depth32Float, RenderAssetUsages::RENDER_WORLD);
            image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
            images.insert(handle, image).expect("a far shadow map");
        }
        images.insert(&FAR_DATA, data).expect("the far shadow maps' data");
        let shader = app.world().resource::<AssetServer>().load("embedded://game/far_shadow.wgsl");
        let enabled = enabled(app.world().resource::<Args>());
        app.insert_resource(FarFrame { shader, maps: Default::default() }).add_plugins(ExtractResourcePlugin::<FarFrame>::default());
        if enabled {
            app.add_systems(PostUpdate, plan);
        }
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.init_resource::<FarRender>().add_systems(Render, draw.in_set(RenderSystems::Render).before(bevy::render::renderer::render_system));
        }
    }
}

/// What a map was last drawn for, and whether it is due again.
#[derive(Default, Clone, Copy)]
struct Last {
    sun: Vec3,
    cam: Vec2,
    drawn: bool,
    /// Since which frame it has been due (None: not due).
    due: Option<u64>,
}

/// The main world's side: each map's last drawing, every far column's
/// ground (x and z bounds), the frame.
#[derive(Default)]
struct Planner {
    last: [Last; 3],
    columns: HashMap<Entity, (Vec2, Vec2)>,
    frame: u64,
}

/// Asks for a map to be drawn again when its sun has moved, the camera
/// travelled or a column within its reach changed: the one due longest, a
/// map a frame.
#[allow(clippy::too_many_arguments)]
fn plan(
    args: Res<Args>,
    sky: Res<Sky>,
    mut frame: ResMut<FarFrame>,
    mut planner: Local<Planner>,
    anchor: Single<&Transform, With<StreamAnchor>>,
    casters: Query<(Entity, &Mesh3d, &Transform, &Aabb, Option<&crate::terrain::Fading>), With<FarCaster>>,
) {
    planner.frame += 1;
    let now = planner.frame;
    let cam = anchor.translation.xz();
    let ground = |t: &Transform, aabb: &Aabb| {
        let (c, h) = (t.translation + Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
        (c.xz() - h.xz(), c.xz() + h.xz())
    };
    // The columns come and gone this frame, where they stand (the finer
    // of two fading casts no shadow, see `terrain::Fading`).
    let mut changed: Vec<(Vec2, Vec2)> = Vec::new();
    let mut now_cast: HashMap<Entity, (Vec2, Vec2)> = HashMap::with_capacity(planner.columns.len());
    for (entity, _, t, aabb, fading) in &casters {
        if fading.is_some_and(|f| f.quiet) {
            continue;
        }
        let g = ground(t, aabb);
        if !planner.columns.contains_key(&entity) {
            changed.push(g);
        }
        now_cast.insert(entity, g);
    }
    changed.extend(planner.columns.iter().filter(|(e, _)| !now_cast.contains_key(*e)).map(|(_, g)| *g));
    planner.columns = now_cast;

    let step = args.num("far_sun_step", 0.2).to_radians();
    let travel = args.num("far_move", 100.0);
    // (What stands beyond a map's ground may still shade it.)
    let margin = args.num("far_margin", 1000.0);
    for (k, map) in MAPS.iter().enumerate() {
        let dir = sun_direction(map.sun, sky.time);
        let last = &mut planner.last[k];
        if dir.y <= 0.0 {
            // Its sun down: off, once.
            if last.drawn {
                last.drawn = false;
                last.due = None;
                frame.maps[k].data[4].w = 0.0;
                frame.maps[k].generation += 1;
            }
            continue;
        }
        let reach = radius(&args, k) + margin;
        let near = |(lo, hi): &(Vec2, Vec2)| lo.x < last.cam.x + reach && hi.x > last.cam.x - reach && lo.y < last.cam.y + reach && hi.y > last.cam.y - reach;
        let due = !last.drawn
            || last.sun.angle_between(dir) > step
            || last.cam.distance(cam) > travel * map.moves
            || changed.iter().any(near);
        if due && last.due.is_none() {
            last.due = Some(now);
        }
    }
    let Some(k) = (0..MAPS.len()).filter(|&k| planner.last[k].due.is_some()).min_by_key(|&k| planner.last[k].due) else { return };
    let map = &MAPS[k];
    let dir = sun_direction(map.sun, sky.time);
    let radius = radius(&args, k);
    let curvature = if args.opt("flat") { 0.0 } else { 0.5 / PLANET_RADIUS };

    // Seen from the sun: the map's box fitted round its ground, as low as
    // the bend takes it and as high as the columns stand, the near side
    // drawn on far towards the sun for what stands outside it but shades it.
    let view = Mat4::look_to_rh(Vec3::new(cam.x, 0.0, cam.y), -dir, Vec3::Y);
    let bounds: Vec<(Entity, AssetId<Mesh>, Vec3, Vec3, Vec3)> = casters
        .iter()
        .filter(|c| !c.4.is_some_and(|f| f.quiet))
        .map(|(e, mesh, t, aabb, _)| {
            let c = t.translation + Vec3::from(aabb.center);
            let h = Vec3::from(aabb.half_extents);
            (e, mesh.id(), t.translation, c - h, c + h)
        })
        .collect();
    if bounds.is_empty() {
        return;
    }
    let (low, high) = bounds.iter().fold((f32::MAX, f32::MIN), |(lo, hi), b| (lo.min(b.3.y), hi.max(b.4.y)));
    let low = low - 2.0 * radius * radius * curvature;
    let in_view = |lo: Vec3, hi: Vec3| {
        let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
        for i in 0..8 {
            let corner = Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
            let p = view.transform_point3(corner);
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    };
    let (min, max) = in_view(Vec3::new(cam.x - radius, low, cam.y - radius), Vec3::new(cam.x + radius, high, cam.y + radius));
    // (Its corners on whole texels, so a redraw for the camera alone leaves
    // what is still in shadow where it was.)
    let texel = Vec2::new(max.x - min.x, max.y - min.y) / map.size as f32;
    let (x0, y0) = ((min.x / texel.x).floor() * texel.x, (min.y / texel.y).floor() * texel.y);
    let (x1, y1) = (x0 + texel.x * map.size as f32, y0 + texel.y * map.size as f32);
    let (near, far) = (-max.z - args.num("far_reach", 3000.0), -min.z + 10.0);
    let clip = Mat4::orthographic_rh(x0, x1, y0, y1, near, far) * view;

    // Only the columns seen in it (each as low as the bend takes it there).
    let list: Vec<(AssetId<Mesh>, Vec3)> = bounds
        .iter()
        .filter(|(_, _, _, lo, hi)| {
            let d = (lo.xz() - cam).abs().max((hi.xz() - cam).abs());
            let (a, b) = in_view(Vec3::new(lo.x, lo.y - d.length_squared() * curvature, lo.z), *hi);
            a.x < x1 && b.x > x0 && a.y < y1 && b.y > y0 && b.z > -far
        })
        .map(|&(_, mesh, t, _, _)| (mesh, t))
        .collect();
    let right = view.row(0).truncate();
    let out = &mut frame.maps[k];
    out.data = [
        clip.x_axis,
        clip.y_axis,
        clip.z_axis,
        clip.w_axis,
        Vec4::new(cam.x, cam.y, curvature, 1.0),
        dir.extend(0.0),
        right.extend(0.0),
        Vec4::new(texel.x, texel.y, far - near, args.num("far_bias", 0.5)),
    ];
    if out.generation == 0 {
        info!("far shadow map {k}: texels {:.2} x {:.2} m, {} of {} columns", texel.x, texel.y, list.len(), bounds.len());
    }
    out.casters = Arc::new(list);
    out.generation += 1;
    planner.last[k] = Last { sun: dir, cam, drawn: true, due: None };
}

/// The render world's side: the pipeline, and what each map was last drawn
/// with.
#[derive(Resource, Default)]
struct FarRender {
    drawn: [u64; 3],
    data: [[Vec4; BLOCK]; 3],
    pipeline: Option<(CachedRenderPipelineId, BindGroupLayoutDescriptor)>,
}

/// Draws the far maps asked for, before the frame's own passes.
#[allow(clippy::too_many_arguments)]
fn draw(
    frame: Option<Res<FarFrame>>,
    mut state: ResMut<FarRender>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let Some(frame) = frame else { return };
    if (0..MAPS.len()).all(|k| frame.maps[k].generation == state.drawn[k]) {
        return;
    }
    let Some(data) = images.get(FAR_DATA.id()) else { return };

    if state.pipeline.is_none() {
        // (Every column's vertices laid out alike: positions, normals,
        // colours; only the positions read.)
        let Some(mesh) = frame.maps.iter().flat_map(|m| m.casters.iter()).find_map(|(id, _)| meshes.get(*id)) else { return };
        let buffer: VertexBufferLayout = mesh
            .layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])
            .expect("terrain columns have positions");
        let layout = BindGroupLayoutDescriptor::new(
            "far_shadow",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX,
                (binding_types::uniform_buffer_sized(false, None), binding_types::storage_buffer_read_only_sized(false, None)),
            ),
        );
        let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("far_shadow".into()),
            layout: vec![layout.clone()],
            immediate_size: 0,
            vertex: VertexState { shader: frame.shader.clone(), shader_defs: vec![], entry_point: Some("vertex".into()), buffers: vec![buffer] },
            primitive: PrimitiveState { cull_mode: None, ..default() },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::Less),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            fragment: None,
            zero_initialize_workgroup_memory: false,
        });
        state.pipeline = Some((id, layout));
    }
    let Some((id, layout)) = state.pipeline.clone() else { return };
    let Some(pipeline) = cache.get_render_pipeline(id) else { return };

    let mut encoder = device.create_command_encoder(&default());
    for (k, map) in frame.maps.iter().enumerate() {
        if map.generation == state.drawn[k] {
            continue;
        }
        if map.data[4].w == 0.0 {
            state.data[k] = map.data;
            state.drawn[k] = map.generation;
            continue;
        }
        let Some(target) = images.get(FAR_MAPS[k].id()) else { continue };
        let uniform: Vec<u8> = map.data[..5].iter().flat_map(|v| v.to_array()).flat_map(f32::to_le_bytes).collect();
        let offsets: Vec<u8> = map.casters.iter().flat_map(|(_, t)| t.extend(0.0).to_array()).flat_map(f32::to_le_bytes).collect();
        // (Its storage never empty.)
        let offsets = if offsets.is_empty() { vec![0; 16] } else { offsets };
        let uniform = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("far_shadow"), contents: &uniform, usage: BufferUsages::UNIFORM });
        let offsets = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("far_shadow_offsets"), contents: &offsets, usage: BufferUsages::STORAGE });
        let bind_group = device.create_bind_group("far_shadow", &cache.get_bind_group_layout(&layout), &BindGroupEntries::sequential((uniform.as_entire_binding(), offsets.as_entire_binding())));
        let mut missing = 0;
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("far_shadow"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &target.texture_view,
                depth_ops: Some(Operations { load: LoadOp::Clear(1.0), store: StoreOp::Store }),
                stencil_ops: None,
            }),
            ..default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        for (i, (mesh_id, _)) in map.casters.iter().enumerate() {
            let i = i as u32;
            let (Some(mesh), Some(vertices)) = (meshes.get(*mesh_id), allocator.mesh_vertex_slice(mesh_id)) else {
                missing += 1;
                continue;
            };
            pass.set_vertex_buffer(0, (**vertices.buffer).slice(..));
            match &mesh.buffer_info {
                RenderMeshBufferInfo::Indexed { count, index_format } => {
                    let Some(indices) = allocator.mesh_index_slice(mesh_id) else {
                        missing += 1;
                        continue;
                    };
                    pass.set_index_buffer((**indices.buffer).slice(..), *index_format);
                    pass.draw_indexed(indices.range.start..indices.range.start + count, vertices.range.start as i32, i..i + 1);
                }
                RenderMeshBufferInfo::NonIndexed => pass.draw(vertices.range.start..vertices.range.start + mesh.vertex_count, i..i + 1),
            }
        }
        state.data[k] = map.data;
        // (A column not on the GPU yet: drawn again next frame, till all are.)
        if missing == 0 {
            state.drawn[k] = map.generation;
        }
    }
    let bytes: Vec<u8> = state.data.iter().flatten().flat_map(|v| v.to_array()).flat_map(f32::to_le_bytes).collect();
    queue.write_texture(
        TexelCopyTextureInfo { texture: &data.texture, mip_level: 0, origin: Origin3d::ZERO, aspect: TextureAspect::All },
        &bytes,
        TexelCopyBufferLayout { offset: 0, bytes_per_row: Some((BLOCK * MAPS.len()) as u32 * 16), rows_per_image: None },
        Extent3d { width: (BLOCK * MAPS.len()) as u32, height: 1, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
}
