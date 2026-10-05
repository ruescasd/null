//! Engine-agnostic procedural world generation.
//!
//! The world is a torus: flat locally, wrapping in x and z with period
//! [`WORLD_SIZE`]. Nothing here depends on a renderer or game engine.

pub mod mesh;
pub mod noise;
pub mod canal;
pub mod cellunit;
pub mod compose;
pub mod cull;
pub mod district;
pub mod forms;
pub mod ifs;
pub mod lab;
pub mod lattice;
pub mod mega;
pub mod plates;
pub mod reactor;
pub mod sites;
pub mod structure;
pub mod tower;

pub use mesh::{COLUMN_CELLS, ColumnMesh, LOD_FACTOR, LOD_LEVELS, cell_size, column_origin, column_size};

/// The world's wrap period in metres (a multiple of the column size and of
/// every noise wavelength).
pub const WORLD_SIZE: f32 = 16384.0;
