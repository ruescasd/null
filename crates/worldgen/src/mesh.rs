//! The terrain's meshes: the world is split horizontally into square
//! columns of `COLUMN_CELLS` cells, meshed at a few levels of detail.


/// Cells along each horizontal side of a column.
pub const COLUMN_CELLS: usize = 32;
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
    /// The size of the face each vertex belongs to (its narrowest width,
    /// metres), so surface detail can scale with the geometry. Built faces
    /// fill it in; it may run short of `positions`, and missing entries are
    /// [`OPEN_GROUND`].
    pub face: Vec<f32>,
    /// How brightly each vertex glows; it may run short of `positions`,
    /// and missing entries are 0.
    pub glow: Vec<f32>,
    pub indices: Vec<u32>,
}

/// The face size of everything that does not give one: open ground.
pub const OPEN_GROUND: f32 = 1000.0;

impl ColumnMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Makes the `count` vertices just added glow.
    pub fn glow_of(&mut self, count: usize, glow: f32) {
        let end = self.positions.len();
        self.glow.resize(end - count, 0.0);
        self.glow.resize(end, glow);
    }

    /// Records the size of the face whose `count` vertices were just added.
    pub fn face_size(&mut self, count: usize, size: f32) {
        let end = self.positions.len();
        self.face.resize(end - count, OPEN_GROUND);
        self.face.resize(end, size);
    }
}

/// The narrowest width of a flat convex polygon: over its edges, the
/// largest distance of any corner from that edge's line, at its smallest.
pub fn polygon_width(points: &[glam::Vec3]) -> f32 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let normal = (points[1] - points[0]).cross(points[2] - points[0]);
    let mut best = f32::INFINITY;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let Some(across) = normal.cross(b - a).try_normalize() else { continue };
        let far = points.iter().map(|p| (*p - a).dot(across).abs()).fold(0.0, f32::max);
        best = best.min(far);
    }
    if best.is_finite() { best } else { 0.0 }
}

/// World-space origin of a column (its minimum x and z corner).
pub fn column_origin(lod: u32, cx: i32, cz: i32) -> (f32, f32) {
    (cx as f32 * column_size(lod), cz as f32 * column_size(lod))
}
