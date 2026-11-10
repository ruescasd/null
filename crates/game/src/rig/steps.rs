//! Another body's walk, as the shape of its steps (`tools/step_shape.py`):
//! when each foot lifts and lands, its swing, where it lands, and how the
//! body rises, sways, surges and turns over a cycle, lengths in its legs. A
//! rig walks with it on its own legs, its own feet planted where they land
//! (the user, 2026-11-06: the stalker, fighting, sheds its struggling walk
//! for the gladiator's, energetic; its joints' turns on other legs would
//! slide and dig in).

use std::sync::OnceLock;

/// Samples in each curve.
const POINTS: usize = 24;

/// A walk's steps (see `creatures/gladiator.steps`).
#[derive(Clone, Debug)]
pub struct Steps {
    /// Its cycle (s). (Its own leg, speed and hips' height, in the file,
    /// are not needed: a body walks it at its own.)
    pub cycle: f32,
    /// Its stride (legs); how much of the cycle a foot is down; where each
    /// foot lifts (of the cycle, from the left landing; left, right).
    pub stride: f32,
    pub down: f32,
    pub lifts: [f32; 2],
    /// Where a foot lands from its hip (legs: ahead, out).
    pub land: (f32, f32),
    /// Through a swing: how far along its way, how high (legs).
    along: [f32; POINTS],
    up: [f32; POINTS],
    /// Through the cycle: the hips' rise, sway (to the left), surge (legs);
    /// the pelvis's turn (degrees). (Its shoulders' turn and its trunk's
    /// lean and roll, in the file, are not taken: an upper body swinging
    /// against arms swinging, on the stalker, its lance in both hands, read
    /// as wiggling, the user; see `rig::WALK_CHEST`.)
    rise: [f32; POINTS],
    sway: [f32; POINTS],
    pelvis: [f32; POINTS],
    /// Through the cycle: how fast its body goes, of its pace (its hips'
    /// surge, as their speed: they went on at an even pace, the body
    /// floating, its feet stepping under it, the user).
    pace: [f32; POINTS],
}

/// The body through its cycle, at a moment: the hips' rise and sway (to
/// the left; legs), and the pelvis's turn (radians).
#[derive(Clone, Copy, Debug, Default)]
pub struct Body {
    pub rise: f32,
    pub sway: f32,
    pub pelvis: f32,
}

impl Steps {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut found = std::collections::HashMap::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (name, values) = line.split_once('=').ok_or(format!("line {}: not `name = numbers`", n + 1))?;
            let values: Vec<f32> = values.split_whitespace().map(|v| v.parse::<f32>().map_err(|e| format!("line {}: {v}: {e}", n + 1))).collect::<Result<_, _>>()?;
            found.insert(name.trim().to_string(), values);
        }
        let get = |name: &str, count: usize| -> Result<Vec<f32>, String> {
            let v = found.get(name).ok_or(format!("no `{name}`"))?;
            if v.len() != count {
                return Err(format!("`{name}`: {} numbers, not {count}", v.len()));
            }
            Ok(v.clone())
        };
        let one = |name: &str| get(name, 1).map(|v| v[0]);
        let curve = |name: &str| get(name, POINTS).map(|v| <[f32; POINTS]>::try_from(v).unwrap());
        let (lifts, land) = (get("lifts", 2)?, get("land", 2)?);
        // Its pace through the cycle, from its hips' surge (legs): their
        // speed over its own, about 1.
        let (surge, leg, cycle, speed) = (curve("surge")?, one("leg")?, one("cycle")?, one("speed")?);
        let pace = std::array::from_fn(|i| {
            let d = (surge[(i + 1) % POINTS] - surge[(i + POINTS - 1) % POINTS]) * POINTS as f32 / 2.0;
            1.0 + d * leg / (cycle * speed)
        });
        Ok(Self {
            cycle: one("cycle")?,
            stride: one("stride")?,
            down: one("down")?,
            lifts: [lifts[0], lifts[1]],
            land: (land[0], land[1]),
            along: curve("along")?,
            up: curve("up")?,
            rise: curve("rise")?,
            sway: curve("sway")?,
            pelvis: curve("pelvis")?,
            pace,
        })
    }

    /// The gladiator's, read once (failing loudly: the file and what is
    /// wrong).
    pub fn gladiator() -> &'static Self {
        static READ: OnceLock<Steps> = OnceLock::new();
        READ.get_or_init(|| {
            let path = crate::combat::creatures_folder().join("gladiator.steps");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e} (the stalker hunting walks with it; tools/step_shape.py --write makes it)", path.display()));
            Self::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        })
    }

    /// Through a swing (`s`, 0..1): how far along its way, how high (legs),
    /// from the ground at both ends. (Its ankle's height: at its toes'
    /// push-off already a tenth of its leg up, its heel raised; a stilt's
    /// tip, heelless, leapt 17 cm off the ground in a frame as it lifted.)
    pub fn swing(&self, s: f32) -> (f32, f32) {
        let ends = self.up[0] + (self.up[POINTS - 1] - self.up[0]) * s.clamp(0.0, 1.0);
        (span(&self.along, s), span(&self.up, s) - ends)
    }

    /// The body at `u` through the cycle (0..1, from the left foot landing).
    pub fn body(&self, u: f32) -> Body {
        let r = |c: &[f32; POINTS]| round(c, u);
        Body { rise: r(&self.rise), sway: r(&self.sway), pelvis: r(&self.pelvis).to_radians() }
    }

    /// How fast its body goes at `u` through the cycle, of its pace.
    pub fn pace(&self, u: f32) -> f32 {
        round(&self.pace, u)
    }
}

/// A curve over a span, its ends at 0 and 1, at `s`.
fn span(c: &[f32; POINTS], s: f32) -> f32 {
    let x = s.clamp(0.0, 1.0) * (POINTS - 1) as f32;
    let i = (x as usize).min(POINTS - 2);
    c[i] + (c[i + 1] - c[i]) * (x - i as f32)
}

/// A curve round a cycle (its last sample before its first) at `u`.
fn round(c: &[f32; POINTS], u: f32) -> f32 {
    let x = u.rem_euclid(1.0) * POINTS as f32;
    let i = (x as usize).min(POINTS - 1);
    c[i] + (c[(i + 1) % POINTS] - c[i]) * (x - i as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gladiator's reads, and makes sense: a walk (each foot down over
    /// half the cycle), the feet a half cycle apart, a swing from its start
    /// to its end.
    #[test]
    fn the_gladiators_reads() {
        let s = Steps::gladiator();
        assert!(s.down > 0.5 && s.down < 0.7, "down {}", s.down);
        let apart = (s.lifts[0] - s.lifts[1]).rem_euclid(1.0);
        assert!((apart - 0.5).abs() < 0.05, "its feet lift {apart} apart");
        assert!(s.swing(0.0).0 < 0.01 && s.swing(1.0).0 > 0.99);
        assert!(s.swing(0.0).1.abs() < 1e-6 && s.swing(1.0).1.abs() < 1e-6, "a swing off the ground at its ends");
        assert!(s.body(0.3).rise.abs() < 0.1);
        // Its pace through the cycle: about its own on the whole, faster
        // and slower within each step.
        let paces: Vec<f32> = (0..100).map(|k| s.pace(k as f32 / 100.0)).collect();
        let mean = paces.iter().sum::<f32>() / 100.0;
        let (lo, hi) = paces.iter().fold((f32::MAX, f32::MIN), |(a, b), &p| (a.min(p), b.max(p)));
        println!("its pace {lo:.2} to {hi:.2} of its own, {mean:.3} on the whole");
        assert!((mean - 1.0).abs() < 0.01 && lo > 0.5 && hi > 1.1, "pace {lo} to {hi}, {mean}");
    }
}
