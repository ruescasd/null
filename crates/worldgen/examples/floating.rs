//! Finds floating pieces: builds the sites near a point and the test
//! structures, and reports pieces with no chain of contacts down to the
//! ground. Pieces are compared by their bounding boxes, so this errs towards
//! calling things supported.
//!
//! `cargo run -p worldgen --release --example floating -- [x z radius]`

use std::collections::HashMap;

use glam::{Vec2, Vec3};
use worldgen::{
    forms::Prism,
    plates::PlateWorld,
    sites,
    structure::{self, Library, Solid},
};

/// Gaps up to this (metres) still count as touching: grooves between
/// blocks are joints, not air.
const TOUCH: f32 = 0.5;

#[derive(Clone, Copy)]
struct Piece {
    min: Vec3,
    max: Vec3,
}

fn solid_box(s: &Solid, offset: Vec3) -> Piece {
    let mut min = Vec3::MAX;
    let mut max = Vec3::MIN;
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                let p = offset + s.center + s.rotation * (Vec3::new(x, y, z) * s.half);
                min = min.min(p);
                max = max.max(p);
            }
        }
    }
    Piece { min, max }
}

fn prism_box(p: &Prism, offset: Vec3) -> Piece {
    let mut min = Vec3::MAX;
    let mut max = Vec3::MIN;
    for q in p.hull_points() {
        min = min.min(offset + q);
        max = max.max(offset + q);
    }
    Piece { min, max }
}

fn touching(a: &Piece, b: &Piece) -> bool {
    (0..3).all(|i| a.min[i] <= b.max[i] + TOUCH && b.min[i] <= a.max[i] + TOUCH)
}

/// Pieces not connected to the ground: (count, biggest floating piece and
/// how high its bottom is above the ground under it).
fn floating(world: &PlateWorld, pieces: &[Piece]) -> Vec<(Piece, f32)> {
    const CELL: f32 = 4.0;
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, p) in pieces.iter().enumerate() {
        let (x0, z0) = ((p.min.x / CELL).floor() as i32, (p.min.z / CELL).floor() as i32);
        let (x1, z1) = ((p.max.x / CELL).floor() as i32, (p.max.z / CELL).floor() as i32);
        for z in z0..=z1 {
            for x in x0..=x1 {
                grid.entry((x, z)).or_default().push(i);
            }
        }
    }
    let ground_under = |p: &Piece| {
        let c = (p.min + p.max) * 0.5;
        [Vec2::new(c.x, c.z), Vec2::new(p.min.x, p.min.z), Vec2::new(p.max.x, p.max.z), Vec2::new(p.min.x, p.max.z), Vec2::new(p.max.x, p.min.z)]
            .iter()
            .map(|q| world.height_at(q.x, q.y))
            .fold(f32::MIN, f32::max)
    };
    let mut supported = vec![false; pieces.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        if p.min.y <= ground_under(p) + 0.3 {
            supported[i] = true;
            stack.push(i);
        }
    }
    while let Some(i) = stack.pop() {
        let p = pieces[i];
        let (x0, z0) = ((p.min.x / CELL).floor() as i32, (p.min.z / CELL).floor() as i32);
        let (x1, z1) = ((p.max.x / CELL).floor() as i32, (p.max.z / CELL).floor() as i32);
        for z in z0..=z1 {
            for x in x0..=x1 {
                for &j in grid.get(&(x, z)).into_iter().flatten() {
                    if !supported[j] && touching(&p, &pieces[j]) {
                        supported[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
    }
    pieces
        .iter()
        .zip(&supported)
        .filter(|(_, s)| !**s)
        .map(|(p, _)| (*p, p.min.y - ground_under(p)))
        .collect()
}

fn report(world: &PlateWorld, name: &str, at: (f32, f32), pieces: &[Piece]) {
    let lost = floating(world, pieces);
    if lost.is_empty() {
        println!("{name:>24} at {:6.0}, {:6.0}: {} pieces, none floating", at.0, at.1, pieces.len());
        return;
    }
    let volume = |p: &Piece| (p.max - p.min).element_product();
    let biggest = lost.iter().max_by(|a, b| volume(&a.0).total_cmp(&volume(&b.0))).unwrap();
    let c = (biggest.0.min + biggest.0.max) * 0.5;
    println!(
        "{name:>24} at {:6.0}, {:6.0}: {} of {} pieces floating; biggest {:.0}x{:.0}x{:.0} m at {:.0}, {:.0}, {:.1} m above the ground",
        at.0,
        at.1,
        lost.len(),
        pieces.len(),
        biggest.0.max.x - biggest.0.min.x,
        biggest.0.max.y - biggest.0.min.y,
        biggest.0.max.z - biggest.0.min.z,
        c.x,
        c.z,
        biggest.1,
    );
}

fn main() {
    let args: Vec<f32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let (x, z, radius) = match args[..] {
        [x, z, r, ..] => (x, z, r),
        _ => (1200.0, 900.0, 3000.0),
    };
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron");
    let library = Library::parse(&std::fs::read_to_string(path).expect("read structures.ron")).expect("parse");
    let world = PlateWorld::new(16384.0, 1).with_sites(&library);

    for placement in &library.structures {
        let (px, pz) = placement.at;
        let offset = Vec3::new(px, world.height_at(px, pz), pz);
        let pieces: Vec<Piece> = structure::build(&library, placement, 40_000).iter().map(|s| solid_box(s, offset)).collect();
        report(&world, &format!("test {}", placement.style), placement.at, &pieces);
    }
    for site in sites::near(&library, &world, x, z, radius) {
        let built = sites::build(&library, &world, &site, 40_000);
        let offset = Vec3::new(site.at.0, built.base, site.at.1);
        let mut pieces: Vec<Piece> = built.solids.iter().map(|s| solid_box(s, offset)).collect();
        pieces.extend(built.prisms.iter().map(|p| prism_box(p, offset)));
        let name = site
            .centrepiece
            .iter()
            .map(|p| p.style.as_str())
            .chain(site.plates.as_deref())
            .collect::<Vec<_>>()
            .join("+");
        report(&world, &format!("site {name}"), site.at, &pieces);
    }
}
