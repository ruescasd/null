//! The brute's body's skin, to compare (`body=N`; the user, 2026-11-12: its
//! black body hides its cables as its anatomy; make it of a lighter stuff,
//! or with lighter patterned elements on it): 1 black, as made (Meshy's
//! texture); 2 grey; 3 pale ash; 4 pale crests along its fused cabling (its
//! torso read as made of cables); 5 pale bands about it, mirrored left and
//! right. All but 1 drop its texture's colour, keeping its relief.

use std::collections::HashMap;

use bevy::{mesh::VertexAttributeValues, prelude::*};

/// Its black, and the pale of its markings (sRGB).
const BLACK: f32 = 0.08;
const PALE: f32 = 0.66;
/// Its crests: the share of its surface pale (the most convex), and how
/// soft their edge is (of the same).
const CRESTS: (f32, f32) = (0.22, 0.06);
/// Its bands: how far apart (mesh units), the share of each pale, and how
/// far they wave about it.
const BANDS: (f32, f32, f32) = (0.16, 0.3, 0.035);

/// Its body's mesh and material (the one with Meshy's texture) as `body`
/// has them: its material, if not as made (the mesh given its colours).
pub(super) fn paint(body: u8, mesh: &mut Mesh, made: &StandardMaterial) -> Option<StandardMaterial> {
    let plain = |grey: f32, rough: f32| StandardMaterial { base_color: Color::srgb(grey, grey, grey * 1.02), base_color_texture: None, metallic_roughness_texture: None, perceptual_roughness: rough, metallic: 0.0, ..made.clone() };
    let marked = |pale: Vec<f32>, mesh: &mut Mesh| {
        let (black, light) = (Color::srgb(BLACK, BLACK, BLACK).to_linear(), Color::srgb(PALE, PALE, PALE * 1.02).to_linear());
        let colors: Vec<[f32; 4]> = pale.iter().map(|&k| [black.red + (light.red - black.red) * k, black.green + (light.green - black.green) * k, black.blue + (light.blue - black.blue) * k, 1.0]).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        StandardMaterial { base_color: Color::WHITE, base_color_texture: None, ..made.clone() }
    };
    match body {
        2 => Some(plain(0.3, 0.55)),
        3 => Some(plain(0.55, 0.7)),
        4 => {
            let pale = crests(mesh);
            Some(marked(pale, mesh))
        }
        5 => {
            let pale = bands(mesh);
            Some(marked(pale, mesh))
        }
        _ => None,
    }
}

fn positions(mesh: &Mesh) -> Vec<Vec3> {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| Vec3::from(*p)).collect(),
        _ => panic!("the impaled brute's body has no positions to paint"),
    }
}

/// How pale each point is (0..1): its most convex, the crests of the
/// cables its body is fused from.
fn crests(mesh: &Mesh) -> Vec<f32> {
    let points = positions(mesh);
    let normals: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|n| Vec3::from(*n)).collect(),
        _ => panic!("the impaled brute's body has no normals to paint by"),
    };
    let Some(indices) = mesh.indices() else { panic!("the impaled brute's body has no triangles to paint") };
    let indices: Vec<usize> = indices.iter().collect();
    // (Its points welded where its texture's seams split them.)
    let mut weld = HashMap::new();
    let id: Vec<usize> = points
        .iter()
        .map(|p| {
            let key = (p * 1e5).round().as_ivec3();
            let n = weld.len();
            *weld.entry(key.to_array()).or_insert(n)
        })
        .collect();
    let n = weld.len();
    let (mut at, mut normal, mut count) = (vec![Vec3::ZERO; n], vec![Vec3::ZERO; n], vec![0.0f32; n]);
    for (i, p) in points.iter().enumerate() {
        at[id[i]] = *p;
        normal[id[i]] += normals[i];
    }
    let mut around = vec![Vec3::ZERO; n];
    let mut reach = vec![0.0f32; n];
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); n];
    for t in indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (id[a], id[b]);
            for (x, y) in [(a, b), (b, a)] {
                around[x] += at[y];
                reach[x] += at[x].distance_squared(at[y]);
                count[x] += 1.0;
                neighbours[x].push(y);
            }
        }
    }
    // (Its mean curvature, roughly: how far its neighbours fall away below
    // it, for how far off they are.)
    let mut bend: Vec<f32> = (0..n)
        .map(|i| {
            if count[i] == 0.0 {
                return 0.0;
            }
            let lap = around[i] / count[i] - at[i];
            -2.0 * lap.dot(normal[i].normalize_or_zero()) / (reach[i] / count[i]).max(1e-12)
        })
        .collect();
    for _ in 0..2 {
        bend = (0..n).map(|i| (bend[i] + neighbours[i].iter().map(|&j| bend[j]).sum::<f32>()) / (1 + neighbours[i].len()) as f32).collect();
    }
    let mut sorted = bend.clone();
    sorted.sort_by(f32::total_cmp);
    let at_share = |s: f32| sorted[((1.0 - s) * (n - 1) as f32) as usize];
    let (from, to) = (at_share(CRESTS.0 + CRESTS.1 * 0.5), at_share(CRESTS.0 - CRESTS.1 * 0.5));
    id.iter().map(|&i| ((bend[i] - from) / (to - from).max(1e-6)).clamp(0.0, 1.0)).collect()
}

/// How pale each point is (0..1): bands about it, up its height, waving
/// round it, the same on its left and right.
fn bands(mesh: &Mesh) -> Vec<f32> {
    let (every, share, wave) = BANDS;
    positions(mesh)
        .iter()
        .map(|p| {
            let round = p.z.atan2(p.x.abs());
            let u = ((p.y + wave * (round * 3.0).sin()) / every).rem_euclid(1.0);
            // (Soft at its edges, a little.)
            let edge = 0.04;
            ((share - u) / edge + 0.5).clamp(0.0, 1.0) * ((u / edge) + 0.5).clamp(0.0, 1.0)
        })
        .collect()
}
