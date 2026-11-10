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
//! in, it took a minute to come in, sniping from afar.) Having seen you it
//! sheds its struggling walk for the gladiator's (`rig::Steps`), energetic;
//! and between its shots it mostly steps aside, its gun on you, gathering
//! as it goes, and fires as it plants its feet, its steps and its shot one
//! motion (`SHIFT`; standing dead still to fire was a turret's, and slow
//! steps were barely seen, without strength, the user, 2026-11-06). Out of
//! its sight a while, it goes where it saw you last and
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
/// Its pace (m/s): hunting (walking the gladiator's walk; its own 1.0, a
/// struggle), and wandering.
const PACE: (f32, f32) = (1.6, 0.5);
/// Between shots, hovering (`rig::HOVER`), it adjusts where it is all the
/// while, its gun on you (its `shift` option, 1; 0, standing), till it is
/// to fire: as its halo gathers again after a shot (`halo::AGAIN`), and is
/// gathered but for its lock (`halo::GATHERED`), it comes down on its
/// stilts, locks and fires, its landing the tell (the user, 2026-11-06:
/// the shifting the norm, not the exception). Its adjustments small and
/// sudden, at no beat: a dart, so long (s, at least and at most), as fast
/// as its `shift_pace` option (m/s) and how much either way of it; then a
/// hold, hovering, so long (s, at least and at most; mostly short, now and
/// then long); now and then a second dart at once (of its darts). Any way,
/// but towards you only half as far (nearer than `BACK`, never); and more
/// than so far (m) from where it settled, back towards it, give or take
/// (degrees). (Back and forth, each way 0.5 to 0.9 s at 3 to 4.5 m/s, it
/// read as a dance from side to side, predictable, smooth; it should read
/// as small adjustments to where it is, the user.)
pub(super) const SHIFT: f32 = 1.0;
const SHIFT_DART: (f32, f32) = (0.12, 0.4);
pub(super) const SHIFT_PACE: f32 = 3.75;
const SHIFT_SPREAD: f32 = 0.4;
const SHIFT_HOLD: (f32, f32) = (0.08, 1.0);
const SHIFT_TWICE: f32 = 0.2;
const SHIFT_ROAM: f32 = 2.5;
const SHIFT_HOME: f32 = 40.0;
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
    /// Whether it shifts between shots (`SHIFT`, 1, or 0); its shots as
    /// last told, how long since the last (s); shifting: which
    /// way round you (1 or -1), how slanted (radians), how fast (m/s), how
    /// long yet (s), and how far from you it began (m); and how long
    /// yet (s); the side it last went (1 or -1), and how many it has drawn.
    pub shifts: f32,
    /// How fast it shifts (m/s, `SHIFT_PACE`); and where it settled to
    /// fire, shifting about it.
    pub shift_pace: f32,
    post: Option<Vec3>,
    shots: i32,
    since: f32,
    shift: Option<Shift>,
    side: f32,
    draws: i32,
}

/// A step aside (see `Mind`'s `shift`).
#[derive(Clone, Copy, Debug)]
struct Shift {
    way: Vec3,
    left: f32,
}

/// What it senses this frame: whether it has a clear line to you, how fast
/// you are going (m/s, over the ground), whether you fired, and how many
/// times it has let go of its aim (fired, mostly).
pub(super) struct Senses {
    pub clear: bool,
    pub speed: f32,
    pub shot: bool,
    pub shots: i32,
}

/// What it does: how it goes (m/s), where it looks, whether it may fire
/// now (standing or stepping aside, you in its sight and reach), and
/// whether it faces you whichever way it goes (stepping aside), and
/// whether it hovers (going, or adjusting where it is between shots;
/// not, it comes down on its stilts: see `rig::HOVER`). (Its gun is up
/// but when it wanders: see its `mode`.)
#[derive(Debug)]
pub(super) struct Orders {
    pub velocity: Vec3,
    pub look: Vec3,
    pub fire: bool,
    pub strafe: bool,
    pub hover: bool,
}

impl Mind {
    pub fn new(home: Vec3, seed: i32) -> Self {
        Self { mode: Mode::Wander, known: home, unseen: f32::MAX, heard: None, left: 0.0, way: Vec3::Z, home, seed, turns: 0, shifts: SHIFT, shift_pace: SHIFT_PACE, post: None, shots: 0, since: f32::MAX, shift: None, side: if hash01(seed, 0, 9, 0x5f0) < 0.5 { -1.0 } else { 1.0 }, draws: 0 }
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
        // (Its first shot from where it stops, walking in.)
        self.since += dt;
        if senses.shots != self.shots {
            self.since = 0.0;
        }
        self.shots = senses.shots;
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
                // Between shots: hovering, small sudden adjustments to where
                // it is, darts and holds, its gun on you, till it is to
                // fire, gathered: it comes down, locks and fires. (Not
                // while it walks in, while you run, or out of its sight.)
                let fire_at = super::halo::AGAIN + super::halo::GATHERED;
                self.post = if velocity != Vec3::ZERO { None } else { self.post.or(Some(root)) };
                let adjusting = velocity == Vec3::ZERO && !runner && sees && self.since < fire_at && self.shifts >= 0.5;
                if !adjusting {
                    self.shift = None;
                } else if self.shift.is_none() {
                    self.draws += 1;
                    let r = |j: i32| hash01(self.seed, self.draws, j, 0x5f1);
                    // After a dart, mostly a hold; after a hold, a dart.
                    let darted = self.side > 0.0;
                    let dart = !darted || r(0) < SHIFT_TWICE;
                    self.side = if dart { 1.0 } else { -1.0 };
                    let left = fire_at - self.since;
                    self.shift = Some(if dart {
                        // Any way round, towards you half as far, never
                        // near; far from where it settled, back to it.
                        let a = r(1) * std::f32::consts::TAU;
                        let mut way = Vec3::new(a.cos(), 0.0, a.sin());
                        let near = way.dot(toward);
                        if near > 0.0 {
                            way -= toward * near * if d < BACK { 1.0 } else { 0.5 };
                        }
                        let off = self.post.map_or(Vec3::ZERO, |p| Vec3::new(p.x - root.x, 0.0, p.z - root.z));
                        if off.length() > SHIFT_ROAM {
                            let turn = (r(2) * 2.0 - 1.0) * SHIFT_HOME.to_radians();
                            way = Quat::from_rotation_y(turn) * off.normalize();
                        }
                        let pace = self.shift_pace * (1.0 + SHIFT_SPREAD * (2.0 * r(3) - 1.0));
                        let long = SHIFT_DART.0 + (SHIFT_DART.1 - SHIFT_DART.0) * r(4);
                        Shift { way: way.normalize_or(Vec3::X) * pace, left: long.min(left) }
                    } else {
                        let long = SHIFT_HOLD.0 + (SHIFT_HOLD.1 - SHIFT_HOLD.0) * r(5) * r(5);
                        Shift { way: Vec3::ZERO, left: long.min(left) }
                    });
                }
                if let Some(s) = self.shift.as_mut() {
                    s.left -= dt;
                }
                self.shift = self.shift.filter(|s| s.left > 0.0);
                let velocity = self.shift.map_or(velocity, |s| s.way);
                // (Facing you as it adjusts; hovering while it goes or
                // adjusts, come down only to fire.)
                let strafe = adjusting;
                Orders { velocity, look: self.known, fire: sees && d < FIRE && (velocity == Vec3::ZERO || strafe), strafe, hover: adjusting || velocity != Vec3::ZERO }
            }
            Mode::Search => {
                self.left -= dt;
                let to = level(self.way);
                if to.length() > 2.0 && self.left > SEARCH.0 - 12.0 {
                    // Going there, looking ahead.
                    return Orders { velocity: to.normalize() * PACE.0, look: eyes(self.way), fire: false, strafe: false, hover: true };
                }
                if self.left <= 0.0 {
                    self.mode = Mode::Wander;
                    self.left = 0.0;
                }
                // There: sweeping its head round, a new way every so often.
                let sweep = (self.left / SEARCH.1) as i32;
                let a = hash01(self.seed, sweep, 7, 0x57a) * std::f32::consts::TAU;
                let look = self.heard.map_or(eye + Vec3::new(a.cos(), 0.0, a.sin()) * 10.0, |h| h.0);
                Orders { velocity: Vec3::ZERO, look, fire: false, strafe: false, hover: false }
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
                Orders { velocity: if self.heard.is_some() { Vec3::ZERO } else { velocity }, look, fire: false, strafe: false, hover: false }
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
    /// (Its halo's part too, as `halo.rs`: told to fire, it gathers (s),
    /// stepping aside up to its lock, standing on to its shot, counted in
    /// `shots`; fired, it waits `AGAIN` (s since) to gather again.)
    struct Sim {
        mind: Mind,
        root: Vec3,
        head: Vec3,
        charge: f32,
        since: Option<f32>,
        shots: i32,
        still: f32,
        stood: Vec<f32>,
    }

    impl Sim {
        fn new(root: Vec3, head: Vec3) -> Self {
            Self { mind: Mind::new(root, 5), root, head, charge: 0.0, since: None, shots: 0, still: 0.0, stood: Vec::new() }
        }

        fn run(&mut self, secs: f32, you: impl Fn(f32) -> Vec3, clear: impl Fn(Vec3, Vec3) -> bool, speed: f32) -> Vec<Orders> {
            let dt = 0.02;
            let mut out = Vec::new();
            for k in 0..(secs / dt) as usize {
                let t = k as f32 * dt;
                let eye = self.root + Vec3::Y * 2.5;
                let y = you(t);
                let o = self.mind.think(dt, self.root, eye, self.head, y, Senses { clear: clear(eye, y), speed, shot: false, shots: self.shots });
                if let Some(t) = self.since.as_mut() {
                    *t += dt;
                }
                self.since = self.since.filter(|&t| o.fire && t <= super::super::halo::AGAIN);
                let standing = o.velocity == Vec3::ZERO && !o.hover;
                self.still = if standing { self.still + dt } else { 0.0 };
                if !o.fire {
                    self.charge = 0.0;
                } else if self.since.is_none() {
                    let gathered = super::super::halo::GATHERED;
                    self.charge = (self.charge + dt).min(if standing { gathered + LOCK } else { gathered });
                    if self.charge >= gathered + LOCK {
                        self.shots += 1;
                        self.stood.push(self.still);
                        self.charge = 0.0;
                        self.since = Some(0.0);
                    }
                }
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
    /// How long its aim is locked before it fires (s: its halo's, far).
    const LOCK: f32 = 0.5;

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
        let d = Vec2::new(s.root.x, 8.0 - s.root.z).length();
        println!("at 8 m it stood {d:.1} m off");
        assert!(d > 8.0 - 0.01, "it came nearer, to {d} m off");
        assert!(orders[orders.len() - 200..].iter().any(|o| o.fire), "it fires near too");
    }

    /// Between shots it adjusts where it is, hovering, darting and
    /// holding, facing you, then plants its feet and fires, as often as standing, still
    /// a while before each shot (the tell); within its band,
    /// and near, never nearer; with `shifts` 0, never.
    #[test]
    fn steps_aside_between_shots() {
        for (at, shifts) in [(30.0, SHIFT), (8.0, SHIFT), (30.0, 0.0)] {
            let mut s = Sim::new(Vec3::ZERO, Vec3::Z);
            s.mind.shifts = shifts;
            let orders = s.run(60.0, |_| Vec3::new(0.0, 1.0, at), OPEN, 0.0);
            let fast = orders.iter().map(|o| o.velocity.length()).fold(0.0, f32::max);
            let moving = orders.iter().filter(|o| o.velocity != Vec3::ZERO).count() as f32 * 0.02;
            let near = Vec2::new(s.root.x, at - s.root.z).length();
            println!("{at} m, shifts {shifts}: {} shots, moving {moving:.1} s of 60, at most {fast:.2} m/s, ended {:.1} m aside, {near:.1} m off", s.shots, s.root.x);
            assert!(s.shots >= 12, "it fired {} times in 60 s", s.shots);
            assert!(orders.iter().all(|o| o.velocity == Vec3::ZERO || o.strafe), "adjusting, it faces you");
            let least = if at < BACK { at - 0.01 } else { BAND.0 };
            assert!(near > least, "it came nearer, to {near} m");
            assert!(at >= BACK || near < at + 2.0, "near, it backed off, to {near} m");
            if shifts > 0.0 {
                assert!(moving > 6.0 && fast <= SHIFT_PACE * (1.0 + SHIFT_SPREAD) + 1e-3, "moving {moving} s, at most {fast} m/s");
                // Small adjustments, not walking off: within a few metres
                // of where it began.
                assert!(Vec2::new(s.root.x, at - s.root.z - at).length() < 6.0, "it went {} m aside", s.root.x);
                // At no beat: its darts and holds of many lengths.
                let mut runs = Vec::new();
                let mut run = 0;
                for o in &orders {
                    if o.velocity != Vec3::ZERO {
                        run += 1;
                    } else if run > 0 {
                        runs.push(run);
                        run = 0;
                    }
                }
                runs.sort();
                runs.dedup();
                assert!(runs.len() > 8, "its darts all alike: {runs:?}");
                // Still before each shot, its lock's time at least: the tell.
                let least = s.stood.iter().copied().fold(f32::MAX, f32::min);
                assert!(least >= LOCK - 0.03, "it stood only {least} s before a shot");
            } else {
                assert_eq!(moving, 0.0, "it moved");
            }
        }
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
