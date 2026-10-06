//! A procedural creature rig: no animation clips, every pose computed each
//! frame from a body plan and what the creature wants to do.
//!
//! - A gait clock coordinates the legs: each leg swings during its own part
//!   of the cycle, its foot planted the rest of the time and swinging on an
//!   arc to where the body will be when it lands. The gait changes with the
//!   speed, blending from one to the next (a beast walks, trots, then
//!   gallops, rocking and flexing its spine), and speeding up and slowing
//!   down take time. Standing still, the clock stops once every foot is
//!   down, and a foot left far from its place takes a step on its own.
//! - A beast's shoulder blades rise as each front leg takes the weight; the
//!   head is held steady while the body bobs; walking, the spine undulates
//!   from side to side.
//! - Nothing is quite regular: the tempo drifts, and each step lifts a
//!   little differently and lands a little off its ideal place.
//! - Two-bone legs (and arms) solved by IK, each bending its own way.
//! - The body bobs with the steps, its chest leading a turn and its hips
//!   following, leaning into the turn; it breathes when still.
//! - The head tracks a target with lag, and now and then glances away.
//! - A tail trails behind, each link following the last.
//! - Poses for attacking: a crouch (lowered, head down, tail up) and a leap
//!   (a fast, flat arc, the body pitching with it: forelimbs thrown out wide
//!   to grab, hind legs kicking back and then swinging under to land).
//! - The head can tilt (rolling about where it looks) and turn suddenly.
//!
//! The rig outputs bones (segments with a thickness); what a creature is
//! made of hangs on them (see `combat/hunter.rs`).

use bevy::prelude::*;
use worldgen::noise::hash01;

/// Where a limb is rooted: the hips or the chest.
#[derive(Clone, Copy, PartialEq)]
pub enum Root {
    Hips,
    Chest,
}

#[derive(Clone, Copy)]
pub struct LegPlan {
    pub root: Root,
    /// Hip offset from its root: x to the right, z forward.
    pub side: f32,
    pub forward: f32,
    pub upper: f32,
    pub lower: f32,
    /// Which way the knee points: +1 forward, -1 back.
    pub knee: f32,
    pub radius: (f32, f32),
}

/// A gait, used from `speed` up (blending into the next): when in the
/// cycle each leg swings (0..1, in the order of `legs`), the fraction of
/// the cycle a leg swings, and the stride.
#[derive(Clone)]
pub struct Gait {
    pub speed: f32,
    pub offsets: Vec<f32>,
    pub swing: f32,
    pub stride: f32,
}

#[derive(Clone, Copy)]
pub struct ArmPlan {
    pub side: f32,
    pub upper: f32,
    pub lower: f32,
    pub radius: (f32, f32),
}

/// A body plan.
#[derive(Clone)]
pub struct Plan {
    /// Hips and chest relative to the root on the ground, when standing:
    /// (forward, height). A biped's chest is above its hips; a beast's in
    /// front.
    pub hips: (f32, f32),
    pub chest: (f32, f32),
    pub torso_radius: f32,
    /// The torso's thickness at the hips, the waist and the chest (times
    /// `torso_radius`): a beast is deep-chested with a tucked-up waist.
    pub torso_profile: [f32; 3],
    /// The neck's thickness at its base (it tapers to the head).
    pub neck_radius: f32,
    /// Neck length and direction (forward, up), head length and radius.
    pub neck: (f32, f32, f32),
    pub head: (f32, f32),
    pub legs: Vec<LegPlan>,
    pub arms: Vec<ArmPlan>,
    /// Tail links and their length (0 for none).
    pub tail: (usize, f32),
    /// Its gaits, slowest first, and how high a foot lifts.
    pub gaits: Vec<Gait>,
    pub lift: f32,
    /// Acceleration and deceleration (m/s²).
    pub accel: f32,
    pub decel: f32,
    /// How far the body bobs, leans into turns, rocks and flexes in the
    /// fastest gait, and how far a shoulder blade rises.
    pub bob: f32,
    pub lean: f32,
    pub rock: f32,
    pub shoulder: f32,
    /// How far the spine swings from side to side when walking.
    pub sway: f32,
}

impl Plan {
    /// A tall biped with long arms, about 3 m.
    pub fn biped() -> Self {
        let leg = |side: f32| LegPlan { root: Root::Hips, side, forward: 0.0, upper: 0.8, lower: 0.8, knee: 1.0, radius: (0.16, 0.1) };
        let arm = |side: f32| ArmPlan { side, upper: 0.75, lower: 0.9, radius: (0.11, 0.07) };
        Plan {
            hips: (0.0, 1.5),
            chest: (0.1, 2.3),
            torso_radius: 0.3,
            torso_profile: [1.0, 1.0, 1.15],
            neck_radius: 0.2,
            neck: (0.25, 0.15, 0.25),
            head: (0.38, 0.18),
            legs: vec![leg(-0.22), leg(0.22)],
            arms: vec![arm(-0.38), arm(0.38)],
            tail: (0, 0.0),
            gaits: vec![
                Gait { speed: 0.0, offsets: vec![0.0, 0.5], swing: 0.42, stride: 1.4 },
                Gait { speed: 6.0, offsets: vec![0.0, 0.5], swing: 0.5, stride: 2.6 },
            ],
            lift: 0.3,
            accel: 8.0,
            decel: 12.0,
            bob: 0.06,
            lean: 0.04,
            rock: 0.0,
            shoulder: 0.0,
            sway: 0.03,
        }
    }

    /// A beast the size of a horse, built like a big cat: low long body,
    /// heavy shoulders, a long tail.
    pub fn beast() -> Self {
        let front = |side: f32| LegPlan { root: Root::Chest, side, forward: 0.1, upper: 0.65, lower: 0.65, knee: -1.0, radius: (0.2, 0.13) };
        let hind = |side: f32| LegPlan { root: Root::Hips, side, forward: -0.05, upper: 0.7, lower: 0.7, knee: -1.0, radius: (0.23, 0.12) };
        Plan {
            hips: (-0.85, 1.25),
            chest: (0.85, 1.3),
            torso_radius: 0.33,
            torso_profile: [0.82, 0.72, 1.12],
            neck_radius: 0.32,
            neck: (0.55, 0.25, 0.22),
            head: (0.68, 0.25),
            // Front left, front right, hind left, hind right.
            legs: vec![front(-0.3), front(0.3), hind(-0.28), hind(0.28)],
            arms: vec![],
            tail: (8, 0.22),
            gaits: vec![
                // A walk: one foot after another down each side.
                Gait { speed: 0.0, offsets: vec![0.25, 0.75, 0.0, 0.5], swing: 0.3, stride: 1.5 },
                // A trot: diagonal pairs together.
                Gait { speed: 5.0, offsets: vec![0.0, 0.5, 0.5, 0.0], swing: 0.45, stride: 2.8 },
                // A gallop: the front pair, then the hind pair.
                // (Each foot is down for only about a third of the cycle.)
                Gait { speed: 11.0, offsets: vec![0.0, 0.1, 0.6, 0.5], swing: 0.66, stride: 5.0 },
            ],
            lift: 0.28,
            accel: 9.0,
            decel: 13.0,
            bob: 0.05,
            lean: 0.06,
            rock: 0.09,
            shoulder: 0.06,
            sway: 0.08,
        }
    }
}

impl Plan {
    /// The gait at a speed, blended between the two nearest (offsets,
    /// swing, stride), and how far into the fastest it is (0..1).
    fn gait(&self, speed: f32) -> (Vec<f32>, f32, f32, f32) {
        let g = &self.gaits;
        let last = g.len() - 1;
        let k = g.iter().rposition(|x| x.speed <= speed).unwrap_or(0);
        let fastest = if last > 0 { ((speed - g[last - 1].speed) / (g[last].speed - g[last - 1].speed)).clamp(0.0, 1.0) } else { 0.0 };
        if k >= last {
            return (g[last].offsets.clone(), g[last].swing, g[last].stride * (speed / g[last].speed.max(0.1)).clamp(1.0, 1.4), fastest);
        }
        let (a, b) = (&g[k], &g[k + 1]);
        let t = ((speed - a.speed) / (b.speed - a.speed)).clamp(0.0, 1.0);
        let offsets = a
            .offsets
            .iter()
            .zip(&b.offsets)
            .map(|(&x, &y)| {
                let d = (y - x + 0.5).rem_euclid(1.0) - 0.5;
                (x + d * t).rem_euclid(1.0)
            })
            .collect();
        (offsets, a.swing + (b.swing - a.swing) * t, a.stride + (b.stride - a.stride) * t, fastest)
    }
}

/// What the creature wants this frame.
#[derive(Clone, Copy, Default)]
pub struct Intent {
    /// Ground velocity wanted (m/s).
    pub velocity: Vec3,
    /// Where to look.
    pub look: Vec3,
    /// 0 standing, 1 fully crouched.
    pub crouch: f32,
    /// A leg to hold raised (standing still): a paw lifted, poised.
    pub paw: Option<usize>,
    /// The head's roll (radians), and how fast it turns to look (0: its
    /// usual rate; much more for a sudden snap).
    pub tilt: f32,
    pub head_rate: f32,
}

#[derive(Clone, Copy)]
struct Foot {
    planted: Vec3,
    from: Vec3,
    swinging: bool,
}

/// A bone the body hangs on: a segment, its thickness, and a side axis.
#[derive(Clone, Copy)]
pub struct Bone {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    /// Perpendicular to the bone, to the body's right: with the bone it
    /// fixes the frame things hang in.
    pub side: Vec3,
}

impl Bone {
    pub fn rotation(&self) -> Quat {
        let y = (self.b - self.a).normalize_or(Vec3::Y);
        let x = (self.side - y * self.side.dot(y)).normalize_or(y.any_orthonormal_vector());
        let z = x.cross(y);
        Quat::from_mat3(&Mat3::from_cols(x, y, z))
    }

    /// Nearest hit of a ray on this bone as a capsule, if any.
    pub fn ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        // Closest approach between the ray and the segment.
        let u = self.b - self.a;
        let w = origin - self.a;
        let (a, b, c, d, e) = (dir.dot(dir), dir.dot(u), u.dot(u), dir.dot(w), u.dot(w));
        let den = a * c - b * b;
        let s = if den.abs() < 1e-6 { 0.0 } else { ((b * e - c * d) / den).clamp(0.0, max) };
        let t = ((b * s + e) / c.max(1e-6)).clamp(0.0, 1.0);
        let p = origin + dir * s;
        let q = self.a + u * t;
        let gap = p.distance(q);
        if gap > self.radius {
            return None;
        }
        // Back off to the capsule's surface.
        Some((s - (self.radius * self.radius - gap * gap).sqrt()).max(0.0))
    }
}

pub struct Rig {
    pub plan: Plan,
    /// Where it stands (on the ground) and which way it faces.
    pub root: Vec3,
    pub heading: f32,
    pub velocity: Vec3,
    /// Off the ground in a leap (vertical speed), or None.
    pub airborne: Option<f32>,
    /// Pushed by hits (m/s, decaying).
    pub knock: Vec3,
    chest_yaw: f32,
    hips_yaw: f32,
    turn_rate: f32,
    phase: f32,
    /// The current gait: each leg's place in the cycle and where it is now,
    /// the swing fraction, the stride, and how far into the fastest gait.
    offsets: Vec<f32>,
    local: Vec<f32>,
    swing: f32,
    stride: f32,
    gallop: f32,
    /// The head's height, held steady while the body bobs.
    head_y: Option<f32>,
    /// Steps each foot has taken (to vary each one).
    steps: Vec<u32>,
    /// A raised paw: which leg, and how far up (0..1).
    paw: (usize, f32),
    /// In a leap: seconds since it left the ground, and how long it will be
    /// in the air.
    air: (f32, f32),
    tilt: f32,
    feet: Vec<Foot>,
    head_dir: Vec3,
    glance: Vec3,
    glance_in: f32,
    tail: Vec<Vec3>,
    time: f32,
    crouch: f32,
    seed: u32,
    pub bones: Vec<Bone>,
}

/// Two-bone IK: where the middle joint goes so that a limb from `a` of
/// lengths `l1`, `l2` reaches `target`, bending towards `pole`.
fn ik(a: Vec3, target: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - a;
    let d = to.length().clamp(0.01, (l1 + l2) * 0.999);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let end = a + dir * d;
    // Law of cosines: distance along the line to the joint's foot, and how
    // far out it sits.
    let x = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d);
    let h = (l1 * l1 - x * x).max(0.0).sqrt();
    let bend = (pole - dir * pole.dot(dir)).normalize_or(dir.any_orthonormal_vector());
    (a + dir * x + bend * h, end)
}

impl Rig {
    pub fn new(plan: Plan, root: Vec3, heading: f32, seed: u32, ground: &impl Fn(f32, f32) -> f32) -> Self {
        let mut rig = Rig {
            feet: Vec::new(),
            tail: Vec::new(),
            plan,
            root,
            heading,
            velocity: Vec3::ZERO,
            airborne: None,
            knock: Vec3::ZERO,
            chest_yaw: heading,
            hips_yaw: heading,
            turn_rate: 0.0,
            phase: 0.0,
            offsets: Vec::new(),
            local: Vec::new(),
            swing: 0.4,
            stride: 2.0,
            gallop: 0.0,
            head_y: None,
            steps: Vec::new(),
            paw: (0, 0.0),
            air: (0.0, 1.0),
            tilt: 0.0,
            head_dir: Vec3::new(heading.sin(), 0.0, heading.cos()),
            glance: Vec3::ZERO,
            glance_in: 1.0,
            time: 0.0,
            crouch: 0.0,
            seed,
            bones: Vec::new(),
        };
        let (offsets, swing, stride, _) = rig.plan.gait(0.0);
        rig.local = offsets.clone();
        (rig.offsets, rig.swing, rig.stride) = (offsets, swing, stride);
        rig.steps = vec![0; rig.plan.legs.len()];
        let homes: Vec<Vec3> = (0..rig.plan.legs.len()).map(|i| rig.home(i, ground)).collect();
        rig.feet = homes.into_iter().map(|h| Foot { planted: h, from: h, swinging: false }).collect();
        let back = -rig.forward_of(heading);
        let hips = rig.anchor(Root::Hips);
        rig.tail = (0..=rig.plan.tail.0).map(|k| hips + back * rig.plan.tail.1 * k as f32).collect();
        rig.solve();
        rig
    }

    fn forward_of(&self, yaw: f32) -> Vec3 {
        Vec3::new(yaw.sin(), 0.0, yaw.cos())
    }

    /// The hips or the chest in the world, posed (crouch, bob, breath).
    fn anchor(&self, root: Root) -> Vec3 {
        let (yaw, (f, h)) = match root {
            Root::Hips => (self.hips_yaw, self.plan.hips),
            Root::Chest => (self.chest_yaw, self.plan.chest),
        };
        let mid = (self.plan.hips.0 + self.plan.chest.0) * 0.5;
        let forward = self.forward_of(yaw);
        let moving = (self.velocity.length() / 3.0).min(1.0);
        let bob = -self.plan.bob * moving * (1.0 - self.gallop) * (1.0 - (self.phase * std::f32::consts::TAU * 2.0).cos()) * 0.5;
        // Galloping: the body rocks (chest up as the hips go down) and the
        // spine flexes, the hips reaching forward under the body.
        let wave = (self.phase * std::f32::consts::TAU).sin();
        let rock = self.plan.rock * self.gallop * wave * if root == Root::Chest { 1.0 } else { -1.0 };
        let flex = if root == Root::Hips { self.plan.rock * 2.5 * self.gallop * (self.phase * std::f32::consts::TAU + 1.6).sin() } else { 0.0 };
        // Walking, the spine swings from side to side, chest and hips in
        // opposition.
        let sway = self.plan.sway * moving * (1.0 - self.gallop) * wave * if root == Root::Chest { 1.0 } else { -1.0 };
        let breath = 0.012 * (self.time * 1.6).sin() * (1.0 - moving);
        // Crouched: lower, and the chest lower still (the head goes down).
        // Running, the body drops, so the legs can reach far fore and aft.
        let run = 0.2 * (Vec2::new(self.velocity.x, self.velocity.z).length() / 12.0).min(1.0);
        let low = (self.crouch * if root == Root::Chest { 0.42 } else { 0.3 }).max(run);
        // In a leap the body pitches with its arc: nose up rising, down
        // falling.
        let pitch = (self.airborne.unwrap_or(0.0) * 0.04).clamp(-0.25, 0.25) * if root == Root::Chest { 1.0 } else { -1.0 };
        let height = h * (1.0 - low) + bob + rock + pitch + if root == Root::Chest { breath } else { 0.0 };
        // Leaning into a turn: the body swings towards its inside.
        let speed = Vec2::new(self.velocity.x, self.velocity.z).length();
        let roll = (self.turn_rate * speed * self.plan.lean).clamp(-0.35, 0.35);
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        self.root + forward * (f - mid + flex) + Vec3::Y * height + right * (sway - roll * height)
    }

    fn hip(&self, i: usize) -> Vec3 {
        let leg = self.plan.legs[i];
        let yaw = if leg.root == Root::Hips { self.hips_yaw } else { self.chest_yaw };
        let forward = self.forward_of(yaw);
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        // A shoulder blade rises while its leg bears the weight.
        let local = self.local.get(i).copied().unwrap_or(0.5);
        let stance = if local < self.swing { 0.0 } else { (std::f32::consts::PI * (local - self.swing) / (1.0 - self.swing)).sin() };
        let blade = if leg.root == Root::Chest { self.plan.shoulder * stance * (self.velocity.length() / 2.0).min(1.0) } else { 0.0 };
        self.anchor(leg.root) + right * leg.side + forward * leg.forward + Vec3::Y * blade
    }

    /// Where a foot belongs: under its hip, a little ahead when moving.
    fn home(&self, i: usize, ground: &impl Fn(f32, f32) -> f32) -> Vec3 {
        let hip = self.hip(i);
        // A foot lands half a stance ahead of its hip (it is down while the
        // body passes over it), but never beyond the leg's reach.
        let leg = self.plan.legs[i];
        let speed = Vec2::new(self.velocity.x, self.velocity.z).length();
        let ahead = ((1.0 - self.swing) * self.stride * 0.5).min((leg.upper + leg.lower) * 0.6) * (speed / 1.5).min(1.0);
        let lead = Vec3::new(self.velocity.x, 0.0, self.velocity.z).normalize_or_zero() * ahead;
        // Each step lands a little off its ideal place.
        let k = self.steps.get(i).copied().unwrap_or(0) as i32;
        let r = |j: i32| hash01(self.seed as i32 + i as i32 * 101, k, j, 0x61b) - 0.5;
        let jitter = Vec3::new(r(0), 0.0, r(1)) * 0.12 * (self.velocity.length() / 2.0).min(1.0);
        let p = Vec3::new(hip.x, 0.0, hip.z) + Vec3::new(lead.x, 0.0, lead.z) + jitter;
        Vec3::new(p.x, ground(p.x, p.z), p.z)
    }

    /// One frame: move towards the intent, step, pose, and output bones.
    pub fn update(&mut self, dt: f32, intent: Intent, ground: &impl Fn(f32, f32) -> f32) {
        self.time += dt;
        self.crouch += (intent.crouch - self.crouch) * (1.0 - (-dt * 8.0).exp());
        // A raised paw goes up slowly, comes down quicker.
        let still = Vec2::new(self.velocity.x, self.velocity.z).length() < 0.4;
        match intent.paw {
            Some(leg) if still && (self.paw.1 < 0.01 || self.paw.0 == leg) => {
                self.paw = (leg, (self.paw.1 + dt * 2.5).min(1.0));
            }
            _ => self.paw.1 = (self.paw.1 - dt * 5.0).max(0.0),
        }
        let old_heading = self.heading;
        if let Some(vy) = self.airborne.as_mut() {
            // A leap: ballistic.
            self.air.0 += dt;
            *vy -= 25.0 * dt;
            self.root += Vec3::new(self.velocity.x, *vy, self.velocity.z) * dt;
            let floor = ground(self.root.x, self.root.z);
            if self.root.y <= floor && *vy < 0.0 {
                self.root.y = floor;
                self.airborne = None;
                // Landing: the feet come down where they are.
                for i in 0..self.feet.len() {
                    let h = self.home(i, ground);
                    self.feet[i] = Foot { planted: h, from: h, swinging: false };
                }
                self.crouch = 0.7;
            }
        } else {
            // On the ground: accelerate towards the wanted velocity, turn to
            // face where it goes (or where it looks, standing).
            let faster = intent.velocity.length() > self.velocity.length();
            let change = (intent.velocity - self.velocity).clamp_length_max(if faster { self.plan.accel } else { self.plan.decel } * dt);
            self.velocity += change;
            self.velocity.y = 0.0;
            let face = if self.velocity.length() > 0.5 { self.velocity } else { intent.look - self.root };
            if Vec2::new(face.x, face.z).length() > 0.01 {
                let target = face.x.atan2(face.z);
                let turn = (target - self.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                self.heading += turn.clamp(-3.0 * dt, 3.0 * dt);
            }
            self.root += (self.velocity + self.knock) * dt;
            self.root.y = ground(self.root.x, self.root.z);
        }
        self.knock *= (-dt * 6.0).exp();
        self.turn_rate += ((self.heading - old_heading) / dt.max(1e-4) - self.turn_rate) * (1.0 - (-dt * 6.0).exp());
        // The chest leads a turn, the hips follow.
        let lead = (self.turn_rate * 0.15).clamp(-0.4, 0.4);
        let follow = |from: f32, to: f32, rate: f32| {
            let d = (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            from + d * (1.0 - (-dt * rate).exp())
        };
        self.chest_yaw = follow(self.chest_yaw, self.heading + lead, 10.0);
        self.hips_yaw = follow(self.hips_yaw, self.heading - lead * 0.5, 5.0);

        // The gait clock runs with the speed; standing, it runs on only until
        // every foot is down.
        let speed = Vec2::new(self.velocity.x, self.velocity.z).length();
        let any_swinging = self.feet.iter().any(|f| f.swinging);
        let (offsets, swing, stride, gallop) = self.plan.gait(speed);
        (self.offsets, self.swing, self.stride) = (offsets, swing, stride);
        self.gallop += (gallop - self.gallop) * (1.0 - (-dt * 4.0).exp());
        if self.airborne.is_none() && (speed > 0.3 || any_swinging) {
            // The tempo drifts a little.
            let drift = 1.0 + 0.09 * ((self.time * 0.9 + self.seed as f32).sin() * 0.6 + (self.time * 2.3).sin() * 0.4);
            self.phase = (self.phase + speed.max(1.0) / self.stride * drift * dt).fract();
        }
        for i in 0..self.feet.len() {
            let home = self.home(i, ground);
            let local = (self.phase - self.offsets[i]).rem_euclid(1.0);
            self.local[i] = local;
            if self.airborne.is_some() {
                continue;
            }
            let in_swing = local < self.swing && (speed > 0.3 || self.feet[i].swinging);
            let foot = &mut self.feet[i];
            if in_swing {
                if !foot.swinging {
                    foot.swinging = true;
                    foot.from = foot.planted;
                    self.steps[i] += 1;
                }
                let s = local / self.swing;
                // Higher the faster: at a gallop the paw is tucked up high.
                let reach = self.plan.legs[i].upper + self.plan.legs[i].lower;
                let lift = (self.plan.lift * (1.0 + speed / 5.0)).min(reach * 0.5) * (0.75 + 0.5 * hash01(self.seed as i32, i as i32, self.steps[i] as i32, 0x61c));
                let arc = (s * std::f32::consts::PI).sin() * lift;
                foot.planted = foot.from.lerp(home, s * s * (3.0 - 2.0 * s)) + Vec3::Y * arc;
            } else if foot.swinging {
                foot.swinging = false;
                foot.planted = home;
            } else if speed <= 0.3 && foot.planted.distance(home) > self.stride * 0.3 {
                // Standing with a foot out of place: a settling step.
                foot.from = foot.planted;
                foot.swinging = true;
                self.phase = (self.offsets[i] + 0.01).fract();
            }
        }

        // The head: towards the target, with lag; now and then a glance.
        self.glance_in -= dt;
        if self.glance_in <= 0.0 {
            let r = |k: i32| hash01(self.seed as i32, (self.time * 10.0) as i32, k, 0x61a) - 0.5;
            self.glance = if r(0) > 0.1 { Vec3::new(r(1), r(2) * 0.4, r(3)) * 1.6 } else { Vec3::ZERO };
            self.glance_in = 0.6 + 2.2 * (r(4) + 0.5);
        }
        let chest = self.anchor(Root::Chest);
        let want = ((intent.look - chest).normalize_or(self.forward_of(self.chest_yaw)) + self.glance * 0.35).normalize_or(Vec3::Z);
        // In a leap the head is thrown at its prey.
        let rate = if self.airborne.is_some() { 25.0 } else if intent.head_rate > 0.0 { intent.head_rate } else { 7.0 };
        self.head_dir = self.head_dir.lerp(want, 1.0 - (-dt * rate).exp()).normalize_or(want);
        self.tilt += (intent.tilt - self.tilt) * (1.0 - (-dt * 3.0).exp());
        // The head's height follows the body's slowly: steady while it bobs.
        let raw = self.neck_end().y;
        self.head_y = Some(match self.head_y {
            Some(y) => raw + (y + (raw - y) * (1.0 - (-dt * 4.0).exp()) - raw).clamp(-0.15, 0.15),
            None => raw,
        });

        // The tail: each link follows the last at its length, sagging, and
        // raised in a crouch.
        if !self.tail.is_empty() {
            let hips = self.anchor(Root::Hips);
            let back = -self.forward_of(self.hips_yaw);
            self.tail[0] = hips + back * 0.25;
            let len = self.plan.tail.1;
            for k in 1..self.tail.len() {
                let prev = self.tail[k - 1];
                let sway = Vec3::new(self.forward_of(self.hips_yaw).z, 0.0, -self.forward_of(self.hips_yaw).x) * 0.02 * (self.time * 2.5 + k as f32 * 0.6).sin();
                let droop = Vec3::Y * (-0.04 + 0.09 * self.crouch);
                let mut p = self.tail[k] + droop + sway + back * 0.01;
                p = prev + (p - prev).normalize_or(back) * len;
                self.tail[k] = p;
            }
        }

        self.solve();
    }

    /// The bones, from the pose.
    fn solve(&mut self) {
        let mut bones = Vec::new();
        let hips = self.anchor(Root::Hips);
        let chest = self.anchor(Root::Chest);
        let forward = self.forward_of(self.chest_yaw);
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        // Torso: a few segments from hips to chest.
        let n = 3;
        for k in 0..n {
            let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            let swell = self.plan.torso_profile[k.min(2)];
            bones.push(Bone { a: hips.lerp(chest, t0), b: hips.lerp(chest, t1), radius: self.plan.torso_radius * swell, side: right });
        }
        // Neck and head, along where it looks (the neck goes partway); the
        // head at its steadied height.
        let mut neck_end = self.neck_end();
        if let Some(y) = self.head_y {
            neck_end.y = y;
        }
        bones.push(Bone { a: chest, b: neck_end, radius: self.plan.neck_radius, side: right });
        let head_end = neck_end + self.head_dir * self.plan.head.0;
        let tilted = Quat::from_axis_angle(self.head_dir.normalize_or(forward), self.tilt) * right;
        bones.push(Bone { a: neck_end, b: head_end, radius: self.plan.head.1, side: tilted });
        // Legs.
        for i in 0..self.plan.legs.len() {
            let leg = self.plan.legs[i];
            let hip = self.hip(i);
            let yaw = if leg.root == Root::Hips { self.hips_yaw } else { self.chest_yaw };
            let fwd = self.forward_of(yaw);
            let reach = leg.upper + leg.lower;
            let target = match self.airborne {
                // In a leap: forelimbs thrown forward and out wide to grab,
                // dropping to land; hind legs kicked back, then swung under.
                Some(_) => {
                    let s = (self.air.0 / self.air.1.max(0.05)).clamp(0.0, 1.0);
                    let out = Vec3::new(fwd.z, 0.0, -fwd.x) * leg.side.signum();
                    if leg.root == Root::Chest {
                        hip + fwd * reach * 0.85 + out * reach * 0.35 - Vec3::Y * reach * (0.15 + 0.45 * s)
                    } else {
                        hip - fwd * reach * (0.85 * (1.0 - s) - 0.2 * s) - Vec3::Y * reach * (0.5 + 0.25 * s)
                    }
                }
                None if self.paw.0 == i && self.paw.1 > 0.0 && !self.feet[i].swinging => {
                    // Poised: lifted and drawn up under the chest.
                    let k = self.paw.1 * self.paw.1 * (3.0 - 2.0 * self.paw.1);
                    self.feet[i].planted + Vec3::Y * reach * 0.38 * k - fwd * reach * 0.12 * k
                }
                None => self.feet[i].planted,
            };
            let (knee, end) = ik(hip, target, leg.upper, leg.lower, fwd * leg.knee);
            bones.push(Bone { a: hip, b: knee, radius: leg.radius.0, side: right });
            bones.push(Bone { a: knee, b: end, radius: leg.radius.1, side: right });
        }
        // Arms: hanging and swinging opposite the legs; reaching forward in
        // a crouch.
        for (k, arm) in self.plan.arms.iter().enumerate() {
            let shoulder = chest + right * arm.side - forward * 0.05;
            let swing = (self.phase * std::f32::consts::TAU + if k == 0 { 0.0 } else { std::f32::consts::PI }).sin();
            let moving = (Vec2::new(self.velocity.x, self.velocity.z).length() / 3.0).min(1.0);
            let reach = arm.upper + arm.lower;
            let hang = shoulder + Vec3::Y * -reach * 0.9 + forward * reach * 0.3 * swing * moving + right * arm.side.signum() * 0.1;
            let lunge = shoulder + self.head_dir * reach * 0.95;
            let hand = hang.lerp(lunge, self.crouch);
            let (elbow, end) = ik(shoulder, hand, arm.upper, arm.lower, -forward + Vec3::Y * 0.2);
            bones.push(Bone { a: shoulder, b: elbow, radius: arm.radius.0, side: right });
            bones.push(Bone { a: elbow, b: end, radius: arm.radius.1, side: right });
        }
        // Tail.
        for k in 1..self.tail.len() {
            let t = k as f32 / self.tail.len() as f32;
            bones.push(Bone { a: self.tail[k - 1], b: self.tail[k], radius: 0.09 * (1.0 - t * 0.7), side: right });
        }
        self.bones = bones;
    }

    /// Where the neck ends (the head starts), from the pose.
    fn neck_end(&self) -> Vec3 {
        let chest = self.anchor(Root::Chest);
        let forward = self.forward_of(self.chest_yaw);
        let (neck_len, neck_fwd, neck_up) = self.plan.neck;
        let base = Vec3::new(neck_fwd, neck_up, 0.0).normalize_or(Vec3::Y);
        let neck_dir = (forward * base.x + Vec3::Y * base.y).lerp(self.head_dir, 0.4).normalize_or(Vec3::Y);
        let neck_dir = (neck_dir - Vec3::Y * 0.5 * self.crouch).normalize_or(forward);
        chest + neck_dir * neck_len
    }

    /// The head's tip and the direction it faces (for eyes).
    pub fn head(&self) -> (Vec3, Vec3) {
        // (Three torso segments, the neck, then the head.)
        let head = self.bones[4];
        (head.b, (head.b - head.a).normalize_or(Vec3::Z))
    }

    /// The middle of the chest.
    pub fn chest(&self) -> Vec3 {
        self.anchor(Root::Chest)
    }

    /// How fast it is moving over the ground.
    pub fn speed(&self) -> f32 {
        Vec2::new(self.velocity.x, self.velocity.z).length()
    }

    /// Leap towards `dir` (horizontal) with this speed and upward speed.
    pub fn leap(&mut self, dir: Vec3, speed: f32, up: f32) {
        self.velocity = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::Z) * speed;
        self.airborne = Some(up);
        self.air = (0.0, 2.0 * up / 25.0);
        self.root.y += 0.05;
    }
}
