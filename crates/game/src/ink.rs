//! Ink: dark lines where the geometry breaks, drawn after lighting, so the
//! world reads like the line art it comes from (BLAME!, Moebius, Manifold
//! Garden). A full-screen pass reads the prepass depth and normals: a line
//! where depth jumps (silhouettes) or the surface turns (creases), never
//! where only the light or the tone changes. Lines thin out and fade with
//! distance, into the haze.
//!
//! Adapted from Bevy's `FullscreenMaterial`, which binds only the screen;
//! this also binds the depth and normal prepass textures (the camera has
//! them for SSAO). Off by default (parked after a first review):
//! `--set ink=0.85` turns it on at that strength, `ink_width` sets its
//! width in pixels; F4 turns it off and on.

use bevy::{
    core_pipeline::{
        prepass::ViewPrepassTextures, schedule::Core3d, tonemapping::tonemapping, Core3dSystems,
        FullscreenShader,
    },
    ecs::schedule::IntoScheduleConfigs,
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_resource::{
            binding_types::{texture_2d, texture_depth_2d, uniform_buffer},
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, Operations,
            PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, ShaderStages, ShaderType, TextureFormat, TextureSampleType,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::{ExtractedView, ViewTarget},
        Render, RenderApp, RenderStartup, RenderSystems,
    },
};

use crate::Args;

/// The ink settings on a camera. `a`: strength, width (pixels), depth
/// sensitivity, crease sensitivity; `b`: the camera's near plane, the
/// distance lines fade over, the ink's tone, unused. It reads the depth and
/// normal prepasses, so the camera gets them.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType, Default)]
#[require(bevy::core_pipeline::prepass::DepthPrepass, bevy::core_pipeline::prepass::NormalPrepass)]
pub struct Ink {
    pub a: Vec4,
    pub b: Vec4,
}

pub struct InkPlugin;

impl Plugin for InkPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "ink.wgsl");
        app.add_plugins((ExtractComponentPlugin::<Ink>::default(), UniformComponentPlugin::<Ink>::default()))
            .add_systems(Update, (attach, toggle));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(
                Render,
                (prepare_pipelines.in_set(RenderSystems::Prepare), prepare_bind_groups.in_set(RenderSystems::PrepareBindGroups)),
            )
            .add_systems(Core3d, ink_pass.in_set(Core3dSystems::PostProcess).before(tonemapping));
    }
}

/// Gives the camera its ink once it exists.
fn attach(mut commands: Commands, args: Res<Args>, cameras: Query<(Entity, &Projection), (With<Camera3d>, Without<Ink>)>) {
    // Attached even when off, so F4 can turn it on.
    let strength = args.num("ink", 0.0);
    for (entity, projection) in &cameras {
        let near = match projection {
            Projection::Perspective(p) => p.near,
            _ => 0.1,
        };
        commands.entity(entity).insert(Ink {
            a: Vec4::new(strength, args.num("ink_width", 1.0), args.num("ink_depth", 1.0), args.num("ink_crease", 1.0)),
            b: Vec4::new(near, args.num("ink_fade", 1500.0), args.num("ink_tone", 0.0), 0.0),
        });
    }
}

/// F4 turns the ink off and on.
fn toggle(keys: Res<ButtonInput<KeyCode>>, args: Res<Args>, mut inks: Query<&mut Ink>) {
    if !keys.just_pressed(KeyCode::F4) {
        return;
    }
    for mut ink in &mut inks {
        ink.a.x = if ink.a.x > 0.0 { 0.0 } else { args.num("ink", 0.85).max(0.85) };
        info!("ink {}", if ink.a.x > 0.0 { "on" } else { "off" });
    }
}

#[derive(Resource)]
struct InkPipeline {
    layout: BindGroupLayoutDescriptor,
    descriptor: RenderPipelineDescriptor,
}

fn init_pipeline(mut commands: Commands, asset_server: Res<AssetServer>, fullscreen_shader: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "ink_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_depth_2d(),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<Ink>(true),
            ),
        ),
    );
    let descriptor = RenderPipelineDescriptor {
        label: Some("ink_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load("embedded://game/ink.wgsl"),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };
    commands.insert_resource(InkPipeline { layout, descriptor });
}

#[derive(Component)]
struct InkPipelineId(CachedRenderPipelineId);

#[derive(Component)]
struct InkBindGroups {
    /// One per main texture (either can be the source).
    a: BindGroup,
    b: BindGroup,
    a_view: bevy::render::render_resource::TextureViewId,
}

fn prepare_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Option<Res<InkPipeline>>,
    views: Query<(Entity, &ExtractedView), With<Ink>>,
    mut cache: Local<std::collections::HashMap<TextureFormat, CachedRenderPipelineId>>,
) {
    let Some(pipeline) = pipeline else { return };
    for (entity, view) in &views {
        let id = *cache.entry(view.target_format).or_insert_with(|| {
            let mut descriptor = pipeline.descriptor.clone();
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.targets = vec![Some(ColorTargetState { format: view.target_format, blend: None, write_mask: ColorWrites::ALL })];
            }
            pipeline_cache.queue_render_pipeline(descriptor)
        });
        commands.entity(entity).insert(InkPipelineId(id));
    }
}

fn prepare_bind_groups(
    mut commands: Commands,
    views: Query<(Entity, &ViewTarget, &ViewPrepassTextures), With<Ink>>,
    pipeline: Option<Res<InkPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<Ink>>,
    render_device: Res<RenderDevice>,
) {
    let Some(pipeline) = pipeline else { return };
    let Some(binding) = uniforms.uniforms().binding() else { return };
    for (entity, target, prepass) in &views {
        let (Some(depth), Some(normal)) = (prepass.depth_view(), prepass.normal_view()) else { continue };
        let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);
        let make = |screen| {
            render_device.create_bind_group(
                "ink_bind_group",
                &layout,
                &BindGroupEntries::sequential((screen, depth, normal, binding.clone())),
            )
        };
        commands.entity(entity).insert(InkBindGroups {
            a: make(target.main_texture_view()),
            b: make(target.main_texture_other_view()),
            a_view: target.main_texture_view().id(),
        });
    }
}

fn ink_pass(
    view: ViewQuery<(&ViewTarget, &DynamicUniformIndex<Ink>, &InkBindGroups, &InkPipelineId)>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, index, groups, pipeline_id) = view.into_inner();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id.0) else { return };
    let post = target.post_process_write();
    let group = if post.source.id() == groups.a_view { &groups.a } else { &groups.b };
    let descriptor = RenderPassDescriptor {
        label: Some("ink_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut pass = ctx.command_encoder().begin_render_pass(&descriptor);
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[index.index()]);
    pass.draw(0..3, 0..1);
}
