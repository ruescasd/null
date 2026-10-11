//! A pool of ichor in a basin (Atrium II's; an experiment, the user's: can
//! there be liquids at all, and a strange bubbling ichor in the fountain).
//! The creatures' blood, black and glossy as in `combat/ichor.rs`, thick and
//! slow: its surface a mesh over a grid of heights, each frame a damped wave
//! running between neighbours, so what disturbs it spreads in rings and the
//! lamps glint off them. Bubbles swell slowly out of it here and there,
//! domes of the same stuff glowing faintly from within, lifting the surface
//! round them; each bursts, a
//! dip that rings outward and a few drops flung up that fall back, each a
//! ring of its own.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use worldgen::noise::hash01;

/// Grid points across the pool.
const N: usize = 80;
/// How fast a ripple runs (m/s), and how fast it dies away (per second):
/// thick, slow.
const SPEED: f32 = 0.7;
const DAMPING: f32 = 1.6;
/// Bubbles, and drops in the air, at most.
const BUBBLES: usize = 6;
const DROPS: usize = 24;
const GRAVITY: f32 = 9.8;
/// How brightly the bubbles glow (the lab's lamps' boxes are 60).
const BUBBLE_GLOW: f32 = 1.2;

#[derive(Component)]
pub(crate) struct Pool {
    /// Its surface's middle at rest, and its radius.
    centre: Vec3,
    radius: f32,
    /// Heights and their speeds at the grid's points; which are the pool's.
    h: Vec<f32>,
    v: Vec<f32>,
    wet: Vec<bool>,
    mesh: Handle<Mesh>,
    bubbles: Vec<Bubble>,
    drops: Vec<Drop>,
    /// When the next bubble comes, and how many have.
    next: f32,
    count: i32,
}

/// A bubble swelling (where, as large as it gets, how old, how long it
/// lasts), its dome an entity; idle when it has no life.
struct Bubble {
    at: Vec2,
    size: f32,
    age: f32,
    life: f32,
    dome: Entity,
}

/// A drop in the air (where, how fast), an entity; idle below the surface.
struct Drop {
    at: Vec3,
    velocity: Vec3,
    size: f32,
    live: bool,
    ball: Entity,
}

impl Pool {
    fn dx(&self) -> f32 {
        2.0 * self.radius / (N - 1) as f32
    }

    /// The grid point nearest a point on the pool (from its middle).
    fn cell(&self, p: Vec2) -> Option<usize> {
        let f = (p + Vec2::splat(self.radius)) / self.dx();
        let (i, j) = (f.x.round() as i32, f.y.round() as i32);
        ((0..N as i32).contains(&i) && (0..N as i32).contains(&j)).then(|| i as usize + j as usize * N)
    }

    /// The surface's height at a point (from rest).
    fn height(&self, p: Vec2) -> f32 {
        self.cell(p).map_or(0.0, |k| self.h[k])
    }

    /// Pushes the surface's speed by `by` round `p`, falling off over
    /// `reach`.
    fn push(&mut self, p: Vec2, reach: f32, by: f32) {
        let dx = self.dx();
        let n = (reach / dx).ceil() as i32 + 1;
        let Some(c) = self.cell(p) else { return };
        let (ci, cj) = ((c % N) as i32, (c / N) as i32);
        for j in cj - n..=cj + n {
            for i in ci - n..=ci + n {
                if !(0..N as i32).contains(&i) || !(0..N as i32).contains(&j) {
                    continue;
                }
                let k = i as usize + j as usize * N;
                let d = Vec2::new((i - ci) as f32, (j - cj) as f32).length() * dx;
                if self.wet[k] && d < reach {
                    let fall = 0.5 + 0.5 * (std::f32::consts::PI * d / reach).cos();
                    self.v[k] += by * fall;
                }
            }
        }
    }
}

/// The pool in a basin: its surface `centre` (at rest), `radius` across;
/// its mesh reaching a little past (under the basin's rim).
pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, centre: Vec3, radius: f32) {
    let wet_material = materials.add(StandardMaterial { base_color: Color::srgb(0.01, 0.01, 0.01), perceptual_roughness: 0.12, reflectance: 0.7, ..default() });
    let reach = radius + 0.15;
    let dx = 2.0 * reach / (N - 1) as f32;
    let point = |i: usize, j: usize| Vec2::new(i as f32 * dx - reach, j as f32 * dx - reach);
    let wet: Vec<bool> = (0..N * N).map(|k| point(k % N, k / N).length() <= radius).collect();
    let shown: Vec<bool> = (0..N * N).map(|k| point(k % N, k / N).length() <= reach).collect();
    let mut indices = Vec::new();
    for j in 0..N - 1 {
        for i in 0..N - 1 {
            let k = i + j * N;
            if shown[k] && shown[k + 1] && shown[k + N] && shown[k + N + 1] {
                indices.extend_from_slice(&[k as u32, (k + N) as u32, (k + 1) as u32, (k + 1) as u32, (k + N) as u32, (k + N + 1) as u32]);
            }
        }
    }
    let positions: Vec<[f32; 3]> = (0..N * N).map(|k| { let p = point(k % N, k / N); [p.x, 0.0, p.y] }).collect();
    let mesh = meshes.add(
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; N * N])
            .with_inserted_indices(Indices::U32(indices)),
    );
    commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(wet_material.clone()), Transform::from_translation(centre)));
    // (The bubbles glow faintly from within, the creature's light through
    // its blood: black domes on the black surface hardly showed.)
    let glowing = materials.add(StandardMaterial { base_color: Color::srgb(0.01, 0.01, 0.01), perceptual_roughness: 0.12, reflectance: 0.7, emissive: LinearRgba::gray(BUBBLE_GLOW), ..default() });
    let ball = meshes.add(Sphere::new(1.0).mesh().uv(24, 12));
    let hidden = || Transform::from_translation(centre).with_scale(Vec3::ZERO);
    let bubbles = (0..BUBBLES)
        .map(|_| Bubble { at: Vec2::ZERO, size: 0.0, age: 0.0, life: 0.0, dome: commands.spawn((Mesh3d(ball.clone()), MeshMaterial3d(glowing.clone()), hidden())).id() })
        .collect();
    let drops = (0..DROPS)
        .map(|_| Drop { at: Vec3::ZERO, velocity: Vec3::ZERO, size: 0.0, live: false, ball: commands.spawn((Mesh3d(ball.clone()), MeshMaterial3d(wet_material.clone()), hidden())).id() })
        .collect();
    // (Its grid as the simulation's: the same spacing over the shown
    // reach, so `cell` finds the mesh's points.)
    commands.spawn(Pool { centre, radius: reach, h: vec![0.0; N * N], v: vec![0.0; N * N], wet, mesh, bubbles, drops, next: 0.5, count: 0 });
}

/// The pools' motion: the waves, the bubbles swelling and bursting, the
/// drops; the surface's mesh moved to match.
pub(crate) fn animate(time: Res<Time>, mut pools: Query<&mut Pool>, mut things: Query<&mut Transform, Without<Pool>>, mut meshes: ResMut<Assets<Mesh>>) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let t = time.elapsed_secs();
    for mut pool in &mut pools {
        let pool = &mut *pool;
        // A new bubble now and then, somewhere well in from the rim: most
        // small and quick, now and then a large slow one.
        pool.next -= dt;
        if pool.next <= 0.0 {
            pool.count += 1;
            let count = pool.count;
            let r = |a: i32, b: i32| hash01(count, a, b, 0x1c4);
            pool.next = 0.4 + 1.8 * r(1, 0);
            if let Some(b) = pool.bubbles.iter_mut().find(|b| b.life <= 0.0) {
                let (angle, d) = (r(2, 0) * std::f32::consts::TAU, (pool.radius - 0.3).max(0.1) * 0.8 * r(3, 0).sqrt());
                let big = r(4, 0) < 0.12;
                b.at = Vec2::new(angle.cos(), angle.sin()) * d;
                b.size = if big { 0.22 + 0.12 * r(5, 0) } else { 0.04 + 0.1 * r(5, 0) };
                b.life = if big { 3.0 + 2.0 * r(6, 0) } else { 0.8 + 1.4 * r(6, 0) };
                b.age = 0.0;
            }
        }
        // Bubbles: swelling (eased), lifting the surface round them; at the
        // end, burst.
        let mut bursts = Vec::new();
        for i in 0..pool.bubbles.len() {
            let b = &mut pool.bubbles[i];
            if b.life <= 0.0 {
                continue;
            }
            b.age += dt;
            let f = (b.age / b.life).min(1.0);
            let size = b.size * (f * f * (3.0 - 2.0 * f)).max(0.02);
            let (at, total, done) = (b.at, b.size, b.age >= b.life);
            if done {
                b.life = 0.0;
                bursts.push((at, total));
            }
            let dome = b.dome;
            let lift = pool.height(at);
            if let Ok(mut tr) = things.get_mut(dome) {
                *tr = if done { Transform::from_translation(pool.centre).with_scale(Vec3::ZERO) } else { Transform::from_translation(pool.centre + Vec3::new(at.x, lift - size * 0.35, at.y)).with_scale(Vec3::new(size, size * 0.92, size)) };
            }
            if !done {
                pool.push(at, size * 2.0, size * 0.6 * dt * 60.0 * 0.05);
            }
        }
        // A burst: a dip ringing outward; drops flung up.
        for (k, (at, size)) in bursts.into_iter().enumerate() {
            pool.push(at, size * 1.6, -size * 6.0);
            let n = 2 + (size * 25.0) as i32;
            for d in 0..n {
                let rr = |c: i32| hash01(pool.count * 7 + k as i32, d, c, 0x1c5);
                if let Some(drop) = pool.drops.iter_mut().find(|d| !d.live) {
                    let angle = rr(0) * std::f32::consts::TAU;
                    let out = 0.2 + 0.6 * rr(1);
                    drop.at = Vec3::new(at.x, 0.02, at.y);
                    drop.velocity = Vec3::new(angle.cos() * out, 0.8 + (1.0 + size * 6.0) * rr(2), angle.sin() * out);
                    drop.size = 0.008 + 0.018 * rr(3) * (0.5 + size * 3.0);
                    drop.live = true;
                }
            }
        }
        // Drops: falling; back in the surface, a little ring.
        let mut landed = Vec::new();
        for drop in pool.drops.iter_mut().filter(|d| d.live) {
            drop.velocity.y -= GRAVITY * dt;
            drop.at += drop.velocity * dt;
            if drop.at.y < 0.0 && drop.velocity.y < 0.0 {
                drop.live = false;
                landed.push((Vec2::new(drop.at.x, drop.at.z), drop.size));
            }
        }
        for (at, size) in landed {
            if at.length() < pool.radius - 0.2 {
                pool.push(at, 0.12, -size * 30.0);
            }
        }
        for drop in &pool.drops {
            if let Ok(mut tr) = things.get_mut(drop.ball) {
                *tr = if drop.live {
                    // (Stretched along its flight, as the blood's are.)
                    let along = drop.velocity.normalize_or_zero();
                    let stretch = 1.0 + drop.velocity.length() * 0.25;
                    Transform::from_translation(pool.centre + drop.at).with_rotation(Quat::from_rotation_arc(Vec3::Y, if along == Vec3::ZERO { Vec3::Y } else { along })).with_scale(Vec3::new(drop.size, drop.size * stretch, drop.size))
                } else {
                    Transform::from_translation(pool.centre).with_scale(Vec3::ZERO)
                };
            }
        }
        // The waves: two steps a frame; a slow swell under it all, faint.
        let dx = pool.dx();
        let k2 = SPEED * SPEED / (dx * dx);
        for _ in 0..2 {
            let h = dt * 0.5;
            for j in 1..N - 1 {
                for i in 1..N - 1 {
                    let k = i + j * N;
                    if !pool.wet[k] {
                        continue;
                    }
                    let lap = pool.h[k - 1] + pool.h[k + 1] + pool.h[k - N] + pool.h[k + N] - 4.0 * pool.h[k];
                    pool.v[k] += k2 * lap * h;
                }
            }
            for k in 0..N * N {
                if pool.wet[k] {
                    pool.v[k] *= 1.0 - DAMPING * h;
                    pool.h[k] += pool.v[k] * h;
                } else {
                    pool.h[k] = 0.0;
                    pool.v[k] = 0.0;
                }
            }
        }
        let swell = |p: Vec2| 0.004 * ((p.x * 1.3 + t * 0.4).sin() + (p.y * 1.1 - t * 0.33).sin());
        // The mesh: the heights, and normals from their slopes.
        let reach = pool.radius;
        let point = |k: usize| Vec2::new((k % N) as f32 * dx - reach, (k / N) as f32 * dx - reach);
        let height = |k: usize| pool.h[k] + if pool.wet[k] { swell(point(k)) } else { 0.0 };
        let positions: Vec<[f32; 3]> = (0..N * N).map(|k| { let p = point(k); [p.x, height(k), p.y] }).collect();
        let normals: Vec<[f32; 3]> = (0..N * N)
            .map(|k| {
                let (i, j) = (k % N, k / N);
                let hx = height(if i + 1 < N { k + 1 } else { k }) - height(if i > 0 { k - 1 } else { k });
                let hz = height(if j + 1 < N { k + N } else { k }) - height(if j > 0 { k - N } else { k });
                Vec3::new(-hx, 2.0 * dx, -hz).normalize().to_array()
            })
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&pool.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        }
    }
}
