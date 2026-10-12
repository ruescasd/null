//! Roots out of a painting (the user's intrusion, after a bleeding one did
//! not work: plant-like growth descending from it). They come out of the
//! picture's lower part, thick there, and grow down over the frame's steps
//! and the wall, hugging each face and ledge (off an overhang, hanging
//! straight down through the air), and out across the floor, wandering,
//! branching, thinning to fine tips. Grown in the unfolded surface they run
//! on (`B::lay_roots` traces it, a profile from the picture down to the
//! floor and out across it): a root is a walk across it, across (`u`) and
//! along (`t`), mapped back onto it.

use bevy::prelude::*;
use worldgen::noise::hash01;

/// A step of a root's walk; how thin its tips get.
const STEP: f32 = 0.04;
const TIP: f32 = 0.003;
/// How far out across the floor the profile runs.
pub(crate) const FLOOR: f32 = 1.8;

/// A root: its points, its thickness at its start and its tip.
pub(crate) type Root = (Vec<Vec3>, (f32, f32));

/// The roots grown over `profile` (points (n, y): into the wall, up, from
/// the picture's middle, down to the floor and out across it, its last
/// segment the floor), within `half` across while on the wall; mapped by
/// `at(u, n, y)`.
pub(crate) fn grow(profile: &[(f32, f32)], half: f32, seed: u32, at: &dyn Fn(f32, f32, f32) -> Vec3) -> Vec<Root> {
    let mut along = vec![0.0];
    for w in profile.windows(2) {
        along.push(along.last().unwrap() + Vec2::new(w[1].0 - w[0].0, w[1].1 - w[0].1).length());
    }
    let total = *along.last().unwrap();
    let floor = total - FLOOR;
    // Where `t` along the profile is, lifted off what it runs on by `r` (a
    // face: out of it; a ledge or the floor: up off it).
    let place = |u: f32, t: f32, r: f32| {
        let i = along.partition_point(|&a| a <= t).clamp(1, profile.len() - 1);
        let (p, q) = (profile[i - 1], profile[i]);
        let k = ((t - along[i - 1]) / (along[i] - along[i - 1]).max(1e-4)).clamp(0.0, 1.0);
        let (n, y) = (p.0 + (q.0 - p.0) * k, p.1 + (q.1 - p.1) * k);
        if (q.1 - p.1).abs() > (q.0 - p.0).abs() { at(u, n - r, y) } else { at(u, n, y + r) }
    };
    let mut count = 0;
    let mut rand = || {
        count += 1;
        hash01(seed as i32, count, 0, 0x2007)
    };
    let mut roots = Vec::new();
    // The main roots, spread across the picture's lower edge, thick there.
    let mains = 3 + (rand() * 3.0) as i32;
    let mut todo: Vec<(f32, f32, f32, f32, f32, u32)> = (0..mains)
        .map(|k| {
            let u = ((k as f32 + 0.5) / mains as f32 * 2.0 - 1.0) * half * 0.55 + (rand() - 0.5) * 0.15;
            (u, 0.0, (rand() - 0.5) * 0.3, 0.022 + 0.022 * rand(), floor + 0.5 + 1.1 * rand(), 0)
        })
        .collect();
    while let Some((mut u, mut t, mut a, r, length, depth)) = todo.pop() {
        let mut walk = vec![(u, t)];
        let mut s = 0.0;
        while s < length {
            let ground = t >= floor;
            // (Down the wall, pulled straight down; across the floor, free,
            // but never back to the wall.)
            a += (rand() - 0.5) * 0.4;
            a = if ground { a.clamp(-1.3, 1.3) } else { a * 0.85 };
            u += a.sin() * STEP;
            t += a.cos() * STEP;
            if !ground {
                u = u.clamp(-half, half);
            }
            if t >= total {
                break;
            }
            s += STEP;
            walk.push((u, t));
            // Branches: more on the floor, two generations at most.
            let left = length - s;
            if depth < 2 && left > 0.3 && rand() < if ground { 0.1 } else { 0.035 } {
                let turn = (0.5 + 0.6 * rand()) * if rand() < 0.5 { -1.0 } else { 1.0 };
                let here = r + (TIP - r) * s / length;
                todo.push((u, t, a + turn, here * 0.65, left * (0.4 + 0.3 * rand()), depth + 1));
            }
        }
        if walk.len() < 2 {
            continue;
        }
        let n = walk.len() - 1;
        let points = walk.iter().enumerate().map(|(i, &(u, t))| place(u, t, r + (TIP - r) * i as f32 / n as f32)).collect();
        roots.push((points, (r, TIP)));
    }
    roots
}

/// The roots put in the world, one mesh: dark, a little glossy.
pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, roots: &[Root]) {
    if roots.is_empty() {
        return;
    }
    let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    crate::rope::tubes(&mut mesh, roots.iter().map(|(p, w)| (&p[..], *w, 1.0)));
    let material = materials.add(StandardMaterial { base_color: Color::srgb(0.035, 0.035, 0.035), perceptual_roughness: 0.45, reflectance: 0.5, ..default() });
    commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), Transform::IDENTITY));
}
