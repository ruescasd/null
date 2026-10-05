//! Engine-agnostic procedural world generation.
//!
//! The world is a torus: flat locally, wrapping in x and z with period
//! [`WorldConfig::size`]. Nothing here depends on a renderer or game engine.

pub mod mesh;
pub mod noise;
pub mod bastion;
pub mod canal;
pub mod cellunit;
pub mod cluster;
pub mod compose;
pub mod cull;
pub mod curtain;
pub mod district;
pub mod dressing;
pub mod forms;
pub mod fractal;
pub mod ifs;
pub mod lab;
pub mod lattice;
pub mod plates;
pub mod rack;
pub mod stairs;
pub mod relief;
pub mod sites;
pub mod structure;
pub mod tower;
pub mod world;

pub use mesh::{
    COLUMN_CELLS, ColumnMesh, LOD_FACTOR, LOD_LEVELS, column_origin, column_size, mesh_column,
    voxel_size,
};
pub use world::{World, WorldConfig};
