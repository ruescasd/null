//! Gradient noise that tiles in x and z.
//!
//! The world wraps around in both horizontal axes, so every noise lookup must be
//! periodic. Each function takes a `period` in lattice cells: lattice indices are
//! wrapped modulo the period before hashing, so `f(u + period, v) == f(u, v)`.
//! Callers convert world metres to lattice space with `u = x / world_size * period`.
//! The y axis (3D only) is never wrapped.

#[inline]
fn hash(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb_352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

/// Uniform float in [0, 1) derived from a lattice point.
#[inline]
pub fn hash01(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    (hash(x, y, z, seed) >> 8) as f32 / (1u32 << 24) as f32
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

const GRAD2: [[f32; 2]; 8] = [
    [1.0, 0.0],
    [-1.0, 0.0],
    [0.0, 1.0],
    [0.0, -1.0],
    [0.707_106_8, 0.707_106_8],
    [-0.707_106_8, 0.707_106_8],
    [0.707_106_8, -0.707_106_8],
    [-0.707_106_8, -0.707_106_8],
];

const GRAD3: [[f32; 3]; 16] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [0.0, -1.0, 1.0],
    [0.0, -1.0, -1.0],
];

/// 2D Perlin noise in roughly [-1, 1], periodic with `period` cells in both axes.
pub fn perlin2(u: f32, v: f32, period: i32, seed: u32) -> f32 {
    let (fu, fv) = (u.floor(), v.floor());
    let (xu, xv) = (u - fu, v - fv);
    let (iu, iv) = (fu as i32, fv as i32);
    let corner = |du: i32, dv: i32| {
        let h = hash((iu + du).rem_euclid(period), 0, (iv + dv).rem_euclid(period), seed);
        let g = GRAD2[(h >> 28) as usize & 7];
        g[0] * (xu - du as f32) + g[1] * (xv - dv as f32)
    };
    let (su, sv) = (fade(xu), fade(xv));
    let a = lerp(corner(0, 0), corner(1, 0), su);
    let b = lerp(corner(0, 1), corner(1, 1), su);
    lerp(a, b, sv) * 1.42
}

/// 3D Perlin noise in roughly [-1, 1], periodic in `u` and `w` (the horizontal axes).
pub fn perlin3(u: f32, v: f32, w: f32, period: i32, seed: u32) -> f32 {
    let (fu, fv, fw) = (u.floor(), v.floor(), w.floor());
    let (xu, xv, xw) = (u - fu, v - fv, w - fw);
    let (iu, iv, iw) = (fu as i32, fv as i32, fw as i32);
    let corner = |du: i32, dv: i32, dw: i32| {
        let h = hash(
            (iu + du).rem_euclid(period),
            iv + dv,
            (iw + dw).rem_euclid(period),
            seed,
        );
        let g = GRAD3[(h >> 28) as usize];
        g[0] * (xu - du as f32) + g[1] * (xv - dv as f32) + g[2] * (xw - dw as f32)
    };
    let (su, sv, sw) = (fade(xu), fade(xv), fade(xw));
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), su);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), su);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), su);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), su);
    lerp(lerp(x00, x10, sv), lerp(x01, x11, sv), sw)
}

/// Describes an octave stack in world units.
#[derive(Clone, Copy, Debug)]
pub struct Fbm {
    /// Feature size of the first octave, in metres. It is rounded so that a
    /// whole number of wavelengths fits across the world.
    pub wavelength: f32,
    pub octaves: u32,
    /// Amplitude multiplier per octave (frequency always doubles, which keeps
    /// every octave's period an integer and therefore tileable).
    pub gain: f32,
    pub seed: u32,
}

impl Fbm {
    pub const fn new(wavelength: f32, octaves: u32, gain: f32, seed: u32) -> Self {
        Self { wavelength, octaves, gain, seed }
    }

    #[inline]
    fn base_cells(&self, world: f32) -> i32 {
        ((world / self.wavelength).round() as i32).max(1)
    }

    /// Normalised fractal sum in roughly [-1, 1].
    pub fn sample2(&self, x: f32, z: f32, world: f32) -> f32 {
        let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
        let mut cells = self.base_cells(world);
        for o in 0..self.octaves {
            let s = cells as f32 / world;
            sum += amp * perlin2(x * s, z * s, cells, self.seed.wrapping_add(o * 7919));
            norm += amp;
            amp *= self.gain;
            cells *= 2;
        }
        sum / norm
    }

    /// Normalised fractal sum in roughly [-1, 1].
    pub fn sample3(&self, x: f32, y: f32, z: f32, world: f32) -> f32 {
        let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
        let mut cells = self.base_cells(world);
        for o in 0..self.octaves {
            let s = cells as f32 / world;
            sum += amp * perlin3(x * s, y * s, z * s, cells, self.seed.wrapping_add(o * 7919));
            norm += amp;
            amp *= self.gain;
            cells *= 2;
        }
        sum / norm
    }

    /// Ridged multifractal in [0, 1]: sharp crests where the noise crosses zero,
    /// with each octave weighted by the previous one so detail clusters on ridges.
    pub fn ridged2(&self, x: f32, z: f32, world: f32) -> f32 {
        let (mut sum, mut amp, mut norm, mut weight) = (0.0, 1.0, 0.0, 1.0);
        let mut cells = self.base_cells(world);
        for o in 0..self.octaves {
            let s = cells as f32 / world;
            let n = 1.0 - perlin2(x * s, z * s, cells, self.seed.wrapping_add(o * 7919)).abs();
            let n = n * n * weight;
            weight = (n * 2.0).clamp(0.0, 1.0);
            sum += amp * n;
            norm += amp;
            amp *= self.gain;
            cells *= 2;
        }
        sum / norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perlin_tiles() {
        for i in 0..50 {
            let (u, v, y) = (i as f32 * 0.37, i as f32 * 0.91, i as f32 * 0.13);
            assert!((perlin2(u, v, 8, 1) - perlin2(u + 8.0, v - 16.0, 8, 1)).abs() < 1e-4);
            assert!((perlin3(u, y, v, 8, 1) - perlin3(u - 8.0, y, v + 8.0, 8, 1)).abs() < 1e-4);
        }
    }

    #[test]
    fn fbm_tiles_in_world_space() {
        let f = Fbm::new(256.0, 5, 0.5, 3);
        let w = 1024.0;
        for i in 0..50 {
            let (x, z) = (i as f32 * 13.7, i as f32 * 29.1);
            assert!((f.sample2(x, z, w) - f.sample2(x + w, z - w, w)).abs() < 1e-3);
        }
    }
}
