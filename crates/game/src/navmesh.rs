//! Navigation meshes (the user, 2026-10-31: adopt the industry's own, Recast
//! and Detour, not our map of spots, after creatures pressed for ever into
//! ledges their map and their bodies disagreed about). Recast (a Rust port
//! of it, `rerecast`, patched in `vendor/`) builds a built place's
//! navmeshes from its colliders, the very shapes a creature collides with,
//! one for each body (`Body`), in tiles, in the background; landmass, in
//! Detour's place, finds ways across them: hunters by agents of its
//! (`Walker`), whatever else by asking (`Floors`).
//!
//! The place hands over its colliders and what to check is reached
//! (`Navmeshes::ask`); built, every check is logged (`--opt navdraw` draws
//! the bipeds' mesh). Until then (a few seconds), and off them, a creature
//! goes as in the open.

use std::{collections::HashMap, sync::Arc, time::Instant};

use avian3d::prelude::Collider;
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use bevy_landmass::{
    Agent3dBundle, AgentDesiredVelocity3d, AgentSettings, AgentState, AgentTarget3d, Archipelago3d, ArchipelagoOptions, ArchipelagoRef3d, FromAgentRadius, Island, Island3dBundle, Landmass3dPlugin, NavMesh3d, NavMeshHandle, PointSampleDistance3d, ValidNavigationMesh3d, Velocity3d, coords::ThreeD,
};

use crate::Args;

pub struct NavmeshPlugin;

impl Plugin for NavmeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Landmass3dPlugin::default()).add_systems(Update, (ready, walk.after(ready)));
    }
}

/// The bodies a place's navmeshes are built for, a mesh each: bipeds, and
/// the wider beasts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Body {
    Biped,
    Beast,
}

/// The highest step any body's mesh climbs: less than a creature's body
/// does (`combat::CREATURE_STEP`), or it is sent where it cannot go (the
/// gladiator, pressed into a ledge for ever, the user, 2026-10-31).
pub(crate) const CLIMB: f32 = 0.6;

impl Body {
    pub(crate) const ALL: [Body; 2] = [Body::Biped, Body::Beast];

    /// Its radius, height, the highest step it climbs and the steepest
    /// slope it walks (degrees): its column against others', its height
    /// (the gladiator's, the tallest; a beast's capsule, lifted a step).
    pub(crate) fn size(self) -> (f32, f32, f32, f32) {
        match self {
            Body::Biped => (0.35, 2.6, CLIMB, 45.0),
            Body::Beast => (0.45, 2.2, CLIMB, 45.0),
        }
    }
}

/// Where a body can go in a built place: its navmesh's tiles, as landmass
/// has them, to be asked of.
pub(crate) struct Floors {
    archipelago: landmass::Archipelago<ThreeD>,
    tiles: Vec<Arc<ValidNavigationMesh3d>>,
    polygons: usize,
    /// Its triangles, to be drawn (`--opt navdraw`).
    drawn: Vec<[f32; 3]>,
}

impl Floors {
    /// Over `tiles`, for a body of `radius`.
    pub(crate) fn new(tiles: &[bevy_rerecast::Navmesh], radius: f32, draw: bool) -> Result<Floors, String> {
        let mut archipelago = landmass::Archipelago::new(options(radius));
        let mut valid = Vec::new();
        let mut drawn = Vec::new();
        for (k, tile) in tiles.iter().enumerate() {
            let mesh = Arc::new(landmass_rerecast::convert_rerecast_navmesh_to_landmass_navmesh(tile).validate().map_err(|e| format!("tile {k}: {e:?}"))?);
            archipelago.add_island(landmass::Island::new(landmass::Transform { translation: Vec3::ZERO, rotation: 0.0 }, mesh.clone()));
            valid.push(mesh);
            if draw {
                let detail = &tile.detail;
                for sub in &detail.meshes {
                    let verts = &detail.vertices[sub.base_vertex_index as usize..][..sub.vertex_count as usize];
                    for tri in &detail.triangles[sub.base_triangle_index as usize..][..sub.triangle_count as usize] {
                        drawn.extend(tri.iter().map(|&i| (Vec3::from(verts[i as usize]) + Vec3::Y * 0.06).to_array()));
                    }
                }
            }
        }
        // (Its islands joined, edge to edge.)
        archipelago.update(0.0);
        Ok(Floors { archipelago, tiles: valid, polygons: tiles.iter().map(|t| t.polygon.polygon_count()).sum(), drawn })
    }

    fn sample(&self, p: Vec3, reach: f32, above: f32) -> Option<landmass::SampledPoint<'_, ThreeD>> {
        self.archipelago.sample_point(p, &PointSampleDistance3d { horizontal_distance: reach, distance_above: above, distance_below: 6.0, vertical_preference_ratio: 2.0, animation_link_max_vertical_distance: 0.5 }).ok()
    }

    /// The floor nearest `p`: no more than `reach` off it across, a metre
    /// over it, or 6 m under it.
    pub(crate) fn on(&self, p: Vec3, reach: f32) -> Option<Vec3> {
        self.sample(p, reach, 1.0).map(|s| s.point())
    }

    /// The floor a body standing at `p` (its feet) is on, or gets onto:
    /// as `on`, but no more than a step over it. (Its way begun on one a
    /// metre up, beside the flight it stood on, sent the gladiator at its
    /// edge from below, the user, 2026-10-31.)
    pub(crate) fn standing(&self, p: Vec3, reach: f32) -> Option<Vec3> {
        self.sample(p, reach, CLIMB).map(|s| s.point())
    }

    /// The way from a body's feet at `from` to `to`, its corners, the last
    /// `to`'s floor; none if there is none (or either is off the floors).
    pub(crate) fn way(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let (a, b) = (self.sample(from, 1.5, CLIMB)?, self.sample(to, 1.5, 1.0)?);
        let steps = self.archipelago.find_path(&a, &b, &HashMap::new(), landmass::PermittedAnimationLinks::All).ok()?;
        Some(
            steps
                .into_iter()
                .map(|s| match s {
                    landmass::PathStep::Waypoint(p) => p,
                    landmass::PathStep::AnimationLink { end_point, .. } => end_point,
                })
                .collect(),
        )
    }

    /// How far along the way from `from` to `to` (none, none).
    pub(crate) fn way_length(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let way = self.way(from, to)?;
        Some(std::iter::once(from).chain(way).collect::<Vec<_>>().windows(2).map(|w| w[0].distance(w[1])).sum())
    }
}

/// How an archipelago for a body of `radius` finds things on it: a body's
/// feet within 1.5 m across of its floor (in a corner, it stands nearer a
/// wall than its mesh comes), a goal 3 m over its floor (your eyes).
fn options(radius: f32) -> ArchipelagoOptions<ThreeD> {
    ArchipelagoOptions { point_sample_distance: PointSampleDistance3d { horizontal_distance: 1.5, distance_above: 1.0, distance_below: 3.0, ..PointSampleDistance3d::from_agent_radius(radius) }, ..ArchipelagoOptions::from_agent_radius(radius) }
}

/// What a place asks to be checked reached once its navmeshes are built:
/// in groups, each named, each check from a point to another.
pub(crate) type Checks = Vec<(String, Vec<(String, Vec3, Vec3)>)>;

/// A built place's navmeshes: being built, then each body's floors, and its
/// archipelago for the agents.
#[derive(Resource)]
pub(crate) struct Navmeshes {
    place: &'static str,
    floors: [Option<Floors>; 2],
    archipelagos: [Option<Entity>; 2],
    building: Option<Task<Result<Vec<Floors>, String>>>,
    asked: Instant,
    checks: Checks,
}

impl Navmeshes {
    /// Built for `place` over `colliders` (those it is made of, see
    /// `chasm::solid`) in the background; `checks` then checked.
    pub(crate) fn ask(place: &'static str, colliders: Vec<Collider>, checks: Checks, draw: bool) -> Navmeshes {
        let building = AsyncComputeTaskPool::get().spawn(async move { Body::ALL.into_iter().map(|b| Floors::new(&build(&colliders, b.size(), bounds(&colliders))?, b.size().0, draw && b == Body::Biped)).collect() });
        Navmeshes { place, floors: [None, None], archipelagos: [None, None], building: Some(building), asked: Instant::now(), checks }
    }

    /// A body's floors, once built.
    pub(crate) fn floors(&self, body: Body) -> Option<&Floors> {
        self.floors[body as usize].as_ref()
    }
}

/// Round all of `colliders`, a metre more.
pub(crate) fn bounds(colliders: &[Collider]) -> (Vec3, Vec3) {
    use avian3d::prelude::SimpleCollider;
    colliders.iter().map(|c| c.aabb(Vec3::ZERO, Quat::IDENTITY)).fold((Vec3::MAX, Vec3::MIN), |(lo, hi), b| (lo.min(b.min - Vec3::ONE), hi.max(b.max + Vec3::ONE)))
}

/// Each group of `checks` on `floors`: how many, and those not reached.
pub(crate) fn unreached<'a>(floors: &Floors, checks: &'a Checks) -> Vec<(&'a str, usize, Vec<&'a str>)> {
    checks.iter().map(|(group, list)| (group.as_str(), list.len(), list.iter().filter(|(_, a, b)| floors.way(*a, *b).is_none()).map(|(n, ..)| n.as_str()).collect())).collect()
}

/// Built: each body's archipelago, an island for each of its tiles; how
/// long it took, how big, and what of the checks was not reached; its
/// triangles drawn, asked for.
fn ready(mut commands: Commands, args: Res<Args>, navs: Option<ResMut<Navmeshes>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut navmeshes: ResMut<Assets<NavMesh3d>>) {
    let Some(mut navs) = navs else { return };
    let Some(task) = navs.building.as_mut() else { return };
    let Some(built) = block_on(poll_once(task)) else { return };
    navs.building = None;
    let built = built.unwrap_or_else(|e| panic!("{}: its navmeshes not built: {e}", navs.place));
    let bad = bevy_rerecast::rerecast::BAD_TRIANGULATIONS.load(std::sync::atomic::Ordering::Relaxed);
    info!("{}: navmeshes built in {:.2} s{}", navs.place, navs.asked.elapsed().as_secs_f32(), if bad > 0 { format!(" ({bad} contours not triangulated whole)") } else { String::new() });
    for (floors, body) in built.into_iter().zip(Body::ALL) {
        let failed = unreached(&floors, &navs.checks);
        info!(
            "{}: navmesh for the {body:?}: {} tiles, {} polygons; {}",
            navs.place,
            floors.tiles.len(),
            floors.polygons,
            failed.iter().map(|(group, n, lost)| format!("{group}: {} of {n} not reached{}", lost.len(), lost.iter().take(6).map(|l| format!(" ({l})")).collect::<String>())).collect::<Vec<_>>().join("; ")
        );
        let archipelago = commands.spawn(Archipelago3d::new(options(body.size().0))).id();
        for tile in &floors.tiles {
            commands.spawn(Island3dBundle { island: Island, archipelago_ref: ArchipelagoRef3d::new(archipelago), nav_mesh: NavMeshHandle(navmeshes.add(NavMesh3d { nav_mesh: tile.clone() })) });
        }
        if args.opt("navdraw") && !floors.drawn.is_empty() {
            let drawn = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, floors.drawn.clone()).with_computed_flat_normals();
            let material = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 1.0, 0.4), unlit: true, cull_mode: None, ..default() });
            commands.spawn((Mesh3d(meshes.add(drawn)), MeshMaterial3d(material), Transform::IDENTITY));
        }
        navs.archipelagos[body as usize] = Some(archipelago);
        navs.floors[body as usize] = Some(floors);
    }
}

/// A creature steered across a place's navmesh by an agent of landmass's:
/// where it is (its feet), where it is going, how fast it goes, its body;
/// and what the agent says of its way. (Its agent an entity of its own,
/// gone once this is.)
#[derive(Component)]
pub(crate) struct Walker {
    body: Body,
    pub(crate) at: Vec3,
    pub(crate) goal: Vec3,
    pub(crate) velocity: Vec3,
    pub(crate) speed: f32,
    pub(crate) way: Way,
}

impl Walker {
    pub(crate) fn new(body: Body, at: Vec3, speed: f32) -> Walker {
        Walker { body, at, goal: at, velocity: Vec3::ZERO, speed, way: Way::Waiting }
    }
}

/// What its agent says of a walker's way.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Way {
    /// No agent yet (its place's mesh being built).
    Waiting,
    /// On its way: which way to go (and how fast).
    Moving(Vec3),
    /// At its goal.
    There,
    /// It is off the floors, or its goal is, or there is no way there.
    NoWay,
}

/// A walker's agent.
#[derive(Component)]
struct AgentOf(Entity);

/// Each walker's agent put where it is, heading where it heads, and what
/// it says of its way put on the walker; an agent for each without one,
/// once its body's archipelago is in; gone once its walker is.
#[allow(clippy::type_complexity)]
fn walk(mut commands: Commands, navs: Option<Res<Navmeshes>>, mut walkers: Query<(Entity, &mut Walker)>, mut agents: Query<(Entity, &AgentOf, &mut Transform, &mut AgentTarget3d, &mut Velocity3d, &mut AgentSettings, &AgentState, &AgentDesiredVelocity3d)>) {
    let mut walking = std::collections::HashSet::new();
    for (agent, of, mut at, mut target, mut velocity, mut settings, state, desired) in &mut agents {
        let Ok((_, mut w)) = walkers.get_mut(of.0) else {
            commands.entity(agent).despawn();
            continue;
        };
        walking.insert(of.0);
        at.translation = w.at;
        *target = AgentTarget3d::Point(w.goal);
        velocity.velocity = w.velocity;
        if settings.desired_speed != w.speed {
            settings.desired_speed = w.speed;
            settings.max_speed = w.speed * 1.5;
        }
        w.way = match state {
            AgentState::Moving => Way::Moving(desired.velocity()),
            AgentState::ReachedTarget => Way::There,
            _ => Way::NoWay,
        };
    }
    let Some(navs) = navs else { return };
    for (entity, w) in &walkers {
        if walking.contains(&entity) {
            continue;
        }
        let Some(archipelago) = navs.archipelagos[w.body as usize] else { continue };
        commands.spawn((
            Agent3dBundle { agent: default(), archipelago_ref: ArchipelagoRef3d::new(archipelago), settings: AgentSettings { radius: w.body.size().0, desired_speed: w.speed, max_speed: w.speed * 1.5 } },
            Transform::from_translation(w.at),
            AgentOf(entity),
        ));
    }
}

/// How many cells along a tile's side (58 m for the gladiator's body).
const TILE: u16 = 500;

/// The navmesh for a body of `(radius, height, climb, slope)` over the
/// triangles of `colliders`, within `aabb` (min, max), in tiles: each tile
/// with anything in it built by Recast's steps (as `bevy_rerecast`'s
/// generator takes them), on every core; neighbouring tiles' edges meet
/// (landmass joins them). (Built whole, the arena's 1200 m took 56 s, and
/// the region's 2 km would not build at all.)
pub(crate) fn build(colliders: &[avian3d::prelude::Collider], body: (f32, f32, f32, f32), aabb: (Vec3, Vec3)) -> Result<Vec<bevy_rerecast::Navmesh>, String> {
    use avian_rerecast::ColliderToTriMesh;
    use avian3d::prelude::{Position, Rotation};
    use bevy_rerecast::rerecast;
    let (radius, height, climb, slope) = body;
    let config = |min: Vec3, max: Vec3| {
        rerecast::ConfigBuilder { agent_height: height, agent_radius: radius, walkable_climb: climb, walkable_slope_angle: slope.to_radians(), tile_size: TILE, tiling: true, aabb: rerecast::Aabb3d { min, max }, ..default() }.build()
    };
    let mut all = rerecast::TriMesh::default();
    for c in colliders {
        all.extend(c.to_trimesh(Position(Vec3::ZERO), Rotation::IDENTITY, 8).ok_or("a collider of no triangles")?);
    }
    let probe = config(aabb.0, aabb.1);
    all.mark_walkable_triangles(probe.walkable_slope_angle);
    let (cell, border) = (probe.cell_size, probe.border_size as f32 * probe.cell_size);
    let side = TILE as f32 * cell;
    let count = (((aabb.1.x - aabb.0.x) / side).ceil() as i32, ((aabb.1.z - aabb.0.z) / side).ceil() as i32);
    // Each triangle in every tile it touches, its border with it.
    let mut tiles: Vec<Vec<u32>> = vec![Vec::new(); (count.0 * count.1) as usize];
    for (i, t) in all.indices.iter().enumerate() {
        let v = [all.vertices[t.x as usize], all.vertices[t.y as usize], all.vertices[t.z as usize]];
        let (lo, hi) = (v[0].min(v[1]).min(v[2]), v[0].max(v[1]).max(v[2]));
        if hi.y < aabb.0.y || lo.y > aabb.1.y {
            continue;
        }
        let range = |lo: f32, hi: f32, min: f32, n: i32| (((lo - border - min) / side).floor().max(0.0) as i32, ((hi + border - min) / side).floor().min(n as f32 - 1.0) as i32);
        let (x0, x1) = range(lo.x, hi.x, aabb.0.x, count.0);
        let (z0, z1) = range(lo.z, hi.z, aabb.0.z, count.1);
        for z in z0..=z1 {
            for x in x0..=x1 {
                tiles[(z * count.0 + x) as usize].push(i as u32);
            }
        }
    }
    let e = |e: &dyn std::fmt::Debug| format!("{e:?}");
    let one = |k: usize, triangles: &[u32]| -> Result<Option<bevy_rerecast::Navmesh>, String> {
        let (x, z) = ((k as i32 % count.0) as f32, (k as i32 / count.0) as f32);
        let min = Vec3::new(aabb.0.x + x * side, aabb.0.y, aabb.0.z + z * side);
        let config = config(min - Vec3::new(border, 0.0, border), Vec3::new(min.x + side + border, aabb.1.y, min.z + side + border));
        // (Its own triangles, and only their vertices.)
        let mut trimesh = rerecast::TriMesh::default();
        let mut index: HashMap<u32, u32> = HashMap::new();
        for &i in triangles {
            let t = all.indices[i as usize].to_array().map(|v| {
                *index.entry(v).or_insert_with(|| {
                    trimesh.vertices.push(all.vertices[v as usize]);
                    trimesh.vertices.len() as u32 - 1
                })
            });
            trimesh.indices.push(t.into());
            trimesh.area_types.push(all.area_types[i as usize]);
        }
        let mut heightfield = rerecast::HeightfieldBuilder { aabb: config.aabb, cell_size: config.cell_size, cell_height: config.cell_height }.build().map_err(|x| e(&x))?;
        heightfield.rasterize_triangles(&trimesh, config.walkable_climb).map_err(|x| e(&x))?;
        heightfield.filter_low_hanging_walkable_obstacles(config.walkable_climb);
        heightfield.filter_ledge_spans(config.walkable_height, config.walkable_climb);
        heightfield.filter_walkable_low_height_spans(config.walkable_height);
        let mut compact = heightfield.into_compact(config.walkable_height, config.walkable_climb).map_err(|x| e(&x))?;
        compact.erode_walkable_area(config.walkable_radius);
        compact.build_distance_field();
        compact.build_regions(config.border_size, config.min_region_area, config.merge_region_area).map_err(|x| e(&x))?;
        let contours = compact.build_contours(config.max_simplification_error, config.max_edge_len, config.contour_flags);
        let polygon = contours.into_polygon_mesh(config.max_vertices_per_polygon).map_err(|x| e(&x))?;
        if polygon.polygon_count() == 0 {
            return Ok(None);
        }
        let detail = rerecast::DetailNavmesh::new(&polygon, &compact, config.detail_sample_dist, config.detail_sample_max_error).map_err(|x| e(&x))?;
        let settings = bevy_rerecast::NavmeshSettings { walkable_climb: climb, walkable_slope_angle: slope.to_radians(), ..bevy_rerecast::NavmeshSettings::from_agent_3d(radius, height) };
        Ok(Some(bevy_rerecast::Navmesh { polygon, detail, settings }))
    };
    // (Half the cores less one, each taking the next tile: the game's own
    // threads left the rest. On every core, the region's frames took 50 to
    // 250 ms while it was built, 6 s.)
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).saturating_sub(1).max(1));
    let built: Vec<Result<Vec<bevy_rerecast::Navmesh>, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if k >= tiles.len() {
                            return Ok(out);
                        }
                        if !tiles[k].is_empty() {
                            out.extend(one(k, &tiles[k]).map_err(|x| format!("tile {k}: {x}"))?);
                        }
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("a tile's worker")).collect()
    });
    Ok(built.into_iter().collect::<Result<Vec<_>, _>>()?.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    /// No body's mesh climbs a step higher than a creature's body does.
    #[test]
    fn steps_climbable() {
        assert!(super::CLIMB < crate::combat::CREATURE_STEP, "the navmeshes climb {} m; a creature {} m", super::CLIMB, crate::combat::CREATURE_STEP);
    }
}
