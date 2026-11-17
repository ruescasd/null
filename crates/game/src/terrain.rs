//! Streams world columns around the camera at several levels of detail.
//!
//! Columns are identified two ways: an *unwrapped* key (where an instance sits
//! in the scene, relative to the camera) and a *wrapped* key (which piece of the
//! torus it shows). Meshes are generated and cached by wrapped key; instances
//! are spawned by unwrapped key, so the same mesh appears again after you walk
//! once around the world.
//!
//! Each level covers a disc around the camera. A coarse column is drawn until
//! every finer column covering it is ready, so there are never holes; where a
//! coarse column still overlaps finer ones it sits slightly lower so the finer
//! surface wins. A column replacing another fades in as that one fades out,
//! dithered (see `Fading`).

use std::sync::Arc;

use avian3d::prelude::{Collider, RigidBody};
use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};
use worldgen::{
    ColumnMesh, LOD_FACTOR, LOD_LEVELS, WORLD_SIZE, column_size,
    plates::{CanalFlow, PlateWorld},
};

use bevy::camera::{
    primitives::Aabb,
    visibility::{NoAutoAabb, VisibilityRange},
};
use bevy::mesh::MeshTag;

use crate::Args;

/// Radius of each detail level's disc, in metres.
pub const LOD_RADIUS: [f32; LOD_LEVELS as usize] = [450.0, 1700.0, 4800.0];

/// The levels' radii (`LOD_RADIUS`); DIAGNOSTIC `--set lod_near N`: the
/// finest level out to N m (the middle level at least as far), to see what
/// changes when nothing near changes level.
pub fn lod_radius(lod: u32) -> f32 {
    let near = NEAR_RADIUS.load(std::sync::atomic::Ordering::Relaxed);
    let near = if near > 0 { near as f32 } else { LOD_RADIUS[0] };
    match lod {
        0 => near,
        1 => LOD_RADIUS[1].max(near + 600.0),
        _ => LOD_RADIUS[lod as usize].max(near + 1800.0),
    }
}

/// See `lod_radius` (0: as `LOD_RADIUS`).
pub static NEAR_RADIUS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// How far each coarser level of detail is lowered (m, see `WorldGen::sink`)
/// drawn at a fixed depth (`--set far_px 0`).
const SINK: f32 = 3.0;
/// How far round the camera the finest terrain must be in place to play
/// (`Streamer::near`); the rest streams in while you do.
const NEAR: f32 = 80.0;

/// Radius of the fake planet used to bend the world below the horizon.
pub const PLANET_RADIUS: f32 = 40_000.0;

/// The canals' rises and dips (see worldgen's `canal::Rise`): `--set
/// canal_rise` (m either way), `canal_wave` (m from crest to crest).
pub fn rise(args: &Args) -> worldgen::canal::Rise {
    let d = worldgen::canal::Rise::default();
    worldgen::canal::Rise { height: args.num("canal_rise", d.height as f32) as f64, wave: args.num("canal_wave", d.wave as f32) as f64 }
}

/// The stones lining the canals' pipes (see worldgen's `canal::Stones`):
/// `--set stone_length` (m), `stone_tone` (a share of the albedo),
/// `stone_tilt` (degrees).
pub fn stones(args: &Args) -> worldgen::canal::Stones {
    let d = worldgen::canal::Stones::default();
    worldgen::canal::Stones {
        length: args.num("stone_length", d.length as f32) as f64,
        tone: args.num("stone_tone", d.tone),
        tilt: args.num("stone_tilt", d.tilt),
    }
}

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<Args>();
        // (The ground lab's stands on the lab's too: see `groundlab.rs`.)
        let world = if args.opt("lab") || args.opt("chasm") || args.ground.is_some() {
            // The lab: flat plates, nothing else (see `structures.rs`); the
            // chasm stands on them (see `chasm.rs`).
            WorldGen(Arc::new(PlateWorld::lab(WORLD_SIZE, args.seed)))
        } else {
            let world = plate_world(args);
            // (Slabs on: those near where it starts, to find and judge them,
            // each its own steepness and length.)
            if world.0.slabs > 0 {
                let [x, _, z, ..] = args.cam.unwrap_or([0.0; 5]);
                let mut near = world.0.slabs_near(x as f64, z as f64, 1500.0);
                let off = |s: &worldgen::plates::SlabInfo| (s.middle.x - x as f64).hypot(s.middle.y - z as f64);
                near.sort_by(|a, b| off(a).total_cmp(&off(b)));
                for s in near {
                    info!("slab at {:.0},{:.0}: {:.0} deg, {:.0} m long, {:.0} m wide, {} piece(s)", s.middle.x, s.middle.y, s.tilt, s.length, s.width, s.pieces);
                }
            }
            world
        };
        embedded_asset!(app, "terrain.wgsl");
        embedded_asset!(app, "terrain_vertex.wgsl");
        embedded_asset!(app, "terrain_prepass.wgsl");
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default())
            .insert_resource(world)
            .init_resource::<Streamer>()
            .add_systems(Startup, setup_material)
            .add_systems(
                Update,
                (receive_meshes, stream_columns, fade_columns, update_material).chain().in_set(StreamSet),
            );
    }
}

/// The coarser levels drawn as planes fitted to the finest within `--set
/// far_px` pixels (3) seen from the level's nearest (65 degrees over 900 px;
/// see worldgen's `PlateWorld::far_tol`): how far (m) at each level. (At a
/// fixed depth, `far_px 0`, the far districts read as boxes and each level
/// popped in, the user, 2026-11-17.)
fn far_tol(args: &Args) -> [f64; 3] {
    let px = args.num("far_px", 3.0) as f64;
    let angle = 65f64.to_radians() / 900.0;
    [0.0, LOD_RADIUS[0] as f64 * angle * px, LOD_RADIUS[1] as f64 * angle * px]
}

/// How far a level of detail is lowered (see `WorldGen::sink`), its
/// tolerances `tol`.
fn sink(tol: [f64; 3], lod: u32) -> f32 {
    if tol[lod as usize] > 0.0 {
        return tol[..=lod as usize].iter().sum::<f64>() as f32;
    }
    SINK * lod as f32
}

/// The main world, as `args` asks for it: the plates, their canals and
/// districts, reshaped by the sites in the structure library.
pub fn plate_world(args: &Args) -> WorldGen {
    NEAR_RADIUS.store(args.num("lod_near", 0.0) as u32, std::sync::atomic::Ordering::Relaxed);
    let world = plates(args);
    match crate::structures::load_library() {
        Ok(library) if !args.opt("nosites") => WorldGen(Arc::new(world.with_sites(&library))),
        _ => WorldGen(Arc::new(world)),
    }
}

/// The plates as `args` asks for them, before the sites reshape them: every
/// setting in one place, for the world at the start and for its rebuilding
/// when the sites change (see `structures.rs`). (Copied there field by
/// field, the voids and the far view's tolerances were left behind, and
/// the rebuilt world had neither, 2026-11-17.)
pub fn plates(args: &Args) -> PlateWorld {
    let mut world = PlateWorld::new(WORLD_SIZE, args.seed);
    world.canals.stones = stones(args);
    world.canals.rise = rise(args);
    world.canals.relief = args.num("canal_relief", 1.5) as f64;
    // (`--set slabs N`: 1 slabs, 2 cliff blocks; see worldgen's
    // slabs.rs.)
    world.slabs = args.num("slabs", 0.0) as u8;
    // (`--opt kerbs`: the ground's brick along the flat plates'
    // edges; see worldgen's bricks.rs.)
    world.kerbs = args.opt("kerbs");
    world.far_tol = far_tol(args);
    // (`--set voids`: the share of the region map's cells that are voids,
    // no ground, falling for ever (0 by default while they are judged; the
    // user's, 20%, 2026-11-17); `coast`, how far their outlines are warped,
    // m.)
    world.districts.voids = args.num("voids", 0.0) as f64;
    world.districts.coast = args.num("coast", 600.0) as f64;
    // (`--set void_depth`: how far below the world their walls reach, m.)
    world.void_depth = -(args.num("void_depth", -worldgen::plates::VOID_DEPTH as f32) as f64);
    // (PROBE `--opt vaults`: the world the top of vaults on pillars, see
    // worldgen's `Vaults`; `--set vault_bay`, `vault_crown`, m.)
    if args.opt("vaults") {
        world.vaults = Some(worldgen::plates::Vaults { bay: args.num("vault_bay", 1024.0) as f64, crown: args.num("vault_crown", 60.0) as f64 });
    }
    // (`--set strata N`: the void walls' strata while judged, 0 none.)
    world.strata = args.num("strata", 0.0) as u8;
    // (`--set wall_least`: the lowest wall panelled, m; see worldgen.)
    world.wall_least = args.num("wall_least", 1.5) as f64;
    // (`--set district=N`: one district everywhere, in the order of
    // `District::ALL`: 0 floor, 1 tiers, 2 stacks, 3 broken, 4
    // uplands, 5 highlands.)
    let only = args.num("district", -1.0);
    if only >= 0.0 {
        world.districts.force = worldgen::district::District::ALL.get(only as usize).copied();
    }
    world
}

/// Systems that move the stream anchor must run before this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamSet;

/// The world being shown: the plate world.
#[derive(Resource, Clone)]
pub struct WorldGen(pub Arc<PlateWorld>);

impl WorldGen {
    pub fn size(&self) -> f32 {
        self.0.size()
    }

    pub fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.0.height_at(x, z)
    }

    /// The ground walked on (see `PlateWorld::walk_height_at`).
    pub fn walk_height(&self, x: f32, z: f32) -> f32 {
        self.0.walk_height_at(x, z)
    }

    /// The canal flow at a point, if it is inside a canal's pipe.
    pub fn canal_at(&self, x: f32, z: f32) -> Option<CanalFlow> {
        self.0.canal_at(x, z)
    }

    /// The nearest canal centreline point, its flow direction and floor.
    pub fn nearest_canal(&self, x: f32, z: f32) -> Option<(Vec2, Vec2, f32)> {
        self.0.nearest_canal(x, z)
    }

    fn mesh(&self, lod: u32, cx: i32, cz: i32) -> ColumnMesh {
        self.0.mesh_column(lod, cx, cz)
    }

    /// How far a coarse column is lowered so finer overlapping terrain wins:
    /// a fitted level (see `far_tol`) by its tolerance and the finer
    /// levels', at most that far from the finest; at a fixed depth, finer
    /// plates step at most a few metres from their parent.
    fn sink(&self, lod: u32) -> f32 {
        sink(self.0.far_tol, lod)
    }

    pub fn columns_per_side(&self, lod: u32) -> i32 {
        (self.size() / column_size(lod)) as i32
    }
}

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExtension>;

/// Applies baked sky visibility (vertex colour alpha) and bends the world
/// around the camera to fake a planet's curvature.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TerrainExtension {
    /// x: baked occlusion strength, y: curvature (1 / 2R), zw: camera x and z.
    #[uniform(100)]
    pub params: Vec4,
    /// x: world wrap period (m), for patterns to tile with the world; the
    /// plates' subdivision (`--opt nosubdiv` for none): y on, z the chance
    /// a piece splits again, w the largest pieces drawn (m).
    #[uniform(101)]
    pub wrap: Vec4,
    /// x: brightness of lit geometry (glowing etchings and seams); the
    /// plates' subdivision: y the pieces' tone, z their tilt (degrees), w how
    /// dark the joints are.
    #[uniform(102)]
    pub glow: Vec4,
    /// Paving on some of the ground (`--set paving=N`, off by default): x the
    /// pattern (0 none, 1 checkerboard, 2 fractal), y the tile size (m), z
    /// the contrast, w the share of the ground paved.
    #[uniform(103)]
    pub paving: Vec4,
    /// The plates' subdivision: x the smallest pieces drawn (m), y the slope
    /// of the joints' sides (degrees; 0, flat dark lines); z how far a
    /// void's walls (and the vaults and pillars) go into the dark: a factor
    /// e every z m below their first 60 (`--set void_fade`, m).
    #[uniform(104)]
    pub pieces: Vec4,
    /// The plates' walls' panels (`--set walls N`, see `wall` in
    /// terrain.wgsl): x on (0 off), y a course's height (m), z a joint's
    /// width (m), w each piece's tone.
    #[uniform(105)]
    pub walls: Vec4,
    /// The walls' panels: x the longest the smallest
    /// pieces may be (in courses), y how much of the wall the big pieces
    /// cover (1 about as much as each other size), z the chance one of the
    /// biggest cuts is stepped.
    #[uniform(106)]
    pub layers: Vec4,
    /// The panels' cuts: x the chance one is left open, y the chance one is
    /// a curve; z and w how far the middle and far levels of detail are
    /// lowered (m, `sink`), so a wall's pattern is laid out where it stands.
    #[uniform(107)]
    pub marks: Vec4,
    /// Walls round a middle (the arena's): xy the middle (x, z), w on (1).
    /// A wall facing it, or away, is panelled along its curve, not each of
    /// its facets along itself.
    #[uniform(108)]
    pub round: Vec4,
    /// The far terrain's cached shadow map, and what it was drawn with
    /// (see far_shadow.rs).
    #[texture(109, sample_type = "depth")]
    pub far_inner: Handle<Image>,
    #[texture(110, sample_type = "float", filterable = false)]
    pub far_data: Handle<Image>,
    #[texture(111, sample_type = "depth")]
    pub far_outer: Handle<Image>,
    #[texture(112, sample_type = "depth")]
    pub far_second: Handle<Image>,
    /// PROBE: the shell's mask (see shell.rs, `SHELL`).
    #[texture(113)]
    #[sampler(114)]
    pub shell: Handle<Image>,
    /// SPIKE: the suns' traced shadows, a texel a pixel (see rt_shadow.rs;
    /// read when `pieces.w` is 1).
    #[texture(115)]
    pub rt_mask: Handle<Image>,
}

impl MaterialExtension for TerrainExtension {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/terrain_vertex.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/terrain.wgsl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://game/terrain_prepass.wgsl".into()
    }
}

#[derive(Resource)]
pub struct TerrainMaterialHandle(pub [Handle<TerrainMaterial>; LOD_LEVELS as usize]);

/// How much further back each coarser level of detail is drawn, in the
/// depth buffer's own units (a constant depth bias): where two levels
/// overlap, as one takes over from the other, the finer wins every tie.
/// (A void's walls, the same at every level, lay in one plane at both,
/// hundreds of metres tall; the camera's jitter decided which was drawn,
/// whole walls blinking, the user's "blink", 2026-11-17.)
const LOD_DEPTH_BIAS: f32 = 2000.0;

/// The same surface for structures, with its own panelling.
#[derive(Resource)]
pub struct StructureMaterialHandle(pub Handle<TerrainMaterial>);

#[derive(Component)]
pub struct TerrainColumn;

/// A column fading in, or out, over `--set lod_fade` s (0.4; 0, none):
/// since when (real time). Where one level of detail takes over from
/// another, the one leaving fades out as the one coming fades in, dithered,
/// their patterns each other's complement, so there is never a hole (Bevy's
/// visibility ranges' crossfade, timed here rather than by distance: a
/// level's columns are ready one by one; the coarse one leaves only as the
/// last of them comes). Told to the shaders in the column's tag (see
/// `fade_tag`). (A level's switch at once popped, the user, 2026-11-17.)
/// Meanwhile the finer of the two casts no shadow, live or in the far maps
/// (`quiet`): the coarser under it, sunk, lay in its shadow, a dark flash.
/// The coarser casts on: it lies under the finer, and the shadows go over
/// from one to the other as they end.
#[derive(Component)]
pub struct Fading {
    since: f32,
    out: bool,
    pub quiet: bool,
}

/// A fading column's tag: Bevy's dither level (-16, hidden, to 0, fading
/// in; 0 to 16, hidden, fading out) plus 64; 0, not fading (read in
/// terrain_vertex.wgsl and terrain_prepass.wgsl).
fn fade_tag(level: i32) -> MeshTag {
    MeshTag(if level == 0 { 0 } else { (level + 64) as u32 })
}

/// A range never reached, with a margin, so Bevy's pipelines dither a
/// fading column (see `Fading`): only while it fades (every column so, the
/// terrain's shader discarding, its depth tested late: the stacks from
/// above 9.3 to 13.2 ms).
fn dithered() -> VisibilityRange {
    VisibilityRange { start_margin: 0.0..0.001, end_margin: 1.0e9..2.0e9, use_aabb: false }
}

/// Moves each fading column on, and lets one gone out go. (Its mesh told
/// changed as it begins and ends: Bevy chooses an entity's pipelines again
/// only for that, not for its visibility range; ended, the columns kept
/// the dithered ones, 2.7 ms in the stacks.)
fn fade_columns(
    args: Res<Args>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut fading: Query<(Entity, &Fading, &mut MeshTag, &mut Mesh3d, Has<crate::far_shadow::FarCaster>)>,
    begun: Query<(), Added<Fading>>,
) {
    let length = args.num("lod_fade", 0.4);
    let now = time.elapsed_secs();
    for (entity, fade, mut tag, mut mesh, far) in &mut fading {
        if begun.contains(entity) {
            mesh.set_changed();
        }
        let t = if length > 0.0 { ((now - fade.since) / length).clamp(0.0, 1.0) } else { 1.0 };
        let step = (t * 16.0).round() as i32;
        if t >= 1.0 {
            if fade.out {
                commands.entity(entity).despawn();
            } else {
                commands.entity(entity).remove::<(Fading, VisibilityRange)>();
                // (The nearest level casting live again.)
                if fade.quiet && !far && !crate::rt_shadow::enabled(&args) {
                    commands.entity(entity).remove::<bevy::light::NotShadowCaster>();
                }
                *tag = fade_tag(0);
                mesh.set_changed();
            }
            continue;
        }
        let new = fade_tag(if fade.out { step } else { step - 16 });
        if *tag != new {
            *tag = new;
        }
    }
}

/// Marks the entity whose position drives streaming.
#[derive(Component, Default)]
pub struct StreamAnchor;

type Key = (u32, IVec2);

/// A generated column, ready to instance.
struct Built {
    mesh: Handle<Mesh>,
    /// Its bounds, computed once (see `bounds`).
    aabb: Aabb,
    /// Collision shape; only the finest level gets one, since that is the
    /// only level the player can reach.
    collider: Option<Collider>,
    /// Its blocks (see worldgen's `ColumnMesh::blocks`).
    blocks: Arc<Vec<(u32, u32)>>,
}

/// A column's blocks, for the traced shadows' scene (see worldgen's
/// `ColumnMesh::blocks`, rt_shadow.rs).
#[derive(Component, Clone)]
pub struct ColumnBlocks(pub Arc<Vec<(u32, u32)>>);

#[derive(Resource, Default)]
pub struct Streamer {
    /// (lod, wrapped key) -> mesh (None when the column is empty).
    cache: HashMap<Key, Option<Built>>,
    pending: HashMap<Key, Task<(Option<(Mesh, Aabb, Vec<(u32, u32)>)>, Option<Collider>, [f32; 2])>>,
    /// (lod, unwrapped key) -> instance.
    spawned: HashMap<Key, Entity>,
    /// Instances of a world that has since changed, kept until their
    /// replacements are drawn so the ground never disappears.
    stale: HashMap<Key, Entity>,
    /// True once every column in view has been generated and spawned.
    pub settled: bool,
    /// True once the finest ring round the camera is (enough to stand, walk
    /// and fight on while the distance is still coming in).
    pub near: bool,
    /// Columns wanted but not yet generated, at each level, and being
    /// generated now (`--opt startlog`).
    pub missing: [usize; LOD_LEVELS as usize],
    pub in_flight: usize,
    /// What the columns cost the workers (`--opt flybench`), at each level:
    /// how many were made, their meshing and their colliders (s of a
    /// worker's time), and how many were made again (dropped, then wanted).
    pub spent: [(u32, f32, f32, u32); LOD_LEVELS as usize],
    made: HashSet<Key>,
}

impl Streamer {
    /// Regenerates everything, for when the world has changed.
    pub fn reset(&mut self) {
        let spawned: Vec<_> = self.spawned.drain().collect();
        self.stale.extend(spawned);
        self.cache.clear();
        self.pending.clear();
        self.settled = false;
        self.near = false;
    }
}

fn setup_material(
    mut commands: Commands,
    args: Res<Args>,
    world: Res<WorldGen>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    // The ground may be paved; `glow` is the brightness of lit geometry.
    // (Its glow for the vaults' pillars' lines of light, the only lit
    // geometry in the ground.)
    let ground = surface(&args, &world, args.num("glow", 3000.0), args.num("paving", 0.0));
    let structures = surface(&args, &world, args.num("glow", 3000.0), 0.0);
    let mut level = |lod: u32| {
        let mut m = ground.clone();
        m.base.depth_bias = -LOD_DEPTH_BIAS * lod as f32;
        materials.add(m)
    };
    let levels = [level(0), level(1), level(2)];
    commands.insert_resource(TerrainMaterialHandle(levels));
    commands.insert_resource(StructureMaterialHandle(materials.add(structures)));
}

/// The ground's surface (the plates' pieces on their tops, panels on their
/// walls), for the `world`; `glow` the brightness of lit
/// geometry, `paving` the paving's pattern. Albedo comes from vertex
/// colours (red; green the plate's code, see terrain.wgsl); the material
/// only sets the surface response. Not bent round the camera until
/// `update_material` makes it so (the ground's and the structures' only).
pub fn surface(args: &Args, world: &WorldGen, glow: f32, paving: f32) -> TerrainMaterial {
    let size = world.size();
    // (The walls' preset, see `walls` below.)
    let preset = args.num("walls", 1.0).round();
    let glyphs = (preset == 2.0) as u8 as f32;
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            ..default()
        },
        extension: TerrainExtension {
            params: Vec4::new(if args.opt("noao") { 0.0 } else { 1.0 }, 0.0, 0.0, 0.0),
            // (The subdivision's numbers: `--set subdiv_split`,
            // `subdiv_largest`, `subdiv_tone`, `subdiv_tilt` and
            // `subdiv_joint`; the tone and tilt the canal stones'.)
            wrap: Vec4::new(
                size,
                if args.opt("nosubdiv") { 0.0 } else { 1.0 },
                args.num("subdiv_split", 0.8),
                args.num("subdiv_largest", 32.0),
            ),
            glow: Vec4::new(glow, args.num("subdiv_tone", 0.08), args.num("subdiv_tilt", 2.0), args.num("subdiv_joint", 0.35)),
            paving: Vec4::new(paving, args.num("tile", 2.0), args.num("paving_contrast", 0.18), args.num("paved", 0.45)),
            // (`--set subdiv_smallest`: the smaller pieces read as painted,
            // the user, between 8 and 32 m perhaps a district's own;
            // `subdiv_groove`, the joints' slope.)
            pieces: Vec4::new(args.num("subdiv_smallest", 8.0), args.num("subdiv_groove", 45.0), args.num("void_fade", 120.0), if crate::rt_shadow::enabled(args) { 1.0 + args.num("rt_debug", 0.0) } else { 0.0 }),
            // (`--set walls N`, the panels' presets while they are judged:
            // 0 none; 1 rectangles, the user's best (`wall_split` 1.5); 2
            // glyphs, some cuts left open, some curved (`wall_open` 0.25,
            // `wall_curve` 0.2), weirder. Each number its own too:
            // `wall_course`, `wall_joint`, `wall_tone`, `wall_stretch`,
            // `wall_split`, `wall_key`, `wall_open`, `wall_curve`.)
            walls: Vec4::new(
                (preset > 0.5) as u8 as f32,
                args.num("wall_course", 1.0),
                args.num("wall_joint", 0.06),
                args.num("wall_tone", 0.12),
            ),
            layers: Vec4::new(args.num("wall_stretch", 4.0), args.num("wall_split", 1.5), args.num("wall_key", 0.0), 0.0),
            marks: Vec4::new(args.num("wall_open", glyphs * 0.25), args.num("wall_curve", glyphs * 0.2), sink(far_tol(args), 1), sink(far_tol(args), 2)),
            round: Vec4::ZERO,
            far_inner: crate::far_shadow::FAR_MAPS[0].clone(),
            far_data: crate::far_shadow::FAR_DATA,
            far_outer: crate::far_shadow::FAR_MAPS[1].clone(),
            far_second: crate::far_shadow::FAR_MAPS[2].clone(),
            shell: crate::shell::SHELL.clone(),
            rt_mask: crate::rt_shadow::RT_MASK.clone(),
        },
    }
}

fn update_material(
    args: Res<Args>,
    terrain: Res<TerrainMaterialHandle>,
    structures: Res<StructureMaterialHandle>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
    anchor: Single<&Transform, With<StreamAnchor>>,
) {
    let curvature = if args.opt("flat") { 0.0 } else { 0.5 / PLANET_RADIUS };
    let p = anchor.translation;
    for handle in terrain.0.iter().chain([&structures.0]) {
        // (Written only when it changes: each change made Bevy check every
        // mesh drawn with it again, in every view (the main one, the
        // prepass, six shadow cascades with both suns up), every frame:
        // 2.3 ms of a frame on Vulkan at the user's spot standing still.
        // Moving, it still changes every frame: the camera belongs
        // somewhere that is not the material.)
        let Some(old) = materials.get(handle).map(|m| m.extension.params) else { continue };
        let new = Vec4::new(old.x, curvature, p.x, p.z);
        if new == old {
            continue;
        }
        let Some(mut material) = materials.get_mut(handle) else { continue };
        material.extension.params = new;
    }
}

fn wrap_key(key: IVec2, n: i32) -> IVec2 {
    IVec2::new(key.x.rem_euclid(n), key.y.rem_euclid(n))
}

/// Distance from `pos` to the centre of a column.
fn column_distance(lod: u32, key: IVec2, pos: Vec2) -> f32 {
    let size = column_size(lod);
    pos.distance((key.as_vec2() + 0.5) * size)
}

fn children(key: IVec2) -> impl Iterator<Item = IVec2> {
    let base = key * LOD_FACTOR;
    (0..LOD_FACTOR * LOD_FACTOR).map(move |i| base + IVec2::new(i % LOD_FACTOR, i / LOD_FACTOR))
}

struct Coverage<'a> {
    wanted: &'a [HashSet<IVec2>],
    cache: &'a HashMap<Key, Option<Built>>,
    world: &'a WorldGen,
}

impl Coverage<'_> {
    /// Whether a wanted column's area can be drawn: it is generated, or its
    /// children together cover it.
    fn ready(&self, lod: u32, key: IVec2) -> bool {
        self.wanted[lod as usize].contains(&key)
            && (self.cache.contains_key(&(lod, wrap_key(key, self.world.columns_per_side(lod))))
                || self.covered(lod, key))
    }

    /// Whether finer terrain is ready everywhere over this column.
    fn covered(&self, lod: u32, key: IVec2) -> bool {
        lod > 0 && children(key).all(|c| self.ready(lod - 1, c))
    }
}

fn stream_columns(
    args: Res<Args>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut streamer: ResMut<Streamer>,
    world: Res<WorldGen>,
    material: Res<TerrainMaterialHandle>,
    anchor: Single<&Transform, With<StreamAnchor>>,
) {
    // Borrow fields separately.
    let streamer = &mut *streamer;
    let pos = Vec2::new(anchor.translation.x, anchor.translation.z);

    // Columns wanted at each level: those touching the level's disc.
    let mut wanted: Vec<HashSet<IVec2>> = Vec::new();
    for lod in 0..LOD_LEVELS {
        let size = column_size(lod);
        let radius = lod_radius(lod);
        let center = (pos / size).floor().as_ivec2();
        let r = (radius / size).ceil() as i32 + 1;
        let mut set = HashSet::new();
        for dz in -r..=r {
            for dx in -r..=r {
                let key = center + IVec2::new(dx, dz);
                if column_distance(lod, key, pos) <= radius + size * 0.5 {
                    set.insert(key);
                }
            }
        }
        wanted.push(set);
    }

    // A coarse column is needed unless all its children are well inside the
    // finer disc (the margin makes it ready before they unload), and drawn
    // unless finer terrain covering it is ready.
    let ctx = Coverage { wanted: &wanted, cache: &streamer.cache, world: &world };
    let mut draw: HashSet<Key> = HashSet::new();
    let mut requests: Vec<(f32, Key)> = Vec::new();
    let mut settled = true;
    let mut near = true;
    let mut missing = [0; LOD_LEVELS as usize];
    for lod in 0..LOD_LEVELS {
        let n = world.columns_per_side(lod);
        for &key in &wanted[lod as usize] {
            if lod > 0 {
                let fine = lod - 1;
                let margin = column_size(lod);
                let needed = children(key)
                    .any(|c| column_distance(fine, c, pos) > lod_radius(fine) - margin);
                if !needed || ctx.covered(lod, key) {
                    continue;
                }
            }
            let wrapped = wrap_key(key, n);
            if streamer.cache.contains_key(&(lod, wrapped)) {
                draw.insert((lod, key));
            } else {
                settled = false;
                missing[lod as usize] += 1;
                near &= lod > 0 || column_distance(lod, key, pos) > NEAR;
                let urgency = column_distance(lod, key, pos) / column_size(lod);
                requests.push((urgency, (lod, wrapped)));
            }
        }
    }
    streamer.settled = settled;
    streamer.near = near;
    streamer.missing = missing;

    // Start the most urgent generation work.
    requests.sort_by(|a, b| a.0.total_cmp(&b.0));
    let pool = AsyncComputeTaskPool::get();
    let max_in_flight = pool.thread_num().max(1) * 2;
    for (_, key) in requests {
        if streamer.pending.len() >= max_in_flight {
            break;
        }
        if streamer.pending.contains_key(&key) {
            continue;
        }
        let world = world.clone();
        let task = pool.spawn(async move {
            let start = std::time::Instant::now();
            let column = world.mesh(key.0, key.1.x, key.1.y);
            let meshed = start.elapsed().as_secs_f32();
            let collider = (key.0 == 0).then(|| column_collider(&column)).flatten();
            let spent = [meshed, start.elapsed().as_secs_f32() - meshed];
            // (Ready to draw here, with its bounds: made so as it was taken
            // in, in the frame it came.)
            let shown = (!column.is_empty()).then(|| (bounds(&column), column)).map(|(aabb, column)| {
                let blocks = column.blocks.clone();
                (to_bevy_mesh(column, key.0), aabb, blocks)
            });
            (shown, collider, spent)
        });
        streamer.pending.insert(key, task);
    }

    // Despawn instances no longer drawn (including all of them right after the
    // camera wraps around the world; they respawn from the cache this frame):
    // faded out where another level takes over (see `Fading`), else at once.
    let now = time.elapsed_secs();
    let fades = args.num("lod_fade", 0.4) > 0.0;
    // (Whether a coarser level over a column, or a finer one under it.)
    let coarser = |(lod, key): Key, among: &dyn Fn(&Key) -> bool| lod + 1 < LOD_LEVELS && among(&(lod + 1, key.div_euclid(IVec2::splat(LOD_FACTOR as i32))));
    let finer = |(lod, key): Key, among: &dyn Fn(&Key) -> bool| lod > 0 && children(key).any(|c| among(&(lod - 1, c)));
    streamer.spawned.retain(|key, entity| {
        let keep = draw.contains(key);
        if !keep {
            let quiet = coarser(*key, &|k| draw.contains(k));
            if fades && (quiet || finer(*key, &|k| draw.contains(k))) {
                commands.entity(*entity).insert((Fading { since: now, out: true, quiet }, fade_tag(1), dithered()));
                if quiet {
                    commands.entity(*entity).insert(bevy::light::NotShadowCaster);
                }
            } else {
                commands.entity(*entity).despawn();
            }
        }
        keep
    });
    let shown: HashSet<Key> = streamer.spawned.keys().copied().collect();
    let far_cache = crate::far_shadow::enabled(&args);
    for &(lod, key) in &draw {
        if streamer.spawned.contains_key(&(lod, key)) {
            continue;
        }
        let n = world.columns_per_side(lod);
        let Some(Some(built)) = streamer.cache.get(&(lod, wrap_key(key, n))) else { continue };
        if let Some(old) = streamer.stale.remove(&(lod, key)) {
            commands.entity(old).despawn();
        }
        let size = column_size(lod);
        let sink = world.sink(lod);
        // (Fading in where it takes over from another level.)
        let quiet = coarser((lod, key), &|k| shown.contains(k));
        let fade_in = fades && (quiet || finer((lod, key), &|k| shown.contains(k)));
        let mut entity = commands.spawn((
            TerrainColumn,
            ColumnBlocks(built.blocks.clone()),
            Mesh3d(built.mesh.clone()),
            built.aabb,
            NoAutoAabb,
            MeshMaterial3d(material.0[lod as usize].clone()),
            Transform::from_xyz(key.x as f32 * size, -sink, key.y as f32 * size),
            fade_tag(if fade_in { -15 } else { 0 }),
        ));
        if fade_in {
            entity.insert((Fading { since: now, out: false, quiet }, dithered()));
            if quiet {
                entity.insert(bevy::light::NotShadowCaster);
            }
        }
        // The coarser levels' shadows from the cached far map (see
        // far_shadow.rs), not live; traced (SPIKE `--opt rt`), none live.
        if crate::rt_shadow::enabled(&args) {
            entity.insert(bevy::light::NotShadowCaster);
        } else if lod >= 1 && far_cache {
            entity.insert((bevy::light::NotShadowCaster, crate::far_shadow::FarCaster));
        }
        if let Some(collider) = &built.collider {
            // Character movement only collides with colliders on bodies.
            // (Of no density: a static body's mass unused, and a triangle
            // mesh's found by its integrals, ~57 ms on the frame it came,
            // again on the next; see vendor/avian3d's `mass_properties`.)
            entity.insert((RigidBody::Static, collider.clone(), avian3d::prelude::ColliderDensity(0.0)));
        }
        let entity = entity.id();
        streamer.spawned.insert((lod, key), entity);
    }

    if settled {
        for (_, old) in streamer.stale.drain() {
            commands.entity(old).despawn();
        }
    }

    // Forget meshes and cancel work for columns well outside their disc.
    let far = |(lod, key): &Key| {
        let n = world.columns_per_side(*lod);
        let size = column_size(*lod);
        let center = wrap_key((pos / size).floor().as_ivec2(), n);
        let d = (*key - center).abs();
        let d = IVec2::new(d.x.min(n - d.x), d.y.min(n - d.y));
        d.max_element() as f32 * size > lod_radius(*lod) + size * 3.0
    };
    streamer.cache.retain(|key, _| !far(key));
    streamer.pending.retain(|key, _| !far(key));
    streamer.in_flight = streamer.pending.len();
}

/// New meshes' bytes handed to the renderer a frame (`--set upload_mb`, 4).
/// Bevy packs and writes every mesh added in a frame in that frame's render
/// (`allocate_and_free_meshes`, about 1 ms a MB; its own per-frame limit
/// applies only after that, not to it): flying, a structure's ~30 MB at
/// once was a frame of 64 ms (the user's free flight, 2026-11-16).
/// Refilled each frame as terrain is taken in; terrain columns are counted
/// but never wait (the ground under you), structures wait for what is left
/// (`structures::receive`).
#[derive(Resource)]
pub struct MeshBudget {
    pub per_frame: isize,
    pub left: isize,
}

impl MeshBudget {
    /// `mesh` counted.
    pub fn spend(&mut self, mesh: &Mesh) {
        self.left = self.left.saturating_sub(mesh_bytes(mesh) as isize);
    }
}

/// The bytes of `mesh`'s vertices and indices.
pub fn mesh_bytes(mesh: &Mesh) -> usize {
    let vertices: usize = mesh.try_attributes().map_or(0, |a| a.map(|(_, v)| v.get_bytes().len()).sum());
    vertices + mesh.try_indices_option().ok().flatten().map_or(0, |i| i.len() * 4)
}

fn receive_meshes(mut streamer: ResMut<Streamer>, mut meshes: ResMut<Assets<Mesh>>, mut budget: ResMut<MeshBudget>) {
    budget.left = budget.per_frame;
    let mut done = Vec::new();
    for (key, task) in streamer.pending.iter_mut() {
        if let Some((shown, collider, cost)) = check_ready(task) {
            let built = shown.map(|(mesh, aabb, blocks)| {
                budget.spend(&mesh);
                Built { aabb, mesh: meshes.add(mesh), collider, blocks: Arc::new(blocks) }
            });
            done.push((*key, built, cost));
        }
    }
    for (key, built, cost) in done {
        let again = !streamer.made.insert(key);
        let spent = &mut streamer.spent[key.0 as usize];
        *spent = (spent.0 + 1, spent.1 + cost[0], spent.2 + cost[1], spent.3 + again as u32);
        streamer.pending.remove(&key);
        streamer.cache.insert(key, built);
    }
}

/// A mesh's bounds for culling. Bevy would compute them itself, but keeps
/// recomputing them for big meshes every frame (milliseconds with the
/// structures); these meshes never change, so they get them once, with
/// `NoAutoAabb`.
pub fn bounds(mesh: &ColumnMesh) -> Aabb {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for &p in &mesh.positions {
        lo = lo.min(Vec3::from(p));
        hi = hi.max(Vec3::from(p));
    }
    if lo.x > hi.x {
        return Aabb::from_min_max(Vec3::ZERO, Vec3::ZERO);
    }
    Aabb::from_min_max(lo, hi)
}

/// A static triangle-mesh collider for a column's geometry, if it has any.
pub fn column_collider(column: &ColumnMesh) -> Option<Collider> {
    if column.is_empty() {
        return None;
    }
    // (And what is walked on but not drawn: stairs' hidden slopes.)
    let base = column.positions.len() as u32;
    let vertices = column.positions.iter().chain(&column.walk).map(|&p| Vec3::from(p)).collect();
    let indices = column.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).chain(column.walk_indices.chunks_exact(3).map(|t| [base + t[0], base + t[1], base + t[2]])).collect();
    Collider::try_trimesh(vertices, indices)
        .inspect_err(|e| warn!("column collider failed: {e:?}"))
        .ok()
}

pub fn to_bevy_mesh(mut column: ColumnMesh, lod: u32) -> Mesh {
    // Albedo in red, the plate (see `plate_code`, `wall_code`) and the level
    // of detail drawing it in green, glow in blue, sky visibility in alpha
    // (read by terrain.wgsl). (The level, as a coarser one is lowered, see
    // `sink`: a wall's pattern is laid out where it stands, and changed as
    // it was approached, the user.)
    column.plate.resize(column.positions.len(), 0.0);
    for code in column.plate.iter_mut().filter(|c| **c > 0.0) {
        *code += 131072.0 * lod as f32;
    }
    column.glow.resize(column.positions.len(), 0.0);
    // (On every column, so all share one vertex layout: see far_shadow.rs.)
    column.below.resize(column.positions.len(), 0.0);
    column.under.resize(column.positions.len(), 0.0);
    let below: Vec<[f32; 2]> = column.below.iter().zip(&column.under).map(|(&b, &u)| [b, u]).collect();
    let colors: Vec<[f32; 4]> = column
        .albedo
        .iter()
        .zip(&column.ao)
        .zip(column.plate.iter().zip(&column.glow))
        .map(|((&a, &v), (&f, &g))| [a, f, g, v])
        .collect();
    // (In the render world alone: never read back here, their bounds their
    // own, see `bounds`. Kept in both, each was copied whole to the render
    // world as it came, ~6 ms a column, and held twice: 892 MB of them on
    // the GPU flying the stacks.)
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, column.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, column.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        // (A void wall's depth below its top, as the first texture
        // coordinate, and whether it is under the world, the second: see
        // `ColumnMesh::below`, `under`.)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, below)
        .with_inserted_indices(Indices::U32(column.indices))
}
