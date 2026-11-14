//! A tumbler's damage shown (the user, 2026-11-13: damage shown in its glow
//! before it fires, cracks accumulating): cracks spread from where shards
//! struck it, each growing out quickly from where it struck, the more
//! shards there the further (`scars.wgsl`). The light inside it shows
//! through its blades' cracks, faint at rest; as it winds up to fire its
//! body glows whole, its cracks splitting the glow, dark, and its blades'
//! shine bright. Struck, the light inside it flares a moment (a flinch: a
//! rigid X cannot flinch as a body does, and turning it spun it like a toy).

use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

/// Its parts drawn with their cracks.
pub(in crate::combat) type ScarMaterial = ExtendedMaterial<StandardMaterial, Scars>;

/// How many cracks a part shows at most (the shader's).
pub(super) const MOST: usize = 16;
/// A crack: how far it spreads from where it struck (its units, the model
/// 1.9 tall), and a shard more within its reach of it so much further, to
/// at most; how fast it grows out (its units/s); how finely it zigzags and
/// its width (its units).
const REACH: f32 = 0.12;
const FURTHER: f32 = 0.035;
const REACH_MOST: f32 = 0.45;
const GROWS: f32 = 2.0;
const JAG: f32 = 0.015;
const WIDTH: f32 = 0.004;
/// Its cracks at rest (emissive); how dark they split its body's glow
/// (0..1); how much of the light inside it leaks through its blades'.
const EMBER: f32 = 3.0;
const SPLIT: f32 = 1.0;
const LEAK: f32 = 0.5;
/// Struck: how bright the light inside it flares (of its glow firing), and
/// how quickly it fades (s, to a third).
pub(super) const FLARE: (f32, f32) = (0.7, 0.12);

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(in crate::combat) struct Scars {
    /// The light inside it now (emissive), its cracks' at rest, 1 if its
    /// whole surface glows (its body), how many cracks.
    #[uniform(100)]
    light: Vec4,
    /// How finely its cracks zigzag, their width, how dark they split its
    /// body's glow, how much of the light inside leaks through its blades'.
    #[uniform(101)]
    look: Vec4,
    /// Each crack: where it struck (its part's own units), how far it has
    /// spread now.
    #[uniform(102)]
    cracks: [Vec4; MOST],
}

impl MaterialExtension for Scars {
    fn fragment_shader() -> ShaderRef {
        "embedded://game/combat/tumbler/scars.wgsl".into()
    }
}

/// A part's look: on the creatures' dark, its whole surface glowing (its
/// body) or not.
pub(super) fn material(dark: StandardMaterial, glows: bool) -> ScarMaterial {
    ExtendedMaterial {
        base: dark,
        extension: Scars { light: Vec4::new(0.0, EMBER, if glows { 1.0 } else { 0.0 }, 0.0), look: Vec4::new(JAG, WIDTH, SPLIT, LEAK), cracks: [Vec4::ZERO; MOST] },
    }
}

/// A crack: where it struck (its part's own units), how far it spreads, and
/// how far it has grown so far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Crack {
    pub(super) at: Vec3,
    pub(super) reach: f32,
    pub(super) grown: f32,
}

/// A part's cracks.
#[derive(Clone, Debug, Default)]
pub(super) struct Cracks(pub(super) Vec<Crack>);

impl Cracks {
    /// A shard struck it at `at` (its own units): a crack there reaching
    /// further, if it struck within one's reach, else a new one (all of
    /// them shown already: the nearest reaching further).
    pub(super) fn strike(&mut self, at: Vec3) {
        let full = self.0.len() >= MOST;
        let near = self.0.iter_mut().map(|c| (c.at.distance(at) / c.reach.max(REACH), c)).min_by(|a, b| a.0.total_cmp(&b.0));
        match near {
            Some((d, c)) if d <= 1.0 || full => c.reach = (c.reach + FURTHER).min(REACH_MOST),
            _ => self.0.push(Crack { at, reach: REACH, grown: 0.0 }),
        }
    }

    /// Grown on for `dt`; whether any is still growing.
    pub(super) fn grow(&mut self, dt: f32) -> bool {
        let mut growing = false;
        for c in &mut self.0 {
            c.grown = (c.grown + GROWS * dt).min(c.reach);
            growing |= c.grown < c.reach;
        }
        growing
    }

    /// As the shader takes them.
    fn shown(&self) -> [Vec4; MOST] {
        let mut out = [Vec4::ZERO; MOST];
        for (o, c) in out.iter_mut().zip(&self.0) {
            *o = c.at.extend(c.grown);
        }
        out
    }
}

/// A part's look now: the light inside it (emissive), lit or dark (dead:
/// its cracks dark too), and its cracks.
pub(super) fn show(m: &mut ScarMaterial, inner: f32, alive: bool, cracks: &Cracks) {
    let s = &mut m.extension;
    s.light = Vec4::new(inner, if alive { EMBER } else { 0.0 }, s.light.z, cracks.0.len().min(MOST) as f32);
    s.cracks = cracks.shown();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struck_near_grows_far_adds() {
        let mut c = Cracks::default();
        c.strike(Vec3::ZERO);
        c.strike(Vec3::X * 0.05);
        assert_eq!(c.0.len(), 1, "a shard within its reach grows it");
        assert!((c.0[0].reach - (REACH + FURTHER)).abs() < 1e-6);
        c.strike(Vec3::X * 0.5);
        assert_eq!(c.0.len(), 2, "a shard beyond its reach is a crack of its own");
        for k in 0..100 {
            c.strike(Vec3::new(k as f32, 0.0, 0.0) * 3.0);
        }
        assert_eq!(c.0.len(), MOST, "never more than the shader shows");
        assert!(c.0.iter().all(|c| c.reach <= REACH_MOST + 1e-6));
    }

    #[test]
    fn grows_out_quickly() {
        let mut c = Cracks::default();
        c.strike(Vec3::ZERO);
        let mut t = 0.0;
        while c.grow(1.0 / 60.0) {
            t += 1.0 / 60.0;
        }
        assert!(t > 0.02 && t < 0.15, "grown out in {t} s");
        assert_eq!(c.0[0].grown, c.0[0].reach);
    }
}
