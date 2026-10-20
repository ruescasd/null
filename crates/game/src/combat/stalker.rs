//! The stalker's mind: what it knows of you and what it does about it (the
//! user's review, 2026-10-19). Senses in, orders out, kept apart from the
//! body and the world so it can be tested alone.
//!
//! It sees you within `SIGHT` of where its head points (a cone of `CONE`),
//! nothing between (the caller's line of sight, the ground's and what is
//! built: it fired through a hill before); it hears you (your shots far,
//! a sprint nearer, a walk near) and turns to look, but never hunts by ear.
//! Seeing you, it hunts: its gun up, it walks into its band (`BAND`, near
//! enough to be a figure, not a dot on the horizon, the user), without
//! stopping, and there stands and fires; nearer than `BACK`, it comes no
//! nearer (backing off, it turned its back on you, a sitting duck, the
//! user). Running
//! across open ground in its reach, you draw its fire at once, wherever it
//! is (escape must not be trivial, the user). (Stopping to fire on its way
//! in, it took a minute to come in, sniping from afar.) Out of its sight a while, it goes where it saw you last and
//! searches, sweeping its head; then gives up and wanders.

use bevy::prelude::*;
use worldgen::noise::hash01;

/// How far it sees (m), and the cone round where its head points (degrees
/// across).
const SIGHT: f32 = 200.0;
const CONE: f32 = 120.0;
/// How far it hears your shots, a sprint, a walk (m); how fast is a sprint,
/// a walk (m/s); how long it looks where it heard you (s).
const HEAR: (f32, f32, f32) = (150.0, 25.0, 10.0);
const SPRINT: f32 = 6.0;
const WALK: f32 = 0.5;
const LISTEN: f32 = 3.0;
/// How far it fires (m); the band it keeps you in (m); nearer than this it
/// comes no nearer (m), even out of its band.
pub(super) const FIRE: f32 = 150.0;
const BAND: (f32, f32) = (20.0, 45.0);
const BACK: f32 = 15.0;
/// Its pace (m/s): hunting, and wandering.
const PACE: (f32, f32) = (1.0, 0.5);
/// Out of its sight this long (s), it searches where it saw you last, so
/// long (s), sweeping its head this often (s); then gives up.
const GRACE: f32 = 6.0;
const SEARCH: (f32, f32) = (20.0, 2.5);
/// Wandering: a new way this often (s), kept within so far of where it
/// began (m).
const WANDER: (f32, f32) = (8.0, 40.0);

/// What it is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mode {
    Wander,
    Hunt,
    Search,
}

/// What it knows and what it is about.
#[derive(Clone, Debug)]
pub(super) struct Mind {
    pub mode: Mode,
    /// Where it saw you last, and how long since (s).
    known: Vec3,
    unseen: f32,
    /// Where it heard you, and how long it looks there yet (s).
    heard: Option<(Vec3, f32)>,
    /// Searching or wandering: how long yet (s), and where it heads or looks.
    left: f32,
    way: Vec3,
    /// Where it began, and its seed (its wandering its own).
    home: Vec3,
    seed: i32,
    turns: i32,
}

/// What it senses this frame: whether it has a clear line to you, how fast
/// you are going (m/s, over the ground), and whether you fired.
pub(super) struct Senses {
    pub clear: bool,
    pub speed: f32,
    pub shot: bool,
}

/// What it does: how it goes (m/s), where it looks, and whether it may fire
/// now (standing, you in its sight and reach). (Its gun is up but when it
/// wanders: see its `mode`.)
#[derive(Debug)]
pub(super) struct Orders {
    pub velocity: Vec3,
    pub look: Vec3,
    pub fire: bool,
}

impl Mind {
    pub fn new(home: Vec3, seed: i32) -> Self {
        Self { mode: Mode::Wander, known: home, unseen: f32::MAX, heard: None, left: 0.0, way: Vec3::Z, home, seed, turns: 0 }
    }

    /// Whether it sees you (`you`, your middle) from `eye`, its head
    /// pointing `head` (a unit), its line to you clear or not.
    pub fn sees(eye: Vec3, head: Vec3, you: Vec3, clear: bool) -> bool {
        let to = you - eye;
        clear && to.length() < SIGHT && head.angle_between(to) < (CONE * 0.5).to_radians()
    }

    /// A frame's thought: where it stands (`root`), its eye and where its
    /// head points, where you are (your middle).
    pub fn think(&mut self, dt: f32, root: Vec3, eye: Vec3, head: Vec3, you: Vec3, senses: Senses) -> Orders {
        let sees = Self::sees(eye, head, you, senses.clear);
        let far = Vec2::new(you.x - root.x, you.z - root.z).length();
        // Hearing: it turns to look (never hunts by ear).
        let hears = (senses.shot && far < HEAR.0) || (senses.speed > SPRINT && far < HEAR.1) || (senses.speed > WALK && far < HEAR.2);
        if hears && !sees {
            self.heard = Some((you, LISTEN));
        }
        if let Some((_, t)) = self.heard.as_mut() {
            *t -= dt;
        }
        self.heard = self.heard.filter(|h| h.1 > 0.0);
        if sees {
            self.known = you;
            self.unseen = 0.0;
            self.mode = Mode::Hunt;
        } else {
            self.unseen += dt;
        }
        // Out of sight long enough: to where it saw you, to search.
        if self.mode == Mode::Hunt && self.unseen > GRACE {
            self.mode = Mode::Search;
            self.left = SEARCH.0;
            self.way = self.known;
        }
        let level = |p: Vec3| Vec3::new(p.x - root.x, 0.0, p.z - root.z);
        let eyes = |p: Vec3| Vec3::new(p.x, eye.y, p.z);
        match self.mode {
            Mode::Hunt => {
                let to = level(self.known);
                let d = to.length();
                let toward = to.normalize_or(Vec3::Z);
                // Running across open ground in its reach: it stands and
                // fires at once.
                let runner = sees && senses.speed > SPRINT && d < FIRE;
                let velocity = if d > BAND.1 && !runner {
                    toward * PACE.0
                } else {
                    Vec3::ZERO
                };
                // (Out of sight, its gun still on where it saw you, walking on
                // there: it has not given up yet.)
                let velocity = if sees { velocity } else { toward * PACE.0 * (d > BACK) as i32 as f32 };
                Orders { velocity, look: self.known, fire: sees && d < FIRE && velocity == Vec3::ZERO }
            }
            Mode::Search => {
                self.left -= dt;
                let to = level(self.way);
                if to.length() > 2.0 && self.left > SEARCH.0 - 12.0 {
                    // Going there, looking ahead.
                    return Orders { velocity: to.normalize() * PACE.0, look: eyes(self.way), fire: false };
                }
                if self.left <= 0.0 {
                    self.mode = Mode::Wander;
                    self.left = 0.0;
                }
                // There: sweeping its head round, a new way every so often.
                let sweep = (self.left / SEARCH.1) as i32;
                let a = hash01(self.seed, sweep, 7, 0x57a) * std::f32::consts::TAU;
                let look = self.heard.map_or(eye + Vec3::new(a.cos(), 0.0, a.sin()) * 10.0, |h| h.0);
                Orders { velocity: Vec3::ZERO, look, fire: false }
            }
            Mode::Wander => {
                self.left -= dt;
                if self.left <= 0.0 {
                    self.turns += 1;
                    self.left = WANDER.0;
                    // A new way, back towards where it began if gone far.
                    let a = hash01(self.seed, self.turns, 3, 0x57b) * std::f32::consts::TAU;
                    let out = Vec3::new(a.cos(), 0.0, a.sin());
                    let home = level(self.home);
                    self.way = if home.length() > WANDER.1 { home.normalize() } else { out };
                }
                let velocity = self.way * PACE.1;
                let look = self.heard.map_or(eye + self.way * 10.0, |h| h.0);
                Orders { velocity: if self.heard.is_some() { Vec3::ZERO } else { velocity }, look, fire: false }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stalker on flat ground: where it stands and where its head points,
    /// turning after where it looks (at most about 70 degrees a second, as
    /// its rig's head); thinking each step, you at `you(t)`, its line to you
    /// `clear(at, you)`, your speed `speed`.
    struct Sim {
        mind: Mind,
        root: Vec3,
        head: Vec3,
    }

    impl Sim {
        fn new(root: Vec3, head: Vec3) -> Self {
            Self { mind: Mind::new(root, 5), root, head }
        }

        fn run(&mut self, secs: f32, you: impl Fn(f32) -> Vec3, clear: impl Fn(Vec3, Vec3) -> bool, speed: f32) -> Vec<Orders> {
            let dt = 0.02;
            let mut out = Vec::new();
            for k in 0..(secs / dt) as usize {
                let t = k as f32 * dt;
                let eye = self.root + Vec3::Y * 2.5;
                let y = you(t);
                let o = self.mind.think(dt, self.root, eye, self.head, y, Senses { clear: clear(eye, y), speed, shot: false });
                self.root += o.velocity * dt;
                let want = (o.look - eye).with_y(0.0).normalize_or(self.head);
                let angle = self.head.angle_between(want);
                if angle > 1e-4 {
                    self.head = self.head.slerp(want, (1.2 * dt / angle).min(1.0)).normalize();
                }
                out.push(o);
            }
            out
        }
    }

    const OPEN: fn(Vec3, Vec3) -> bool = |_, _| true;

    /// Behind a hill (no clear line), in front of it and near, it never
    /// hunts you.
    #[test]
    fn not_through_hills() {
        let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
        let orders = s.run(10.0, |_| Vec3::new(0.0, 1.0, 30.0), |_, _| false, 0.0);
        assert_eq!(s.mind.mode, Mode::Wander);
        assert!(orders.iter().all(|o| !o.fire));
    }

    /// Standing still behind it, out of its cone, it does not notice you;
    /// before it, it does.
    #[test]
    fn sees_within_its_cone() {
        assert!(!Mind::sees(Vec3::Y * 2.5, Vec3::Z, Vec3::new(0.0, 1.0, -30.0), true), "behind it");
        assert!(Mind::sees(Vec3::Y * 2.5, Vec3::Z, Vec3::new(10.0, 1.0, 30.0), true), "before it");
        assert!(!Mind::sees(Vec3::Y * 2.5, Vec3::Z, Vec3::new(0.0, 1.0, 230.0), true), "too far");
        let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
        s.run(1.0, |_| Vec3::new(5.0, 1.0, 60.0), OPEN, 0.0);
        assert_eq!(s.mind.mode, Mode::Hunt);
    }

    /// Sprinting behind it, near, it hears you, turns, sees you, hunts.
    #[test]
    fn hears_and_turns() {
        let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
        s.run(4.0, |_| Vec3::new(0.0, 1.0, -20.0), OPEN, 8.0);
        assert_eq!(s.mind.mode, Mode::Hunt, "head {}", s.head);
    }

    /// Seen, then out of its sight: it holds on a while, then searches,
    /// then gives up and wanders.
    #[test]
    fn gives_up() {
        let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
        s.run(2.0, |_| Vec3::new(0.0, 1.0, 40.0), OPEN, 0.0);
        assert_eq!(s.mind.mode, Mode::Hunt);
        s.run(GRACE - 0.5, |_| Vec3::new(0.0, 1.0, 40.0), |_, _| false, 0.0);
        assert_eq!(s.mind.mode, Mode::Hunt, "too soon to give up");
        s.run(1.0, |_| Vec3::new(0.0, 1.0, 40.0), |_, _| false, 0.0);
        assert_eq!(s.mind.mode, Mode::Search);
        s.run(SEARCH.0 + 0.5, |_| Vec3::new(0.0, 1.0, 40.0), |_, _| false, 0.0);
        assert_eq!(s.mind.mode, Mode::Wander);
    }

    /// Far, it walks into its band and stands there, firing; near, it
    /// stands its ground, firing, never backing off (turning its back).
    #[test]
    fn keeps_its_band() {
        let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
        let orders = s.run(70.0, |_| Vec3::new(0.0, 1.0, 100.0), OPEN, 0.0);
        let d = 100.0 - s.root.z;
        println!("from 100 m it came to {d:.1} m");
        assert!(d > BAND.0 && d <= BAND.1 + 1.0, "it stands {d} m off");
        assert!(orders[orders.len() - 200..].iter().any(|o| o.fire), "it fires from its band");
        let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
        let orders = s.run(20.0, |_| Vec3::new(0.0, 1.0, 8.0), OPEN, 0.0);
        let d = 8.0 - s.root.z;
        println!("at 8 m it stood {d:.1} m off");
        assert!((d - 8.0).abs() < 0.01, "it moved to {d} m off");
        assert!(orders[orders.len() - 200..].iter().any(|o| o.fire), "it fires near too");
    }

    /// Running across open ground in its reach, you draw its fire at once:
    /// it never walks while you run.
    #[test]
    fn a_runner_draws_fire() {
        let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
        let orders = s.run(10.0, |t| Vec3::new(-60.0 + 8.0 * t, 1.0, 120.0), OPEN, 8.0);
        let late = &orders[100..];
        assert!(late.iter().all(|o| o.velocity == Vec3::ZERO), "it walked while you ran");
        assert!(late.iter().all(|o| o.fire), "it held its fire");
    }
}
