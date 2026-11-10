//! Flight (the user, 2026-11-09: the impaled brute is too heavy to walk and
//! is held up by some propulsion; a chance to try how flight works, for other
//! creatures and for craft that are not creatures). A body held up only by
//! thrust: its mass and gravity always, a thrust that takes time to build and
//! to fall and is barely more than its weight, drag; it tilts to move. The
//! heaviness is not acted, it comes out of that: it climbs slowly and falls
//! fast, overshoots and corrects, sinks in a hard turn, lurches when its
//! thrust stutters (`Flight::cut`).
//!
//! A `Pilot` flies it: given a goal (where to be, how fast, which way to
//! face), the thrust and the turn to ask for. The same pilot with other
//! numbers flies a heavy creature, a light one or a craft.
//!
//! Integrated in fixed steps (`STEP`), so it flies the same at any frame rate.

use bevy::math::{Mat3, Quat, Vec3};

/// Gravity (m/s²).
pub const G: f32 = 9.81;
/// The fixed step (s).
pub const STEP: f32 = 1.0 / 120.0;

/// What a flying body is.
#[derive(Clone, Copy, Debug)]
pub struct Craft {
    /// Its mass (kg).
    pub mass: f32,
    /// Its thrust at full, over its weight (1: it can only just hold itself
    /// up).
    pub lift: f32,
    /// How long its thrust takes to follow a change (s, the time constant).
    pub spool: f32,
    /// Air drag on it: linear (1/s) and quadratic (1/m), as accelerations.
    pub drag: f32,
    pub drag2: f32,
    /// How fast it can start turning its body (rad/s²), and how its turning
    /// dies away by itself (1/s).
    pub turn: f32,
    pub spin_drag: f32,
    /// How far from upright its pilot lets it tilt (rad).
    pub tilt: f32,
    /// The climb (m/s) at which its full thrust only holds its weight: its
    /// thrust falls off the faster it climbs, as a rotor's does (power, not
    /// thrust, limits a climb); falling, it has it all.
    pub climb: f32,
}

impl Craft {
    /// The impaled brute: 600 kg, half as much thrust again as its weight,
    /// slow to spool and slow to tilt.
    pub const BRUTE: Craft = Craft { mass: 600.0, lift: 1.45, spool: 0.35, drag: 0.15, drag2: 0.01, turn: 4.0, spin_drag: 1.0, tilt: 0.35, climb: 2.5 };

    /// Its full thrust (N), climbing at `up` (m/s).
    pub fn full(&self, up: f32) -> f32 {
        let fade = (1.0 - 1.0 / self.lift) / self.climb;
        self.mass * G * self.lift * (1.0 - fade * up.clamp(0.0, self.climb))
    }
}

/// What the pilot asks for: a thrust (N), and how to turn (an angular
/// acceleration, rad/s², in the world's frame).
#[derive(Clone, Copy, Debug, Default)]
pub struct Order {
    pub thrust: f32,
    pub turn: Vec3,
}

/// A flying body as it is.
#[derive(Clone, Copy, Debug)]
pub struct Flight {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Its turn: its +Y is up through its thrust, +Z its front.
    pub rot: Quat,
    /// How fast it turns (rad/s, world).
    pub spin: Vec3,
    /// Its thrust now (N).
    pub thrust: f32,
    /// The share of its thrust it can give (1 whole; less while hit or
    /// failing).
    pub cut: f32,
    /// Time not yet stepped (s).
    owed: f32,
    /// Where it was and how it was turned a step ago, to draw it between
    /// steps (`drawn`).
    before: (Vec3, Quat),
}

impl Flight {
    /// At rest at `pos`, upright, facing `facing`, its thrust holding it up.
    pub fn hovering(craft: &Craft, pos: Vec3, facing: Vec3) -> Flight {
        let rot = upright(Vec3::Y, facing);
        Flight { pos, vel: Vec3::ZERO, rot, spin: Vec3::ZERO, thrust: craft.mass * G, cut: 1.0, owed: 0.0, before: (pos, rot) }
    }

    /// Its up: the way its thrust pushes.
    pub fn up(&self) -> Vec3 {
        self.rot * Vec3::Y
    }

    /// One step of `dt` under `order`.
    pub fn step(&mut self, craft: &Craft, order: Order, dt: f32) {
        let wanted = order.thrust.clamp(0.0, craft.full(self.vel.y) * self.cut.clamp(0.0, 1.0));
        self.thrust += (wanted - self.thrust) * (1.0 - (-dt / craft.spool).exp());
        let a = self.up() * (self.thrust / craft.mass) - Vec3::Y * G - self.vel * craft.drag - self.vel * self.vel.length() * craft.drag2;
        self.vel += a * dt;
        self.pos += self.vel * dt;
        let turn = order.turn.clamp_length_max(craft.turn) - self.spin * craft.spin_drag;
        self.spin += turn * dt;
        self.rot = (Quat::from_scaled_axis(self.spin * dt) * self.rot).normalize();
    }

    /// `dt` of flight under `pilot` towards `goal`, in fixed steps (what is
    /// left over carried to the next call).
    pub fn advance(&mut self, craft: &Craft, pilot: &Pilot, goal: &Goal, dt: f32) {
        self.owed += dt;
        while self.owed >= STEP {
            self.owed -= STEP;
            self.before = (self.pos, self.rot);
            let order = pilot.order(craft, self, goal);
            self.step(craft, order, STEP);
        }
    }

    /// Where to draw it now: between its last two steps by the time owed
    /// (so it moves smoothly at any frame rate).
    pub fn drawn(&self) -> (Vec3, Quat) {
        let k = (self.owed / STEP).clamp(0.0, 1.0);
        (self.before.0.lerp(self.pos, k), self.before.1.slerp(self.rot, k))
    }

    /// Kept above `floor` (its centre's lowest): set down on it, its fall
    /// stopped (what it does when it cannot hold itself up).
    pub fn rest_on(&mut self, floor: f32) {
        if self.pos.y < floor {
            self.pos.y = floor;
            self.vel.y = self.vel.y.max(0.0);
            self.before.0.y = self.before.0.y.max(floor);
        }
    }
}

/// Where a pilot is to take it: a place, how fast to be going there (zero:
/// to stop there), and which way to face (horizontal).
#[derive(Clone, Copy, Debug)]
pub struct Goal {
    pub at: Vec3,
    pub vel: Vec3,
    pub facing: Vec3,
}

/// How a pilot flies: towards its goal at a speed it can still stop from
/// (braking with `brake` of what it has, the rest a margin for its own
/// slowness to tilt and spool), the most `speed` (m/s), and near it `reach`
/// times the distance (1/s); its speed closing on that by `grip` (1/s); the
/// most it accelerates sideways (m/s²); how quickly it turns its body to
/// what it wants (rad/s). (The approach must be well slower than its body
/// tilts, or it outruns itself and swings about the goal.)
#[derive(Clone, Copy, Debug)]
pub struct Pilot {
    pub speed: f32,
    pub brake: f32,
    /// How long it takes to start braking (s): its tilt swung over, its
    /// thrust changed; the distance covered meanwhile counted in.
    pub lag: f32,
    pub reach: f32,
    pub grip: f32,
    pub accel: f32,
    pub attitude: f32,
}

impl Pilot {
    /// The brute's: slow, deliberate.
    pub const BRUTE: Pilot = Pilot { speed: 7.0, brake: 0.5, lag: 0.8, reach: 0.45, grip: 1.2, accel: 3.0, attitude: 2.5 };

    /// What to ask of `craft` to take `f` towards `goal`.
    pub fn order(&self, craft: &Craft, f: &Flight, goal: &Goal) -> Order {
        // Up no more than its spare thrust; down by letting go, gravity.
        let spare = G * (craft.lift * f.cut - 1.0).max(0.0);
        // The speed to be going: towards the goal no faster than it could
        // stop from (sideways and up and down apart, each by what stops it:
        // its sideways acceleration; a descent its spare thrust, a climb
        // gravity), and the goal's own.
        let e = goal.at - f.pos;
        // (The speed v it can stop from within d: v·lag + v²/2ab = d.)
        let toward = |d: f32, stop: f32| {
            let ab = (stop * self.brake).max(1e-3);
            let v = ab * ((self.lag * self.lag + 2.0 * d.abs() / ab).sqrt() - self.lag);
            d.signum() * v.min(self.speed).min(self.reach * d.abs())
        };
        let eh = Vec3::new(e.x, 0.0, e.z);
        let vh = eh.normalize_or_zero() * toward(eh.length(), self.accel).abs();
        let vy = toward(e.y, if e.y < 0.0 { spare } else { G });
        let v = vh + Vec3::Y * vy + goal.vel;
        let mut a = (v - f.vel) * self.grip;
        let side = Vec3::new(a.x, 0.0, a.z).clamp_length_max(self.accel);
        a = side + Vec3::Y * a.y.clamp(-G * 0.8, spare);
        // The force for that, its drag met.
        let need = (a + Vec3::Y * G + f.vel * craft.drag + f.vel * f.vel.length() * craft.drag2) * craft.mass;
        // Its up turned towards the force, within its tilt.
        let mut dir = need.normalize_or(Vec3::Y);
        let angle = dir.angle_between(Vec3::Y);
        if angle > craft.tilt {
            let axis = Vec3::Y.cross(dir).normalize_or(Vec3::X);
            dir = Quat::from_axis_angle(axis, craft.tilt) * Vec3::Y;
        }
        // Only what its up gives now: tilted wrong, it does not push harder.
        let thrust = need.dot(f.up()).max(0.0);
        // Its body turned to `dir`, facing the goal's way.
        let want = upright(dir, goal.facing);
        let mut q = want * f.rot.inverse();
        if q.w < 0.0 {
            q = -q;
        }
        let wa = self.attitude;
        let turn = q.to_scaled_axis() * (wa * wa) - f.spin * (2.0 * wa) + f.spin * craft.spin_drag;
        Order { thrust, turn }
    }
}

/// The turn whose +Y is `up` and whose +Z is `facing` as near as it can be
/// (across `up`).
pub fn upright(up: Vec3, facing: Vec3) -> Quat {
    let y = up.normalize_or(Vec3::Y);
    let mut z = facing - y * facing.dot(y);
    if z.length_squared() < 1e-8 {
        z = y.any_orthonormal_vector();
    }
    let z = z.normalize();
    let x = y.cross(z);
    Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: Craft = Craft::BRUTE;
    const P: Pilot = Pilot::BRUTE;

    fn goal(at: Vec3) -> Goal {
        Goal { at, vel: Vec3::ZERO, facing: Vec3::Z }
    }

    /// Flown `secs` towards `goal`, each step recorded.
    fn fly(f: &mut Flight, g: &Goal, secs: f32) -> Vec<Flight> {
        let mut out = Vec::new();
        for _ in 0..(secs / STEP).round() as usize {
            let order = P.order(&C, f, g);
            f.step(&C, order, STEP);
            out.push(*f);
        }
        out
    }

    #[test]
    fn holds_a_hover() {
        let at = Vec3::new(0.0, 2.0, 0.0);
        let mut f = Flight::hovering(&C, at, Vec3::Z);
        let run = fly(&mut f, &goal(at), 20.0);
        let worst = run.iter().map(|s| s.pos.distance(at)).fold(0.0, f32::max);
        assert!(worst < 0.01, "drifted {worst} m");
        assert!((f.thrust / (C.mass * G) - 1.0).abs() < 0.01, "thrust {} of its weight", f.thrust / (C.mass * G));
    }

    /// Thrust cut for a moment: it falls, and its slow thrust lets it fall
    /// further before it catches itself; then it climbs back.
    #[test]
    fn falls_when_its_thrust_is_cut() {
        let at = Vec3::new(0.0, 20.0, 0.0);
        let mut f = Flight::hovering(&C, at, Vec3::Z);
        f.cut = 0.0;
        fly(&mut f, &goal(at), 0.6);
        let at_cut = f.pos.y;
        f.cut = 1.0;
        let run = fly(&mut f, &goal(at), 15.0);
        let lowest = run.iter().map(|s| s.pos.y).fold(f32::MAX, f32::min);
        eprintln!("cut 0.6 s: fell {:.2} m while cut, lowest {:.2} m below (caught after), back within 5 cm at {:.1} s", at.y - at_cut, at.y - lowest, run.iter().rposition(|s| (s.pos.y - at.y).abs() > 0.05).unwrap_or(0) as f32 * STEP);
        assert!(at.y - at_cut > 0.6, "barely fell: {}", at.y - at_cut);
        assert!(lowest < at_cut - 0.5, "caught itself at once: the thrust is not slow");
        assert!((f.pos.y - at.y).abs() < 0.05, "not back: {}", f.pos.y);
    }

    /// Moving 20 m sideways: it tilts within its limit, gets there, sags a
    /// little as it tilts (heavy), overshoots a little and settles.
    #[test]
    fn moves_like_something_heavy() {
        let start = Vec3::new(0.0, 5.0, 0.0);
        let to = Vec3::new(20.0, 5.0, 0.0);
        let mut f = Flight::hovering(&C, start, Vec3::Z);
        let run = fly(&mut f, &goal(to), 25.0);
        let tilt = run.iter().map(|s| s.up().angle_between(Vec3::Y)).fold(0.0, f32::max);
        let over = run.iter().map(|s| s.pos.x - to.x).fold(f32::MIN, f32::max);
        let sag = run.iter().map(|s| start.y - s.pos.y).fold(0.0, f32::max);
        let settled = run.iter().rposition(|s| s.pos.distance(to) > 0.2).unwrap_or(0) as f32 * STEP;
        let fastest = run.iter().map(|s| s.vel.length()).fold(0.0, f32::max);
        eprintln!("20 m sideways: tilt {:.0}°, top speed {fastest:.1} m/s, overshoot {over:.2} m, sag {sag:.2} m, within 20 cm from {settled:.1} s", tilt.to_degrees());
        assert!(tilt <= C.tilt + 0.05, "tilted {tilt}");
        assert!(over > 0.05 && over < 2.0, "overshoot {over}");
        assert!(sag > 0.02 && sag < 1.0, "sag {sag}");
        assert!(settled < 15.0, "settled at {settled} s");
        assert!(f.pos.distance(to) < 0.05);
    }

    /// Heavy: it climbs much more slowly than a light craft flown the same
    /// (one with thrust and power to spare).
    #[test]
    fn climbs_slowly() {
        let light = Craft { lift: 3.0, climb: 12.0, ..C };
        let time_to = |c: &Craft| {
            let (low, high) = (Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 22.0, 0.0));
            let mut f = Flight::hovering(c, low, Vec3::Z);
            let g = goal(high);
            (0..(30.0 / STEP) as usize)
                .position(|_| {
                    let o = P.order(c, &f, &g);
                    f.step(c, o, STEP);
                    f.pos.y > low.y + 18.0
                })
                .unwrap() as f32
                * STEP
        };
        let (heavy, lightly) = (time_to(&C), time_to(&light));
        eprintln!("20 m up: {heavy:.1} s, a light craft {lightly:.1} s");
        assert!(heavy > lightly * 1.4, "heavy {heavy} s, light {lightly} s");
    }

    /// The same flight at any frame rate.
    #[test]
    fn the_same_at_any_frame_rate() {
        let g = goal(Vec3::new(15.0, 8.0, -6.0));
        let run = |dt: f32| {
            let mut f = Flight::hovering(&C, Vec3::new(0.0, 4.0, 0.0), Vec3::Z);
            let mut t = 0.0;
            while t < 6.0 - 1e-4 {
                f.advance(&C, &P, &g, dt);
                t += dt;
            }
            f.pos
        };
        let (a, b) = (run(1.0 / 30.0), run(1.0 / 144.0));
        assert!(a.distance(b) < 0.05, "{a} vs {b}");
    }

    /// Thrown about by goals far and near for a minute: never tilted past
    /// its limit by much, never flipped, never anything not a number.
    #[test]
    fn stays_upright_whatever_it_is_asked() {
        let mut f = Flight::hovering(&C, Vec3::new(0.0, 10.0, 0.0), Vec3::Z);
        let mut worst: f32 = 0.0;
        for k in 0..30 {
            let a = k as f32 * 2.399;
            let g = Goal { at: Vec3::new(a.cos() * (5.0 + k as f32 * 3.0), 10.0 + (k % 5) as f32 * 4.0, a.sin() * 20.0), vel: Vec3::ZERO, facing: Vec3::new(a.sin(), 0.0, a.cos()) };
            for s in fly(&mut f, &g, 2.0) {
                assert!(s.pos.is_finite() && s.rot.is_finite());
                worst = worst.max(s.up().angle_between(Vec3::Y));
            }
        }
        assert!(worst < C.tilt + 0.15, "tilted {}°", worst.to_degrees());
    }

    /// It faces the way asked.
    #[test]
    fn turns_to_face() {
        let at = Vec3::new(0.0, 3.0, 0.0);
        let mut f = Flight::hovering(&C, at, Vec3::Z);
        let g = Goal { at, vel: Vec3::ZERO, facing: Vec3::X };
        fly(&mut f, &g, 8.0);
        let front = f.rot * Vec3::Z;
        assert!(front.angle_between(Vec3::X) < 0.02, "facing {front}");
    }
}
