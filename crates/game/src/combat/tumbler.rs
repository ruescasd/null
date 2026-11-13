//! The tumbler (the user, 2026-11-13): an X of four long blades about a
//! small core (a diamond body, its point a head), flat. Its two diagonals
//! are cut from the core, each a rigid blade turning about where they cross,
//! so it can move six ways (`move=`), to compare:
//!
//! 1. It cartwheels (the user's thought; it might not work): a rimless
//!    wheel of four spokes rolling in its plane over one planted tip at a
//!    time, under its weight. It slows vaulting over the tip, falls onto the
//!    next and pushes off as that one lands (`push`). Its tips are unevenly
//!    spaced (58° and 122°, alternately), so it lurches, standing tall on
//!    two tips close together and squatting low on two far apart. `hub`: its
//!    core kept upright while its blades roll round it.
//! 2. It strides like dividers: one diagonal planted, the hinge vaulting
//!    over its tip, while the other windmills over the top, its upper tip
//!    coming down ahead as the next foot (swung under, it would scrape the
//!    ground and cut through the planted one). Its diagonals turn by turns
//!    about its core like a clock's hands; the X opens and shuts each step;
//!    its core stays upright.
//! 3. Both (the user, 2026-11-13): its short steps (from its squat across
//!    its near gap, up onto its close tips) cartwheeled under its weight,
//!    its long ones (from standing tall across its far gap, down into its
//!    squat) placed as it strides: its planted diagonal turned about its
//!    tip, the other reaching ahead to set its tip down, the X opening and
//!    shutting again (it lands squatting, as cartwheeling, so that the
//!    next step is short; striding on, it would stand tall, and every step
//!    be long). Its core stays upright.
//! 4. It spins flat like a thrown star: it rises, lies flat and spins,
//!    flies at a hover, then swings upright, its spin running down onto its
//!    feet.
//! 5. It closes into a spear: its feet drawn together under it, it rises
//!    onto one point, leans, leaps point first in an arc and sticks in the
//!    ground; then it swings up over the stuck point, opening as it does,
//!    and stands on the other end.
//! 6. As its shadow (the user's idea, 2026-11-13): a light (not shown)
//!    sinks behind it and its body collapses onto the ground along the
//!    light's rays, into its own shadow, stretched the way its face looks,
//!    front or back, whichever is nearer where it goes (flat, collapsed
//!    along its own plane its shadow would be a line). Its far end where
//!    it goes, it rises there: a light low beyond it rising overhead, it
//!    rising out of the shadow cast back the way it came (the two shadows,
//!    end to end, all but the same: it is near the same upside down).
//!    Its shadow need not be straight (the user: its ends not joined by a
//!    straight line, by stretching, not swimming): it curves from its face
//!    to where it goes, a circle's arc, the light's rays bending with it,
//!    its shadow laid along the curve as a ribbon. (Its shadow swam there
//!    first, but turned on its own axis as it followed a curve, a straight
//!    thing; trips of sweeps, reaches and pulls before that: worse, over
//!    complicated, the user.) On uneven ground its shadow
//!    falls where the light's rays first meet it (`fold.wgsl`): up a step
//!    in its way (no higher than `CLIMB`: it stops short of one higher),
//!    over a ledge and down.
//!
//! A menace (the user, 2026-11-13: never standing still doing nothing; it
//! goes somewhere to fire from there): it moves to a spot from which it
//! sees you, 8 to 16 m off you, swung round to one side or the other of
//! where it was; there its body (the rhombus under its head) glows a
//! moment, and it fires a burst at you, each a rhombus of its body's size
//! spinning about its flight, a trail from each of its four corners, one
//! every `gap=` s, as many as chance has it (1 to `shots=`); its rhombus
//! carries a vortex of light (`vortex.wgsl`) and gathers speed as it
//! flies, its trails the stalker's dark ray; then on to
//! the next spot. It fires only standing steady, never mid-turn. Arriving
//! where it cannot see you, it goes on at once.
//!
//! `spawn tumbler`: it hunts you, as above. `line`: its spots the ends of a
//! line across before you instead, face on, to judge its motion by.
//! `size=`, `push=`, `speed=`: to judge by feel; `log`: its pace each
//! second.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
};

use std::collections::VecDeque;

use crate::camera::FlyCam;

pub(super) mod crack;

/// Its mesh, in `creatures/` (the user's, generated; untouched).
const FILE: &str = "Meshy_AI_Remeshed_Compact_Core_1113024223_generate.glb";
/// Measured from the file, upright as its node stands it (m, its units):
/// its height; where its blades cross; its tips (anticlockwise, seen from
/// its face: upper right, upper left, lower left, lower right; upper right
/// and lower left one diagonal, the first); its radius of gyration about
/// the crossing (its surface's, in its plane); its core: within this of the
/// crossing, and its head (the core's point above it, so high, so wide).
const MESH_HEIGHT: f32 = 1.901;
const MESH_CENTRE: Vec2 = Vec2::new(0.008, 0.033);
const MESH_TIPS: [Vec2; 4] = [Vec2::new(0.523, 0.946), Vec2::new(-0.493, 0.950), Vec2::new(-0.544, -0.946), Vec2::new(0.546, -0.951)];
const MESH_GYRATION: f32 = 0.452;
const MESH_CORE: f32 = 0.14;
const MESH_HEAD: (f32, f32) = (0.34, 0.07);
/// Its core's body (the rhombus under its head): from its bottom point to
/// its top shoulders (its units); made this much taller, upwards, its head
/// lifted with it (the user, 2026-11-13: "a bit taller").
const MESH_BODY: (f32, f32) = (-0.15, 0.12);
const BODY: f32 = 1.4;
/// Its body's size (its units, as modelled: across, and through).
const MESH_BODY_SIZE: (f32, f32) = (0.264, 0.21);
/// Its attack: how far it sees you to (m); how long its glow winds up
/// (s); how long between shots (s, `gap=`: fixed, the user: what varies
/// is how many), how many at most (`shots=`; 0 none), how long after its
/// last before it moves on (s, its glow fading), how bright its glow (its
/// emissive). Its spots: so far from you (m), swung round from where it
/// was by so much (rad), either way; arrived, at rest within this of its
/// spot (m). Its rhombus: its speed setting off (m/s), how fast it gathers
/// speed (m/s², the user, 2026-11-13: not at one speed), its fastest
/// (m/s); its spin (rad/s; 7 was too quick, the user), how far it flies (m), how near you it hits you
/// (m) and how much it hurts; its vortex's brightness (see `vortex.wgsl`).
/// Its corners' trails (the stalker's dark ray, `beam.rs`, the user): how
/// long they last (s), how wide (m) and how much they darken what is
/// behind them (0..1).
const RANGE: f32 = 40.0;
const WIND: f32 = 0.45;
const GAP: f32 = 1.0;
const SHOTS: u32 = 4;
const AFTER: f32 = 0.25;
const BRIGHT: f32 = 40.0;
const SPOT_OFF: (f32, f32) = (8.0, 16.0);
const SPOT_SWING: (f32, f32) = (0.5, 1.4);
const ARRIVED: f32 = 2.5;
const BOLT_START: f32 = 4.0;
const BOLT_GATHER: f32 = 24.0;
const BOLT_TOP: f32 = 34.0;
const BOLT_SPIN: f32 = 1.5;
const BOLT_REACH: f32 = 60.0;
const BOLT_HIT: f32 = 0.55;
const BOLT_HARM: f32 = 20.0;
const TRAIL_LIFE: f32 = 0.5;
const TRAIL_WIDTH: f32 = 0.10;
const TRAIL_DARK: f32 = 0.85;
/// Its trails not drawn within this of your eye (m).
const TRAIL_CLEAR: f32 = 1.5;
const VORTEX: f32 = 6.0;
/// Its diagonals a little apart, either side of its plane (its units: so
/// they pass each other, shutting, as a pair of scissors' blades).
const MESH_APART: f32 = 0.05;
/// How tall it stands (m).
const HEIGHT: f32 = 4.0;
/// Cartwheeling: how much more than it needs to vault the next tip it
/// pushes off with (of that spin), and its fastest spin (rad/s).
const PUSH: f32 = 0.3;
const MAX_SPIN: f32 = 7.0;
/// How fast it turns its plane, about the tip it is on (rad/s).
const TURN: f32 = 1.2;
/// It stops at the first landing within this of where it is going (m, in
/// its plane), and sets off when that is further than this (m).
const THERE: f32 = 0.8;
const OFF: f32 = 2.0;
/// Striding: its pace (m/s; the hybrid's, and the stride's: the user,
/// 2026-11-13, an ingredient for its shadow, 20), and its quickest step
/// (s, at the hybrid's pace: quicker at a quicker pace).
const STRIDE_PACE: f32 = 2.2;
const STRIDE_FAST: f32 = 20.0;
const STRIDE_QUICKEST: f32 = 0.45;
/// The spear: how long it takes to shut (s), to lean (s), how far it leans
/// (rad, from upright: its leap's angle from level is the rest), its
/// shortest and longest leap (m), how deep it sticks (m), how long it stays
/// stuck (s), how long it takes to swing up over its point and open (s).
const SHUT: f32 = 0.5;
const LEAN: (f32, f32) = (0.35, 0.785);
const LEAP: (f32, f32) = (4.0, 12.0);
const STICK: f32 = 0.3;
const STUCK: f32 = 0.6;
const RISE: f32 = 0.8;
/// The star: how long it takes to rise and lie flat (s), how high it flies
/// (m, its centre over the ground), how fast (m/s), how fast it spins
/// (rad/s), how hard it brakes (m/s²), how long it takes to swing upright
/// (s).
const LIFT: f32 = 0.6;
const HOVER: f32 = 1.4;
const STAR_PACE: f32 = 7.0;
const STAR_SPIN: f32 = 18.0;
const BRAKE: f32 = 9.0;
const UPRIGHT: f32 = 0.7;
/// Its line across before you (`line`): how far ahead (m), how far either
/// side (m).
const AHEAD: f32 = 14.0;
const HALF: f32 = 9.0;
const G: f32 = 9.81;
/// As its shadow (ranges its chance is drawn from): how long leaving and
/// arriving each take (s,
/// give or take a fifth; `stretch=`: the user, 2026-11-13, slower, to see;
/// it was 0.9 to 1.3);
/// how low its light sinks at most (degrees over the horizon: its shadow
/// reaches its height over that angle's tangent at most, along its way; 10
/// m at its height of 2.4; lower, longer, its arms thinner, all lines past
/// some 15 m: the user, 2026-11-13, a light no lower, not a length, so it
/// goes as far as it is tall), and going further, how far aside of where
/// it goes it may aim (rad); how
/// far round from its face its shadow's far end may be (rad: its way
/// turns twice that). Its shadow laid this far off the ground (m, back
/// along the light's ray).
const SHADE_TIME: f32 = 1.7;
const SHADE_LOW: f32 = 13.5;
const SHADE_AIM: f32 = 0.4;
const SHADE_OFF: f32 = 1.3;
const SHADOW_LIFT: f32 = 0.015;
/// How much of leaving its body takes to collapse into its shadow, and of
/// arriving to rise out of it, by default (the rest: its light still
/// sinking, its shadow stretching; arriving, drawing in); 0.87 before (the
/// user, 2026-11-13: slow, it looked like falling down).
const SHADE_COLLAPSE: f32 = 0.3;
/// How much quicker than `SHADE_TIME` (about) all of it goes (the user,
/// 2026-11-13).
const SHADE_SPEED: f32 = 3.5;
/// The ground its shadow falls on: how many heights along its way, how far
/// past its far end (m); the highest step up its shadow climbs (m: under
/// its height, its light from above); finding where a ray meets it: at
/// most so many steps (m), then halvings (as `fold.wgsl`'s).
const PROFILE: usize = 128;
const PROFILE_BEYOND: f32 = 3.0;
const CLIMB: f32 = 1.9;
const RAY_STEPS: usize = 400;
const RAY_STEP: f32 = 0.08;
const RAY_HALVINGS: usize = 8;

/// Which diagonal a tip is on (0: upper right and lower left).
const fn diagonal(tip: usize) -> usize {
    tip % 2
}

/// A vector's angle (rad) from its plane's right, anticlockwise.
fn angle(v: Vec2) -> f32 {
    v.y.atan2(v.x)
}

/// An angle into (-π, π].
fn wrapped(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// How it is drawn: its centre (where its diagonals cross), its body's turn
/// (heading, then in its plane), each diagonal's turn in its plane (rad),
/// and its core's own turn if it is kept apart from its body's (upright).
#[derive(Clone, Copy, Debug)]
pub(super) struct Pose {
    pub(super) centre: Vec3,
    pub(super) body: Quat,
    pub(super) turns: [f32; 2],
    pub(super) core: Option<Quat>,
    /// Collapsed toward its shadow, how (see `Flat`).
    pub(super) flat: Option<Flat>,
}

/// Its tips, from its centre, in its plane (m), unturned; each tip's place
/// with its centre at `centre`, heading `yaw`, its diagonals turned by
/// `turns`.
#[derive(Clone, Debug)]
struct Frame {
    tips: [Vec2; 4],
}

impl Frame {
    fn new(scale: f32) -> Frame {
        Frame { tips: MESH_TIPS.map(|t| (t - MESH_CENTRE) * scale) }
    }

    fn arm(&self, j: usize, turns: [f32; 2]) -> Vec2 {
        Vec2::from_angle(turns[diagonal(j)]).rotate(self.tips[j])
    }

    /// Its plane's right, level, at heading `yaw`.
    fn across(yaw: f32) -> Vec3 {
        Quat::from_rotation_y(yaw) * Vec3::X
    }

    fn place(yaw: f32, v: Vec2) -> Vec3 {
        Frame::across(yaw) * v.x + Vec3::Y * v.y
    }

    /// The turn (both diagonals together) at which its two lower tips
    /// touch the ground together.
    fn level(&self) -> f32 {
        -angle(self.tips[3] - self.tips[2])
    }
}

/// Its rolling: a rimless wheel in its plane, on one tip (`on`, planted at
/// `pivot`), or resting on two.
#[derive(Clone, Debug)]
pub(super) struct Roll {
    frame: Frame,
    /// Its radius of gyration, squared (m²).
    k2: f32,
    on: usize,
    pivot: Vec3,
    /// Its turn in its plane (rad, anticlockwise seen from its face), how
    /// fast (rad/s); its plane's heading (rad, about up: 0 its right along
    /// +x); which way along its plane it rolls (+1 its right, -1 its left).
    angle: f32,
    spin: f32,
    yaw: f32,
    way: f32,
    /// At rest: the other tip it stands on.
    rest: Option<usize>,
    /// Its core kept upright.
    hub: bool,
}

impl Roll {
    /// Standing on its two lower tips over `at` (its ground), its face
    /// square to `yaw`'s.
    pub(super) fn new(scale: f32, at: Vec3, yaw: f32, hub: bool) -> Roll {
        let frame = Frame::new(scale);
        let mut roll = Roll { angle: frame.level(), frame, k2: (MESH_GYRATION * scale).powi(2), on: 3, pivot: Vec3::ZERO, spin: 0.0, yaw, way: 1.0, rest: Some(2), hub };
        // (Its centre over `at`, its lower tips on it.)
        roll.pivot = at + Frame::across(yaw) * roll.arm(3).x;
        roll
    }

    fn arm(&self, j: usize) -> Vec2 {
        self.frame.arm(j, [self.angle; 2])
    }

    pub(super) fn centre(&self) -> Vec3 {
        self.pivot - Frame::place(self.yaw, self.arm(self.on))
    }

    pub(super) fn tip(&self, j: usize) -> Vec3 {
        self.centre() + Frame::place(self.yaw, self.arm(j))
    }

    fn pose(&self) -> Pose {
        let heading = Quat::from_rotation_y(self.yaw);
        Pose { centre: self.centre(), body: heading * Quat::from_rotation_z(self.angle), turns: [0.0; 2], core: self.hub.then_some(heading), flat: None }
    }

    /// The spin it needs to vault the tip it is on: to lift its centre over
    /// it (rad/s).
    fn clear(&self) -> f32 {
        let d = -self.arm(self.on);
        (2.0 * G * (d.length() - d.y).max(0.0) / (self.k2 + d.length_squared())).sqrt()
    }

    /// A frame (`dt` s) towards `goal` (none: it stays): setting off from
    /// rest, rolling, landing on the next tip (pushing off again, or
    /// stopping there if it is there). How hard a tip landed (m/s), if one
    /// did.
    pub(super) fn step(&mut self, dt: f32, goal: Option<Vec3>, push: f32, ground: &dyn Fn(Vec3) -> f32) -> Option<f32> {
        if let Some(other) = self.rest {
            self.way = setting_off(goal?, self.centre(), self.yaw)?;
            // (Over the tip ahead.)
            let ahead = if self.arm(other).x * self.way > self.arm(self.on).x * self.way { other } else { self.on };
            self.pivot = self.tip(ahead);
            self.on = ahead;
            self.rest = None;
            self.spin = -self.way * self.clear() * (1.0 + push);
            return None;
        }
        self.yaw = turned(self.yaw, self.way, self.pivot, goal, dt);
        // Its weight about its tip.
        let d = -self.arm(self.on);
        self.spin += -G * d.x / (d.length_squared() + self.k2) * dt;
        self.angle += self.spin * dt;
        // Another tip down.
        let centre = self.centre();
        let (j, t) = (0..4).filter(|&j| j != self.on).map(|j| (j, self.tip(j))).find(|&(_, t)| t.y < ground(t))?;
        let across = Frame::across(self.yaw);
        let flat = |v: Vec3| Vec2::new(v.dot(across), v.y);
        let was = flat(centre - self.pivot);
        let moving = Vec2::new(-was.y, was.x) * self.spin;
        let pivot = t.with_y(ground(t));
        let from = flat(centre - pivot);
        // (Its turn about the new tip as it lands: what it had about it,
        // kept.)
        let after = (self.k2 * self.spin + from.perp_dot(moving)) / (self.k2 + from.length_squared());
        let before = self.on;
        // (Its landed tip on the ground; its centre lifted out by as much
        // as it had sunk, a few millimetres.)
        self.on = j;
        self.pivot = pivot;
        if goal.is_none_or(|g| there(g, self.centre(), self.yaw, self.way)) {
            self.rest = Some(before);
            self.spin = 0.0;
        } else {
            self.spin = -self.way * (self.clear() * (1.0 + push)).max(-self.way * after).min(MAX_SPIN);
        }
        Some(moving.length())
    }
}

/// Whether `goal` is reached from `centre`, rolling `way` along a plane
/// headed `yaw`: within `THERE` ahead, or behind.
fn there(goal: Vec3, centre: Vec3, yaw: f32, way: f32) -> bool {
    (goal - centre).with_y(0.0).dot(Frame::across(yaw) * way) < THERE
}

/// Which way along its plane it sets off for `goal`, if it is far enough.
fn setting_off(goal: Vec3, centre: Vec3, yaw: f32) -> Option<f32> {
    let to = (goal - centre).with_y(0.0);
    (to.length() >= OFF).then(|| if to.dot(Frame::across(yaw)) >= 0.0 { 1.0 } else { -1.0 })
}

/// Its heading turned a frame (`dt` s) towards `goal`, from `from`, going
/// `way` along its plane.
fn turned(yaw: f32, way: f32, from: Vec3, goal: Option<Vec3>, dt: f32) -> f32 {
    let Some(goal) = goal else { return yaw };
    let want = (goal - from).with_y(0.0).normalize_or_zero() * way;
    if want == Vec3::ZERO {
        return yaw;
    }
    yaw + wrapped(f32::atan2(-want.z, want.x) - yaw).clamp(-TURN * dt, TURN * dt)
}

/// Striding like dividers (see the module's account): one tip planted, its
/// diagonal turning about it while the other windmills over to land ahead.
#[derive(Clone, Debug)]
struct Stride {
    frame: Frame,
    on: usize,
    pivot: Vec3,
    yaw: f32,
    way: f32,
    turns: [f32; 2],
    /// The step under way: how far through it (0..1), how long it takes
    /// (s), each diagonal's turn at its start and how far it turns.
    through: f32,
    period: f32,
    from: [f32; 2],
    by: [f32; 2],
    pace: f32,
    rest: bool,
}

impl Stride {
    fn new(scale: f32, at: Vec3, yaw: f32, pace: f32) -> Stride {
        let frame = Frame::new(scale);
        let level = frame.level();
        let mut stride = Stride { frame, on: 3, pivot: Vec3::ZERO, yaw, way: 1.0, turns: [level; 2], through: 0.0, period: 1.0, from: [level; 2], by: [0.0; 2], pace, rest: true };
        stride.pivot = at + Frame::across(yaw) * stride.arm(3).x;
        stride
    }

    fn arm(&self, j: usize) -> Vec2 {
        self.frame.arm(j, self.turns)
    }

    fn centre(&self) -> Vec3 {
        self.pivot - Frame::place(self.yaw, self.arm(self.on))
    }

    fn pose(&self) -> Pose {
        let heading = Quat::from_rotation_y(self.yaw);
        Pose { centre: self.centre(), body: heading, turns: self.turns, core: Some(heading), flat: None }
    }

    /// The next step planned from where it stands on `on` (planted): its
    /// planted diagonal to turn its hinge over to the mirror of where it is
    /// behind it, the other's far tip (from the ground) to land ahead at the
    /// height of `on`.
    fn plan(&mut self) {
        let planted = diagonal(self.on);
        let swung = 1 - planted;
        let hip = -self.arm(self.on);
        let lean = hip.x.atan2(hip.y);
        // (Its lower tip lifts; its other comes down ahead.)
        let (p, q) = (swung, swung + 2);
        let landing = if self.arm(p).y > self.arm(q).y { p } else { q };
        let tip = self.arm(landing);
        let length = tip.length();
        let low = -hip.y;
        let end = Vec2::new(self.way * (length * length - low * low).max(0.0).sqrt(), low);
        let need = angle(end) - angle(tip);
        let swing = if self.way > 0.0 { -(-need).rem_euclid(std::f32::consts::TAU) } else { need.rem_euclid(std::f32::consts::TAU) };
        self.from = self.turns;
        self.by = [0.0; 2];
        self.by[planted] = 2.0 * lean;
        self.by[swung] = swing;
        self.through = 0.0;
        // (Its hinge goes twice its lean's reach forward.)
        self.period = (2.0 * hip.x.abs() / self.pace).max(STRIDE_QUICKEST * STRIDE_PACE / self.pace);
    }

    fn step(&mut self, dt: f32, goal: Option<Vec3>, ground: &dyn Fn(Vec3) -> f32) {
        if self.rest {
            let Some(way) = goal.and_then(|g| setting_off(g, self.centre(), self.yaw)) else { return };
            self.way = way;
            // (On the tip ahead of the two it stands on, its lowest two.)
            let mut low: Vec<usize> = (0..4).collect();
            low.sort_by(|&a, &b| self.arm(a).y.total_cmp(&self.arm(b).y));
            let ahead = low[..2].iter().copied().max_by(|&a, &b| (self.arm(a).x * way).total_cmp(&(self.arm(b).x * way))).expect("two");
            let centre = self.centre();
            self.on = ahead;
            self.pivot = centre + Frame::place(self.yaw, self.arm(ahead));
            self.rest = false;
            self.plan();
            return;
        }
        // (Turning as far for each metre at any pace: at its pace for each
        // second, quicker, it went past a goal off to its side before it
        // turned to it, stepping back and forth for ever.)
        self.yaw = turned(self.yaw, self.way, self.pivot, goal, dt * self.pace / STRIDE_PACE);
        self.through = (self.through + dt / self.period).min(1.0);
        let s = self.through;
        // (Its planted diagonal evenly; the swung one gathering, then
        // falling to its tip, quickest as it lands.)
        let swing = s * s * (3.0 - 2.0 * s);
        let planted = diagonal(self.on);
        for d in 0..2 {
            self.turns[d] = self.from[d] + self.by[d] * if d == planted { s } else { swing };
        }
        if self.through < 1.0 {
            return;
        }
        // Landed: on its new foot, on the ground.
        let swung = 1 - planted;
        let centre = self.centre();
        let landed = (0..4).filter(|&j| diagonal(j) == swung).min_by(|&a, &b| self.arm(a).y.total_cmp(&self.arm(b).y)).expect("two");
        let at = centre + Frame::place(self.yaw, self.arm(landed));
        self.on = landed;
        self.pivot = at.with_y(ground(at));
        if goal.is_none_or(|g| there(g, self.centre(), self.yaw, self.way)) {
            self.rest = true;
        } else {
            self.plan();
        }
    }
}

/// The spear's doings (see the module's account), each with how far through
/// it is (s).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Throw {
    Stand,
    Shut(f32),
    Lean(f32),
    Fly,
    Stuck(f32),
    Rise(f32),
}

/// Shutting into a spear and leaping.
#[derive(Clone, Debug)]
struct Spear {
    frame: Frame,
    centre: Vec3,
    yaw: f32,
    way: f32,
    /// Its body's turn in its plane (rad), and how shut (0 open, 1 a
    /// spear).
    turn: f32,
    shut: f32,
    doing: Throw,
    /// Flying: its velocity, and the tip it leads with. Leaning and stuck:
    /// the point it is on (its tip `lead` when stuck), and the turn it began
    /// from; rising, the turn it ends on.
    velocity: Vec3,
    lead: usize,
    point: Vec3,
    began: f32,
    ends_on: f32,
    /// How far its leap is to go (m).
    reach: f32,
}

impl Spear {
    fn new(scale: f32, at: Vec3, yaw: f32) -> Spear {
        let frame = Frame::new(scale);
        let turn = frame.level();
        let mut spear = Spear { frame, centre: at, yaw, way: 1.0, turn, shut: 0.0, doing: Throw::Stand, velocity: Vec3::ZERO, lead: 0, point: at, began: turn, ends_on: turn, reach: 0.0 };
        spear.centre = at + Vec3::Y * spear.standing();
        spear
    }

    /// Each diagonal's turn, shut by `shut`: both turned to lie along the
    /// line through its upper and lower tips' middles.
    fn turns(&self) -> [f32; 2] {
        let t = self.frame.tips;
        let along = |a: usize, b: usize| wrapped(angle(t[a] - t[b]) - std::f32::consts::FRAC_PI_2);
        [self.turn - along(0, 2) * self.shut, self.turn - along(1, 3) * self.shut]
    }

    fn arm(&self, j: usize) -> Vec2 {
        self.frame.arm(j, self.turns())
    }

    /// How high its centre stands over the ground on its lowest tips.
    fn standing(&self) -> f32 {
        (0..4).map(|j| -self.arm(j).y).fold(f32::MIN, f32::max)
    }

    /// Its lowest tip, and its highest.
    fn ends(&self) -> (usize, usize) {
        let by = |a: &usize, b: &usize| self.arm(*a).y.total_cmp(&self.arm(*b).y);
        ((0..4).min_by(by).expect("four"), (0..4).max_by(by).expect("four"))
    }

    fn pose(&self) -> Pose {
        let heading = Quat::from_rotation_y(self.yaw);
        let turns = self.turns();
        Pose { centre: self.centre, body: heading, turns, core: Some(heading * Quat::from_rotation_z((turns[0] + turns[1]) / 2.0)), flat: None }
    }

    fn step(&mut self, dt: f32, goal: Option<Vec3>, ground: &dyn Fn(Vec3) -> f32) {
        let ease = |s: f32| s * s * (3.0 - 2.0 * s);
        match self.doing {
            Throw::Stand => {
                let Some(goal) = goal else { return };
                let Some(way) = setting_off(goal, self.centre, self.yaw) else { return };
                self.way = way;
                self.reach = ((goal - self.centre).with_y(0.0).length() - THERE * 0.5).clamp(LEAP.0, LEAP.1);
                self.doing = Throw::Shut(0.0);
            }
            Throw::Shut(t) => {
                let t = t + dt;
                // (Its feet drawn together under it, its centre rising.)
                self.shut = ease((t / SHUT).min(1.0));
                self.yaw = turned(self.yaw, self.way, self.centre, goal, dt);
                self.centre.y = ground(self.centre) + self.standing();
                self.doing = if t < SHUT { Throw::Shut(t) } else { Throw::Lean(0.0) };
                if t >= SHUT {
                    let low = self.ends().0;
                    self.point = self.centre + Frame::place(self.yaw, self.arm(low));
                    self.began = self.turn;
                }
            }
            Throw::Lean(t) => {
                let t = t + dt;
                // (Over its point, its top leaning ahead.)
                self.turn = self.began - self.way * LEAN.1 * ease((t / LEAN.0).min(1.0));
                let low = self.ends().0;
                self.centre = self.point - Frame::place(self.yaw, self.arm(low));
                if t >= LEAN.0 {
                    // (Leaping along itself, its far end first: so fast as
                    // to land `reach` off.)
                    let rise = std::f32::consts::FRAC_PI_2 - LEAN.1;
                    let speed = (G * self.reach / (2.0 * rise).sin()).sqrt();
                    self.velocity = Frame::place(self.yaw, Vec2::new(self.way * rise.cos(), rise.sin())) * speed;
                    self.lead = self.ends().1;
                    self.doing = Throw::Fly;
                } else {
                    self.doing = Throw::Lean(t);
                }
            }
            Throw::Fly => {
                self.velocity.y -= G * dt;
                self.centre += self.velocity * dt;
                // (Along its flight, its leading end first.)
                let flight = Vec2::new(self.velocity.dot(Frame::across(self.yaw)), self.velocity.y);
                self.turn += wrapped(angle(flight) - angle(self.arm(self.lead)));
                let tip = self.centre + Frame::place(self.yaw, self.arm(self.lead));
                if tip.y < ground(tip) - STICK {
                    self.point = tip;
                    self.began = self.turn;
                    // (To stand on its stuck end: the level turn with its
                    // stuck tip below, nearest to that tip straight down.)
                    let down = self.turn + wrapped(-std::f32::consts::FRAC_PI_2 - angle(self.arm(self.lead)));
                    let level = self.frame.level();
                    let mut end = level + ((down - level) / std::f32::consts::PI).round() * std::f32::consts::PI;
                    if self.frame.arm(self.lead, [end; 2]).y > 0.0 {
                        end += std::f32::consts::PI * (down - end).signum();
                    }
                    self.ends_on = end;
                    self.doing = Throw::Stuck(0.0);
                }
            }
            Throw::Stuck(t) => {
                let t = t + dt;
                self.doing = if t < STUCK { Throw::Stuck(t) } else { Throw::Rise(0.0) };
            }
            Throw::Rise(t) => {
                let t = t + dt;
                let s = ease((t / RISE).min(1.0));
                // (Swinging up over its stuck point onto that end, opening,
                // pulling it out.)
                self.turn = self.began + (self.ends_on - self.began) * s;
                self.shut = 1.0 - s;
                let pulled = self.point.with_y(self.point.y + (ground(self.point) - self.point.y) * s);
                self.centre = pulled - Frame::place(self.yaw, self.arm(self.lead));
                self.doing = if t < RISE { Throw::Rise(t) } else { Throw::Stand };
            }
        }
    }
}

/// The star's doings, each with how far through it is (s).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Spin {
    Stand,
    Lift(f32),
    Fly,
    Down(f32),
}

/// Spinning flat like a thrown star.
#[derive(Clone, Debug)]
struct Star {
    frame: Frame,
    centre: Vec3,
    yaw: f32,
    /// Its turn in its plane (rad), how fast (rad/s); how flat (0 upright,
    /// 1 flat); how fast it flies (m/s, along `heading`).
    turn: f32,
    spin: f32,
    flat: f32,
    speed: f32,
    heading: Vec3,
    pace: f32,
    /// Its centre's height flying (m: `HOVER` at 2.4 m tall, as tall).
    hover: f32,
    doing: Spin,
    /// Coming down: where from (its centre, its turn) and where to.
    from: (Vec3, f32),
    to: (Vec3, f32),
}

impl Star {
    fn new(scale: f32, at: Vec3, yaw: f32, pace: f32) -> Star {
        let frame = Frame::new(scale);
        let turn = frame.level();
        let mut star = Star { frame, centre: at, yaw, turn, spin: 0.0, flat: 0.0, speed: 0.0, heading: Vec3::X, pace, hover: HOVER * scale * MESH_HEIGHT / 2.4, doing: Spin::Stand, from: (at, turn), to: (at, turn) };
        star.centre = at + Vec3::Y * star.standing(turn);
        star
    }

    fn standing(&self, turn: f32) -> f32 {
        (0..4).map(|j| -self.frame.arm(j, [turn; 2]).y).fold(f32::MIN, f32::max)
    }

    fn pose(&self) -> Pose {
        let body = Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 * self.flat) * Quat::from_rotation_z(self.turn);
        Pose { centre: self.centre, body, turns: [0.0; 2], core: None, flat: None }
    }

    fn step(&mut self, dt: f32, goal: Option<Vec3>, ground: &dyn Fn(Vec3) -> f32) {
        let ease = |s: f32| s * s * (3.0 - 2.0 * s);
        match self.doing {
            Spin::Stand => {
                if goal.and_then(|g| setting_off(g, self.centre, self.yaw)).is_some() {
                    self.from = (self.centre, self.turn);
                    self.doing = Spin::Lift(0.0);
                }
            }
            Spin::Lift(t) => {
                let t = t + dt;
                let s = ease((t / LIFT).min(1.0));
                self.flat = s;
                self.spin = STAR_SPIN * s;
                self.turn += self.spin * dt;
                self.centre.y = self.from.0.y.lerp(ground(self.centre) + self.hover, s);
                self.doing = if t < LIFT { Spin::Lift(t) } else { Spin::Fly };
            }
            Spin::Fly => {
                self.turn += self.spin * dt;
                // (Nowhere to go: down where it is.)
                let goal = goal.unwrap_or(self.centre);
                let to = (goal - self.centre).with_y(0.0);
                let left = to.length();
                self.heading = to.normalize_or(self.heading);
                // (Braking so as to stop over it.)
                let most = (2.0 * BRAKE * left).sqrt();
                self.speed = (self.speed + BRAKE * dt).min(self.pace).min(most);
                self.centre += self.heading * self.speed * dt;
                self.centre.y = ground(self.centre) + self.hover;
                if left < THERE {
                    // (Its spin run down onto its feet: the turn at which it
                    // stands level that it reaches, slowing evenly.)
                    let level = self.frame.level();
                    let coast = self.turn + self.spin * UPRIGHT / 2.0;
                    let k = ((coast - level) / std::f32::consts::PI).ceil();
                    let end = level + k * std::f32::consts::PI;
                    self.from = (self.centre, self.turn);
                    self.to = (self.centre.with_y(ground(self.centre) + self.standing(end)), end);
                    self.doing = Spin::Down(0.0);
                }
            }
            Spin::Down(t) => {
                let t = t + dt;
                let s = (t / UPRIGHT).min(1.0);
                // (Slowing evenly to a stop, upright as it does.)
                let run = 1.0 - (1.0 - s) * (1.0 - s);
                self.turn = self.from.1 + (self.to.1 - self.from.1) * run;
                self.flat = 1.0 - ease(s);
                self.centre = self.from.0.lerp(self.to.0, ease(s));
                // (No tip into the ground as it swings upright.)
                let body = self.pose().body;
                let lowest = (0..4).map(|j| (body * self.frame.tips[j].extend(0.0)).y).fold(f32::MAX, f32::min);
                self.centre.y = self.centre.y.max(ground(self.centre) - lowest);
                self.spin = 0.0;
                self.doing = if t < UPRIGHT { Spin::Down(t) } else { Spin::Stand };
            }
        }
    }
}

/// A step of its hybrid (`move=3`): at rest; a long step (from its tall
/// stance across its far gap), placed as it strides: its planted diagonal
/// turned evenly about its tip, the other reaching ahead (quickest at
/// first) to set down the tip `to`, both turned by `by` in the end (its X
/// as it was, squatting); a short step (from its squat across its near
/// gap), rolled as it cartwheels, under its weight.
#[derive(Clone, Debug)]
enum Phase {
    Rest,
    Long { through: f32, period: f32, from: [f32; 2], by: f32, to: usize },
    Short { spin: f32 },
}

/// Cartwheeling its short steps and striding its long ones (the user,
/// 2026-11-13; see the module's account).
#[derive(Clone, Debug)]
struct Hybrid {
    frame: Frame,
    k2: f32,
    on: usize,
    pivot: Vec3,
    yaw: f32,
    way: f32,
    turns: [f32; 2],
    phase: Phase,
    pace: f32,
}

impl Hybrid {
    fn new(scale: f32, at: Vec3, yaw: f32, pace: f32) -> Hybrid {
        let frame = Frame::new(scale);
        let level = frame.level();
        let mut h = Hybrid { frame, k2: (MESH_GYRATION * scale).powi(2), on: 3, pivot: Vec3::ZERO, yaw, way: 1.0, turns: [level; 2], phase: Phase::Rest, pace };
        h.pivot = at + Frame::across(yaw) * h.arm(3).x;
        h
    }

    fn arm(&self, j: usize) -> Vec2 {
        self.frame.arm(j, self.turns)
    }

    fn centre(&self) -> Vec3 {
        self.pivot - Frame::place(self.yaw, self.arm(self.on))
    }

    fn pose(&self) -> Pose {
        let heading = Quat::from_rotation_y(self.yaw);
        Pose { centre: self.centre(), body: heading, turns: self.turns, core: Some(heading), flat: None }
    }

    /// The tip it next lands on, rolling `way` whole over the tip it is
    /// on: how far round from it (rad), and how far it turns to land it
    /// (rad, none if it never would).
    fn next(&self) -> (usize, f32, Option<f32>) {
        let from = angle(self.arm(self.on));
        let (to, gap) = (0..4).filter(|&j| j != self.on).map(|j| (j, ((angle(self.arm(j)) - from) * self.way).rem_euclid(std::f32::consts::TAU))).min_by(|a, b| a.1.total_cmp(&b.1)).expect("three");
        // (Turned whole by `t`, the height of `to` over the tip it is on.)
        let over = |t: f32| Vec2::from_angle(t).rotate(self.arm(to)).y - Vec2::from_angle(t).rotate(self.arm(self.on)).y;
        let end = -self.way * std::f32::consts::PI;
        let samples = 90;
        let mut turn = None;
        for k in 1..=samples {
            let (a, b) = (end * (k - 1) as f32 / samples as f32, end * k as f32 / samples as f32);
            if over(a) > 0.0 && over(b) <= 0.0 {
                let (mut lo, mut hi) = (a, b);
                for _ in 0..30 {
                    let mid = (lo + hi) / 2.0;
                    if over(mid) > 0.0 { lo = mid } else { hi = mid }
                }
                turn = Some(hi);
                break;
            }
        }
        (to, gap, turn)
    }

    /// The spin it needs to vault the tip it is on (rad/s).
    fn clear(&self) -> f32 {
        let d = -self.arm(self.on);
        (2.0 * G * (d.length() - d.y).max(0.0) / (self.k2 + d.length_squared())).sqrt()
    }

    /// Its next step begun from where it stands on `on`: long across a far
    /// gap, short across a near one.
    fn begin(&mut self, push: f32) {
        let (to, gap, turn) = self.next();
        self.phase = match turn {
            Some(by) if gap > std::f32::consts::FRAC_PI_2 => {
                // (Its centre goes as far as its landing tip is from its
                // planted one.)
                let span = (self.arm(to) - self.arm(self.on)).length();
                Phase::Long { through: 0.0, period: (span / self.pace).max(STRIDE_QUICKEST), from: self.turns, by, to }
            }
            _ => Phase::Short { spin: -self.way * self.clear() * (1.0 + push) },
        };
    }

    /// Landed on `j`: on it, on the ground; there, at rest, else the next
    /// step.
    fn land(&mut self, j: usize, goal: Option<Vec3>, push: f32, ground: &dyn Fn(Vec3) -> f32) {
        let tip = self.centre() + Frame::place(self.yaw, self.arm(j));
        self.on = j;
        self.pivot = tip.with_y(ground(tip));
        if goal.is_none_or(|g| there(g, self.centre(), self.yaw, self.way)) {
            self.phase = Phase::Rest;
        } else {
            self.begin(push);
        }
    }

    fn step(&mut self, dt: f32, goal: Option<Vec3>, push: f32, ground: &dyn Fn(Vec3) -> f32) {
        match self.phase.clone() {
            Phase::Rest => {
                let Some(way) = goal.and_then(|g| setting_off(g, self.centre(), self.yaw)) else { return };
                self.way = way;
                // (On the tip ahead of the two it stands on, its lowest two.)
                let mut low: Vec<usize> = (0..4).collect();
                low.sort_by(|&a, &b| self.arm(a).y.total_cmp(&self.arm(b).y));
                let ahead = low[..2].iter().copied().max_by(|&a, &b| (self.arm(a).x * way).total_cmp(&(self.arm(b).x * way))).expect("two");
                let centre = self.centre();
                self.on = ahead;
                self.pivot = centre + Frame::place(self.yaw, self.arm(ahead));
                self.begin(push);
            }
            Phase::Long { through, period, from, by, to } => {
                self.yaw = turned(self.yaw, self.way, self.pivot, goal, dt);
                let through = (through + dt / period).min(1.0);
                let reach = 1.0 - (1.0 - through) * (1.0 - through);
                let planted = diagonal(self.on);
                self.turns[planted] = from[planted] + by * through;
                self.turns[1 - planted] = from[1 - planted] + by * reach;
                if through >= 1.0 {
                    self.land(to, goal, push, ground);
                } else {
                    self.phase = Phase::Long { through, period, from, by, to };
                }
            }
            Phase::Short { mut spin } => {
                self.yaw = turned(self.yaw, self.way, self.pivot, goal, dt);
                let d = -self.arm(self.on);
                spin += -G * d.x / (d.length_squared() + self.k2) * dt;
                self.turns = self.turns.map(|t| t + spin * dt);
                self.phase = Phase::Short { spin };
                let centre = self.centre();
                if let Some(j) = (0..4).filter(|&j| j != self.on).find(|&j| {
                    let t = centre + Frame::place(self.yaw, self.arm(j));
                    t.y < ground(t)
                }) {
                    self.land(j, goal, push, ground);
                }
            }
        }
    }
}

/// How far it has collapsed toward its shadow (`move=6`; 0 none, 1 all
/// shadow), and the light's height over the horizon (rad).
#[derive(Clone, Copy, Debug)]
pub(super) struct Flat {
    k: f32,
    light: f32,
}

/// The ground's heights along a way: from `start` (on the ground), along
/// `way` (level), from `first` m along it, every `spacing` m; what its
/// shadow is cast on (`fold.wgsl` finds the same).
#[derive(Clone, Debug)]
pub(super) struct Profile {
    start: Vec3,
    way: Vec3,
    first: f32,
    spacing: f32,
    heights: Vec<f32>,
    /// Its way's point (x, z) at each height, and the way to its side.
    points: Vec<Vec4>,
}

impl Profile {
    /// The ground along `way` from `start`, out past `reach` m (and a
    /// little behind): each height found from `CLIMB` m over `start`'s (a
    /// step so high found, its top; one higher, what is under it).
    #[cfg(test)]
    fn sampled(start: Vec3, way: Vec3, reach: f32, ground: &dyn Fn(Vec3) -> f32) -> Profile {
        let flat = (start.xz(), way.xz());
        Profile::along(start, way, reach, &move |s| (flat.0 + flat.1 * s, flat.1), ground)
    }

    /// The same along a way that may curve: `path` its point (x, z) and
    /// heading `s` along it, setting off along `way`.
    fn along(start: Vec3, way: Vec3, reach: f32, path: &dyn Fn(f32) -> (Vec2, Vec2), ground: &dyn Fn(Vec3) -> f32) -> Profile {
        let (first, last) = (-1.0, reach + PROFILE_BEYOND);
        let spacing = (last - first) / (PROFILE - 1) as f32;
        let (mut heights, mut points) = (Vec::with_capacity(PROFILE), Vec::with_capacity(PROFILE));
        for i in 0..PROFILE {
            let (p, h) = path(first + spacing * i as f32);
            heights.push(ground(Vec3::new(p.x, start.y + CLIMB, p.y)));
            // (Its side as `fold.wgsl` measures it: to the right of its way.)
            points.push(Vec4::new(p.x, p.y, h.y, -h.x));
        }
        Profile { start, way, first, spacing, heights, points }
    }

    /// The ground's height `s` m along its way (between its heights, the
    /// first and last beyond them).
    fn height(&self, s: f32) -> f32 {
        let last = (self.heights.len() - 1) as f32;
        let f = ((s - self.first) / self.spacing).clamp(0.0, last);
        let i = f.floor() as usize;
        let j = (i + 1).min(last as usize);
        self.heights[i] + (self.heights[j] - self.heights[i]) * (f - i as f32)
    }

    /// Where the ray from `from` (m along its way, height) along `ray`
    /// (along, up) first meets the ground, if within `RAY_STEPS` steps of
    /// `RAY_STEP` m (then found closer by halving): as `fold.wgsl`.
    fn meets(&self, from: Vec2, ray: Vec2) -> Option<Vec2> {
        let (mut near, mut far) = (0.0, 0.0);
        let under = |t: f32| {
            let q = from + ray * t;
            q.y <= self.height(q.x)
        };
        let mut found = false;
        for _ in 0..RAY_STEPS {
            if under(far) {
                found = true;
                break;
            }
            near = far;
            far += RAY_STEP;
        }
        if !found {
            return None;
        }
        for _ in 0..RAY_HALVINGS {
            let mid = (near + far) / 2.0;
            if under(mid) { far = mid } else { near = mid }
        }
        Some(from + ray * far)
    }

    /// The light's height over the horizon (rad) at which the shadow of a
    /// point `high` m over its start falls `reach` m along its way: found
    /// by halving (a lower light, a longer shadow). If a wall in its way
    /// (a step up too high) stops it short whatever the light, as far as
    /// the wall: the steepest light whose shadow reaches it, up its face.
    fn light_for(&self, high: f32, reach: f32) -> f32 {
        let from = Vec2::new(0.0, self.start.y + high);
        let falls = |e: f32| self.meets(from, Vec2::new(e.cos(), -e.sin())).map_or(f32::MAX, |q| q.x);
        let (mut low, mut high) = (2f32.to_radians(), 89f32.to_radians());
        let reach = reach.min(falls(low) - 0.05);
        for _ in 0..24 {
            let mid = (low + high) / 2.0;
            if falls(mid) > reach { low = mid } else { high = mid }
        }
        (low + high) / 2.0
    }

    /// What `fold.wgsl` is given (see it): collapsed `k`, the light `light`
    /// rad over the horizon.
    fn buffer(&self, k: f32, light: f32) -> Vec<Vec4> {
        let mut data = vec![self.start.extend(k), Vec4::new(self.way.x, self.way.z, light.cos(), -light.sin()), Vec4::new(self.first, self.spacing, self.heights.len() as f32, SHADOW_LIFT)];
        data.extend(self.heights.chunks(4).map(|c| Vec4::new(c[0], c.get(1).copied().unwrap_or(c[0]), c.get(2).copied().unwrap_or(c[0]), c.get(3).copied().unwrap_or(c[0]))));
        data.extend(&self.points);
        data
    }
}

/// Travelling as its shadow: standing (a moment before it sets off);
/// leaving (how far, 0..1: the light sinking from overhead to low behind
/// it, its body into its shadow, stretched along its way); arriving (how
/// far still to go, 1..0: the mirror, at its way's end, a light low beyond
/// it rising overhead, it rising out of its shadow cast back along it).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shadow {
    Stand,
    Leave(f32),
    Arrive(f32),
}

/// A way along the ground (level, x and z): a circle's arc from `from`,
/// setting off along `out`, to `to` (straight if `to` is dead ahead), and
/// straight on beyond either end. Its length; its point and its heading `s`
/// along it.
#[derive(Clone, Copy, Debug)]
struct Arc {
    from: Vec2,
    out: Vec2,
    /// Its radius, signed (to the left of `out` if more than 0; none
    /// straight), and its length.
    radius: f32,
    length: f32,
}

impl Arc {
    fn new(from: Vec2, out: Vec2, to: Vec2) -> Arc {
        let chord = to - from;
        let side = out.perp_dot(chord);
        let ahead = out.dot(chord);
        if side.abs() < 1e-3 * chord.length().max(1.0) {
            return Arc { from, out, radius: 0.0, length: chord.length() };
        }
        let radius = chord.length_squared() / (2.0 * side);
        let turns = 2.0 * side.atan2(ahead);
        Arc { from, out, radius, length: (radius * turns).abs() }
    }

    fn at(&self, s: f32) -> (Vec2, Vec2) {
        if s <= 0.0 || self.radius == 0.0 {
            return (self.from + self.out * s, self.out);
        }
        let on = s.min(self.length);
        let centre = self.from + self.out.perp() * self.radius;
        let turn = Vec2::from_angle(on / self.radius);
        let (point, heading) = (centre + turn.rotate(self.from - centre), turn.rotate(self.out));
        (point + heading * (s - on), heading)
    }

    /// How far it turns (rad, to the left more than 0).
    fn turns(&self) -> f32 {
        if self.radius == 0.0 { 0.0 } else { self.length / self.radius }
    }
}

/// The tumbler travelling as its shadow (the user, 2026-11-13; see the
/// module's account).
#[derive(Clone, Debug)]
struct Shade {
    frame: Frame,
    /// Its feet (on the ground) and its heading (its face square to it:
    /// its shadow sets off the way its face looks).
    at: Vec3,
    yaw: f32,
    /// Its way (leaving), its far end, and its heading there.
    way: Arc,
    to: Vec3,
    to_yaw: f32,
    /// The ground its shadow falls on (leaving; arriving, cast back) and
    /// the light now; its lowest, and how long leaving or arriving takes.
    profile: Option<Profile>,
    light: f32,
    lowest: f32,
    time: f32,
    /// Its chances, drawn from its own seed; how low its light sinks at
    /// most (rad over the horizon).
    seed: u32,
    low: f32,
    /// How long leaving and arriving each take, about (s).
    stretch: f32,
    /// How much of leaving (and arriving) its body takes to collapse (rise).
    collapse: f32,
    doing: Shadow,
    /// What it last began, said once (`log`).
    said: Option<String>,
}

impl Shade {
    fn new(scale: f32, at: Vec3, yaw: f32, seed: u32, low: f32, stretch: f32) -> Shade {
        let way = Arc { from: at.xz(), out: Vec2::Y, radius: 0.0, length: 0.0 };
        Shade { frame: Frame::new(scale), at, yaw, way, to: at, to_yaw: yaw, profile: None, light: std::f32::consts::FRAC_PI_2, lowest: std::f32::consts::FRAC_PI_2, time: 1.0, seed: seed | 1, low, stretch, collapse: SHADE_COLLAPSE, doing: Shadow::Stand, said: None }
    }

    /// A number in 0..1 from its seed, its seed moved on.
    fn chance(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, (a, b): (f32, f32)) -> f32 {
        a + (b - a) * self.chance()
    }

    /// Its centre's height standing level on its two lower tips; its top's.
    fn heights(&self) -> (f32, f32) {
        let level = self.frame.level();
        let arms: Vec<Vec2> = (0..4).map(|j| self.frame.arm(j, [level; 2])).collect();
        let up = arms.iter().map(|a| -a.y).fold(f32::MIN, f32::max);
        (up, up + arms.iter().map(|a| a.y).fold(f32::MIN, f32::max))
    }

    /// Its face's way (level) at heading `yaw`.
    fn facing(yaw: f32) -> Vec3 {
        Quat::from_rotation_y(yaw) * Vec3::Z
    }

    fn pose(&self) -> Pose {
        let heading = Quat::from_rotation_y(self.yaw);
        let level = self.frame.level();
        let centre = self.at + Vec3::Y * self.heights().0;
        let ease = |s: f32| s * s * (3.0 - 2.0 * s);
        // (Its body a little ahead of the light, into its shadow and out.)
        let flat = match self.doing {
            // (Arriving, `s` runs from 1 down: it rises in the last of it.)
            Shadow::Leave(s) | Shadow::Arrive(s) => Some(Flat { k: ease((s / self.collapse).min(1.0)), light: self.light }),
            Shadow::Stand => None,
        };
        Pose { centre, body: heading, turns: [level; 2], core: Some(heading), flat }
    }

    /// The ground its shadow falls on now, if it is travelling as one.
    fn profile(&self) -> Option<&Profile> {
        if self.doing == Shadow::Stand { None } else { self.profile.as_ref() }
    }

    /// Whether one shadow of its takes it to `to` (on flat ground): no
    /// further round from its face (front or back) than `SHADE_OFF`, and its
    /// arc no longer than its light reaches.
    fn reaches(&self, to: Vec3) -> bool {
        let toward = (to - self.at).xz();
        let off = Shade::facing(self.yaw).xz().angle_to(toward).abs();
        let off = off.min(std::f32::consts::PI - off);
        let arc = toward.length() * if off > 1e-3 { off / off.sin() } else { 1.0 };
        off <= SHADE_OFF && arc <= self.heights().1 / self.low.tan() * 0.95
    }

    fn step(&mut self, dt: f32, goal: Option<Vec3>, ground: &dyn Fn(Vec3) -> f32) {
        let ease = |s: f32| s * s * (3.0 - 2.0 * s);
        match self.doing {
            Shadow::Stand => {
                let Some(g) = goal else { return };
                let toward = (g - self.at).with_y(0.0);
                if toward.length() < OFF {
                    return;
                }
                // Its shadow sets off the way its face looks, whichever side
                // is nearer where it goes, and curves there (far off, toward
                // it, a little aside, by chance).
                if toward.dot(Shade::facing(self.yaw)) < 0.0 {
                    self.yaw = wrapped(self.yaw + std::f32::consts::PI);
                }
                let out = Shade::facing(self.yaw).xz();
                // (As far as its lowest light casts its shadow, along its
                // way: further, it goes part of the way, more or less by
                // chance, aimed a little aside.)
                let reach = self.heights().1 / self.low.tan();
                let far = toward.length() > reach;
                let most = if far { reach * self.between((0.6, 1.0)) } else { reach };
                let aim = if far { self.between((-SHADE_AIM, SHADE_AIM)) } else { 0.0 };
                // (Its far end no further round from its face than so far:
                // a sharper bend is too long a way round; the next goes on.)
                let to = Vec2::from_angle(aim).rotate(toward.xz().normalize());
                let off = out.angle_to(to).clamp(-SHADE_OFF, SHADE_OFF);
                let mut way = Arc::new(self.at.xz(), out, self.at.xz() + Vec2::from_angle(off).rotate(out) * toward.length());
                way.length = way.length.min(most);
                let path = |s: f32| way.at(s);
                let mut leave = Profile::along(self.at, Shade::facing(self.yaw), way.length, &path, ground);
                // (Short of a step up too high for its shadow to climb, a
                // metre before it.)
                let mut s = 0.0;
                while s < way.length {
                    if leave.height(s) - self.at.y > CLIMB * 0.95 {
                        way.length = (s - 1.0).max(0.0);
                        let path = |s: f32| way.at(s);
                        leave = Profile::along(self.at, Shade::facing(self.yaw), way.length, &path, ground);
                        break;
                    }
                    s += leave.spacing;
                }
                if way.length < 1.0 {
                    return;
                }
                let (there, heading) = way.at(way.length);
                self.to = Vec3::new(there.x, 0.0, there.y);
                self.to.y = leave.height(way.length);
                self.to_yaw = f32::atan2(-heading.x, -heading.y);
                self.way = way;
                self.lowest = leave.light_for(self.heights().1, way.length);
                self.light = std::f32::consts::FRAC_PI_2;
                self.profile = Some(leave);
                self.time = self.stretch * self.between((0.8, 1.2));
                self.said = Some(format!("leaves along a curve of {:.1} m, turning {:+.0} degrees", way.length, way.turns().to_degrees()));
                self.doing = Shadow::Leave(0.0);
            }
            Shadow::Leave(s) => {
                let s = (s + dt / self.time).min(1.0);
                self.light = std::f32::consts::FRAC_PI_2 + (self.lowest - std::f32::consts::FRAC_PI_2) * ease(s);
                if s < 1.0 {
                    self.doing = Shadow::Leave(s);
                    return;
                }
                // All shadow: now the one cast back along its way from its
                // far end, it there, facing back along it.
                let way = self.way;
                let back = move |s: f32| {
                    let (p, h) = way.at(way.length - s);
                    (p, -h)
                };
                self.at = self.to;
                self.yaw = self.to_yaw;
                let arrive = Profile::along(self.at, Shade::facing(self.yaw), way.length, &back, ground);
                self.lowest = arrive.light_for(self.heights().1, way.length);
                self.light = self.lowest;
                self.profile = Some(arrive);
                self.time = self.stretch * self.between((0.8, 1.2));
                self.doing = Shadow::Arrive(1.0);
            }
            Shadow::Arrive(s) => {
                let s = s - dt / self.time;
                self.light = std::f32::consts::FRAC_PI_2 + (self.lowest - std::f32::consts::FRAC_PI_2) * ease(s.max(0.0));
                self.doing = if s <= 0.0 { Shadow::Stand } else { Shadow::Arrive(s) };
                if s <= 0.0 {
                    self.said = Some("stands".to_string());
                }
            }
        }
    }
}

/// How it moves (`move=`).
#[derive(Clone, Debug)]
enum Gait {
    Wheel(Roll),
    Stride(Stride),
    Hybrid(Hybrid),
    Shade(Shade),
    Spear(Spear),
    Star(Star),
}

impl Gait {
    fn pose(&self) -> Pose {
        match self {
            Gait::Wheel(r) => r.pose(),
            Gait::Stride(s) => s.pose(),
            Gait::Hybrid(h) => h.pose(),
            Gait::Shade(s) => s.pose(),
            Gait::Spear(s) => s.pose(),
            Gait::Star(s) => s.pose(),
        }
    }

    /// Whether it gets to `to` in one move (as its shadow, one shadow; any
    /// other way, walking or rolling, it always does).
    fn reaches(&self, to: Vec3) -> bool {
        match self {
            Gait::Shade(s) => s.reaches(to),
            _ => true,
        }
    }

    /// The ground its shadow falls on now, if it is travelling as one.
    fn profile(&self) -> Option<&Profile> {
        match self {
            Gait::Shade(s) => s.profile(),
            _ => None,
        }
    }

    fn resting(&self) -> bool {
        match self {
            Gait::Wheel(r) => r.rest.is_some(),
            Gait::Stride(s) => s.rest,
            Gait::Hybrid(h) => matches!(h.phase, Phase::Rest),
            Gait::Shade(s) => s.doing == Shadow::Stand,
            Gait::Spear(s) => s.doing == Throw::Stand,
            Gait::Star(s) => s.doing == Spin::Stand,
        }
    }

    fn step(&mut self, dt: f32, goal: Option<Vec3>, push: f32, ground: &dyn Fn(Vec3) -> f32) {
        match self {
            Gait::Wheel(r) => {
                r.step(dt, goal, push, ground);
            }
            Gait::Stride(s) => s.step(dt, goal, ground),
            Gait::Hybrid(h) => h.step(dt, goal, push, ground),
            Gait::Shade(s) => s.step(dt, goal, ground),
            Gait::Spear(s) => s.step(dt, goal, ground),
            Gait::Star(s) => s.step(dt, goal, ground),
        }
    }
}

/// A tumbler: how it moves, how hard it pushes off cartwheeling, its line
/// across before you (if not chasing you) and which end it makes for, how
/// long it has rested; its parts (its two diagonals, its core); logging its
/// pace.
#[derive(Component)]
pub(super) struct Tumbler {
    gait: Gait,
    push: f32,
    line: Option<[Vec3; 2]>,
    leg: usize,
    parts: [Entity; 4],
    scale: f32,
    /// Its body's height made (see `BODY`), its own glowing material; what
    /// it is doing; how many shots a burst at most (0 none), how long
    /// between them (s); its chances, drawn from its own seed.
    body: f32,
    glow: Handle<StandardMaterial>,
    act: Act,
    going: Going,
    shots: u32,
    gap: f32,
    seed: u32,
    /// Its rhombus's vortex (1 light streaks out of a black core, 2 the
    /// same inverted), and how far to your side it aims (m: to see its
    /// shots pass, their trails side on).
    vortex: u8,
    aside: f32,
    /// Its shot (1 its rhombus, 2 a crack through the ground: see
    /// `crack.rs`); how quickly a crack bends after you (rad/s), how long
    /// it lasts (s), how many strands it is.
    attack: u8,
    seek: f32,
    lasts: f32,
    strands: u32,
    /// Travelling as its shadow: its buffer for `fold.wgsl` and its two
    /// looks folding (lit, as it is; shadow, black and unlit); how it is
    /// drawn now.
    fold: Option<(Handle<ShaderBuffer>, Handle<FoldMaterial>, Handle<FoldMaterial>)>,
    look: Look,
    log: bool,
    logged: (f32, Vec3),
}

/// How its going to a spot is going: whether it has set off, how long it
/// has stood without (s).
#[derive(Clone, Copy, Debug, Default)]
struct Going {
    went: bool,
    idle: f32,
}

/// How long it stands, going nowhere, before it takes it that it cannot
/// set off for its spot (s).
const IDLE: f32 = 0.5;

impl Going {
    /// A frame of going (`dt`), it resting or not, near its spot or not;
    /// whether it is there: come to rest after its move, wherever it took
    /// it, if it moves by single moves (its shadow's hop: one cut short,
    /// before a step too high, is still its move; the user, 2026-11-13: it
    /// moves to fire, never twice running); otherwise come to rest at its
    /// spot; or standing a while, unable to set off at all (beside a wall).
    fn there(&mut self, dt: f32, resting: bool, single: bool, near: bool) -> bool {
        if !resting {
            self.went = true;
            self.idle = 0.0;
            return false;
        }
        self.idle += dt;
        let there = (single && self.went) || near || self.idle > IDLE;
        if there {
            *self = Going::default();
        }
        there
    }
}

/// What it is doing: going to a spot (none yet: one chosen); winding up
/// to fire (how long, s); firing (how many still to fire, how long since
/// the last, s); its glow fading after (how long, s).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Act {
    Go(Option<Vec3>),
    Wind(f32),
    Burst(u32, f32),
    After(f32),
}

impl Tumbler {
    /// Its next spot to fire from: on its line, the other end; hunting,
    /// one 8 to 16 m off you, swung round to one side or the other of
    /// where it is, from which it sees you, that it gets to in one move
    /// (the user: it moves to fire, never twice running); none such (you
    /// far off), one it gets to in one move on toward you, from which it
    /// sees you (it fires from there: within its range); none of those
    /// either, the last tried.
    fn spot(&mut self, from: Vec3, you: Vec3, ground: &crate::ground::Ground) -> Vec3 {
        if let Some(ends) = self.line {
            self.leg = 1 - self.leg;
            return ends[self.leg];
        }
        let bearing = f32::atan2((from - you).x, (from - you).z);
        let mut spot = from;
        for _ in 0..24 {
            let swing = self.between(SPOT_SWING) * if self.chance() < 0.5 { -1.0 } else { 1.0 };
            let off = self.between(SPOT_OFF);
            let p = you + Vec3::new((bearing + swing).sin(), 0.0, (bearing + swing).cos()) * off;
            spot = p.with_y(ground.at(p.with_y(you.y)));
            if ground.sight(spot + Vec3::Y * 1.5, you) && (spot - from).with_y(0.0).length() > ARRIVED * 1.5 && self.gait.reaches(spot) {
                return spot;
            }
        }
        let toward = (you - from).with_y(0.0).normalize_or(Vec3::Z);
        for _ in 0..24 {
            let turn = Quat::from_rotation_y(self.between((-0.7, 0.7)));
            let p = from + turn * toward * self.between((ARRIVED * 1.5, SPOT_OFF.1));
            let near = p.with_y(ground.at(p.with_y(from.y + 1.5)));
            if self.gait.reaches(near) && near.distance(you) < RANGE * 0.9 && ground.sight(near + Vec3::Y * 1.5, you) {
                return near;
            }
        }
        spot
    }

    /// A number in 0..1 from its seed, its seed moved on.
    fn chance(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, (a, b): (f32, f32)) -> f32 {
        a + (b - a) * self.chance()
    }
}

/// How its parts are drawn: as they are; folding toward its shadow, lit as
/// they are; folding, as shadow (black, unlit). Folding, never culled by
/// their bounds (they leave them) and casting no shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Solid,
    Lit,
    Shadow,
}

/// Its parts folded toward its shadow on the GPU (`fold.wgsl`): the
/// buffer of its collapse and the ground it falls on (see `Profile::buffer`).
pub(super) type FoldMaterial = ExtendedMaterial<StandardMaterial, Fold>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(super) struct Fold {
    #[storage(100, read_only)]
    data: Handle<ShaderBuffer>,
}

impl MaterialExtension for Fold {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/combat/tumbler/fold.wgsl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://game/combat/tumbler/fold.wgsl".into()
    }
}

/// One of its parts: its two diagonals (0, 1), its core's body (2) and
/// head (3); how much taller its core's body is made.
#[derive(Component)]
pub(super) struct Part(usize, f32);

/// Its parts, cut from its model once it is in (see `cut`), for each body
/// height asked for (in thousandths).
#[derive(Resource, Default)]
pub(super) struct Parts {
    model: Option<Handle<Mesh>>,
    cut: Vec<(u32, [Handle<Mesh>; 4])>,
    /// Its rhombus (as fired; per body height, in thousandths), its vortex,
    /// and its trails' dark ray.
    bolts: Vec<(u32, Handle<Mesh>)>,
    vortices: Vec<(u8, Handle<Vortex>)>,
    trail: Option<Handle<super::beam::Ray>>,
    /// Its cracks' look (see `crack::material`).
    crack: Option<Handle<StandardMaterial>>,
}

/// `spawn tumbler`: before you, standing, its face to you.
pub(super) fn arrive(mut commands: Commands, server: Res<AssetServer>, assets: Res<super::Assets3>, mut materials: ResMut<Assets<StandardMaterial>>, mut parts: ResMut<Parts>, ground: crate::ground::Ground, camera: Single<&Transform, With<FlyCam>>, mut spawns: ResMut<crate::console::Spawns>) {
    let forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::Z);
    let across = Vec3::new(-forward.z, 0.0, forward.x);
    let (asked, others): (Vec<_>, Vec<_>) = std::mem::take(&mut spawns.creatures).into_iter().partition(|(kind, ..)| kind == "tumbler");
    spawns.creatures = others;
    for (_, count, traits, places) in asked {
        parts.model.get_or_insert_with(|| server.load(GltfAssetLabel::Primitive { mesh: 0, primitive: 0 }.from_asset(format!("creatures://{FILE}"))));
        let scale = traits.value("size", HEIGHT) / MESH_HEIGHT;
        let how = traits.value("move", 1.0) as u8;
        let body = traits.value("body", BODY);
        for n in 0..count {
            let middle = match places.len() {
                0 => camera.translation + forward * (AHEAD + n as f32 * 6.0),
                k => places[n as usize % k],
            };
            let at = middle.with_y(ground.at(middle));
            // (Its right along `across`: its face to you.)
            let yaw = f32::atan2(-across.z, across.x);
            let gait = match how {
                2 => Gait::Stride(Stride::new(scale, at, yaw, traits.value("speed", STRIDE_FAST))),
                3 => Gait::Hybrid(Hybrid::new(scale, at, yaw, traits.value("speed", STRIDE_PACE))),
                4 => Gait::Star(Star::new(scale, at, yaw, traits.value("speed", STAR_PACE))),
                5 => Gait::Spear(Spear::new(scale, at, yaw)),
                6 => {
                    let mut shade = Shade::new(scale, at, yaw, at.x.to_bits() ^ at.z.to_bits().rotate_left(11) ^ n.wrapping_mul(2654435761), traits.value("light", SHADE_LOW).clamp(3.0, 80.0).to_radians(), traits.value("stretch", SHADE_TIME).max(0.2) / traits.value("speed", SHADE_SPEED).max(0.05));
                    shade.collapse = traits.value("collapse", SHADE_COLLAPSE).clamp(0.05, 1.0);
                    Gait::Shade(shade)
                }
                _ => Gait::Wheel(Roll::new(scale, at, yaw, traits.has("hub"))),
            };
            let line = traits.has("line").then(|| [at - across * HALF, at + across * HALF]);
            let pose = gait.pose();
            // (Its body its own, to glow.)
            let dark = materials.get(&assets.dark).cloned().expect("the creatures' dark");
            let glow = materials.add(dark);
            let parts = [0, 1, 2, 3].map(|k| commands.spawn((Part(k, body), MeshMaterial3d(if k == 2 { glow.clone() } else { assets.dark.clone() }), Transform::default(), Visibility::Inherited)).id());
            commands
                .spawn((
                    Tumbler { gait, push: traits.value("push", PUSH), line, leg: 1, parts, scale, body, glow, act: Act::Go(None), going: Going::default(), shots: traits.value("shots", SHOTS as f32) as u32, gap: traits.value("gap", GAP).max(0.05), vortex: traits.value("vortex", 2.0) as u8, aside: traits.value("aside", 0.0), attack: traits.value("attack", 1.0) as u8, seek: traits.value("seek", crack::SEEK), lasts: traits.value("lasts", crack::LASTS), strands: traits.value("strands", crack::STRANDS as f32).max(1.0) as u32, seed: (at.x.to_bits() ^ at.z.to_bits().rotate_left(7) ^ n.wrapping_mul(40503)) | 1, fold: None, look: Look::Solid, log: traits.has("log"), logged: (0.0, at) },
                    Transform::from_translation(pose.centre),
                    Visibility::Visible,
                ))
                .add_children(&parts);
            info!("a tumbler stands at {at:.1}, moving by {}{}", ["cartwheeling", "striding", "cartwheeling its short steps and striding its long ones", "spinning as a star", "leaping as a spear", "as its shadow"][how.clamp(1, 6) as usize - 1], if line.is_some() { ", to and fro across before you" } else { ", hunting you" });
        }
    }
}

/// Its model cut into its parts once it is in (in its own units, upright,
/// its crossing at the middle), and given to every part still without.
pub(super) fn cut(mut commands: Commands, mut parts: ResMut<Parts>, mut meshes: ResMut<Assets<Mesh>>, bare: Query<(Entity, &Part), Without<Mesh3d>>) {
    if bare.is_empty() {
        return;
    }
    for (entity, part) in &bare {
        let key = (part.1 * 1000.0).round() as u32;
        if !parts.cut.iter().any(|(k, _)| *k == key) {
            let Some(model) = parts.model.as_ref().and_then(|m| meshes.get(m)) else { return };
            let cut = split(model, part.1).map(|m| meshes.add(m));
            parts.cut.push((key, cut));
        }
        let (_, cut) = parts.cut.iter().find(|(k, _)| *k == key).expect("cut");
        commands.entity(entity).insert(Mesh3d(cut[part.0].clone()));
    }
}

/// Its model's triangles to its two diagonals and its core (see `MESH_CORE`,
/// `MESH_HEAD`), each its own mesh, upright (its node's turn), its crossing
/// at the middle; its core's body `tall` times as tall (see `MESH_BODY`).
fn split(model: &Mesh, tall: f32) -> [Mesh; 4] {
    let read = |name| match model.attribute(name) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|&p| Vec3::from(p)).collect::<Vec<_>>(),
        _ => panic!("{FILE}: its positions and normals wanted"),
    };
    // (Its node stands it up: a quarter turn about x.)
    let upright = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let place: Vec<Vec3> = read(Mesh::ATTRIBUTE_POSITION).into_iter().map(|p| upright * p - MESH_CENTRE.extend(0.0)).collect();
    let normal: Vec<Vec3> = read(Mesh::ATTRIBUTE_NORMAL).into_iter().map(|n| upright * n).collect();
    let corners: Vec<usize> = match model.indices() {
        Some(i) => i.iter().collect(),
        None => (0..place.len()).collect(),
    };
    let t = MESH_TIPS.map(|p| p - MESH_CENTRE);
    let lines = [(t[0] - t[2]).normalize(), (t[1] - t[3]).normalize()];
    let mut each: [(Vec<[f32; 3]>, Vec<[f32; 3]>); 4] = Default::default();
    for tri in corners.as_chunks::<3>().0 {
        let middle = tri.iter().map(|&i| place[i]).sum::<Vec3>() / 3.0;
        let m = middle.truncate();
        let off = |d: Vec2| m.perp_dot(d).abs();
        let core = m.length() < MESH_CORE || (m.x.abs() < MESH_HEAD.1 && m.y > 0.0 && m.y < MESH_HEAD.0);
        // (Its head: its core above its body's shoulders.)
        let k = if core && m.y > MESH_BODY.1 { 3 } else if core { 2 } else if off(lines[0]) < off(lines[1]) { 0 } else { 1 };
        for &i in tri {
            let (mut p, mut n) = (place[i], normal[i]);
            if core {
                // (Its body stretched up from its bottom point, its normals
                // by the inverse; above it, its head lifted.)
                let (low, top) = MESH_BODY;
                if p.y > top {
                    p.y += (top - low) * (tall - 1.0);
                } else if p.y > low {
                    p.y = low + (p.y - low) * tall;
                    n = Vec3::new(n.x, n.y / tall, n.z).normalize_or(n);
                }
            }
            each[k].0.push(p.to_array());
            each[k].1.push(n.to_array());
        }
    }
    each.map(|(p, n)| {
        assert!(!p.is_empty(), "{FILE}: a part with none of its triangles");
        let count = p.len() as u32;
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p).with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, n).with_inserted_indices(Indices::U32((0..count).collect()))
    })
}

/// Its body's middle, from its centre, in its core's frame (m).
fn body_middle(tall: f32, scale: f32) -> Vec3 {
    let (low, top) = MESH_BODY;
    Vec3::Y * (low + (top - low) * tall / 2.0) * scale
}

/// Each tumbler moved a frame: to the far end of its line (resting at
/// each), or at you, unless frozen to fire (glowing, then a moment after);
/// drawn, its parts each turned.
#[allow(clippy::too_many_arguments)]
pub(super) fn tumble(
    mut commands: Commands,
    time: Res<Time>,
    ground: crate::ground::Ground,
    assets: Res<super::Assets3>,
    mut made: ResMut<Parts>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut folds: ResMut<Assets<FoldMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    (mut vortices, mut rays): (ResMut<Assets<Vortex>>, ResMut<Assets<super::beam::Ray>>),
    camera: Single<&Transform, (With<FlyCam>, Without<Tumbler>, Without<Part>)>,
    mut tumblers: Query<(&mut Tumbler, &mut Transform), Without<Part>>,
    mut parts: Query<&mut Transform, (With<Part>, Without<Tumbler>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let you = camera.translation;
    for (mut t, mut transform) in &mut tumblers {
        let t = &mut *t;
        // Going to a spot; there, standing steady, winding up and firing
        // its burst (one every `gap`, as many as chance has it), its glow
        // fading; then on to the next. Never idle (the user, 2026-11-13).
        let before = t.gait.pose();
        let core = before.core.unwrap_or(before.body);
        let body_at = before.centre + core * body_middle(t.body, t.scale);
        let sees = body_at.distance(you) < RANGE && ground.sight(body_at, you);
        let mut shoot = false;
        match t.act {
            Act::Go(None) => t.act = Act::Go(Some(t.spot(before.centre, you, &ground))),
            Act::Go(Some(to)) => {
                let single = matches!(t.gait, Gait::Shade(_));
                let near = (to - before.centre).with_y(0.0).length() < ARRIVED;
                if t.going.there(dt, t.gait.resting(), single, near) {
                    // (There: it fires if it sees you; if not, on at once:
                    // the one way it moves twice running, said.)
                    t.act = if t.shots > 0 && sees { Act::Wind(0.0) } else { Act::Go(Some(t.spot(before.centre, you, &ground))) };
                    if !sees {
                        let far = body_at.distance(you);
                        info!("tumbler: moves on, not seeing you ({far:.1} m off{})", if far >= RANGE { ", out of its range" } else { ", something between" });
                    }
                }
            }
            Act::Wind(w) if w + dt >= WIND => {
                let n = 1 + (t.chance() * t.shots as f32) as u32;
                shoot = true;
                t.act = if n > 1 { Act::Burst(n.min(t.shots) - 1, 0.0) } else { Act::After(0.0) };
                if t.log {
                    info!("tumbler: a burst of {}", n.min(t.shots));
                }
            }
            Act::Wind(w) => t.act = Act::Wind(w + dt),
            Act::Burst(left, since) if since + dt >= t.gap => {
                shoot = true;
                t.act = if left > 1 { Act::Burst(left - 1, 0.0) } else { Act::After(0.0) };
            }
            Act::Burst(left, since) => t.act = Act::Burst(left, since + dt),
            Act::After(a) if a + dt >= AFTER => t.act = Act::Go(Some(t.spot(before.centre, you, &ground))),
            Act::After(a) => t.act = Act::After(a + dt),
        }
        if shoot && t.attack == 2 {
            // (Black lightning through the ground, from under it to under
            // you: `crack.rs`.)
            let feet = before.centre.with_y(ground.at(before.centre));
            let across = Vec3::Y.cross(you - feet).normalize_or_zero();
            let look = made.crack.get_or_insert_with(|| materials.add(crack::material())).clone();
            let seed = t.seed;
            t.chance();
            crack::fire(&mut commands, &mut meshes, look, feet, you - Vec3::Y * crate::player::EYE + across * t.aside, (t.seek, t.lasts, t.strands), seed, time.elapsed_secs(), &ground);
            if t.log {
                info!("tumbler: a crack from {feet:.1}, {:.1} m from you", feet.distance(you));
            }
        } else if shoot {
            let key = (t.body * 1000.0).round() as u32;
            if !made.bolts.iter().any(|(k, _)| *k == key) {
                let mesh = meshes.add(rhombus(t.body, t.scale));
                made.bolts.push((key, mesh));
            }
            let mesh = made.bolts.iter().find(|(k, _)| *k == key).expect("made").1.clone();
            if !made.vortices.iter().any(|(k, _)| *k == t.vortex) {
                let look = vortices.add(Vortex { look: Vec4::new(VORTEX, if t.vortex == 2 { 1.0 } else { 0.0 }, 0.0, 0.0) });
                made.vortices.push((t.vortex, look));
            }
            let vortex = made.vortices.iter().find(|(k, _)| *k == t.vortex).expect("made").1.clone();
            // (Aimed aside of you, if asked, to see it pass.)
            let across = Vec3::Y.cross(you - body_at).normalize_or_zero();
            let ray = made.trail.get_or_insert_with(|| rays.add(super::beam::Ray::dark(TRAIL_WIDTH, TRAIL_DARK))).clone();
            fire(&mut commands, &mut meshes, (mesh, vortex, ray), body_at, core, you + across * t.aside, t.body, t.scale);
            if t.log {
                info!("tumbler: fires from {body_at:.1}, {:.1} m from you", body_at.distance(you));
            }
        }
        // (Its glow: gathering, quickest at the last, before each shot (the
        // user, 2026-11-13: each its own wind-up), gone as it fires.)
        let glow = match t.act {
            Act::Wind(w) => (w / WIND).powi(2),
            // (Gathering again before each shot, in the last of its gap.)
            Act::Burst(_, since) => {
                let wind = WIND.min(t.gap);
                ((since - (t.gap - wind)) / wind).clamp(0.0, 1.0).powi(2)
            }
            Act::After(_) => 0.0,
            Act::Go(_) => 0.0,
        };
        if let Some(mut m) = materials.get_mut(&t.glow) {
            m.emissive = LinearRgba::rgb(1.0, 1.0, 1.0) * BRIGHT * glow;
        }
        let frozen = !matches!(t.act, Act::Go(_));
        let goal = match t.act {
            Act::Go(to) => to,
            _ => None,
        };
        // (Small steps: its tips land within a few millimetres.)
        let steps = if frozen { 0 } else { (dt / (1.0 / 240.0)).ceil().max(1.0) as usize };
        for _ in 0..steps {
            t.gait.step(dt / steps as f32, goal, t.push, &|p| ground.at(p));
        }
        let pose = t.gait.pose();
        transform.translation = pose.centre;
        transform.rotation = pose.body;
        // Travelling as its shadow: its collapse and the ground under it
        // given to its fold; most of the way in, it is shadow.
        let look = match pose.flat {
            None => Look::Solid,
            Some(f) if f.k > 0.5 => Look::Shadow,
            Some(_) => Look::Lit,
        };
        if let (Some(flat), Some(profile)) = (pose.flat, t.gait.profile()) {
            let data = profile.buffer(flat.k, flat.light);
            if t.fold.is_none() {
                let buffer = buffers.add(ShaderBuffer::from(data.clone()));
                let lit = materials.get(&assets.dark).cloned().expect("the creatures' dark");
                let shadow = materials.get(&assets.shade).cloned().expect("the shade");
                t.fold = Some((buffer.clone(), folds.add(ExtendedMaterial { base: lit, extension: Fold { data: buffer.clone() } }), folds.add(ExtendedMaterial { base: shadow, extension: Fold { data: buffer } })));
            }
            let (buffer, ..) = t.fold.as_ref().expect("made");
            if let Some(mut b) = buffers.get_mut(buffer) {
                b.set_data(data);
            }
        }
        if look != t.look {
            t.look = look;
            let (lit, shadow) = t.fold.as_ref().map(|f| (f.1.clone(), f.2.clone())).unzip();
            for (k, part) in t.parts.iter().enumerate() {
                let mut part = commands.entity(*part);
                match look {
                    Look::Solid => {
                        part.remove::<(MeshMaterial3d<FoldMaterial>, bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster)>().insert(MeshMaterial3d(if k == 2 { t.glow.clone() } else { assets.dark.clone() }));
                    }
                    Look::Lit | Look::Shadow => {
                        let fold = if look == Look::Lit { lit.clone() } else { shadow.clone() }.expect("its fold, made as it set off");
                        part.remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(fold), bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster));
                    }
                }
            }
        }
        let apart = MESH_APART * t.scale;
        for (k, part) in t.parts.iter().enumerate() {
            let Ok(mut p) = parts.get_mut(*part) else { continue };
            p.scale = Vec3::splat(t.scale);
            (p.translation, p.rotation) = match k {
                2 | 3 => (Vec3::ZERO, pose.core.map_or(Quat::IDENTITY, |c| pose.body.inverse() * c)),
                _ => (Vec3::Z * if k == 0 { apart } else { -apart }, Quat::from_rotation_z(pose.turns[k])),
            };
        }
        if t.log
            && let Gait::Shade(sh) = &mut t.gait
            && let Some(said) = sh.said.take()
        {
            info!("tumbler: {said}");
        }
        if t.log {
            t.logged.0 += dt;
            if t.logged.0 >= 1.0 {
                info!("tumbler: {:.1} m/s, centre {:.2} m up{}", (pose.centre - t.logged.1).with_y(0.0).length() / t.logged.0, pose.centre.y - ground.at(pose.centre), if t.gait.resting() { ", resting" } else { "" });
                t.logged = (0.0, pose.centre);
            }
        }
    }
}

/// Its rhombus's look: a vortex of light (`vortex.wgsl`): x how bright.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct Vortex {
    #[uniform(0)]
    look: Vec4,
}

impl Material for Vortex {
    fn fragment_shader() -> ShaderRef {
        "embedded://game/combat/tumbler/vortex.wgsl".into()
    }
}

/// A rhombus it fired: where it is, which way, how fast, its turn (its face to its
/// flight) and how far it has spun about its flight; its corners (from its
/// middle, unturned); its corners' trails (where, when), their mesh and its
/// entity; whether it is spent (hit or flown its reach), how far it has
/// flown.
#[derive(Component)]
pub(super) struct Bolt {
    at: Vec3,
    way: Vec3,
    speed: f32,
    turn: Quat,
    spin: f32,
    corners: [Vec3; 4],
    trails: [VecDeque<(Vec3, f32)>; 4],
    trail: Entity,
    trail_mesh: Handle<Mesh>,
    origin: Vec3,
    spent: bool,
    flown: f32,
}

/// Its rhombus, its body's size: an octahedron, four corners round its
/// middle (across and up: its body's width and height) and two points
/// through it (its body's depth), its faces flat; each corner's uv its
/// face's (-1..1 across and up: its vortex's, centred on its points).
fn rhombus(tall: f32, scale: f32) -> Mesh {
    let (low, top) = MESH_BODY;
    let (w, h, d) = (MESH_BODY_SIZE.0 * scale / 2.0, (top - low) * tall * scale / 2.0, MESH_BODY_SIZE.1 * scale / 2.0);
    let (mut place, mut normal, mut uv) = (Vec::new(), Vec::new(), Vec::new());
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                let (a, b, c) = (Vec3::X * w * sx, Vec3::Y * h * sy, Vec3::Z * d * sz);
                let n = (b - a).cross(c - a).normalize();
                // (Wound to face out.)
                let out = n.dot(a + b + c) > 0.0;
                for p in if out { [a, b, c] } else { [a, c, b] } {
                    place.push(p.to_array());
                    normal.push((if out { n } else { -n }).to_array());
                    uv.push([p.x / w, p.y / h]);
                }
            }
        }
    }
    let count = place.len() as u32;
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, place)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32((0..count).collect()))
}

/// A rhombus fired from `from` at `you`, its face to its flight, its up as
/// near its core's (`core`) as can be.
#[allow(clippy::too_many_arguments)]
fn fire(commands: &mut Commands, meshes: &mut Assets<Mesh>, (mesh, vortex, ray): (Handle<Mesh>, Handle<Vortex>, Handle<super::beam::Ray>), from: Vec3, core: Quat, you: Vec3, tall: f32, scale: f32) {
    let way = (you - from).normalize_or(core * Vec3::Z);
    let turn = Transform::default().looking_to(way, core * Vec3::Y).rotation;
    let (low, top) = MESH_BODY;
    let (w, h) = (MESH_BODY_SIZE.0 * scale / 2.0, (top - low) * tall * scale / 2.0);
    let corners = [Vec3::X * w, Vec3::Y * h, -Vec3::X * w, -Vec3::Y * h];
    let trail_mesh = meshes.add(trails(&Default::default(), from, 0.0, from, you));
    // (Drawn where its trails are, in the world, turned to the camera in
    // their shader: never culled by where they first were; casting and
    // taking no shadow.)
    let trail = commands.spawn((Mesh3d(trail_mesh.clone()), MeshMaterial3d(ray), Transform::from_translation(from), Visibility::default(), bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster, bevy::light::NotShadowReceiver)).id();
    commands.spawn((
        Bolt { at: from, way, speed: BOLT_START, turn, spin: 0.0, corners, trails: Default::default(), trail, trail_mesh, origin: from, spent: false, flown: 0.0 },
        Mesh3d(mesh),
        MeshMaterial3d(vortex),
        Transform::from_translation(from).with_rotation(turn),
        Visibility::Visible,
        bevy::light::NotShadowCaster,
    ));
}

/// Its corners' trails as the dark ray draws them (`beam.wgsl`): each
/// point twice, on its line, along it (its normal), to either side (uv x,
/// -1 or 1, so much of its width as it is young: narrowing to nothing);
/// never empty (a mesh of nothing upset the renderer's allocator).
fn trails(corners: &[VecDeque<(Vec3, f32)>; 4], at: Vec3, now: f32, origin: Vec3, eye: Vec3) -> Mesh {
    let (mut place, mut normal, mut uv, mut index) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for trail in corners {
        let n = trail.len();
        if n < 2 {
            continue;
        }
        // (None of it right by your eye: there its width filled your view.)
        let trail: Vec<(Vec3, f32)> = trail.iter().copied().filter(|(p, _)| p.distance(eye) > TRAIL_CLEAR).collect();
        let n = trail.len();
        if n < 2 {
            continue;
        }
        let first = place.len() as u32;
        for (i, &(p, when)) in trail.iter().enumerate() {
            // (Along it as its points run, newest to oldest: the ray's
            // shader turns it to the camera by this, its faces one way;
            // the other way, all were culled, and none was seen.)
            let along = (trail[(i + 1).min(n - 1)].0 - trail[i.saturating_sub(1)].0).normalize_or(Vec3::Y);
            let young = (1.0 - (now - when) / TRAIL_LIFE).clamp(0.0, 1.0);
            for side in [-1.0, 1.0] {
                place.push((p - origin).to_array());
                normal.push(along.to_array());
                uv.push([side * young, 0.0]);
            }
        }
        for k in 0..(n as u32 - 1) {
            let v = first + 2 * k;
            index.extend([v, v + 1, v + 3, v, v + 3, v + 2]);
        }
    }
    if place.is_empty() {
        place = vec![(at - origin).to_array(); 3];
        normal = vec![[0.0, 1.0, 0.0]; 3];
        uv = vec![[0.0, 0.0]; 3];
        index = vec![0, 1, 2];
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, place)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(index))
}

/// Each rhombus flown a frame, spinning about its flight; hitting you
/// (hurt), the ground or what is built, or flown its reach (spent, gone
/// from sight); gathering speed; its trails drawn, each a dark ray from a
/// corner back along where it was, narrowing to nothing with age; spent,
/// its trails left to run out, then gone.
#[allow(clippy::too_many_arguments)]
pub(super) fn bolts(
    mut commands: Commands,
    time: Res<Time>,
    ground: crate::ground::Ground,
    assets: Res<super::Assets3>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut director: ResMut<super::Director>,
    mut gun: ResMut<super::Gun>,
    camera: Single<(&Transform, &mut crate::player::Player), (With<FlyCam>, Without<Bolt>)>,
    mut bolts: Query<(Entity, &mut Bolt, &mut Transform, &mut Visibility)>,
) {
    let dt = time.delta_secs().min(0.05);
    let now = time.elapsed_secs();
    let (camera, mut player) = camera.into_inner();
    let you = camera.translation;
    for (entity, mut b, mut transform, mut shown) in &mut bolts {
        let b = &mut *b;
        if !b.spent {
            let from = b.at;
            b.speed = (b.speed + BOLT_GATHER * dt).min(BOLT_TOP);
            let step = b.way * b.speed * dt;
            b.at += step;
            b.flown += step.length();
            b.spin += BOLT_SPIN * dt;
            // (You: nearest the way it flew this frame.)
            let along = ((you - from).dot(step) / step.length_squared().max(1e-9)).clamp(0.0, 1.0);
            if (from + step * along).distance(you) < BOLT_HIT {
                b.spent = true;
                player.health -= BOLT_HARM;
                director.hurt = 1.0;
                gun.jolt += Vec2::new(0.05, 0.0);
                commands.spawn((AudioPlayer::new(assets.player_pain.clone()), PlaybackSettings::DESPAWN));
                info!("tumbler: its rhombus hit you after {:.1} m", b.flown);
                // (Its trails gone with it: left, the last of them ran on up
                // to your eye, a dark wedge across your view.)
                for trail in &mut b.trails {
                    trail.clear();
                }
            } else if b.at.y < ground.at(b.at) || ground.hit(from, step, step.length()).is_some() || b.flown > BOLT_REACH {
                b.spent = true;
            }
            let spun = b.turn * Quat::from_rotation_z(b.spin);
            transform.translation = b.at;
            transform.rotation = spun;
            for k in 0..4 {
                let corner = b.at + spun * b.corners[k];
                b.trails[k].push_front((corner, now));
            }
            if b.spent {
                *shown = Visibility::Hidden;
            }
        }
        // Its trails: the old dropped; spent and run out, it is gone.
        for trail in &mut b.trails {
            while trail.back().is_some_and(|&(_, when)| now - when > TRAIL_LIFE) {
                trail.pop_back();
            }
        }
        if b.spent && b.trails.iter().all(|t| t.len() < 2) {
            commands.entity(b.trail).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        // (Made anew each frame, for the render world only.)
        let mut fresh = trails(&b.trails, b.at, now, b.origin, you);
        fresh.asset_usage = RenderAssetUsages::RENDER_WORLD;
        let old = std::mem::replace(&mut b.trail_mesh, meshes.add(fresh));
        meshes.remove(&old);
        commands.entity(b.trail).insert(Mesh3d(b.trail_mesh.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tolerance (m) at its first size (2.4 m), at its size now.
    fn sized(m: f32) -> f32 {
        m * HEIGHT / 2.4
    }

    /// Each way it moves, on flat ground, it gets where it is sent, either
    /// way, and stands there; its tips never sinking (but the spear's,
    /// stuck), and, striding, cartwheeling or both, always one down.
    #[test]
    fn gets_there_and_stands() {
        let scale = HEIGHT / MESH_HEIGHT;
        let floor = |_: Vec3| 0.0;
        for how in 1..=6 {
            for goal in [Vec3::new(20.0, 0.0, 0.0), Vec3::new(-20.0, 0.0, 0.0)] {
                let mut gait = match how {
                    1 => Gait::Wheel(Roll::new(scale, Vec3::ZERO, 0.0, false)),
                    2 => Gait::Stride(Stride::new(scale, Vec3::ZERO, 0.0, STRIDE_FAST)),
                    3 => Gait::Hybrid(Hybrid::new(scale, Vec3::ZERO, 0.0, STRIDE_PACE)),
                    4 => Gait::Star(Star::new(scale, Vec3::ZERO, 0.0, STAR_PACE)),
                    5 => Gait::Spear(Spear::new(scale, Vec3::ZERO, 0.0)),
                    _ => Gait::Shade(Shade::new(scale, Vec3::ZERO, 0.0, 7, SHADE_LOW.to_radians(), SHADE_TIME)),
                };
                let (mut low, mut high, mut when) = (f32::MAX, f32::MIN, None);
                for frame in 0..240 * 30 {
                    gait.step(1.0 / 240.0, Some(goal), PUSH, &floor);
                    let pose = gait.pose();
                    let tips: Vec<Vec3> = (0..4)
                        .map(|j| {
                            let turn = Vec2::from_angle(pose.turns[diagonal(j)]).rotate(Frame::new(scale).tips[j]);
                            pose.centre + pose.body * turn.extend(0.0)
                        })
                        .collect();
                    let deepest = tips.iter().map(|t| t.y).fold(f32::MAX, f32::min);
                    let stuck = matches!(&gait, Gait::Spear(s) if matches!(s.doing, Throw::Stuck(_) | Throw::Rise(_) | Throw::Fly));
                    assert!(stuck || deepest > -sized(0.03), "move={how}: a tip {deepest:.3} m into the ground at frame {frame}");
                    if how <= 3 {
                        assert!(deepest < sized(0.05), "move={how}: no tip down at frame {frame} (lowest {deepest:.3})");
                    }
                    (low, high) = (low.min(pose.centre.y), high.max(pose.centre.y));
                    if gait.resting() && frame > 240 && when.is_none() && (pose.centre - goal).with_y(0.0).length() < 3.0 {
                        when = Some(frame as f32 / 240.0);
                    }
                }
                let at = gait.pose().centre;
                println!("move={how} to {goal}: there in {when:?} s, centre {low:.2} to {high:.2} m up, stopped at {at:.2}");
                assert!(gait.resting(), "move={how}: still going");
                assert!((at - goal).with_y(0.0).length() < 3.0, "move={how}: stopped at {at}, not near {goal}");
            }
        }
    }

    /// Striding, at any pace, to a goal any way round it (most of it off to
    /// its side), it gets there and stands; never stepping to and fro (the
    /// user, 2026-11-13: at a high `speed`, a step one way, a step back,
    /// for ever).
    #[test]
    fn strides_there_any_way_at_any_pace() {
        let scale = HEIGHT / MESH_HEIGHT;
        let floor = |_: Vec3| 0.0;
        let mut worst = 0usize;
        for pace in [STRIDE_PACE, STRIDE_FAST, 8.0, 15.0, 30.0] {
            for k in 0..48 {
                for far in [2.0, 2.5, 3.0, 4.0, 6.0, 9.0, 12.0, 20.0] {
                    let a = k as f32 / 48.0 * std::f32::consts::TAU;
                    let goal = Vec3::new(a.cos(), 0.0, a.sin()) * far;
                    let mut s = Stride::new(scale, Vec3::ZERO, 0.0, pace);
                    let (mut flips, mut way, mut steps) = (0, s.way, 0);
                    for _ in 0..240 * 30 {
                        let was = s.rest;
                        s.step(1.0 / 240.0, Some(goal), &floor);
                        if was && !s.rest {
                            steps += 1;
                            if s.way != way {
                                flips += 1;
                            }
                            way = s.way;
                        }
                    }
                    let off = (s.centre() - goal).with_y(0.0).length();
                    worst = worst.max(flips);
                    assert!(s.rest && off < ARRIVED, "pace {pace}, goal {goal:.1}: {} {off:.1} m off after {steps} set-offs, {flips} turned back", if s.rest { "stood" } else { "still going" });
                    assert!(flips <= 1, "pace {pace}, goal {goal:.1}: turned back {flips} times in {steps} set-offs");
                }
            }
        }
        println!("striding: turned back at most {worst} times");
    }

    /// Its rhombus is its body's size, its faces wound and facing out.
    /// Its shadow's hop cut short (a step too high before its spot), it is
    /// there all the same, after that one hop (it fires there, never moving
    /// twice running); unable to set off at all, there within `IDLE`.
    #[test]
    fn one_move_then_there() {
        let wall = |p: Vec3| if p.z > 6.0 { 3.0 } else { 0.0 };
        let mut gait = Gait::Shade(Shade::new(HEIGHT / MESH_HEIGHT, Vec3::ZERO, 0.0, 7, SHADE_LOW.to_radians(), SHADE_TIME));
        let goal = Vec3::new(0.0, 3.0, 9.0);
        assert!(gait.reaches(goal), "a spot it would choose");
        let mut going = Going::default();
        let (dt, mut hops, mut was) = (1.0 / 60.0, 0, true);
        let frames = (0..1200).find(|_| {
            let resting = gait.resting();
            if was && !resting {
                hops += 1;
            }
            was = resting;
            let near = (goal - gait.pose().centre).with_y(0.0).length() < ARRIVED;
            if going.there(dt, resting, true, near) {
                return true;
            }
            gait.step(dt, Some(goal), PUSH, &wall);
            false
        });
        assert!(frames.is_some(), "never there");
        assert_eq!(hops, 1, "hopped {hops} times");
        assert!(gait.pose().centre.z < 6.0, "short of the wall: {}", gait.pose().centre);
        // (Against the wall: it cannot set off.)
        let mut going = Going::default();
        let stuck = (0..120).find(|_| {
            let resting = gait.resting();
            let near = (goal - gait.pose().centre).with_y(0.0).length() < ARRIVED;
            let there = going.there(dt, resting, true, near);
            gait.step(dt, Some(goal), PUSH, &wall);
            there
        });
        assert!(stuck.is_some_and(|f| f as f32 * dt <= IDLE + 0.05), "stood for ever: {stuck:?}");
    }

    #[test]
    fn rhombus_its_body() {
        let scale = HEIGHT / MESH_HEIGHT;
        let mesh = rhombus(BODY, scale);
        let Some(VertexAttributeValues::Float32x3(place)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!("no positions") };
        let Some(VertexAttributeValues::Float32x3(normal)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL) else { panic!("no normals") };
        let place: Vec<Vec3> = place.iter().map(|&p| Vec3::from(p)).collect();
        for (k, tri) in place.chunks(3).enumerate() {
            let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            assert!(n.dot(tri[0]) > 0.0 && n.dot(Vec3::from(normal[k * 3])) > 0.0, "face {k} faces in");
        }
        let high = place.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((high * 2.0 - (MESH_BODY.1 - MESH_BODY.0) * BODY * scale).abs() < 1e-5, "its height {}", high * 2.0);
    }

    /// Stopped on its way (to fire), each way it moves comes to rest soon,
    /// cartwheeling, striding or both, on two tips down (steady, not
    /// mid-turn).
    #[test]
    fn halts_on_its_feet() {
        let scale = HEIGHT / MESH_HEIGHT;
        let floor = |_: Vec3| 0.0;
        for how in 1..=6 {
            for go in [0.4, 0.9, 1.3, 2.1] {
                let mut gait = match how {
                    1 => Gait::Wheel(Roll::new(scale, Vec3::ZERO, 0.0, false)),
                    2 => Gait::Stride(Stride::new(scale, Vec3::ZERO, 0.0, STRIDE_FAST)),
                    3 => Gait::Hybrid(Hybrid::new(scale, Vec3::ZERO, 0.0, STRIDE_PACE)),
                    4 => Gait::Star(Star::new(scale, Vec3::ZERO, 0.0, STAR_PACE)),
                    5 => Gait::Spear(Spear::new(scale, Vec3::ZERO, 0.0)),
                    _ => Gait::Shade(Shade::new(scale, Vec3::ZERO, 0.0, 7, SHADE_LOW.to_radians(), SHADE_TIME)),
                };
                let goal = Vec3::new(30.0, 0.0, 0.0);
                for _ in 0..(go * 240.0) as usize {
                    gait.step(1.0 / 240.0, Some(goal), PUSH, &floor);
                }
                let mut took = None;
                for frame in 0..240 * 4 {
                    gait.step(1.0 / 240.0, None, PUSH, &floor);
                    if gait.resting() {
                        took = Some(frame as f32 / 240.0);
                        break;
                    }
                }
                assert!(took.is_some(), "move={how}, stopped after {go} s: never came to rest");
                if how <= 3 {
                    let pose = gait.pose();
                    let down = (0..4)
                        .filter(|&j| {
                            let turn = Vec2::from_angle(pose.turns[diagonal(j)]).rotate(Frame::new(scale).tips[j]);
                            (pose.centre + pose.body * turn.extend(0.0)).y < sized(0.03)
                        })
                        .count();
                    assert_eq!(down, 2, "move={how}, stopped after {go} s: at rest on {down} tips");
                }
                println!("move={how}, stopped after {go} s: at rest {took:?} s later");
            }
        }
    }

    /// Both ways at once: its long steps placed and its short ones rolled,
    /// by turns.
    #[test]
    fn steps_long_and_short_by_turns() {
        let mut h = Hybrid::new(HEIGHT / MESH_HEIGHT, Vec3::ZERO, 0.0, STRIDE_PACE);
        let mut kinds = Vec::new();
        for _ in 0..240 * 8 {
            h.step(1.0 / 240.0, Some(Vec3::new(30.0, 0.0, 0.0)), PUSH, &|_| 0.0);
            let kind = match h.phase {
                Phase::Long { .. } => 'L',
                Phase::Short { .. } => 'S',
                Phase::Rest => 'R',
            };
            if kinds.last() != Some(&kind) {
                kinds.push(kind);
            }
        }
        let seen: String = kinds.iter().collect();
        println!("its steps: {seen}");
        assert!(seen.starts_with("LSLSLS"), "not long and short by turns: {seen}");
    }

    /// Its shadow falls where the light's rays meet the ground: on flat
    /// ground as far as its height over the light's tangent, and its light
    /// found so; up the face of a step in its way; over a ledge and down.
    #[test]
    fn shadow_falls_on_the_ground() {
        let flat = Profile::sampled(Vec3::ZERO, Vec3::X, 10.0, &|_| 0.0);
        let e = flat.light_for(2.4, 10.0);
        assert!((e - 2.4f32.atan2(10.0)).abs() < 1e-3, "its light {e}, not {}", 2.4f32.atan2(10.0));
        let ray = |deg: f32| Vec2::new(deg.to_radians().cos(), -deg.to_radians().sin());
        // (A step 1 m up, 4 m along: the ray from 2 m up at 20 degrees
        // meets its face at about 0.54 m.)
        let step = Profile::sampled(Vec3::ZERO, Vec3::X, 10.0, &|p| if p.x >= 4.0 { 1.0 } else { 0.0 });
        let hit = step.meets(Vec2::new(0.0, 2.0), ray(20.0)).expect("it meets the step");
        assert!((hit.x - 4.0).abs() < step.spacing + 0.02 && hit.y > 0.3 && hit.y < 1.0, "on the step's face: {hit}");
        // (A ledge 3 m along, 2 m down: the same ray goes on past it, down.)
        let ledge = Profile::sampled(Vec3::ZERO, Vec3::X, 10.0, &|p| if p.x >= 3.0 { -2.0 } else { 0.0 });
        let hit = ledge.meets(Vec2::new(0.0, 2.0), ray(20.0)).expect("it meets the ground below");
        assert!((hit.y + 2.0).abs() < 0.05 && hit.x > 10.0, "on the ground below, past the ledge: {hit}");
        // (Cast back up a wall too high to climb, 3 m up 5 m along: its
        // light no lower than reaches the wall.)
        let wall = Profile::sampled(Vec3::ZERO, Vec3::X, 10.0, &|p| if p.x >= 5.0 { 3.0 } else { 0.0 });
        let e = wall.light_for(2.4, 10.0);
        let hit = wall.meets(Vec2::new(0.0, 2.4), Vec2::new(e.cos(), -e.sin())).expect("it meets the wall");
        assert!(e > 10f32.to_radians() && (hit.x - 5.0).abs() < 0.2, "its light {:.1} degrees, its shadow at {hit}", e.to_degrees());
        // (Its buffer: three of what, its heights four a vector, its way's
        // points.)
        assert_eq!(flat.buffer(0.5, e).len(), 3 + PROFILE.div_ceil(4) + PROFILE);
    }

    /// As its shadow its way curves from its face to where it goes: its
    /// arc sets off along its face and ends there; it rises there; its
    /// top's shadow, with the light lowest, falls at its way's end.
    #[test]
    fn curves_there() {
        let arc = Arc::new(Vec2::ZERO, Vec2::Y, Vec2::new(4.0, 6.0));
        let (end, _) = arc.at(arc.length);
        assert!(end.distance(Vec2::new(4.0, 6.0)) < 1e-3, "its arc ends at {end}");
        let (start, heading) = arc.at(0.0);
        assert!(start.length() < 1e-6 && heading.distance(Vec2::Y) < 1e-6, "its arc sets off from {start} along {heading}");
        let scale = HEIGHT / MESH_HEIGHT;
        let goal = Vec3::new(5.0, 0.0, 6.0);
        let mut sh = Shade::new(scale, Vec3::ZERO, 0.0, 3, SHADE_LOW.to_radians(), SHADE_TIME);
        let mut checked = false;
        for _ in 0..240 * 8 {
            sh.step(1.0 / 240.0, Some(goal), &|_| 0.0);
            if let (Shadow::Leave(_), false) = (sh.doing, checked) {
                checked = true;
                let p = sh.profile.as_ref().expect("its ground");
                let top = Vec2::new(0.0, sh.heights().1);
                let hit = p.meets(top, Vec2::new(sh.lowest.cos(), -sh.lowest.sin())).expect("its top's shadow");
                assert!((hit.x - sh.way.length).abs() < 0.1, "its top's shadow {} along, its way {}", hit.x, sh.way.length);
                assert!(sh.way.turns().abs() > 0.3, "its way does not curve: {}", sh.way.turns());
            }
        }
        assert!(checked && sh.doing == Shadow::Stand && (sh.at - goal).length() < 0.3, "not there: {}", sh.at);
    }

    /// How far its shadow goes is how low its light sinks: its height over
    /// that angle's tangent at most, along its way (twice as tall, twice as
    /// far); sent further, at least part of that.
    #[test]
    fn reaches_by_its_light() {
        let mut longest = Vec::new();
        for (size, low) in [(2.4f32, SHADE_LOW), (4.8, SHADE_LOW), (2.4, 25.0)] {
            let mut sh = Shade::new(size / MESH_HEIGHT, Vec3::ZERO, 0.0, 5, low.to_radians(), SHADE_TIME);
            let reach = sh.heights().1 / low.to_radians().tan();
            let mut most: f32 = 0.0;
            for _ in 0..240 * 30 {
                let before = sh.doing;
                sh.step(1.0 / 240.0, Some(Vec3::new(0.0, 0.0, 200.0)), &|_| 0.0);
                if before == Shadow::Stand && sh.doing == Shadow::Leave(0.0) {
                    assert!(sh.way.length <= reach + 1e-3 && sh.way.length >= reach * 0.6 - 1e-3, "size {size}, light {low}: a way of {} m, its reach {reach}", sh.way.length);
                    most = most.max(sh.way.length);
                }
            }
            assert!(most > 0.0, "size {size}: never left");
            longest.push(reach);
        }
        assert!((longest[1] / longest[0] - 2.0).abs() < 0.01, "twice as tall, not twice as far: {longest:?}");
        assert!(longest[2] < longest[0] * 0.6, "a higher light, not shorter: {longest:?}");
    }
}
