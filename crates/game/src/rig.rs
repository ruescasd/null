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
    /// Which way the knee points: +1 forward, -1 back; and how far up and
    /// out to its side as well (a knee above the back).
    pub knee: f32,
    pub knee_up: f32,
    pub knee_out: f32,
    /// How far out to its side from its hip the foot is set down.
    pub spread: f32,
    pub radius: (f32, f32),
}

/// How a body moves.
#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    /// As a mammal: walk, trot, gallop, the body carried over the feet.
    Gallop,
    /// Long forelimbs reach far ahead, plant, and haul the body after them
    /// in a surge; the hind legs, short, knees high, hop up behind.
    Hauler,
    /// Legs out from its sides, knees high over a body slung low between
    /// them, the body gliding level, turning short.
    Sprawl,
    /// On stilts (the user's reference: Half-Life 2's stalker): stilts from
    /// the knee down; the leg it stands on locked straight (a stilt does not
    /// give), the body vaulting up over it and dipping as the other comes
    /// down; the swinging one folding at the knee to clear the ground. Slow,
    /// uncertain, its weight shifting over each, its arms out for balance.
    Stilt,
    /// Bearing a load (the porter, `--opt porter`; the user's idea): a
    /// heavy thing on its back (a plain slab for now), fixed to it, hunches
    /// it forward to counter it, its chest ahead of its hips; its legs do not
    /// stride but catch up, its feet dragged till they step (each step its
    /// own); its body moving as the stalker's, heavier (a sprung chest, its
    /// load's weight jolting it down at each footfall, a torso that hauls
    /// each leg through, a head held in the world); its arms hanging dead
    /// (gripping its load, they flailed); its head low, watching its way.
    /// (It began as a puppet: a human walk under "wrong control",
    /// decorations with no cause, the human trap; then towed from its
    /// chest, workable, but a struggle like the stalker's: the load is its
    /// own cause and its own shape.)
    Porter,
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
    /// How far the body runs on behind the hip joints (a beast's rump, the
    /// thighs hanging from under it rather than from its end).
    pub rump: f32,
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
    /// How it moves, and how fast it turns to face where it goes (rad/s).
    pub style: Style,
    pub turn: f32,
    /// Its legs about its body, each set out from it the way it stands
    /// from the body's middle (a tripod's), not to the left or the right.
    pub radial: bool,
}

/// On stilts, its body sways on its hips (a creature's motion, neither a
/// robot's nor a man's: every motion with a cause in the body): its chest
/// sprung after where it would be (how stiff, how damped: a little under,
/// so it overshoots), leaning away from a stilt swinging out (m); its head
/// held still in the world, aimed at what it looks at, the body moving
/// under it, till the neck has been carried this far (m) and it catches up
/// to its new place (how fast). (Held 18 cm, caught up at 30/s, it seemed
/// to teleport after the body, the user: now short holds, eased.)
const STILT_SWAY: (f32, f32, f32) = (120.0, 10.0, 0.035);
const HEAD_HOLD: (f32, f32) = (0.06, 10.0);
/// On stilts, its arms hang dead from its shoulders, carried only by its
/// body's motion (the user: held out for balance they read as a tightrope,
/// awkward; it is impaired but used to it, its arms long given up): its
/// hands on springs (how stiff, how damped), swinging as weights.
const STILT_HANDS: (f32, f32) = (20.0, 3.5);
/// The porter, hunched under its load: the chest drawn ahead of its hips (m,
/// at its pace); a foot dragged along the ground once the body has drawn it
/// this far from its place (m), sliding after it (how fast), stepping once
/// drawn this far (m, each step its own) and taking this long (s, each its
/// own), lifted this little (m), landing this far ahead of its hip (m, at
/// its pace); its head lolling (how fast it follows). (Each step alike,
/// the same reach and time and an eased arc, its feet set fast till they
/// lifted: robotic, the user.)
const PORTER_TOW: f32 = 0.3;
const PORTER_DRAG: (f32, f32) = (0.2, 3.0);
const PORTER_STEP: ((f32, f32), (f32, f32), f32, f32) = ((0.35, 0.6), (0.24, 0.36), 0.05, 0.2);
const PORTER_LOLL: f32 = 3.0;
/// The porter's cadence: its legs' effort moves its body (gliding on over
/// still legs, it moved without cause, the user). A step, the body pushed
/// on with it, rising from its creep to most mid-step and settling as the
/// foot lands (switched on and off, it lurched); then a rest (s, each its
/// own, longer the slower it goes), the body all but still (of its pace);
/// then the foot furthest behind steps. (Rests and steps of very different
/// lengths made some surges lunge: narrower ranges. Rests of a fifth of a
/// second, the body all but stopped, read as lunges, not a walk, the user:
/// short, the body keeping some way on.)
const PORTER_REST: ((f32, f32), f32) = ((0.06, 0.12), 0.35);
/// Its load: a slab on its back, lying along it (a cube, upright, sat at
/// the wrong angle and overspecified its shape: the user), how big (m:
/// across, along the back, deep). Fixed to the back: it moves only as the
/// back does (sprung, bobbing of itself, it did not fit a massive thing,
/// the user); its weight in the body instead: at each footfall the chest
/// is jolted down (m/s).
pub(crate) const LOAD_SIZE: Vec3 = Vec3::new(0.55, 0.75, 0.3);
const LOAD_JOLT: f32 = 0.5;
/// On stilts, its body throws itself into each step (it leaned back as if
/// dragged by its legs, the user): moving, its chest leads (m at its pace);
/// as each leg swings, it pitches on a little further (m).
const STILT_HEAVE: (f32, f32) = (0.18, 0.08);
/// On stilts, its torso turns to haul each leg through (the user): before
/// a leg swings it winds back on that side (rad), over the last of the leg's
/// stance (of the cycle), then turns on (rad) as it swings, pulling it:
/// clockwise for the left, counter for the right.
const STILT_HAUL: (f32, f32, f32) = (0.08, 0.15, 0.18);
/// The stalker through its beat: how much of its pace it keeps (creeping
/// on, not stopped dead), and how quickly it eases to it and back (per s):
/// stopped and started at once, it went in fits and starts (the user; the
/// porter's lesson: motion switched on and off reads as a lurch).
pub(crate) const STILT_GATHER: (f32, f32) = (0.45, 10.0);
/// The stalker's beat after each footfall: at least, and up to how much
/// more (s; 0.12 and 0.23 first, too erratic, the user), times how much
/// (`stilt_beat`, the user's 0.9).
const STILT_BEAT: (f32, f32) = (0.17, 0.12);
pub(crate) const STILT_BEAT_SCALE: f32 = 0.9;
/// How much of its swing a stilt takes to let go of the body (it rising
/// smoothly over the stilt it stands on); and how far it rises over the
/// stilt it stands on, from where it sank between them, of all the way (its
/// knee bent the rest: `stilt_vault`, the user's 0.3).
const STILT_LET_GO: f32 = 0.4;
pub(crate) const STILT_VAULT: f32 = 0.3;
/// The porter's body, heavier under its load (the stalker's swayed too much
/// for its mass, the user): its chest's spring (stiffer, more damped, a
/// smaller lean), its heave (m), its haul (wound back, turned on: rad), its
/// head held (m) and caught up (how fast).
const PORTER_SWAY: (f32, f32, f32) = (220.0, 20.0, 0.015);
const PORTER_HEAVE: (f32, f32) = (0.08, 0.03);
const PORTER_HAUL: (f32, f32) = (0.04, 0.08);
const PORTER_HOLD: (f32, f32) = (0.03, 14.0);

impl Plan {
    /// The stalker (`--opt stalker`): a small, thin human body high on two
    /// long kneeless stilts, slow, its arms long and thin.
    pub fn stalker() -> Self {
        // (For judging its motion, the user: a longer body on shorter
        // legs, 1.6 m, than first, 2.1, under a 0.55 m body.) Stilts from
        // the knee down (as the reference's; kneeless, a stilt swinging
        // through had to circle out to clear the ground, and read as a
        // waddle, comic, the user): a short thigh folding a long thin stilt.
        // (The knee a little lower, the user: 0.65 over 0.95, was 0.5 over
        // 1.1.)
        let leg = |side: f32| LegPlan { root: Root::Hips, side, forward: 0.0, upper: 0.65, lower: 0.95, knee: 1.0, knee_up: 0.0, knee_out: 0.0, spread: 0.08, radius: (0.05, 0.022) };
        let arm = |side: f32| ArmPlan { side, upper: 0.6, lower: 0.7, radius: (0.04, 0.03) };
        Plan {
            hips: (0.0, 1.55),
            // (Hunched over its stilts: its chest well before its hips.)
            chest: (0.28, 2.36),
            torso_radius: 0.16,
            torso_profile: [0.8, 0.85, 1.1],
            rump: 0.0,
            neck_radius: 0.05,
            neck: (0.2, 0.2, 1.0),
            head: (0.25, 0.11),
            legs: vec![leg(-0.13), leg(0.13)],
            arms: vec![arm(-0.21), arm(0.21)],
            tail: (0, 0.0),
            gaits: vec![
                Gait { speed: 0.0, offsets: vec![0.0, 0.5], swing: 0.45, stride: 0.8 },
                Gait { speed: 1.2, offsets: vec![0.0, 0.5], swing: 0.42, stride: 1.1 },
            ],
            lift: 0.28,
            accel: 1.5,
            decel: 3.0,
            bob: 0.0,
            lean: 0.0,
            rock: 0.0,
            shoulder: 0.0,
            sway: 0.12,
            style: Style::Stilt,
            turn: 1.2,
            radial: false,
        }
    }

    /// The porter: the biped's build at about 2.4 m (`Style::Porter`).
    pub fn porter() -> Self {
        Plan { style: Style::Porter, sway: 0.0, bob: 0.0, lean: 0.0, ..Plan::biped().scaled(0.8) }
    }

    /// A tall biped with long arms, about 3 m.
    pub fn biped() -> Self {
        let leg = |side: f32| LegPlan { root: Root::Hips, side, forward: 0.0, upper: 0.8, lower: 0.8, knee: 1.0, knee_up: 0.0, knee_out: 0.0, spread: 0.0, radius: (0.16, 0.1) };
        let arm = |side: f32| ArmPlan { side, upper: 0.75, lower: 0.9, radius: (0.11, 0.07) };
        Plan {
            hips: (0.0, 1.5),
            chest: (0.1, 2.3),
            torso_radius: 0.3,
            torso_profile: [1.0, 1.0, 1.15],
            rump: 0.0,
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
            style: Style::Gallop,
            turn: 3.0,
            radial: false,
        }
    }

    /// The same plan, every length times `k` (a bigger or smaller animal of
    /// the same build; its gaits change at the same speeds).
    pub fn scaled(mut self, k: f32) -> Self {
        self.hips = (self.hips.0 * k, self.hips.1 * k);
        self.chest = (self.chest.0 * k, self.chest.1 * k);
        self.torso_radius *= k;
        self.rump *= k;
        self.neck_radius *= k;
        self.neck.0 *= k;
        self.head = (self.head.0 * k, self.head.1 * k);
        for leg in &mut self.legs {
            leg.side *= k;
            leg.forward *= k;
            leg.upper *= k;
            leg.lower *= k;
            leg.spread *= k;
            leg.radius = (leg.radius.0 * k, leg.radius.1 * k);
        }
        for arm in &mut self.arms {
            arm.side *= k;
            arm.upper *= k;
            arm.lower *= k;
            arm.radius = (arm.radius.0 * k, arm.radius.1 * k);
        }
        self.tail.1 *= k;
        for g in &mut self.gaits {
            g.stride *= k;
        }
        self.lift *= k;
        self.bob *= k;
        self.rock *= k;
        self.shoulder *= k;
        self.sway *= k;
        self
    }

    /// A four-legged hunter that moves as no mammal does (a mammal's gallop,
    /// however tuned, read as a dog running to its master, the user): long
    /// forelimbs, elbows above the back, reaching far ahead and hauling the
    /// body after them in surges; short hind legs, knees high, hopping up
    /// behind; the chest high, the rump low, the head hung low between the
    /// shoulders; no tail. Thin legs: hard, a spider's.
    pub fn beast() -> Self {
        let fore = |side: f32| LegPlan { root: Root::Chest, side, forward: 0.15, upper: 1.3, lower: 1.4, knee: -0.6, knee_up: 1.0, knee_out: 0.6, spread: 0.35, radius: (0.14, 0.09) };
        let hind = |side: f32| LegPlan { root: Root::Hips, side, forward: 0.0, upper: 0.75, lower: 0.8, knee: 0.3, knee_up: 1.0, knee_out: 0.5, spread: 0.3, radius: (0.16, 0.09) };
        Plan {
            hips: (-0.8, 0.8),
            chest: (0.8, 1.2),
            torso_radius: 0.33,
            torso_profile: [0.82, 0.72, 1.12],
            rump: 0.35,
            neck_radius: 0.32,
            neck: (0.55, 0.35, -0.05),
            head: (0.68, 0.25),
            // Front left, front right, hind left, hind right.
            legs: vec![fore(-0.42), fore(0.42), hind(-0.3), hind(0.3)],
            arms: vec![],
            tail: (0, 0.0),
            gaits: vec![
                // Slow, the arms one after the other.
                Gait { speed: 0.0, offsets: vec![0.0, 0.5, 0.25, 0.75], swing: 0.3, stride: 2.0 },
                // Fast, both arms reaching together (one a little behind),
                // the body hauled after them as they bear, the hind legs
                // hopping up in the haul.
                Gait { speed: 6.0, offsets: vec![0.0, 0.06, 0.45, 0.5], swing: 0.4, stride: 5.5 },
            ],
            lift: 0.45,
            accel: 20.0,
            decel: 24.0,
            bob: 0.0,
            lean: 0.06,
            rock: 0.0,
            shoulder: 0.1,
            sway: 0.0,
            style: Style::Hauler,
            turn: 3.0,
            radial: false,
        }
    }

    /// The sprawl (`--opt sprawl`): a tripod (the user's idea), three long
    /// legs at a third of a turn about a central body, their knees up over
    /// it and out, their feet set wide; no front, so it never turns, going
    /// any way as it is (a flanker's: no turn to give it away). Its body
    /// upright, of a mass with its legs (a small body on long legs read as a
    /// cartoon spider); no head: three eyes round its body.
    pub fn sprawl() -> Self {
        let leg = |a: f32| LegPlan { root: Root::Chest, side: 0.3 * a.sin(), forward: 0.3 * a.cos(), upper: 1.4, lower: 1.7, knee: 0.0, knee_up: 1.0, knee_out: 0.7, spread: 1.6, radius: (0.18, 0.11) };
        let third = std::f32::consts::TAU / 3.0;
        let beast = Plan::beast();
        Plan {
            hips: (0.0, 1.3),
            chest: (0.0, 2.3),
            rump: 0.0,
            torso_radius: 0.6,
            torso_profile: [0.85, 1.1, 0.9],
            neck: (0.15, 0.0, 1.0),
            neck_radius: 0.12,
            head: (0.15, 0.12),
            legs: vec![leg(0.0), leg(third), leg(2.0 * third)],
            gaits: vec![
                // One leg after another, round the body.
                Gait { speed: 0.0, offsets: vec![0.0, 0.33, 0.67], swing: 0.3, stride: 1.6 },
                Gait { speed: 8.0, offsets: vec![0.0, 0.33, 0.67], swing: 0.38, stride: 2.8 },
            ],
            lift: 0.25,
            bob: 0.0,
            lean: 0.0,
            rock: 0.0,
            sway: 0.0,
            shoulder: 0.0,
            style: Style::Sprawl,
            turn: 0.0,
            radial: true,
            ..beast
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
            return (g[last].offsets.clone(), g[last].swing, g[last].stride * (speed / g[last].speed.max(0.1)).clamp(1.0, 1.15), fastest);
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
    /// How far the neck stretches out (0..1: up to 70% longer), the head
    /// reaching ahead of the body.
    pub stretch: f32,
    /// Dead still: no breath, no glances, no sway.
    pub still: bool,
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
    /// The body thrown back by a hit, chest more than hips (m), sprung
    /// back, and how fast.
    reel: Vec3,
    reel_v: Vec3,
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
    /// On stilts: the hips' height over the ground where it stands, as the
    /// stilts it stands on hold them (see `Style::Stilt`).
    stilt: f32,
    /// Dangling or gripping hands, sprung (where, how fast); the last
    /// frame's length.
    hands: Vec<(Vec3, Vec3)>,
    /// On stilts: its chest swaying on its hips (where, how fast); its head
    /// held in the world (where), and snapping to its new place.
    sway: Option<(Vec3, Vec3)>,
    head_at: Option<Vec3>,
    head_snap: bool,
    /// A beat after each foot lands, before the next step (s left): a
    /// creature that struggles to move gathers itself for each stride (the
    /// user; the stalker); and how much of its pace its body keeps, easing
    /// to the beat and out of it (see `STILT_GATHER`).
    beat: f32,
    gather: f32,
    /// How high the stalker's hips were last, standing on both stilts.
    dip: f32,
    dt: f32,
    /// Steps each foot has taken (to vary each one).
    steps: Vec<u32>,
    /// The porter: how far through its step each foot is (0..1); its
    /// cadence: the rest left before its next step, and the last step's rest
    /// and length (s).
    step_t: Vec<f32>,
    rest: f32,
    step_rest: f32,
    step_time: f32,
    /// A raised paw: which leg, and how far up (0..1).
    paw: (usize, f32),
    /// In a leap: seconds since it left the ground, and how long it will be
    /// in the air.
    air: (f32, f32),
    tilt: f32,
    stretch: f32,
    /// 1 moving as usual, 0 dead still.
    alive: f32,
    feet: Vec<Foot>,
    head_dir: Vec3,
    glance: Vec3,
    glance_in: f32,
    tail: Vec<Vec3>,
    time: f32,
    crouch: f32,
    seed: u32,
    /// Strides taken (each draws its own variations: see `vary`).
    strides: u32,
    /// Its legs lost (shot off): neither set down nor drawn, the body
    /// sagging at their corner.
    pub lost: Vec<bool>,
    /// Rearing (0..1, wanted; and as it is): the chest raised.
    pub rear: f32,
    reared: f32,
    /// Where its forefeet are driven into the ground (left, right), held
    /// there out of the gait: an attack's grab, the body hauled after them.
    pub grab: Option<[Vec3; 2]>,
    /// How long the porter's steps take (times `PORTER_STEP`'s): slower
    /// steps at the same pace are longer ones (the user: slower steps of
    /// greater reach; slowing its pace only made them shorter).
    pub step_scale: f32,
    /// The stalker's walk, live (`stilt_vault`, `stilt_creep`,
    /// `stilt_beat` at the console: how far it rises over the stilt it stands on,
    /// how much of its pace it keeps through its beat, how long its beat,
    /// times its own): the user, to tell the one from the other.
    pub stilt_tune: (f32, f32, f32),
    /// An arm holding something before it (the stalker's, its porter's
    /// chain): its upper arm hanging, its forearm level, square to it.
    pub holding: Option<usize>,
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
            reel: Vec3::ZERO,
            reel_v: Vec3::ZERO,
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
            stilt: 0.0,
            hands: Vec::new(),
            beat: 0.0,
            gather: 1.0,
            dip: 0.0,
            sway: None,
            head_at: None,
            head_snap: false,
            dt: 0.0,
            steps: Vec::new(),
            step_t: Vec::new(),
            rest: 0.0,
            step_rest: 0.2,
            step_time: 0.3,
            paw: (0, 0.0),
            air: (0.0, 1.0),
            tilt: 0.0,
            stretch: 0.0,
            alive: 1.0,
            head_dir: Vec3::new(heading.sin(), 0.0, heading.cos()),
            glance: Vec3::ZERO,
            glance_in: 1.0,
            time: 0.0,
            crouch: 0.0,
            seed,
            strides: 0,
            lost: Vec::new(),
            rear: 0.0,
            reared: 0.0,
            grab: None,
            step_scale: 1.0,
            stilt_tune: (STILT_VAULT, STILT_GATHER.0, STILT_BEAT_SCALE),
            holding: None,
            bones: Vec::new(),
        };
        rig.stilt = rig.plan.hips.1;
        rig.dip = rig.plan.hips.1;
        let (offsets, swing, stride, _) = rig.plan.gait(0.0);
        rig.local = offsets.clone();
        (rig.offsets, rig.swing, rig.stride) = (offsets, swing, stride);
        rig.steps = vec![0; rig.plan.legs.len()];
        rig.step_t = vec![0.0; rig.plan.legs.len()];
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
        // (Galloping, a bound three times as deep, the body lowest as each
        // pair of feet takes its weight: it glided at one height, on wheels,
        // the user.)
        // (Each stride its own depth, a half to half again: the same bound
        // every stride was a machine's, the user.)
        let bob = -self.plan.bob * moving * (1.0 + 2.0 * self.gallop) * (0.5 + self.vary(1)) * (1.0 - (self.bound() * std::f32::consts::TAU * 2.0).cos()) * 0.5;
        // Galloping: the body rocks (chest up as the hips go down) and the
        // spine flexes, the hips reaching forward under the body.
        let wave = (self.phase * std::f32::consts::TAU).sin();
        let rock = self.plan.rock * self.gallop * (0.6 + 0.8 * self.vary(2)) * wave * if root == Root::Chest { 1.0 } else { -1.0 };
        let flex = if root == Root::Hips { self.plan.rock * 2.5 * self.gallop * (self.phase * std::f32::consts::TAU + 1.6).sin() } else { 0.0 };
        // Walking, the spine swings from side to side, chest and hips in
        // opposition.
        let sway = self.plan.sway * moving * (1.0 - self.gallop) * wave * if root == Root::Chest { 1.0 } else { -1.0 };
        let breath = 0.012 * (self.time * 1.6).sin() * (1.0 - moving) * self.alive;
        // Crouched: lower, and the chest lower still (the head goes down).
        // Running, the body drops, so the legs can reach far fore and aft.
        let run = 0.3 * (Vec2::new(self.velocity.x, self.velocity.z).length() / 12.0).min(1.0);
        let low = (self.crouch * if root == Root::Chest { 0.42 } else { 0.3 }).max(run);
        // In a leap the body pitches with its arc: nose up rising, down
        // falling.
        let pitch = (self.airborne.unwrap_or(0.0) * 0.04).clamp(-0.25, 0.25) * if root == Root::Chest { 1.0 } else { -1.0 };
        // Hauling, the chest gathers low as the arms reach and heaves up as
        // they haul (the rump less).
        let heave = if self.plan.style == Style::Hauler { 0.15 * moving * (std::f32::consts::TAU * (self.phase - 0.7)).cos() * if root == Root::Chest { 1.0 } else { 0.4 } } else { 0.0 };
        // (On stilts, as high as the stilts it stands on hold it.)
        let h = if self.plan.style == Style::Stilt { self.stilt + if root == Root::Chest { self.plan.chest.1 - self.plan.hips.1 } else { 0.0 } } else { h };
        let height = h * (1.0 - low) + bob + rock + pitch + heave + if root == Root::Chest { breath } else { 0.0 };
        // A leg lost, its corner sags and slumps towards it; both of a pair
        // lost, that end drags on the ground.
        let (gone, slump) = self.plan.legs.iter().enumerate().filter(|&(i, leg)| leg.root == root && self.lost.get(i).copied().unwrap_or(false)).fold((0, 0.0), |(n, s), (_, leg)| (n + 1, s + leg.side.signum()));
        let height = (height - h * [0.0, 0.4, 0.75][gone.min(2)]).max(0.25) + self.reared * if root == Root::Chest { 0.6 } else { 0.1 };
        // Leaning into a turn: the body swings towards its inside.
        let speed = Vec2::new(self.velocity.x, self.velocity.z).length();
        let roll = (self.turn_rate * speed * self.plan.lean).clamp(-0.35, 0.35);
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let reel = self.reel * if root == Root::Chest { 1.0 } else { 0.4 };
        // Each stride lurches a little to one side or the other, the chest
        // more than the hips.
        let lurch = if self.plan.style == Style::Porter { 0.0 } else { (self.vary(4) - 0.5) * 0.2 * moving * if root == Root::Chest { 1.0 } else { 0.4 } };
        // (The porter's chest ahead of its hips, countering its load.)
        let tow = if self.plan.style == Style::Porter && root == Root::Chest { PORTER_TOW * (speed / 1.5).min(1.0) } else { 0.0 };
        self.root + forward * (f - mid + flex + tow) + Vec3::Y * height + right * (sway - roll * height + lurch + slump * 0.18) + reel
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
        // (Galloping, further: three quarters of the leg out before it,
        // as far behind as it leaves, the legs reaching, not under it.)
        let ahead = ((1.0 - self.swing) * self.stride * 0.5).min((leg.upper + leg.lower) * (0.6 + 0.15 * self.gallop)) * (speed / 1.5).min(1.0);
        // (Hauling, the forefeet set far out ahead, the body hauled up to
        // them and on past.)
        let ahead = if self.plan.style == Style::Hauler && leg.root == Root::Chest { (leg.upper + leg.lower) * 0.7 * (speed / 1.5).min(1.0) } else { ahead };
        // (The porter's, a little ahead of the hip: caught up, not striding.)
        let ahead = if self.plan.style == Style::Porter { PORTER_STEP.3 * (speed / 1.5).min(1.0) } else { ahead };
        let yaw = if leg.root == Root::Hips { self.hips_yaw } else { self.chest_yaw };
        let fwd = self.forward_of(yaw);
        let out = self.out_of(&leg, fwd) * leg.spread;
        let lead = Vec3::new(self.velocity.x, 0.0, self.velocity.z).normalize_or_zero() * ahead + out;
        // Each step lands a little off its ideal place.
        let k = self.steps.get(i).copied().unwrap_or(0) as i32;
        let r = |j: i32| hash01(self.seed as i32 + i as i32 * 101, k, j, 0x61b) - 0.5;
        let jitter = Vec3::new(r(0), 0.0, r(1)) * 0.12 * (self.velocity.length() / 2.0).min(1.0);
        let p = Vec3::new(hip.x, 0.0, hip.z) + Vec3::new(lead.x, 0.0, lead.z) + jitter;
        Vec3::new(p.x, ground(p.x, p.z), p.z)
    }

    /// One frame: move towards the intent, step, pose, and output bones.
    /// (`sweep`: where the body gets to, moved from where it is by a step:
    /// what is built stops it.)
    pub fn update(&mut self, dt: f32, intent: Intent, ground: &impl Fn(f32, f32) -> f32, sweep: &impl Fn(Vec3, Vec3) -> Vec3) {
        self.time += dt;
        self.dt = dt;
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
            let before = self.root;
            self.root = sweep(self.root, Vec3::new(self.velocity.x, *vy, self.velocity.z) * dt);
            // (Against a wall it drops, not flying on into it.)
            let got = (self.root - before) / dt.max(1e-4);
            self.velocity.x = got.x;
            self.velocity.z = got.z;
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
            // Faster or slower by its acceleration; turning by its grip, half
            // as much again, about 3 g at a gallop: it swings wide round a
            // corner and slams into the wall, not turning on the spot (the
            // user: slamming into walls before toy physics).
            let change = intent.velocity - self.velocity;
            let dir = self.velocity.normalize_or_zero();
            let along = dir * change.dot(dir);
            let rate = if change.dot(dir) > 0.0 || dir == Vec3::ZERO { self.plan.accel } else { self.plan.decel };
            self.velocity += along.clamp_length_max(rate * dt) + (change - along).clamp_length_max(rate * 1.5 * dt);
            self.velocity.y = 0.0;
            let face = if self.velocity.length() > 0.5 { self.velocity } else { intent.look - self.root };
            if Vec2::new(face.x, face.z).length() > 0.01 {
                let target = face.x.atan2(face.z);
                let turn = (target - self.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                self.heading += turn.clamp(-self.plan.turn * dt, self.plan.turn * dt);
            }
            let before = self.root;
            // (Gathering itself between steps, it all but holds where it is,
            // easing to it and out of it.)
            let creep = if self.beat > 0.0 { self.stilt_tune.1 } else { 1.0 };
            self.gather += (creep - self.gather) * (1.0 - (-STILT_GATHER.1 * dt).exp());
            let want = self.velocity * self.surge() * self.gather;
            self.root = sweep(self.root, (want + self.knock) * dt);
            self.root.y = ground(self.root.x, self.root.z);
            // (Stopped by a wall, it goes no faster than it gets: not running
            // on the spot against it.)
            let got = Vec2::new(self.root.x - before.x, self.root.z - before.z).length() / dt.max(1e-4);
            let wanted = Vec2::new(want.x, want.z).length();
            // (Short by more than a float can tell where it is: out at 1200
            // m, at 360 frames a second, its steps of millimetres rounded
            // short of what was asked a little, every frame, read as a
            // wall, and its speed bled away to nothing; on the user's
            // machine the stalker fell behind its porter, stepping small.)
            let rounding = self.root.length() * f32::EPSILON * 4.0;
            if got < wanted && wanted > 1e-3 && (wanted - got) * dt > rounding {
                let k = got / wanted;
                self.velocity.x *= k;
                self.velocity.z *= k;
            }
        }
        self.knock *= (-dt * 6.0).exp();
        self.reared += (self.rear - self.reared) * (1.0 - (-dt * 10.0).exp());
        // (Sprung, a little under damped: thrown, it overshoots once.)
        self.reel_v += (-self.reel * 160.0 - self.reel_v * 14.0) * dt;
        self.reel += self.reel_v * dt;
        self.turn_rate += ((self.heading - old_heading) / dt.max(1e-4) - self.turn_rate) * (1.0 - (-dt * 6.0).exp());
        // The chest leads a turn, the hips follow.
        let lead = (self.turn_rate * 0.15).clamp(-0.4, 0.4);
        let follow = |from: f32, to: f32, rate: f32| {
            let d = (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            from + d * (1.0 - (-dt * rate).exp())
        };
        // (On stilts, turning to haul each leg through: see `STILT_HAUL`.)
        let haul = if self.plan.style == Style::Porter && Vec2::new(self.velocity.x, self.velocity.z).length() > 0.3 {
            // (The porter's: turning on as a foot steps; wound back as one is
            // dragged, the further the more.)
            (0..self.plan.legs.len())
                .map(|i| {
                    let side = -self.plan.legs[i].side.signum();
                    match self.stepping(i) {
                        Some(s) => side * PORTER_HAUL.1 * (s * std::f32::consts::PI).sin(),
                        None => {
                            let hip = self.hip(i);
                            let off = Vec2::new(self.feet[i].planted.x - hip.x, self.feet[i].planted.z - hip.z).length();
                            -side * PORTER_HAUL.0 * ((off - PORTER_DRAG.0) / (PORTER_STEP.0.1 - PORTER_DRAG.0)).clamp(0.0, 1.0)
                        }
                    }
                })
                .sum::<f32>()
        } else if self.plan.style == Style::Stilt && Vec2::new(self.velocity.x, self.velocity.z).length() > 0.3 {
            self.plan
                .legs
                .iter()
                .enumerate()
                .map(|(i, leg)| {
                    let local = self.local.get(i).copied().unwrap_or(0.5);
                    let side = -leg.side.signum();
                    if local < self.swing {
                        side * STILT_HAUL.2 * (local / self.swing * std::f32::consts::PI).sin()
                    } else if local > 1.0 - STILT_HAUL.1 {
                        -side * STILT_HAUL.0 * ((local - (1.0 - STILT_HAUL.1)) / STILT_HAUL.1 * std::f32::consts::PI).sin()
                    } else {
                        0.0
                    }
                })
                .sum::<f32>()
        } else {
            0.0
        };
        self.chest_yaw = follow(self.chest_yaw, self.heading + lead + haul, 10.0);
        self.hips_yaw = follow(self.hips_yaw, self.heading - lead * 0.5, 5.0);

        // The gait clock runs with the speed; standing, it runs on only until
        // every foot is down.
        let speed = Vec2::new(self.velocity.x, self.velocity.z).length();
        let any_swinging = self.feet.iter().any(|f| f.swinging);
        let (mut offsets, swing, stride, gallop) = self.plan.gait(speed);
        // (Only a mammal gallops: its bound, rock and reaching swing.)
        let gallop = if self.plan.style == Style::Gallop { gallop } else { 0.0 };
        // Half lead with the other side.
        if offsets.len() == 4 && self.seed % 2 == 1 {
            offsets.swap(0, 1);
            offsets.swap(2, 3);
        }
        // Each foot a little early or late, differently each stride: the
        // footfalls never quite the same twice.
        for (i, o) in offsets.iter_mut().enumerate() {
            *o += (self.vary(10 + i as i32) - 0.5) * 0.1;
        }
        (self.offsets, self.swing, self.stride) = (offsets, swing, stride);
        self.gallop += (gallop - self.gallop) * (1.0 - (-dt * 4.0).exp());
        let held = self.beat > 0.0;
        self.beat = (self.beat - dt).max(0.0);
        if self.airborne.is_none() && (speed > 0.3 || any_swinging) && !held {
            // The tempo drifts a little.
            let drift = 1.0 + 0.09 * ((self.time * 0.9 + self.seed as f32).sin() * 0.6 + (self.time * 2.3).sin() * 0.4);
            // (Each stride a little quicker or slower, by up to 15%; the
            // stalker's by up to 10%, less erratic, the user.)
            let spread = if self.plan.style == Style::Stilt { 0.2 } else { 0.3 };
            let tempo = 1.0 - spread * 0.5 + spread * self.vary(0);
            let next = self.phase + speed.max(1.0) / self.stride * drift * tempo * dt;
            if next >= 1.0 {
                self.strides += 1;
            }
            self.phase = next.fract();
        }
        // The porter's next step, after its rest: the foot furthest behind.
        let porter_next = if self.plan.style == Style::Porter && !any_swinging && speed > 0.1 {
            self.rest -= dt;
            (self.rest <= 0.0).then(|| (0..self.feet.len()).max_by(|&a, &b| self.feet[a].planted.distance(self.home(a, ground)).total_cmp(&self.feet[b].planted.distance(self.home(b, ground))))).flatten()
        } else {
            None
        };
        for i in 0..self.feet.len() {
            let home = self.home(i, ground);
            let local = (self.phase - self.offsets[i]).rem_euclid(1.0);
            self.local[i] = local;
            if self.lost.get(i).copied().unwrap_or(false) {
                continue;
            }
            // Grabbing: a forefoot shoots out to where it is driven in, and
            // stays.
            if let Some(grab) = self.grab
                && self.plan.legs[i].root == Root::Chest
            {
                let foot = &mut self.feet[i];
                foot.planted = foot.planted.lerp(grab[(self.plan.legs[i].side > 0.0) as usize], 1.0 - (-dt * 30.0).exp());
                foot.swinging = false;
                continue;
            }
            if self.airborne.is_some() {
                continue;
            }
            // The porter's feet only catch up: one at a time, a foot steps
            // once the body has drawn it too far from its place, quick and
            // low, scuffing.
            if self.plan.style == Style::Porter {
                let other = self.feet.iter().enumerate().any(|(j, f)| j != i && f.swinging);
                let r = |j: i32| hash01(self.seed as i32 + i as i32 * 37, self.steps[i] as i32, j, 0x61f);
                let (reach, time) = (PORTER_STEP.0.0 + (PORTER_STEP.0.1 - PORTER_STEP.0.0) * r(0), (PORTER_STEP.1.0 + (PORTER_STEP.1.1 - PORTER_STEP.1.0) * r(1)) * self.step_scale);
                let foot = &mut self.feet[i];
                if foot.swinging {
                    // Swung as a dead weight: hanging back, then whipped on.
                    self.step_t[i] += dt / time;
                    let s = self.step_t[i].min(1.0);
                    let e = s * s;
                    foot.planted = foot.from.lerp(home, e) + Vec3::Y * PORTER_STEP.2 * (s * std::f32::consts::PI).sin();
                    if s >= 1.0 {
                        foot.swinging = false;
                        foot.planted = home;
                        // A rest before the next, longer the slower it goes.
                        let slow = (1.4 / speed.max(0.2)).clamp(0.6, 2.0);
                        self.step_rest = (PORTER_REST.0.0 + (PORTER_REST.0.1 - PORTER_REST.0.0) * r(2)) * slow;
                        self.rest = self.step_rest;
                        // (Its load's weight landing: the chest jolted down.)
                        if let Some((_, v)) = self.sway.as_mut() {
                            *v -= Vec3::Y * LOAD_JOLT;
                        }
                    }
                } else {
                    // Dragged: slid along the ground after the body, scuffing.
                    let off = foot.planted.distance(home);
                    if off > PORTER_DRAG.0 {
                        // (Slower than the body, at most half its pace: at a slow
                        // pace a foot slid fast enough to keep up, and never
                        // stepped; one leg did all the stepping.)
                        let want = foot.planted.lerp(home, (1.0 - (-dt * PORTER_DRAG.1).exp()) * ((off - PORTER_DRAG.0) / off));
                        let to = foot.planted + (want - foot.planted).clamp_length_max(0.5 * speed * dt);
                        foot.planted = Vec3::new(to.x, ground(to.x, to.z), to.z);
                    }
                    // Its turn after its rest (or, dragged too far, at once).
                    if porter_next == Some(i) || (!other && off > reach + 0.25) {
                        foot.swinging = true;
                        foot.from = foot.planted;
                        self.step_t[i] = 0.0;
                        self.steps[i] += 1;
                        self.step_time = time;
                    }
                }
                continue;
            }
            // (No step begun in a beat: its clock stopped, a foot just down
            // on the edge of its swing, the pace wavering, lifted and landed
            // again and again, each landing a new beat, the other foot's
            // turn never coming: one stilt stepping in place, the body
            // creeping off from the other, sinking to the ground, the user.)
            let in_swing = local < self.swing && (speed > 0.3 || self.feet[i].swinging) && (self.feet[i].swinging || !held);
            let foot = &mut self.feet[i];
            if in_swing {
                if !foot.swinging {
                    foot.swinging = true;
                    foot.from = foot.planted;
                    self.steps[i] += 1;
                }
                let s = local / self.swing;
                // Higher the faster: slow, the paw all but drags (a high
                // step at a prowl read as a small dog's trot, the user); at
                // a gallop it is tucked up high.
                let reach = self.plan.legs[i].upper + self.plan.legs[i].lower;
                // (On stilts, the knee folding the stilt up clear, as high at
                // any pace.)
                let pace = if self.plan.style == Style::Stilt { 1.0 } else { (speed / 5.0).clamp(0.3, 2.5) };
                let lift = (self.plan.lift * pace).min(reach * 0.5) * (0.75 + 0.5 * hash01(self.seed as i32, i as i32, self.steps[i] as i32, 0x61c));
                // The paw goes forward early and comes down reaching, not
                // an even arc; galloping, more so.
                let even = s * s * (3.0 - 2.0 * s);
                let across = even + (1.0 - (1.0 - s).powi(3) - even) * (0.5 + 0.5 * self.gallop);
                let arc = (s.powf(0.85 - 0.1 * self.gallop) * std::f32::consts::PI).sin() * lift;
                foot.planted = foot.from.lerp(home, across) + Vec3::Y * arc;
            } else if foot.swinging {
                foot.swinging = false;
                foot.planted = home;
                // (A struggling walker: a beat once the foot is down, each
                // its own length.)
                if self.plan.style == Style::Stilt && speed > 0.3 {
                    self.beat = (STILT_BEAT.0 + STILT_BEAT.1 * hash01(self.seed as i32, i as i32, self.steps[i] as i32, 0x61e)) * self.stilt_tune.2;
                }
            } else if speed <= 0.3 && foot.planted.distance(home) > self.stride * 0.3 {
                // Standing with a foot out of place: a settling step.
                foot.from = foot.planted;
                foot.swinging = true;
                self.phase = (self.offsets[i] + 0.01).fract();
            }
        }

        // On stilts: the hips as high as the stilts it stands on reach (the
        // lowest, standing on both), vaulting over each. A stilt lifting lets
        // go of it over the first of its swing (`STILT_LET_GO`): let go at
        // once, the body sprang up at every step, as with hiccups (the user).
        if self.plan.style == Style::Stilt {
            let fwd = self.forward_of(self.hips_yaw);
            let right = Vec3::new(fwd.z, 0.0, -fwd.x);
            // (Each stilt's reach: a lifting one's from where it lifted.)
            let reach = |i: usize| {
                let (leg, f) = (&self.plan.legs[i], &self.feet[i]);
                let hip = self.root + right * leg.side;
                let at = if f.swinging { f.from } else { f.planted };
                let d = Vec2::new(at.x - hip.x, at.z - hip.z).length();
                let reach = leg.upper + leg.lower;
                at.y - self.root.y + (reach * reach - d * d).max(0.0).sqrt()
            };
            let standing = (0..self.plan.legs.len()).filter(|&i| !self.feet[i].swinging).map(reach).fold(f32::MAX, f32::min);
            // (A lifting stilt holding it down eases up to the reach of the
            // one it stands on, over the first of its swing.)
            let lifting = (0..self.plan.legs.len()).filter_map(|i| self.stepping(i).map(|s| (i, s))).map(|(i, s)| {
                let (h, t) = (reach(i), (s / STILT_LET_GO).min(1.0));
                if standing == f32::MAX { f32::MAX } else { h + (standing - h).max(0.0) * t * t * (3.0 - 2.0 * t) }
            });
            let held = lifting.fold(standing, f32::min);
            // (Rising only so far over the stilt it stands on, from where it
            // sank between them, its knee bent: rising all the way, it rose
            // too much at each step, the user. Sinking less instead held it
            // higher than its trailing stilt could reach.)
            let top = (0..self.plan.legs.len()).filter(|&i| !self.feet[i].swinging).map(reach).fold(f32::MIN, f32::max);
            // (On both stilts it never rises, only sinks as they make it:
            // recomputing its height from both as a foot came down, it rose
            // a second time at each landing, the user. It rises only on one,
            // over it, from where it was as it lifted the other.)
            let held = if self.feet.iter().all(|f| !f.swinging) {
                let held = held.min(self.stilt);
                self.dip = held;
                held
            } else if top > f32::MIN {
                held.min(self.dip + (top - self.dip).max(0.0) * self.stilt_tune.0)
            } else {
                held
            };
            if held < f32::MAX {
                self.stilt += (held - self.stilt) * (1.0 - (-dt * 20.0).exp());
            }
        }

        // The neck stretches fast and draws back slowly; stillness sets in
        // slowly and breaks at once.
        let k = if intent.stretch > self.stretch { 12.0 } else { 3.0 };
        self.stretch += (intent.stretch - self.stretch) * (1.0 - (-dt * k).exp());
        self.alive = if intent.still { self.alive * (-dt * 2.5).exp() } else { 1.0 };
        // The head: towards the target, with lag; now and then a glance (not
        // when still).
        self.glance_in -= dt * self.alive;
        self.glance *= self.alive.max(0.02).powf(dt);
        if self.glance_in <= 0.0 {
            let r = |k: i32| hash01(self.seed as i32, (self.time * 10.0) as i32, k, 0x61a) - 0.5;
            self.glance = if r(0) > 0.1 { Vec3::new(r(1), r(2) * 0.4, r(3)) * 1.6 } else { Vec3::ZERO };
            self.glance_in = 0.6 + 2.2 * (r(4) + 0.5);
        }
        // The stalker's and the porter's chest sprung after where it would
        // be, leading into its motion, leaning away from a leg swinging
        // through and heaving on with it, the torso its length (it tilts,
        // not stretches).
        if self.sways() {
            let hips = self.anchor(Root::Hips);
            let mut target = self.anchor(Root::Chest);
            let fwd = self.forward_of(self.hips_yaw);
            let right = Vec3::new(fwd.z, 0.0, -fwd.x);
            let moving = (Vec2::new(self.velocity.x, self.velocity.z).length() / 1.0).min(1.0);
            let porter = self.plan.style == Style::Porter;
            let (sway, heave) = if porter { (PORTER_SWAY, PORTER_HEAVE) } else { (STILT_SWAY, STILT_HEAVE) };
            target += fwd * heave.0 * moving;
            for (i, leg) in self.plan.legs.iter().enumerate() {
                if let Some(s) = self.stepping(i) {
                    let lift = (s * std::f32::consts::PI).sin();
                    target -= right * leg.side.signum() * sway.2 * lift;
                    target += fwd * heave.1 * lift;
                }
            }
            let length = (target - hips).length();
            let (mut p, mut v) = self.sway.unwrap_or((target, Vec3::ZERO));
            let step = dt.min(0.05);
            v += ((target - p) * sway.0 - v * sway.1) * step;
            p += v * step;
            p = hips + (p - hips).normalize_or(Vec3::Y) * length;
            self.sway = Some((p, v));
        }
        let chest = self.chest_now();
        let want = ((intent.look - chest).normalize_or(self.forward_of(self.chest_yaw)) + self.glance * 0.35).normalize_or(Vec3::Z);
        // (The porter's head low, watching its way.)
        let want = if self.plan.style == Style::Porter { (self.forward_of(self.chest_yaw) * 0.85 - Vec3::Y * 0.5).normalize() } else { want };
        // In a leap the head is thrown at its prey.
        let rate = if self.airborne.is_some() { 25.0 } else if intent.head_rate > 0.0 { intent.head_rate } else if self.plan.style == Style::Porter { PORTER_LOLL } else if self.plan.style == Style::Stilt { 12.0 } else { 7.0 };
        self.head_dir = self.head_dir.lerp(want, 1.0 - (-dt * rate).exp()).normalize_or(want);
        self.tilt += (intent.tilt - self.tilt) * (1.0 - (-dt * 3.0).exp());
        // The head's height follows the body's slowly: steady while it bobs.
        // (Running, it follows the body, and pumps deeper still as the
        // forefeet land: steadied, it ran as on rails.)
        let moving = (self.velocity.length() / 3.0).min(1.0);
        let pump = 0.1 * self.gallop * moving * (0.4 + 1.2 * self.vary(3)) * (1.0 - (self.bound() * std::f32::consts::TAU * 2.0).cos()) * 0.5;
        let raw = self.neck_end().y - pump;
        let rate = 4.0 + 26.0 * self.gallop;
        self.head_y = Some(match self.head_y {
            Some(y) => raw + (y + (raw - y) * (1.0 - (-dt * rate).exp()) - raw).clamp(-0.15, 0.15),
            None => raw,
        });
        // The stalker's and the porter's head held where it is in the world,
        // the body moving under it, till the neck has been carried too far;
        // then it catches up, and holds again.
        if self.sways() {
            let natural = self.neck_end();
            let at = self.head_at.get_or_insert(natural);
            let hold = if self.plan.style == Style::Porter { PORTER_HOLD } else { HEAD_HOLD };
            if !self.head_snap && at.distance(natural) > hold.0 {
                self.head_snap = true;
            }
            if self.head_snap {
                *at = at.lerp(natural, 1.0 - (-dt * hold.1).exp());
                if at.distance(natural) < 0.02 {
                    self.head_snap = false;
                }
            }
        }

        // The tail: each link follows the last at its length, sagging, and
        // raised in a crouch.
        if !self.tail.is_empty() {
            let hips = self.anchor(Root::Hips);
            let back = -self.forward_of(self.hips_yaw);
            self.tail[0] = hips + back * 0.25;
            let len = self.plan.tail.1;
            for k in 1..self.tail.len() {
                let prev = self.tail[k - 1];
                let sway = Vec3::new(self.forward_of(self.hips_yaw).z, 0.0, -self.forward_of(self.hips_yaw).x) * 0.02 * (self.time * 2.5 + k as f32 * 0.6).sin() * self.alive;
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
        let chest = self.chest_now();
        let forward = self.forward_of(self.chest_yaw);
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        // Torso: a few segments from the rump (behind the hips, rising a
        // little) to the chest.
        let rump = hips - self.forward_of(self.hips_yaw) * self.plan.rump + Vec3::Y * self.plan.rump * 0.35;
        let n = 3;
        for k in 0..n {
            let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            let swell = self.plan.torso_profile[k.min(2)];
            bones.push(Bone { a: rump.lerp(chest, t0), b: rump.lerp(chest, t1), radius: self.plan.torso_radius * swell, side: right });
        }
        // Neck and head, along where it looks (the neck goes partway); the
        // head at its steadied height.
        let mut neck_end = self.neck_end();
        if let Some(y) = self.head_y {
            neck_end.y = y;
        }
        // (On stilts, where it holds it.)
        if let (true, Some(at)) = (self.sways(), self.head_at) {
            neck_end = at;
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
                // Not as one, a pair mirrored (held all the way down a
                // flight, the user): the side it leads with first, the other
                // a beat behind and reaching less.
                Some(_) => {
                    let leads = i % 2 == (self.seed % 2) as usize;
                    let (lag, short) = if leads { (0.0, 1.0) } else { (0.2, 0.8) };
                    let s = (self.air.0 / self.air.1.max(0.05) - lag).clamp(0.0, 1.0);
                    let out = self.out_of(&leg, fwd);
                    if leg.root == Root::Chest {
                        hip + fwd * reach * 0.85 * short + out * reach * 0.35 - Vec3::Y * reach * (0.15 + 0.45 * s)
                    } else {
                        hip - fwd * reach * (0.85 * (1.0 - s) * short - 0.2 * s) - Vec3::Y * reach * (0.5 + 0.25 * s)
                    }
                }
                None if self.paw.0 == i && self.paw.1 > 0.0 && !self.feet[i].swinging => {
                    // Poised: lifted and drawn up under the chest.
                    let k = self.paw.1 * self.paw.1 * (3.0 - 2.0 * self.paw.1);
                    self.feet[i].planted + Vec3::Y * reach * 0.38 * k - fwd * reach * 0.12 * k
                }
                None => self.feet[i].planted,
            };
            let out = self.out_of(&leg, fwd);
            let (knee, end) = ik(hip, target, leg.upper, leg.lower, fwd * leg.knee + Vec3::Y * leg.knee_up + out * leg.knee_out);
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
            // (On stilts, straight down from the shoulder: dead, swung only
            // by the body, its springs below.)
            let hang = if self.plan.style == Style::Stilt { shoulder - Vec3::Y * reach * 0.97 } else { hang };
            // (The porter's too: gripping its load over its shoulders, its
            // arms flailed, the user.)
            let hang = if self.plan.style == Style::Porter { shoulder - Vec3::Y * reach * 0.97 } else { hang };
            // (Holding something: the forearm level before it, square to the
            // upper arm hanging; holding, not held, the user: drawn out, the
            // arm read as chained itself.)
            let holds = self.holding == Some(k);
            let hang = if holds { shoulder - Vec3::Y * arm.upper + forward * arm.lower * 0.98 } else { hang };
            let lunge = shoulder + self.head_dir * reach * 0.95;
            let hand = hang.lerp(lunge, self.crouch);
            // (Dangling, the stalker's and the porter's: sprung, weights.)
            let hand = if matches!(self.plan.style, Style::Porter | Style::Stilt) {
                if self.hands.len() <= k {
                    self.hands.push((hand, Vec3::ZERO));
                }
                let (stiff, damp) = STILT_HANDS;
                let (p, v) = &mut self.hands[k];
                let dt = self.dt.min(0.05);
                *v += ((hand - *p) * stiff - *v * damp) * dt;
                *p += *v * dt;
                *p = shoulder + (*p - shoulder).clamp_length_max(reach * 0.99);
                *p
            } else {
                hand
            };
            let (elbow, end) = ik(shoulder, hand, arm.upper, arm.lower, if holds { -forward - Vec3::Y * 0.5 } else { -forward + Vec3::Y * 0.2 });
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
        let chest = self.chest_now();
        let forward = self.forward_of(self.chest_yaw);
        let (neck_len, neck_fwd, neck_up) = self.plan.neck;
        let base = Vec3::new(neck_fwd, neck_up, 0.0).normalize_or(Vec3::Y);
        let neck_dir = (forward * base.x + Vec3::Y * base.y).lerp(self.head_dir, 0.4).normalize_or(Vec3::Y);
        let neck_dir = (neck_dir - Vec3::Y * 0.5 * self.crouch).normalize_or(forward);
        // Stretched, the neck reaches along where it looks.
        let neck_dir = neck_dir.lerp(self.head_dir, self.stretch * 0.8).normalize_or(forward);
        chest + neck_dir * neck_len * (1.0 + 0.7 * self.stretch)
    }

    /// The head's tip and the direction it faces (for eyes).
    pub fn head(&self) -> (Vec3, Vec3) {
        // (Three torso segments, the neck, then the head.)
        let head = self.bones[4];
        (head.b, (head.b - head.a).normalize_or(Vec3::Z))
    }

    /// Which way a leg is set out from the body: to its side; a tripod's,
    /// the way it stands from the body's middle.
    fn out_of(&self, leg: &LegPlan, fwd: Vec3) -> Vec3 {
        let right = Vec3::new(fwd.z, 0.0, -fwd.x);
        if self.plan.radial {
            (right * leg.side + fwd * leg.forward).normalize_or(right)
        } else {
            right * leg.side.signum()
        }
    }

    /// The middle of the chest.
    pub fn chest(&self) -> Vec3 {
        self.chest_now()
    }

    /// The porter's back, as it stands now: its middle, and which way is
    /// up along it, out behind it, and across it.
    fn back(&self) -> (Vec3, Vec3, Vec3, Vec3) {
        let (hips, chest) = (self.anchor(Root::Hips), self.chest_now());
        let up = (chest - hips).normalize_or(Vec3::Y);
        let fwd = self.forward_of(self.chest_yaw);
        let ahead = (fwd - up * fwd.dot(up)).normalize_or(fwd);
        (hips.lerp(chest, 0.55), up, -ahead, up.cross(ahead))
    }

    /// The porter's load: where (against its back), turned how (lying
    /// along it), and how big; fixed to it.
    pub fn load(&self) -> Option<(Vec3, Quat, Vec3)> {
        if self.plan.style != Style::Porter {
            return None;
        }
        let (middle, up, behind, across) = self.back();
        Some((middle + behind * (self.plan.torso_radius + LOAD_SIZE.z * 0.5), Quat::from_mat3(&Mat3::from_cols(across, up, -behind)), LOAD_SIZE))
    }

    /// The chest as it is: swaying on its hips (the stalker's, the
    /// porter's); else as posed.
    fn chest_now(&self) -> Vec3 {
        match self.sway {
            Some((p, _)) if self.sways() => p,
            _ => self.anchor(Root::Chest),
        }
    }

    /// Whether its body moves as a creature's (the stalker's, the
    /// porter's: a sprung chest, effort in each step, a torso that hauls
    /// each leg through, a head held in the world): see `Style::Stilt`.
    fn sways(&self) -> bool {
        matches!(self.plan.style, Style::Stilt | Style::Porter)
    }

    /// How far through its step a leg is (0..1), swinging; the porter's
    /// its own, the others' from the gait.
    fn stepping(&self, i: usize) -> Option<f32> {
        if !self.feet[i].swinging {
            return None;
        }
        Some(if self.plan.style == Style::Porter { self.step_t[i].min(1.0) } else { (self.local[i] / self.swing.max(1e-3)).clamp(0.0, 1.0) })
    }

    /// Which leg a bone is of (the bones: three of torso, the neck, the
    /// head, then two to a leg), if any.
    pub fn leg_of(&self, bone: usize) -> Option<usize> {
        let k = bone.checked_sub(5)? / 2;
        (k < self.plan.legs.len()).then_some(k)
    }

    /// Whether leg `i` is lost.
    pub fn is_lost(&self, i: usize) -> bool {
        self.lost.get(i).copied().unwrap_or(false)
    }

    /// This stride's variation `j` (0..1), easing into the next stride's
    /// over the cycle (so nothing jumps as one stride gives way to the
    /// next): no two strides quite alike.
    fn vary(&self, j: i32) -> f32 {
        let at = |k: u32| hash01(self.seed as i32, k as i32, j, 0x61d);
        let s = self.phase;
        let (a, b) = (at(self.strides), at(self.strides + 1));
        a + (b - a) * s * s * (3.0 - 2.0 * s)
    }

    /// How much of its speed the body moves at this point in the stride:
    /// hauling, almost still as the arms reach (a sixth), surging as they
    /// haul (nearly twice), its speed on the whole; else all of it.
    fn surge(&self) -> f32 {
        // (The porter: its body surging with each step, all but still as it
        // rests; the same pace overall.)
        if self.plan.style == Style::Porter {
            // (A push shaped as a half sine over the step, from its creep: as
            // far over a step and its rest as at its pace throughout.)
            let s = (0..self.feet.len()).find(|&i| self.feet[i].swinging).map(|i| self.step_t[i].min(1.0));
            let creep = PORTER_REST.1;
            return match s {
                Some(s) => creep + std::f32::consts::FRAC_PI_2 * (1.0 - creep) * (self.step_time + self.step_rest) / self.step_time.max(0.05) * (s * std::f32::consts::PI).sin(),
                None => creep,
            };
        }
        if self.plan.style != Style::Hauler || self.grab.is_some() {
            return 1.0;
        }
        let moving = (Vec2::new(self.velocity.x, self.velocity.z).length() / 3.0).min(1.0);
        1.0 + 0.85 * moving * (std::f32::consts::TAU * (self.phase - 0.7)).cos()
    }

    /// The gait's clock for the body's rise and fall: galloping, shifted so
    /// it is lowest as the fore and the hind pairs bear its weight.
    fn bound(&self) -> f32 {
        self.phase + 0.3 * self.gallop
    }

    /// How fast it is moving over the ground.
    pub fn speed(&self) -> f32 {
        Vec2::new(self.velocity.x, self.velocity.z).length()
    }

    /// A hit throws the body back along `kick` (its length up to 1, a
    /// whole blast), the chest rearing a little.
    pub fn reel(&mut self, kick: Vec3) {
        self.reel_v += (kick + Vec3::Y * kick.length() * 0.4) * 6.0;
    }

    /// Leap towards `dir` (horizontal) with this speed and upward speed.
    pub fn leap(&mut self, dir: Vec3, speed: f32, up: f32) {
        self.velocity = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::Z) * speed;
        self.airborne = Some(up);
        self.air = (0.0, 2.0 * up / 25.0);
        self.root.y += 0.05;
    }
}







#[cfg(test)]
mod stalker {
    use super::*;

    /// Walking on flat ground, the stalker's planted stilts stay on the
    /// ground and within their reach, its hips high (held higher than its
    /// trailing stilt could reach, it fell into the splits, the user).
    #[test]
    fn stilts_hold() {
        for (seed, pace) in [(1, 1.0), (3, 1.0), (7, 1.0), (11, 1.6), (13, 1.6)] {
            let mut rig = Rig::new(Plan::stalker(), Vec3::ZERO, 0.0, seed, &|_, _| 0.0);
            for k in 0..1000 {
                let intent = Intent { velocity: Vec3::Z * pace, look: rig.root + Vec3::new(0.0, 1.5, 5.0), ..Default::default() };
                rig.update(1.0 / 100.0, intent, &|_, _| 0.0, &|from, step| from + step);
                for i in 0..rig.plan.legs.len() {
                    if rig.feet[i].swinging {
                        continue;
                    }
                    let (leg, foot) = (rig.plan.legs[i], rig.feet[i].planted);
                    assert!(foot.y.abs() < 1e-3, "seed {seed}: a planted foot {} m off the ground", foot.y);
                    let reach = rig.bones[5 + 2 * i].a.distance(foot) / (leg.upper + leg.lower);
                    assert!(reach < 1.01, "seed {seed}: a stilt at {reach} of its reach");
                }
                let hips = rig.anchor(Root::Hips).y;
                assert!(k < 300 || hips > 1.1, "seed {seed}, at {pace} m/s: its hips sank to {hips} m");
            }
        }
    }

    /// Circling, its pace wavering a little, the stalker steps with each
    /// stilt in turn (its clock stopped in a beat, a foot on the edge of its
    /// swing lifted and landed again and again, the other's turn never
    /// coming, and it sank to the ground: the user, with a log).
    #[test]
    fn stilts_take_turns() {
        for seed in 1..20u32 {
            let mut rig = Rig::new(Plan::stalker(), Vec3::new(6.0, 0.0, 0.0), 0.0, seed, &|_, _| 0.0);
            let (mut last, mut run) = (usize::MAX, 0);
            let mut was = [false, false];
            for k in 0..3000 {
                let out = Vec3::new(rig.root.x, 0.0, rig.root.z).normalize_or(Vec3::X);
                let around = Vec3::new(-out.z, 0.0, out.x);
                let wobble = 1.0 + 0.04 * (hash01(seed as i32, k, 0, 0x777) - 0.5);
                let intent = Intent { velocity: (around + out * ((6.0 - rig.root.length()) / 3.0).clamp(-1.0, 1.0)).normalize() * wobble, look: Vec3::Y * 1.5, ..Default::default() };
                rig.update(1.0 / 100.0, intent, &|_, _| 0.0, &|from, step| from + step);
                for i in 0..2 {
                    let s = rig.feet[i].swinging;
                    if s && !was[i] {
                        run = if i == last { run + 1 } else { 1 };
                        last = i;
                        assert!(run <= 2, "seed {seed}: one stilt stepped {run} times running");
                    }
                    was[i] = s;
                }
            }
        }
    }

    /// Out in the world (1200 m from its middle), at a small time step (360
    /// frames a second, or slowed time), the stalker goes as fast as it is
    /// asked (its steps rounded short read as a wall, its speed bled away to
    /// nothing, on the user's machine).
    #[test]
    fn keeps_its_pace_far_out() {
        let mut rig = Rig::new(Plan::stalker(), Vec3::new(1200.0, 0.0, 900.0), std::f32::consts::FRAC_PI_2, 3, &|_, _| 0.0);
        let start = rig.root;
        for _ in 0..(360 * 20) {
            let intent = Intent { velocity: Vec3::X * 1.6, look: rig.root + Vec3::new(5.0, 1.5, 0.0), ..Default::default() };
            rig.update(1.0 / 360.0, intent, &|_, _| 0.0, &|from, step| from + step);
        }
        let pace = (rig.root - start).length() / 20.0;
        assert!(pace > 1.0, "it went {pace} m/s, asked 1.6");
    }
}







