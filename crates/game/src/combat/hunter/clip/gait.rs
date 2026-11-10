//! An authored body's walk as waves (the user, 2026-11-05: its walk as code
//! of our own, to be bent; first the clip's own, unchanged, to compare
//! blind): `creatures/NAME.gait`, written by `tools/gait_fit.py --write`.
//! Each joint the walk moves is its mean turn and its turn from that, a
//! rotation vector, as a constant and a few waves through the clip (Unuma
//! et al. 1995); its hips' place likewise. With `walk=1` it is played over
//! the clip's walk, in its share of what is played (see `animate`), at the
//! same point of the stride the clip would be.

use bevy::prelude::*;

use super::Body;

/// A joint's walk: its name, its mean turn, its turn from that and its
/// place (rows: the constant, then each wave's cosine and sine parts).
#[derive(Clone, Debug)]
pub(crate) struct Track {
    pub(crate) name: String,
    mean: Quat,
    turn: Vec<Vec3>,
    place: Option<Vec<Vec3>>,
}

/// A walk as waves: the clip it was fitted to, its joints, and the fit's own
/// poses at a few points (a joint, u, its turn and place) for the test; and
/// where its steps may change their shape (see `Plan`), once read.
#[derive(Clone, Debug)]
pub(crate) struct Gait {
    pub(crate) clip: String,
    pub(crate) tracks: Vec<Track>,
    #[cfg_attr(not(test), allow(dead_code))]
    checks: Vec<(String, f32, Quat, Vec3)>,
    pub(crate) plan: Option<Plan>,
}

/// What a joint does in a step's shape: a thigh swings as far as the step
/// reaches; a knee bends as high as its foot is lifted (0 left, 1 right); an
/// arm swings as far as the arms do; its trunk (hips' turn, spine, neck and
/// head) swings as far as `torso` says; the rest as fitted.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Thigh,
    Knee(usize),
    Arm,
    Trunk,
    Other,
}

/// Where in its stride (its phase, 0..1, from its left foot coming down) a
/// step's shape may change without moving a planted foot: the two moments
/// both thighs are nearest their middle, its feet under it and its arms near
/// theirs (`lulls`); and each knee's bend through its swing, its foot in the
/// air (`swings`, from and how long). Found from the waves at load, so they
/// are each walk's own (the gladiator's lulls 0.40 and 0.90).
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    roles: Vec<Role>,
    pub(crate) lulls: [f32; 2],
    swings: [(f32, f32); 2],
    /// Where its clip is at its phase 0 (`u = phase + offset`).
    offset: f32,
    /// Its hips' track: their turn quietened, its thighs are turned back by
    /// as much, so its legs go as fitted.
    pelvis: usize,
}

/// How long (of a stride) either side of a lull its reach and arms change.
pub(crate) const LULL: f32 = 0.06;

/// How far `phase` is into the stretch from `from`, `long` long (0..1), if
/// within it, round the stride's end.
pub(crate) fn within(phase: f32, from: f32, long: f32) -> Option<f32> {
    let d = (phase - from).rem_euclid(1.0);
    (d < long).then(|| d / long)
}

/// Whether going from `was` to `now` (round the stride's end) passed `at`.
pub(crate) fn passed(was: f32, now: f32, at: f32) -> bool {
    let at = at.rem_euclid(1.0);
    if was <= now { was < at && at <= now } else { was < at || at <= now }
}

/// A step's shape as drawn (see `Steps`): how far it reaches and its arms
/// and trunk swing (of its walk's), how high each foot is lifted, and its
/// trunk's lean (radians: forward, sideways, twisted).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    pub(crate) reach: f32,
    pub(crate) arms: f32,
    pub(crate) torso: f32,
    pub(crate) sway: f32,
    pub(crate) lift: [f32; 2],
    pub(crate) lean: Vec3,
}

impl Default for Shape {
    fn default() -> Self {
        Shape { reach: 1.0, arms: 1.0, torso: 1.0, sway: 1.0, lift: [1.0; 2], lean: Vec3::ZERO }
    }
}

/// How far each step's shape varies either way: its reach, its arms' swing
/// and its feet's lift (of its walk's), its lean (radians); all 0, every
/// step alike. (Spawn options `step_reach`, `step_arms`, `step_lift`,
/// `step_lean`.)
#[derive(Clone, Copy, Debug)]
pub(crate) struct Amounts {
    pub(crate) reach: f32,
    pub(crate) arms: f32,
    pub(crate) lift: f32,
    pub(crate) lean: f32,
}

/// No two steps alike (the user, 2026-11-05): each step's shape, drawn as it
/// comes (its reach and arms at a lull, from the last; each foot's lift as
/// it leaves the ground; its lean as a foot comes down, eased to), and how
/// they stand now; how far they vary, and how far its arms and trunk swing
/// at all (of its walk's: the user, 2026-11-05, its clip's arms too far;
/// 2026-11-07, its arms and trunk swinging the most human of it, a swagger).
#[derive(Clone, Debug)]
pub(crate) struct Steps {
    amounts: Amounts,
    arms: f32,
    torso: f32,
    sway: f32,
    from: Shape,
    to: Shape,
    lean: Vec3,
    draws: u32,
}

impl Steps {
    pub(crate) fn new(amounts: Amounts, arms: f32, torso: f32, sway: f32) -> Steps {
        Steps { amounts, arms, torso, sway, from: Shape::default(), to: Shape::default(), lean: Vec3::ZERO, draws: 0 }
    }

    /// Its phase gone from `was` to `now`: a new reach and arms drawn at a
    /// lull's start, a foot's lift as its swing begins, its lean as a foot
    /// comes down (at 0 and a half), each varied either way as far as its
    /// amounts say (`vary`: 0..1, uniform); its lean eased.
    pub(crate) fn step(&mut self, plan: &Plan, was: f32, now: f32, dt: f32, vary: impl Fn(u32, i32) -> f32) {
        let amounts = self.amounts;
        let mut draw = |k: i32| {
            self.draws = self.draws.wrapping_add(1);
            2.0 * vary(self.draws, k) - 1.0
        };
        for lull in plan.lulls {
            if passed(was, now, lull - LULL) {
                self.from.reach = self.to.reach;
                self.from.arms = self.to.arms;
                self.to.reach = 1.0 + amounts.reach * draw(0);
                self.to.arms = 1.0 + amounts.arms * draw(1);
            }
        }
        for (side, &(from, _)) in plan.swings.iter().enumerate() {
            if passed(was, now, from) {
                self.to.lift[side] = 1.0 + amounts.lift * draw(2);
            }
        }
        if passed(was, now, 0.0) || passed(was, now, 0.5) {
            self.to.lean = Vec3::new(draw(3), draw(4), draw(5)) * amounts.lean;
        }
        self.lean += (self.to.lean - self.lean) * (1.0 - (-dt / 0.3).exp());
    }

    /// How it stands at `phase`: its reach and arms eased from the last
    /// step's through a lull.
    pub(crate) fn now(&self, plan: &Plan, phase: f32) -> Shape {
        let ease = |t: f32| t * t * (3.0 - 2.0 * t);
        let t = plan.lulls.iter().find_map(|&l| within(phase, l - LULL, 2.0 * LULL)).map_or(1.0, ease);
        let arms = self.from.arms + (self.to.arms - self.from.arms) * t;
        Shape { reach: self.from.reach + (self.to.reach - self.from.reach) * t, arms: arms * self.arms, torso: self.torso, sway: self.sway, lift: self.to.lift, lean: self.lean }
    }
}

/// `rows` at `u` (0..1 through the clip): the constant, and each wave's
/// cosine and sine parts.
fn at(rows: &[Vec3], u: f32) -> Vec3 {
    let mut out = rows[0];
    for k in 1..=(rows.len() - 1) / 2 {
        let a = std::f32::consts::TAU * k as f32 * u;
        out += rows[2 * k - 1] * a.cos() + rows[2 * k] * a.sin();
    }
    out
}

impl Track {
    /// Its turn, and its place if the walk moves it, at `u`.
    pub(crate) fn at(&self, u: f32) -> (Quat, Option<Vec3>) {
        (self.turned(at(&self.turn, u)), self.place.as_ref().map(|p| at(p, u)))
    }

    fn turned(&self, v: Vec3) -> Quat {
        (self.mean * Quat::from_scaled_axis(v)).normalize()
    }

    /// Its turn's waves at `u`, `by` times as far either side of their
    /// constant (a thigh's swing, an arm's).
    fn swung(&self, u: f32, by: f32) -> Quat {
        self.turned(self.turn[0] + (at(&self.turn, u) - self.turn[0]) * by)
    }

    /// A knee's bend `by` times as deep through its swing (`from`, `long`
    /// long, of the stride: its foot in the air), from the line between how
    /// it stands at either end, so it is unchanged where its swing begins
    /// and ends.
    fn lifted(&self, plan: &Plan, phase: f32, (from, long): (f32, f32), by: f32) -> Quat {
        let u = |p: f32| (p + plan.offset).rem_euclid(1.0);
        let v = at(&self.turn, u(phase));
        let Some(t) = within(phase, from, long) else { return self.turned(v) };
        let line = at(&self.turn, u(from)).lerp(at(&self.turn, u(from + long)), t);
        self.turned(v + (v - line) * (by - 1.0))
    }

    /// Its place at `u` with its sway sideways (its parent's x) `by` times
    /// as far either side of its constant; and how far that moved it.
    fn swayed(&self, u: f32, by: f32) -> Option<(Vec3, f32)> {
        let rows = self.place.as_ref()?;
        let p = at(rows, u);
        let x = rows[0].x + (p.x - rows[0].x) * by;
        Some((p.with_x(x), p.x - x))
    }

    /// How far it is turned from straight (radians): a knee's bend.
    fn bend(&self, u: f32) -> f32 {
        let q = self.turned(at(&self.turn, u));
        2.0 * q.w.abs().min(1.0).acos()
    }
}

impl Gait {
    /// Where its steps may change their shape (see `Plan`), from its thighs,
    /// knees and arms, and its trunk (its spine from the hips up, and its
    /// head), as its `.creature` names them; its walk's clip at its phase 0
    /// is at `offset`.
    pub(crate) fn plan(&self, legs: &[[String; 3]; 2], arms: &[[String; 3]; 2], trunk: &[String], offset: f32) -> Result<Plan, String> {
        let find = |name: &String| self.tracks.iter().position(|t| &t.name == name).ok_or_else(|| format!("no waves for {name:?}, a joint its .creature names"));
        let thighs = [find(&legs[0][0])?, find(&legs[1][0])?];
        let knees = [find(&legs[0][1])?, find(&legs[1][1])?];
        let mut roles = vec![Role::Other; self.tracks.len()];
        for side in 0..2 {
            roles[thighs[side]] = Role::Thigh;
            roles[knees[side]] = Role::Knee(side);
            for name in &arms[side] {
                roles[find(name)?] = Role::Arm;
            }
        }
        let pelvis = find(trunk.first().ok_or("no spine")?)?;
        for name in trunk {
            roles[find(name)?] = Role::Trunk;
        }
        const N: usize = 400;
        let phase = |i: usize| i as f32 / N as f32;
        let u = |i: usize| (phase(i) + offset).rem_euclid(1.0);
        // Its lulls: both thighs nearest their middle, half a stride or so
        // apart.
        let off = |k: usize, i: usize| (at(&self.tracks[k].turn, u(i)) - self.tracks[k].turn[0]).length();
        let apart: Vec<f32> = (0..N).map(|i| off(thighs[0], i) + off(thighs[1], i)).collect();
        let least = |ok: &dyn Fn(usize) -> bool| (0..N).filter(|&i| ok(i)).min_by(|&a, &b| apart[a].total_cmp(&apart[b])).expect("some");
        let first = least(&|_| true);
        let round = |a: usize, b: usize| (phase(a) - phase(b)).rem_euclid(1.0).min((phase(b) - phase(a)).rem_euclid(1.0));
        let second = least(&|i| round(i, first) > 0.3);
        let mut lulls = [phase(first), phase(second)];
        lulls.sort_by(f32::total_cmp);
        // Each knee's swing: about its deepest bend, as long as it is bent
        // more than a quarter of the way from its least to its most.
        let mut swings = [(0.0, 0.0); 2];
        for side in 0..2 {
            let bends: Vec<f32> = (0..N).map(|i| self.tracks[knees[side]].bend(u(i))).collect();
            let (low, high) = bends.iter().fold((f32::MAX, f32::MIN), |(l, h), &b| (l.min(b), h.max(b)));
            if high - low < 10f32.to_radians() {
                return Err(format!("{:?} hardly bends walking ({:.0}° to {:.0}°): is it the knee?", legs[side][1], low.to_degrees(), high.to_degrees()));
            }
            let over = low + 0.25 * (high - low);
            let peak = (0..N).max_by(|&a, &b| bends[a].total_cmp(&bends[b])).expect("some");
            let (mut a, mut b) = (peak, peak);
            while bends[(a + N - 1) % N] > over && (peak + N - a) % N < N / 2 {
                a = (a + N - 1) % N;
            }
            while bends[(b + 1) % N] > over && (b + N - peak) % N < N / 2 {
                b = (b + 1) % N;
            }
            swings[side] = (phase(a), ((b + N - a) % N) as f32 / N as f32);
        }
        Ok(Plan { roles, lulls, swings, offset, pelvis })
    }
}

impl Gait {
    pub(crate) fn parse(text: &str) -> Result<Gait, String> {
        let (mut clip, mut waves, mut tracks, mut checks) = (None, None, Vec::<Track>::new(), Vec::new());
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fail = |what: &str| format!("line {}: {what}: {line}", n + 1);
            if let Some((k, v)) = line.split_once('=')
                && !line.contains('|')
            {
                match k.trim() {
                    "clip" => clip = Some(v.trim().to_string()),
                    "waves" => waves = Some(v.trim().parse::<usize>().map_err(|_| fail("waves: a whole number"))?),
                    _ => return Err(fail("not clip = or waves =")),
                }
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, '|').map(str::trim).collect();
            let [what, name, numbers] = parts.as_slice() else { return Err(fail("not `what | joint | numbers`")) };
            let words: Vec<&str> = numbers.split_whitespace().collect();
            let numbers: Vec<f32> = words.iter().map(|w| w.parse::<f32>()).collect::<Result<_, _>>().map_err(|_| fail("not numbers"))?;
            let waves = waves.ok_or_else(|| fail("waves = must come first"))?;
            let rows = |v: &[f32]| v.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect::<Vec<_>>();
            let count = 3 * (1 + 2 * waves);
            match *what {
                "rotation" if numbers.len() == 4 + count => tracks.push(Track { name: name.to_string(), mean: Quat::from_array([numbers[0], numbers[1], numbers[2], numbers[3]]).normalize(), turn: rows(&numbers[4..]), place: None }),
                "translation" if numbers.len() == count => match tracks.iter_mut().find(|t| t.name == *name) {
                    Some(t) => t.place = Some(rows(&numbers)),
                    None => return Err(fail("a translation before its joint's rotation")),
                },
                "check" if numbers.len() == 8 => checks.push((name.to_string(), numbers[0], Quat::from_array([numbers[1], numbers[2], numbers[3], numbers[4]]), Vec3::new(numbers[5], numbers[6], numbers[7]))),
                "rotation" | "translation" | "check" => return Err(fail(&format!("the wrong count of numbers for {waves} waves"))),
                _ => return Err(fail("not rotation, translation or check")),
            }
        }
        let clip = clip.ok_or("no `clip =` line")?;
        if tracks.is_empty() {
            return Err("no joints".into());
        }
        Ok(Gait { clip, tracks, checks, plan: None })
    }
}

/// With `walk=1`, its walk's joints set from its waves, each step shaped as
/// drawn (see `Steps`), over what its clips set, by the walk's share of
/// them; before `aim` turns them further (and leans its trunk as drawn).
pub(super) fn play(kits: Res<super::Kits>, bodies: Query<&Body>, mut joints: Query<&mut Transform, Without<Body>>, globals: Query<&GlobalTransform>, parents: Query<&ChildOf>) {
    for body in &bodies {
        let (Some(fitted), Some(steps)) = (&body.fitted, &body.shaping) else { continue };
        if body.walking <= 0.0 || body.fallen.is_some() || body.dying.is_some() {
            continue;
        }
        let spec = &kits.0[body.kit].spec;
        let Some(gait) = &kits.0[body.kit].gait else { continue };
        let plan = gait.plan.as_ref().expect("planned at load");
        let shape = steps.now(plan, body.phase);
        // (Where the clip would be: its walk's seek, see `animate`.)
        let u = (body.phase + spec.walk.numbers[1]).fract();
        // Its hips' turn quietened, its thighs turned back by as much; and
        // their sway sideways, each thigh rolled out by as much about the
        // hips' forward (its parent's z), as far as that leg is from its
        // hip down to its foot (as it stood last frame), so its feet stay
        // where they were (to within a millimetre, measured on the clip).
        let hips = &gait.tracks[plan.pelvis];
        let turned = hips.swung(u, shape.torso);
        let back = turned.inverse() * hips.at(u).0;
        let (hip_place, dx) = hips.swayed(u, shape.sway).map_or((None, 0.0), |(p, dx)| (Some(p), dx));
        let rolls = body.joints.as_ref().filter(|_| dx != 0.0).map_or([None; 2], |j| {
            // (Its hips' sway in the world: their parent's scale.)
            let unit = parents.get(j.spine[0]).ok().and_then(|p| globals.get(p.parent()).ok()).map_or(0.0, |g| g.to_scale_rotation_translation().0.x);
            j.legs.map(|leg| {
                let (Ok(hip), Ok(foot)) = (globals.get(leg[0]), globals.get(leg[2])) else { return None };
                let lever = (hip.translation().y - foot.translation().y).max(0.1);
                Some((leg[0], Quat::from_axis_angle(turned.inverse() * Vec3::Z, (dx * unit / lever).clamp(-0.5, 0.5).asin())))
            })
        });
        let roll = |e: Entity| rolls.iter().flatten().find(|r| r.0 == e).map_or(Quat::IDENTITY, |r| r.1);
        for &(e, k) in fitted {
            let Ok(mut t) = joints.get_mut(e) else { continue };
            let track = &gait.tracks[k];
            let (turn, place) = match plan.roles[k] {
                Role::Thigh => (roll(e) * back * track.swung(u, shape.reach), None),
                Role::Arm => (track.swung(u, shape.arms), None),
                Role::Knee(side) => (track.lifted(plan, body.phase, plan.swings[side], shape.lift[side]), None),
                Role::Trunk if k == plan.pelvis => (turned, hip_place),
                Role::Trunk => (track.swung(u, shape.torso), track.at(u).1),
                Role::Other => track.at(u),
            };
            t.rotation = t.rotation.slerp(turn, body.walking);
            if let Some(p) = place {
                t.translation = t.translation.lerp(p, body.walking);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's waves give what the fit gave (its `check` lines): one sum,
    /// written in Python and read here, the same.
    #[test]
    fn plays_as_fitted() {
        let path = super::super::folder().join("gladiator.gait");
        let gait = Gait::parse(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap();
        assert!(gait.checks.len() >= 6);
        for (name, u, q, p) in &gait.checks {
            let track = gait.tracks.iter().find(|t| &t.name == name).expect("a checked joint");
            let (turn, place) = track.at(*u);
            assert!(turn.dot(*q).abs() > 1.0 - 1e-5, "{name} at {u}: {turn} against {q}");
            if let Some(place) = place {
                assert!(place.distance(*p) < 1e-3 * p.length().max(1.0), "{name} at {u}: {place} against {p}");
            }
        }
    }

    fn gladiator() -> (Gait, Plan) {
        let read = |n: &str| std::fs::read_to_string(super::super::folder().join(n)).unwrap();
        let gait = Gait::parse(&read("gladiator.gait")).unwrap();
        let line = |key: &str| read("gladiator.creature").lines().find(|l| l.starts_with(key)).unwrap().split('=').nth(1).unwrap().split('#').next().unwrap().to_string();
        let limbs = |key: &str| {
            let l = line(key);
            let mut sides = l.split(',').map(|s| s.split_whitespace().map(String::from).collect::<Vec<_>>());
            let mut side = || <[String; 3]>::try_from(sides.next().unwrap()).unwrap();
            [side(), side()]
        };
        let offset: f32 = line("walk").split_whitespace().nth(2).unwrap().parse().unwrap();
        let mut trunk: Vec<String> = line("spine").split_whitespace().map(String::from).collect();
        trunk.push(line("head ").trim().to_string());
        let plan = gait.plan(&limbs("legs"), &limbs("arms"), &trunk, offset).unwrap();
        (gait, plan)
    }

    /// Its steps change shape where its thighs pass their middle, half a
    /// stride apart, and each knee's swing is in the air, the two half a
    /// stride apart (the gladiator's, read from its waves 2026-11-05: lulls
    /// about 0.40 and 0.90; its knees most bent about 0.88 and 0.40).
    #[test]
    fn plans_its_steps() {
        let (gait, plan) = gladiator();
        let near = |a: f32, b: f32, by: f32| (a - b).rem_euclid(1.0).min((b - a).rem_euclid(1.0)) < by;
        assert!(near(plan.lulls[0], 0.40, 0.05) && near(plan.lulls[1], 0.90, 0.05), "{:?}", plan.lulls);
        for (side, peak) in [(0, 0.88), (1, 0.40)] {
            let (from, long) = plan.swings[side];
            assert!((0.15..0.5).contains(&long), "{side}: {:?}", plan.swings);
            assert!(within(peak, from, long).is_some(), "{side}: {:?}", plan.swings);
        }
        assert_eq!(plan.roles.iter().filter(|r| **r == Role::Arm).count(), 6);
        assert_eq!(plan.roles.iter().filter(|r| **r == Role::Thigh).count(), 2);
        assert_eq!(plan.roles.iter().filter(|r| **r == Role::Trunk).count(), 6);
        assert_eq!(gait.tracks[plan.pelvis].name, "mixamorig:Hips");
    }

    /// Every step alike, at its clip's own arm swing: posed just as fitted;
    /// and a knee lifted more is unchanged where its swing begins and ends.
    #[test]
    fn unshaped_is_as_fitted() {
        let (gait, plan) = gladiator();
        let steps = Steps::new(Amounts { reach: 0.0, arms: 0.0, lift: 0.0, lean: 0.0 }, 1.0, 1.0, 1.0);
        let shape = steps.now(&plan, 0.3);
        for (k, track) in gait.tracks.iter().enumerate() {
            for i in 0..20 {
                let phase = i as f32 / 20.0;
                let u = (phase + plan.offset).rem_euclid(1.0);
                let got = match plan.roles[k] {
                    Role::Thigh => track.swung(u, shape.reach),
                    Role::Arm => track.swung(u, shape.arms),
                    Role::Knee(side) => track.lifted(&plan, phase, plan.swings[side], shape.lift[side]),
                    Role::Trunk => track.swung(u, shape.torso),
                    Role::Other => track.at(u).0,
                };
                assert!(got.dot(track.at(u).0).abs() > 1.0 - 1e-5, "{} at {phase}", track.name);
            }
            if let Role::Knee(side) = plan.roles[k] {
                let (from, long) = plan.swings[side];
                for p in [from, from + long - 1e-4] {
                    let u = (p + plan.offset).rem_euclid(1.0);
                    assert!(track.lifted(&plan, p, plan.swings[side], 1.4).dot(track.at(u).0).abs() > 1.0 - 1e-4, "{} at {p}", track.name);
                }
            }
        }
    }

    /// Its trunk quietened (the user, 2026-11-07: its arms and trunk
    /// swinging, a human swagger): its chest (hips to its spine's top, as
    /// joined) turns through its stride within a degree, where fitted it
    /// swings about 10° either way; and its thighs, turned back by its hips'
    /// lost turn, go as fitted.
    #[test]
    fn quiet_trunk() {
        let (gait, plan) = gladiator();
        let trunk: Vec<&Track> = ["mixamorig:Hips", "mixamorig:Spine", "mixamorig:Spine1", "mixamorig:Spine2"].iter().map(|n| gait.tracks.iter().find(|t| &t.name == n).unwrap()).collect();
        let chest = |u: f32, by: f32| trunk.iter().fold(Quat::IDENTITY, |q, t| q * t.swung(u, by));
        let spread = |by: f32| {
            let mean = chest(0.0, 0.0);
            (0..40).map(|i| chest(i as f32 / 40.0, by).angle_between(mean).to_degrees()).fold(0.0f32, f32::max)
        };
        let (quiet, fitted) = (spread(0.0), spread(1.0));
        assert!(quiet < 1.0 && fitted > 5.0, "chest swings {quiet:.1}° quiet, {fitted:.1}° fitted");
        let hips = &gait.tracks[plan.pelvis];
        let thigh = gait.tracks.iter().find(|t| t.name == "mixamorig:LeftUpLeg").unwrap();
        for i in 0..20 {
            let u = i as f32 / 20.0;
            let back = hips.swung(u, 0.0).inverse() * hips.at(u).0;
            let got = hips.swung(u, 0.0) * back * thigh.at(u).0;
            assert!(got.dot(hips.at(u).0 * thigh.at(u).0).abs() > 1.0 - 1e-5, "thigh at {u}");
        }
    }

    #[test]
    fn says_what_is_wrong() {
        assert!(Gait::parse("waves = 1\nrotation | a | 1 2").unwrap_err().contains("line 2"));
        assert!(Gait::parse("rotation | a | 0 0 0 1 0 0 0 0 0 0 0 0 0").unwrap_err().contains("waves = must come first"));
        assert!(Gait::parse("waves = 1").unwrap_err().contains("clip"));
        let ok = Gait::parse("clip = Walking\nwaves = 1\nrotation | a | 0 0 0 1 0 0 0 0.1 0 0 0 0 0").unwrap();
        assert!((ok.tracks[0].at(0.0).0.to_scaled_axis().x - 0.1).abs() < 1e-6);
    }
}
