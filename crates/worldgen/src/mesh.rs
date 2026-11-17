//! The terrain's meshes: the world is split horizontally into square
//! columns of `COLUMN_CELLS` cells, meshed at a few levels of detail.


/// Cells along each horizontal side of a column.
pub const COLUMN_CELLS: usize = 64;
/// Number of detail levels. Each level's cells are `LOD_FACTOR` times larger.
pub const LOD_LEVELS: u32 = 3;
pub const LOD_FACTOR: i32 = 4;

/// Edge length of one cell at a detail level, in metres.
pub fn cell_size(lod: u32) -> f32 {
    (LOD_FACTOR as f32).powi(lod as i32)
}

/// Horizontal size of a column at a detail level, in metres.
pub fn column_size(lod: u32) -> f32 {
    COLUMN_CELLS as f32 * cell_size(lod)
}

#[derive(Default, Debug)]
pub struct ColumnMesh {
    /// Positions relative to the column's (x, z) origin; y is absolute.
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Linear greyscale albedo per vertex.
    pub albedo: Vec<f32>,
    /// Sky visibility per vertex (0 = fully occluded, 1 = open sky).
    pub ao: Vec<f32>,
    /// Which plate each vertex is on (its top or its walls), for the
    /// shader's subdivision (see [`plate_code`]); it may run short of
    /// `positions`, and missing entries are 0: not a plate's.
    pub plate: Vec<f32>,
    /// How brightly each vertex glows; it may run short of `positions`,
    /// and missing entries are 0.
    pub glow: Vec<f32>,
    /// On a wall going down into a void, how far each vertex is below the
    /// wall's top (m), for its fading into the dark (terrain.wgsl); it may
    /// run short of `positions`, and missing entries are 0.
    pub below: Vec<f32>,
    /// What is under the world (the vaults' undersides, their pillars), 1:
    /// in the shell's shadow (terrain.wgsl); it may run short of
    /// `positions`, and missing entries are 0. By what it is, not by its
    /// height against the vaults' (a straight piece across the crease where
    /// two bays' vaults meet stood above it, lit).
    pub under: Vec<f32>,
    pub indices: Vec<u32>,
    /// Each triangle's block (see `blocks`); it may run short of the
    /// triangles, and missing entries are 0.
    pub tri_block: Vec<u8>,
    /// A coarser level's column in the blocks a finer level's columns
    /// cover, 4 by 4 (`z * 4 + x`), each block's triangles together in
    /// `indices`: (first index, how many). What lies in a block is what the
    /// finer column there holds (a plate by its big plate's site, as
    /// columns hold them), so the two never both stand for one place (the
    /// traced shadows' scene, game's rt_shadow.rs). The finest level: one.
    pub blocks: Vec<(u32, u32)>,
    /// What is walked on but not drawn (as `positions`, `indices`): the
    /// hidden slopes over stairs' steps (see `plates::Stair`), in the
    /// collider only.
    pub walk: Vec<[f32; 3]>,
    pub walk_indices: Vec<u32>,
}

/// What a plate's top tells the shader: its level (0..=2), a number of its
/// own (0..1024), and whether its district keeps to the big pieces (32 m,
/// see `Rules::pieces`), from 1 up (0 is no plate). Read in `terrain.wgsl`.
pub fn plate_code(level: usize, id: f32, big: bool) -> f32 {
    1.0 + level as f32 + 4.0 * (id + if big { 1024.0 } else { 0.0 })
}

/// What a plate's wall tells the shader: its plate's code and a number of
/// its own (0..16), from its line, not its plate, so it keeps its pattern
/// whichever level of detail draws it (a far wall is a coarser plate's; its
/// panels changed as it was approached, the user). Under 2^17: the game
/// adds the level of detail above it, and the colour it all travels in,
/// interpolated, must still round to it.
pub fn wall_code(code: f32, line: u32) -> f32 {
    code + 8192.0 * (line % 16) as f32
}

impl ColumnMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty() && self.walk_indices.is_empty()
    }

    /// Makes the `count` vertices just added glow.
    pub fn glow_of(&mut self, count: usize, glow: f32) {
        let end = self.positions.len();
        self.glow.resize(end - count, 0.0);
        self.glow.resize(end, glow);
    }

    /// Marks the `count` vertices just added as a plate's (`code`, see
    /// [`plate_code`]).
    pub fn plate_of(&mut self, count: usize, code: f32) {
        let end = self.positions.len();
        self.plate.resize(end - count, 0.0);
        self.plate.resize(end, code);
    }

    /// Marks the `count` vertices just added as under the world (see
    /// `under`).
    pub fn under_of(&mut self, count: usize) {
        let end = self.positions.len();
        self.under.resize(end - count, 0.0);
        self.under.resize(end, 1.0);
    }

    /// The triangles added since index `first` are in block `block`.
    pub fn block_from(&mut self, first: usize, block: u8) {
        self.tri_block.resize(first / 3, 0);
        self.tri_block.resize(self.indices.len() / 3, block);
    }

    /// Sorts the triangles into `n` blocks (see `blocks`), each together.
    pub fn sort_blocks(&mut self, n: usize) {
        let count = self.indices.len() / 3;
        self.tri_block.resize(count, 0);
        let mut order: Vec<usize> = (0..count).collect();
        order.sort_by_key(|&t| self.tri_block[t]);
        let indices: Vec<u32> = order.iter().flat_map(|&t| [self.indices[3 * t], self.indices[3 * t + 1], self.indices[3 * t + 2]]).collect();
        let blocks: Vec<u8> = order.iter().map(|&t| self.tri_block[t]).collect();
        self.blocks = (0..n as u8)
            .map(|b| {
                let first = blocks.partition_point(|&x| x < b);
                let end = blocks.partition_point(|&x| x <= b);
                (3 * first as u32, 3 * (end - first) as u32)
            })
            .collect();
        self.indices = indices;
        self.tri_block = blocks;
    }

    /// The last `values.len()` vertices' depths below their wall's top
    /// (see `below`).
    pub fn below_of(&mut self, values: &[f32]) {
        let end = self.positions.len();
        self.below.resize(end - values.len(), 0.0);
        self.below.extend_from_slice(values);
    }
}

/// World-space origin of a column (its minimum x and z corner).
pub fn column_origin(lod: u32, cx: i32, cz: i32) -> (f32, f32) {
    (cx as f32 * column_size(lod), cz as f32 * column_size(lod))
}
