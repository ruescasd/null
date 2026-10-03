//! Engine-agnostic procedural world generation.
//!
//! The world is a torus: flat locally, wrapping in x and z with period
//! [`WorldConfig::size`]. Nothing here depends on a renderer or game engine.

pub mod mesh;
pub mod noise;
pub mod canal;
pub mod district;
pub mod landmarks;
pub mod plates;
pub mod world;

pub use mesh::{
    COLUMN_CELLS, ColumnMesh, LOD_FACTOR, LOD_LEVELS, column_origin, column_size, mesh_column,
    voxel_size,
};
pub use world::{World, WorldConfig};
