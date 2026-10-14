//! The bishop (the user's idea, after Hexen's Dark Bishops): a robed thing
//! that floats, its body never seen, only the cloth round it and the stiff
//! things on it. After the Dark Bishop: a hood bowed as in concentration, a
//! void for a face; a mantle over its shoulders, horns curving up from it; a
//! stole down its front; sleeves falling from under it; a robe narrow at the
//! waist, belled out to a heavy hem. Hexen's gold and blue trims are pale
//! markings on dark here (the creatures' way). What it is shows in how the
//! cloth moves: the hem lagging a sudden move, swinging back, settling.
//!
//! It hunts you (see `float`): floating at a distance, facing you, blinking
//! aside. `spawn bishop [N]` at the console; `--opt bishop` (with `--opt
//! peace`) the lab's, which moves in turn instead.

use super::*;
use crate::cloth::Cloth;

/// Points round the robe and down it; its hidden shoulders' height over
/// what it floats above, and the robe's length from them.
const ROUND: usize = 28;
const DOWN: usize = 20;
const SHOULDERS: f32 = 2.36;
const LENGTH: f32 = 2.01;
/// The cloth's shade and its markings' (sRGB).
const DARK: f32 = 0.035;
/// How far its head is bowed (radians).
const BOW: f32 = 0.85;
const PALE: f32 = 0.5;

/// Points round a sleeve and along it; where its hidden arm runs (from its
/// shoulders' middle, its frame: x across, z forward): shoulder, elbow,
/// wrist, the right arm's (the left's mirrored).
const SLEEVE_ROUND: usize = 10;
const SLEEVE_ALONG: usize = 9;
const ARM: [Vec3; 3] = [Vec3::new(0.36, -0.15, -0.02), Vec3::new(0.42, -0.68, 0.04), Vec3::new(0.12, -0.73, 0.25)];

/// Its needles beside a harrow's (too large on it, the user): 0.25 m long,
/// 4 cm thick.
const NEEDLE_SCALE: f32 = 0.6;

/// How far from you it likes to float (m), how fast it drifts (m/s), and
/// how far it blinks aside (m).
const RANGE: (f32, f32) = (7.0, 14.0);
const DRIFT: f32 = 3.0;
const BLINK: (f32, f32) = (3.0, 6.0);
/// What it takes before it breaks (a harrow 45).
const BISHOP_HEALTH: f32 = 40.0;
/// Its cast: how long the light gathers (s), how long between casts, how
/// near and far you may be. Then lightning (the user's idea; fireball motes
/// read as balls, see `samples/shaders/fireball`): three strands from the
/// orb at its hands, joined to it all the way, each on its own way to you
/// (the user: not in step, a triple helix; each its own). Each leaves the
/// orb straight out from where on it it starts, slow, then speeds up and
/// bends round to you, homing a little (late, a step aside still beats it);
/// past you, straight on. Each strand's tip: how fast at most (m/s, each its
/// own), how fast it turns once on its way (rad/s) and at first (curling
/// out), how long at most (s), what it does to you. (The user: as it was it
/// posed no threat: it cast seldom, after a long gathering, slow, aimed only
/// where you had been, and stood idle after.)
const GATHER: f32 = 0.6;
const BETWEEN: (f32, f32) = (2.5, 4.0);
const CAST_RANGE: (f32, f32) = (4.0, 24.0);
const STRANDS: usize = 3;
const BOLT_SPEED: (f32, f32) = (20.0, 30.0);
const BOLT_TURN: f32 = 1.2;
const BOLT_CURL: f32 = 5.0;
const BOLT_LIFE: f32 = 3.0;
const BOLT_HARM: f32 = 12.0;
/// What a strand's tip strikes: you, as a body from your shins to the top
/// of your head (m below your eye, and above), so wide (m). (A sphere at
/// your chest: one passing by your knees or your head, a hand off, missed.)
const BODY: (f32, f32, f32) = (1.3, 0.15, 0.55);
/// The spiral round each strand's way (how far out, m; how fast round,
/// rad/s, either way: each its own), closing on you over its last few
/// metres; how much it wanders (of its heading) and how fast; the orb they
/// start on (its radius, m).
const SPIRAL: (f32, f32) = (0.1, 0.35);
const SPIN: (f32, f32) = (5.0, 13.0);
const CLOSING: f32 = 4.0;
const WANDER: (f32, (f32, f32)) = (0.2, (2.5, 6.0));
const ORB: f32 = 0.13;
/// A strand's look: how wide (m), its kinks (how far apart, how far aside,
/// m), how often they are drawn anew (s: shimmering), how many forks off it
/// and how long (m), how bright (HDR); once it strikes (or is broken), how
/// long it holds on you and how long it fades (s).
const BOLT_WIDTH: f32 = 0.035;
const BOLT_KINK: (f32, f32) = (0.16, 0.07);
const BOLT_EVERY: f32 = 0.035;
const BOLT_FORKS: (usize, (f32, f32)) = (4, (0.25, 0.7));
const BOLT_GLOW: f32 = 40.0;
const BOLT_HOLD: f32 = 0.3;
/// Each strand's dark twin (the user's idea, for strangeness): a black
/// thread right beside it, along its very kinks (the user: apart, with its
/// own, it was clutter), winding from one side of it to the other (rad per
/// kink, and per s), how wide (of the strand's).
const TWIN_TWIST: (f32, f32) = (0.5, 6.0);
const TWIN_WIDTH: f32 = 0.35;
const BOLT_FADE: f32 = 0.25;
/// A cast at its longest (s): it ends sooner, its orb out, once its last
/// strand has faded (the orb lit on, nothing coming out of it, read as a
/// fault).
const CASTING: f32 = BOLT_LIFE + BOLT_HOLD + BOLT_FADE;
/// The light it gathers and holds as it casts, at its fullest (lm, m); a
/// strand's tip's (lm, m); a strike's flash (lm, m, s).
const GLEAM_LIGHT: (f32, f32) = (1.5e5, 7.0);
const BOLT_LIGHT: (f32, f32) = (5.0e4, 5.0);
const FLASH: (f32, f32, f32) = (6.0e5, 8.0, 0.18);
/// Where its light gathers, from its neck: where its hands would meet.
const LIGHT: Vec3 = Vec3::new(0.0, -0.79, 0.32);
/// How long its empty robes lie after it breaks (s).
const LIES: f32 = 6.0;

#[derive(Component)]
pub(super) struct Bishop {
    cloth: Cloth,
    mesh: Handle<Mesh>,
    /// Its sleeves (left, right), and their meshes.
    sleeves: Vec<(Cloth, Handle<Mesh>)>,
    /// Its hood and mantle, and their mesh.
    cape: Cloth,
    cape_mesh: Handle<Mesh>,
    /// Its cloth's drape, settled, in its own frame (from its shoulders,
    /// unturned): robe, hood and mantle, sleeves. Once it has settled (see
    /// `DRAPED`) its cloth is frozen in it while it lives: its meshes made
    /// in its own frame once and carried with it, nothing simulated or
    /// made again (the user: simulated, its cloth's motion as it floated and
    /// blinked was imperceptible, or a stiff thing twitching; 0.4 ms for
    /// nothing). Broken, it is simulated again, to fall.
    drape: Option<[Vec<Vec3>; 4]>,
    /// Where it floats over (on the floor under it), which way it faces,
    /// how fast it drifts; its clock.
    at: Vec3,
    heading: f32,
    velocity: Vec3,
    time: f32,
    /// Its shoulders and its turn this frame (its stiff parts follow).
    top: Vec3,
    place: Quat,
    /// The lab's, moving in turn (`--opt bishop`); else it hunts.
    lab: Option<Lab>,
    /// Hunting: where it drifts to and when it picks another spot; when it
    /// may blink again; how long you have held your aim on it; whether it
    /// was hit this frame.
    goal: Vec3,
    next_goal: f32,
    next_blink: f32,
    aimed: f32,
    struck: bool,
    /// Hit since its pain was last heard; when it may cry out again; when it
    /// next calls.
    pained: bool,
    pain_in: f32,
    call_in: f32,
    pub(super) health: f32,
    /// Since it broke (s): its robes falling empty.
    broken: Option<f32>,
    /// Gathering its light (s into it), and when it may cast again; casting
    /// (its strands out: s left at most).
    charge: Option<f32>,
    next_cast: f32,
    casting: Option<f32>,
}

/// The lab's bishop: where it hangs from, its sideways, how far aside it is
/// and wants to be (and how fast it goes), which way it wants to face, when
/// it next moves.
struct Lab {
    home: Vec3,
    side: Vec3,
    aside: f32,
    want_aside: f32,
    pace: f32,
    want_heading: f32,
    next: f32,
}

impl Bishop {
    /// Round all of it: a shard missing this misses it (a first look,
    /// before its cloth).
    fn bounds(&self) -> crate::rig::Bone {
        crate::rig::Bone { a: self.top + Vec3::Y * 0.6, b: self.top - Vec3::Y * (LENGTH + 0.1), radius: 0.8, side: Vec3::X }
    }

    /// Where its chest is, unbroken (to be watched, in captures).
    pub(super) fn chest(&self) -> Option<Vec3> {
        self.broken.is_none().then_some(self.top - Vec3::Y * 0.6)
    }

    /// The nearest a ray from `origin` along `dir` strikes it, within `max`:
    /// its cloth itself, frozen in its drape (a shard through a gap, between
    /// a sleeve and its robe, misses; a capsule round it, its needles stood
    /// off the robe where it is narrow and in it where it is wide).
    pub(super) fn ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        if self.broken.is_some() || self.bounds().ray(origin, dir, max).is_none() {
            return None;
        }
        let to = self.place.inverse();
        let (o, d) = (to * (origin - self.top), to * dir);
        [&self.cloth, &self.cape, &self.sleeves[0].0, &self.sleeves[1].0].into_iter().filter_map(|c| c.ray(o, d, max)).min_by(f32::total_cmp)
    }

    /// A shard stuck in it where it struck (`at`, flying along `dir`), white
    /// hot a moment (see `Needle`), carried with it, as its horns are.
    pub(super) fn embed(&self, commands: &mut Commands, assets: &Assets3, owner: Entity, at: Vec3, dir: Vec3, delay: f32) {
        let to = self.place.inverse();
        // Shrapnel: each at its own slant off the shard's line (up to 50
        // degrees, any way round), its own length, its own depth in the
        // cloth (all along the shot's line, alike, they read as the
        // interface's marks, not the world's, the user).
        let r = |j: i32| hash01((at.x * 97.0) as i32, (at.y * 89.0) as i32 ^ (at.z * 83.0) as i32, j, 0x8b4);
        let across = dir.any_orthonormal_vector();
        let tilt = Quat::from_axis_angle(dir, r(0) * std::f32::consts::TAU) * Quat::from_axis_angle(across, 0.87 * r(1).sqrt());
        let dir = (tilt * dir).normalize();
        let long = 0.6 + 0.7 * r(2);
        let base = at - dir * NEEDLE_OUT * NEEDLE_SCALE * long * (0.5 + r(3));
        commands.spawn((
            OfBishop(owner),
            Stiff { offset: to * (base - self.top - Vec3::Y * 0.06), turn: to * Quat::from_rotation_arc(Vec3::Y, dir), dark: true },
            Needle { delay, life: NEEDLE_LIFE, size: NEEDLE_SIZE * NEEDLE_SCALE * Vec3::new(1.0, long, 1.0) },
            Mesh3d(assets.shard.clone()),
            MeshMaterial3d(assets.bright.clone()),
            Transform::from_scale(Vec3::ZERO),
        ));
    }

    /// A shard struck it, flying along `dir`: hurt, and pushed a little
    /// (it floats: light).
    pub(super) fn hurt(&mut self, dir: Vec3, damage: f32) {
        self.health -= damage;
        self.velocity += Vec3::new(dir.x, 0.0, dir.z) * 0.6;
        self.struck = true;
        self.pained = true;
    }
}

/// Every part of a bishop, and whose.
#[derive(Component)]
pub(super) struct OfBishop(Entity);

/// A stiff part (the horns) and the darkness in it: carried with its
/// shoulders, where it is from them and turned how; whether darkness (gone
/// when it breaks; the horns fall).
#[derive(Component)]
pub(super) struct Stiff {
    offset: Vec3,
    turn: Quat,
    dark: bool,
}

/// One of a bishop's cloth meshes (0 the robe, 1 the hood and mantle, 2
/// and 3 the sleeves): carried with it while frozen.
#[derive(Component)]
pub(super) struct Drape;

/// Its light, at its chest, grown as it gathers (else none).
#[derive(Component)]
pub(super) struct Gleam;

/// A strand of its lightning: whose; where it is aimed; where its tip's way
/// runs, how fast, how far round the spiral it is, how long it has run; the
/// points it has passed, from the orb; its tip; struck (s since), its glow
/// and its light; drawn anew now and then (its seed, how many times, when
/// next).
#[derive(Component)]
pub(super) struct Bolt {
    owner: Entity,
    /// Past you (on straight, till it strikes what is built: stopping
    /// short, a step back saved you; turning back, it would circle you).
    passed: bool,
    /// It has struck you (once only).
    hurt: bool,
    centre: Vec3,
    velocity: Vec3,
    /// Its own way: how fast at most, its spiral (where round it starts,
    /// how fast round, signed, how far out), its wander (how fast, each
    /// way, and where in it).
    speed: f32,
    phase: f32,
    spin: f32,
    spiral: f32,
    wander: Vec3,
    wander_at: Vec3,
    age: f32,
    path: Vec<Vec3>,
    tip: Vec3,
    struck: Option<f32>,
    material: Handle<StandardMaterial>,
    light: Entity,
    /// Its dark twin: its mesh and its black (fading).
    twin: Entity,
    twin_mesh: Handle<Mesh>,
    twin_material: Handle<StandardMaterial>,
    seed: i32,
    roll: i32,
    next_roll: f32,
}

/// Whether a strand (the points it passed, then its tip) touches you (your
/// body: from `feet` to `head`).
fn touches(path: &[Vec3], tip: Vec3, feet: Vec3, head: Vec3) -> bool {
    path.windows(2).any(|w| between(w[0], w[1], feet, head) < BODY.2) || path.last().is_some_and(|&p| between(p, tip, feet, head) < BODY.2)
}

/// A strand strikes you: it hurts you, shoves you along its way (`dir`),
/// jolts the view; it bursts on you, and you cry out.
#[allow(clippy::too_many_arguments)]
fn shock(commands: &mut Commands, assets: &Assets3, player: &mut Player, right: Vec3, director: &mut Director, gun: &mut Gun, dir: Vec3) {
    player.health -= BOLT_HARM;
    player.velocity += dir.with_y(0.0).normalize_or(Vec3::ZERO) * 4.0 + Vec3::Y * 1.5;
    director.hurt = 1.0;
    gun.jolt += Vec2::new(0.06, -dir.dot(right) * 0.08);
    commands.spawn((AudioPlayer::new(assets.mote_burst.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
    commands.spawn((AudioPlayer::new(assets.player_pain.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
}

/// How near two segments come.
fn between(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> f32 {
    let (d1, d2, r) = (a1 - a0, b1 - b0, a0 - b0);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    let (s, t) = if a <= 1e-9 {
        (0.0, (f / e.max(1e-9)).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        let b = d1.dot(d2);
        let denom = a * e - b * b;
        let s = if denom > 1e-9 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
        let t = (b * s + f) / e.max(1e-9);
        if t < 0.0 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else if t > 1.0 {
            (((b - c) / a).clamp(0.0, 1.0), 1.0)
        } else {
            (s, t)
        }
    };
    (a0 + d1 * s).distance(b0 + d2 * t)
}

/// A strand drawn: from the orb along the points it passed to its tip,
/// kinked, a flat ribbon turned to the eye (a tube read as bent wire), with
/// forks off it, thinning to their ends; and its dark twin, a thread beside
/// it along its kinks, winding from side to side (`turn` where it starts).
fn bolt_mesh(points: &[Vec3], eye: Vec3, seed: i32, width: f32, turn: f32) -> (Mesh, Mesh) {
    let r = |k: i32, j: i32| hash01(seed, k, j, 0x6d3) - 0.5;
    // Kinked: a point every so often along it, each pushed aside.
    let mut line = vec![points[0]];
    let mut k = 0;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = ((a.distance(b) / BOLT_KINK.0).ceil() as usize).max(1);
        for s in 1..=n {
            k += 1;
            let p = a.lerp(b, s as f32 / n as f32);
            let aside = Vec3::new(r(k, 1), r(k, 2), r(k, 3)) * 2.0 * BOLT_KINK.1;
            line.push(if k > 0 && (s < n || w[1] != *points.last().unwrap()) { p + aside } else { p });
        }
    }
    // A ribbon along a line, so wide, set aside from it by `shift(i)` (m,
    // at its i-th point).
    let ribbon = |out: &mut (Vec<[f32; 3]>, Vec<[f32; 3]>), line: &[Vec3], width: f32, shift: &dyn Fn(usize) -> f32| {
        for (i, w) in line.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let along = (b - a).normalize_or(Vec3::Y);
            let side = along.cross((eye - a).normalize_or(Vec3::Z)).normalize_or(Vec3::X);
            // (No wider close up than a thread: a fixed width, near the
            // eye, was a fat white band.)
            let near = |p: Vec3| (p.distance(eye) * 0.004 / width).min(1.0);
            let t0 = (1.0 - i as f32 / line.len() as f32 * 0.3) * near(a);
            let t1 = (1.0 - (i + 1) as f32 / line.len() as f32 * 0.3) * near(b);
            let (a, b) = (a + side * shift(i) * near(a), b + side * shift(i + 1) * near(b));
            let (a0, a1, b0, b1) = (a - side * width * t0, a + side * width * t0, b - side * width * t1, b + side * width * t1);
            let normal = (eye - a).normalize_or(Vec3::Z);
            for p in [a0, a1, b1, a0, b1, b0] {
                out.0.push(p.to_array());
                out.1.push(normal.to_array());
            }
        }
    };
    let mut strand = (Vec::new(), Vec::new());
    ribbon(&mut strand, &line, width * 0.5, &|_| 0.0);
    // Its twin: along the very same kinks, just beside it, winding from one
    // side to the other.
    let mut twin = (Vec::new(), Vec::new());
    let beside = width * 0.5 * (1.0 + TWIN_WIDTH) + 0.004;
    ribbon(&mut twin, &line, width * 0.5 * TWIN_WIDTH, &|i| beside * (turn + i as f32 * TWIN_TWIST.0).cos());
    // Forks: from a few of its points, out aside and on, kinked, thinner.
    if line.len() > 4 {
        for f in 0..BOLT_FORKS.0 as i32 {
            let at = 1 + ((r(100 + f, 0) + 0.5) * (line.len() - 3) as f32) as usize;
            let along = (line[at + 1] - line[at - 1]).normalize_or(Vec3::Y);
            let out = (along + Vec3::new(r(100 + f, 1), r(100 + f, 2), r(100 + f, 3)) * 2.2).normalize_or(along);
            let length = BOLT_FORKS.1.0 + (BOLT_FORKS.1.1 - BOLT_FORKS.1.0) * (r(100 + f, 4) + 0.5);
            let mut fork = vec![line[at]];
            for s in 1..=4 {
                let aside = Vec3::new(r(200 + f * 8 + s, 1), r(200 + f * 8 + s, 2), r(200 + f * 8 + s, 3)) * BOLT_KINK.1 * 1.5;
                fork.push(line[at] + out * length * s as f32 / 4.0 + aside);
            }
            ribbon(&mut strand, &fork, width * 0.3, &|_| 0.0);
        }
    }
    let mesh = |(positions, normals): (Vec<[f32; 3]>, Vec<[f32; 3]>)| {
        Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    };
    (mesh(strand), mesh(twin))
}

/// An afterimage of a blink, fading.
#[derive(Component)]
pub(super) struct Ghost {
    life: f32,
    total: f32,
    material: Handle<StandardMaterial>,
}

/// The robe's points where it starts, in its own frame (x across, z
/// forward), from its shoulders: the neck ring, the shoulders, then narrow
/// at the waist and belled out to the hem, the hem ragged a little (each
/// column of it a little longer or shorter).
fn robe_at(c: usize, r: usize) -> Vec3 {
    let a = c as f32 / ROUND as f32 * std::f32::consts::TAU;
    let (across, depth, y) = match r {
        0 => (0.13, 0.12, 0.08),
        1 => (0.27, 0.17, -0.05),
        _ => {
            let t = (r - 1) as f32 / (DOWN - 2) as f32;
            let ragged = if r + 3 > DOWN { (hash01(c as i32, 0, 0, 0x8b1) - 0.5) * 0.12 * (r + 3 - DOWN) as f32 / 3.0 } else { 0.0 };
            (0.27 + 0.02 * t + 0.32 * t.powi(3), 0.18 + 0.02 * t + 0.27 * t.powi(3), -0.05 - LENGTH * t - ragged)
        }
    };
    Vec3::new(a.sin() * across, y, a.cos() * depth)
}

/// Where along the hidden arm (0 shoulder, 1 wrist), on side `side` (-1
/// left, 1 right).
fn arm_at(s: f32, side: f32) -> Vec3 {
    let p = if s < 0.5 { ARM[0].lerp(ARM[1], s * 2.0) } else { ARM[1].lerp(ARM[2], s * 2.0 - 1.0) };
    Vec3::new(p.x * side, p.y, p.z)
}

/// A sleeve's points where it starts (its frame as the robe's): rings round
/// the arm, widening from the shoulder to the wrist, and past the wrist a
/// bell, wide, the cuff.
fn sleeve_at(c: usize, r: usize, side: f32) -> Vec3 {
    let cuff = r + 1 == SLEEVE_ALONG;
    let s = (r.min(SLEEVE_ALONG - 2)) as f32 / (SLEEVE_ALONG - 2) as f32;
    let along = (arm_at((s + 0.02).min(1.0), side) - arm_at((s - 0.02).max(0.0), side)).normalize_or(Vec3::NEG_Y);
    let across = along.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = across.cross(along);
    let a = c as f32 / SLEEVE_ROUND as f32 * std::f32::consts::TAU;
    let radius = if cuff { 0.19 } else { 0.09 + 0.06 * s };
    let centre = arm_at(s, side) + if cuff { along * 0.14 - Vec3::Y * 0.05 } else { Vec3::ZERO };
    centre + (across * a.cos() + up * a.sin()) * radius
}

/// What shade a point of the robe is: pale on the stole (two bands down its
/// front) and the hem; dark else. (A belt across the stole made a cross.)
fn robe_shade(c: usize, r: usize) -> f32 {
    let stole = (c == 2 || c == ROUND - 2) && r >= 2;
    let hem = r + 1 == DOWN;
    if stole || hem { PALE } else { DARK }
}

/// Where the hood bows from (its frame: from the neck, y up, z forward).
const PIVOT: Vec3 = Vec3::new(0.0, -0.02, 0.04);

/// Points round the hood and mantle.
const CAPE_ROUND: usize = 32;

/// The hood and the mantle, one piece, cloth, as a hooded cape is (the user,
/// with references: stiff, the hood's top read as a triangle; hoods drape,
/// in swags off the shoulders): rings from the hood's crown down round a
/// head, slit at the front for the face, the slit closing under the chin,
/// round the throat (held there) and out over the shoulders. Each ring:
/// height, half width, half depth, how far forward, and how wide the slit is
/// each side of the front (radians). Its frame: from the neck, y up, z
/// forward.
const CAPE: [(f32, f32, f32, f32, f32); 15] = [
    (0.43, 0.005, 0.01, -0.03, 0.0),
    (0.42, 0.05, 0.07, -0.01, 0.0),
    (0.395, 0.1, 0.13, 0.02, 0.0),
    (0.375, 0.125, 0.16, 0.035, 0.25),
    (0.35, 0.145, 0.185, 0.045, 0.42),
    (0.29, 0.18, 0.225, 0.065, 0.6),
    (0.21, 0.2, 0.245, 0.075, 0.68),
    (0.11, 0.21, 0.24, 0.08, 0.7),
    (0.02, 0.2, 0.21, 0.075, 0.58),
    (-0.04, 0.18, 0.17, 0.05, 0.28),
    (-0.08, 0.2, 0.17, 0.02, 0.0),
    (-0.12, 0.33, 0.23, 0.0, 0.0),
    (-0.2, 0.5, 0.28, 0.0, 0.0),
    (-0.27, 0.52, 0.29, 0.0, 0.0),
    (-0.3, 0.52, 0.29, 0.0, 0.0),
];
/// Its rings held round the throat and the collar.
const THROAT: [usize; 2] = [10, 11];

/// A point of the hood and mantle where it starts: the hood bowed (`BOW`),
/// easing in from the throat; the mantle longer at its front and back.
fn cape_at(c: usize, r: usize) -> Vec3 {
    let (y, w, d, fwd, open) = CAPE[r];
    let a = if open > 0.0 { open + (std::f32::consts::TAU - 2.0 * open) * c as f32 / (CAPE_ROUND - 1) as f32 } else { c as f32 / CAPE_ROUND as f32 * std::f32::consts::TAU };
    let droop = [0.0, 0.0, 0.5, 0.9, 1.0][r.saturating_sub(10).min(4)];
    let p = Vec3::new(a.sin() * w, y - 0.14 * droop * a.cos().powi(2), a.cos() * d + fwd);
    let t = ((y + 0.06) / 0.14).clamp(0.0, 1.0);
    PIVOT + Quat::from_rotation_x(BOW * t * t * (3.0 - 2.0 * t)) * (p - PIVOT)
}

/// What shade a point of the hood and mantle is: pale round the face's
/// opening (down its sides, and across its top from the ring over it) and
/// at the mantle's lip; dark else.
fn cape_shade(c: usize, r: usize) -> f32 {
    let open = CAPE[r].4;
    let edge = open > 0.0 && (c == 0 || c + 1 == CAPE_ROUND);
    let a = c as f32 / CAPE_ROUND as f32 * std::f32::consts::TAU;
    let brow = open == 0.0 && r + 1 < CAPE.len() && CAPE[r + 1].4 > 0.0 && CAPE[r].0 > 0.0 && (a < 0.3 || a > std::f32::consts::TAU - 0.3);
    if edge || brow || r + 1 == CAPE.len() { PALE } else { DARK }
}

/// Where its head is (the hood falls over it), from the neck: bowed.
fn head() -> Vec3 {
    PIVOT + Quat::from_rotation_x(BOW) * (Vec3::new(0.0, 0.17, 0.06) - PIVOT)
}

/// Keeps a tube of cloth round its body: each point (from row `from` on) no
/// more than 75 degrees round from its own side of it (`angle`, by column:
/// 0 its front, its frame turned by `place`, round the upright through
/// `axis`). Swung far aside, a robe went over to the other side, inside out,
/// and stayed so: inside out is as much at rest as right way round.
fn keep_round(points: &mut [Vec3], cols: usize, from: usize, axis: Vec3, place: Quat, angle: &dyn Fn(usize) -> f32) {
    let limit = 75f32.to_radians();
    for (i, p) in points.iter_mut().enumerate().skip(from * cols) {
        let a = angle(i % cols);
        let side = place * Vec3::new(a.sin(), 0.0, a.cos());
        let out = Vec3::new(p.x - axis.x, 0.0, p.z - axis.z);
        let far = out.length();
        if far < 0.05 {
            continue;
        }
        let dir = out / far;
        if dir.dot(side) >= limit.cos() {
            continue;
        }
        let across = (dir - side * dir.dot(side)).normalize_or(Vec3::Y.cross(side));
        let kept = side * limit.cos() + across * limit.sin();
        p.x = axis.x + kept.x * far;
        p.z = axis.z + kept.z * far;
    }
}

/// What holds a living bishop's cloth, and what it hangs over, from its
/// shoulders `top` turned by `place`: the robe's held points and bodies, the
/// hood and mantle's, each sleeve's.
#[allow(clippy::type_complexity)]
fn holds(top: Vec3, place: Quat) -> [(Vec<(u32, Vec3)>, Vec<(Vec3, Vec3, f32)>); 4] {
    let neck = top + Vec3::Y * 0.06;
    // The robe: held at its neck and shoulders, over its hidden torso,
    // shoulders and hips (not its forearms: the robe at its sides, pushed out
    // of them through their middles, flipped and tangled).
    let robe = (
        (0..2 * ROUND).map(|i| (i as u32, top + place * robe_at(i % ROUND, i / ROUND))).collect(),
        vec![
            (top + Vec3::Y * -0.13, top + Vec3::Y * -1.05, 0.17),
            (top + place * Vec3::new(-0.22, -0.1, 0.0), top + place * Vec3::new(0.22, -0.1, 0.0), 0.1),
            (top + Vec3::Y * -1.1, top + Vec3::Y * -1.6, 0.14),
        ],
    );
    // The hood and mantle: held round the throat and the collar, over its
    // head, neck, shoulders and upper arms.
    let cape = (
        THROAT.iter().flat_map(|&r| (0..CAPE_ROUND).map(move |c| ((r * CAPE_ROUND + c) as u32, neck + place * cape_at(c, r)))).collect(),
        vec![
            (neck + place * head(), neck + place * head(), 0.17),
            (neck + place * Vec3::new(0.0, -0.1, 0.0), neck + place * head(), 0.08),
            (top + place * Vec3::new(-0.27, -0.1, 0.0), top + place * Vec3::new(0.27, -0.1, 0.0), 0.1),
            (top + place * arm_at(0.0, -1.0), top + place * arm_at(0.45, -1.0), 0.11),
            (top + place * arm_at(0.0, 1.0), top + place * arm_at(0.45, 1.0), 0.11),
            (top + Vec3::Y * -0.11, top + Vec3::Y * -0.77, 0.19),
        ],
    );
    // The sleeves: held at the shoulder all round and at the wrist along its
    // top (the cuff hangs), over the hidden arm.
    let sleeve = |side: f32| {
        (
            (0..SLEEVE_ROUND)
                .map(|c| (c, 0))
                .chain((0..SLEEVE_ROUND).filter(|&c| (c as f32 / SLEEVE_ROUND as f32 * std::f32::consts::TAU).sin() > 0.0).map(|c| (c, SLEEVE_ALONG - 2)))
                .map(|(c, r)| ((r * SLEEVE_ROUND + c) as u32, top + place * sleeve_at(c, r, side)))
                .collect(),
            vec![(top + place * arm_at(0.0, side), top + place * arm_at(0.5, side), 0.09), (top + place * arm_at(0.5, side), top + place * arm_at(1.0, side), 0.08)],
        )
    };
    [robe, cape, sleeve(-1.0), sleeve(1.0)]
}

/// A bishop's cloth as it is made, hanging as cut: robe, hood and mantle,
/// sleeves (in its own frame, from its shoulders, unturned, when `top` is 0
/// and `place` none).
fn cut(top: Vec3, place: Quat) -> [Cloth; 4] {
    let neck = top + Vec3::Y * 0.06;
    [
        // (The hem's last rows keep their bell; the rest folds softly.)
        Cloth::tube(ROUND, DOWN, |c, r| top + place * robe_at(c, r), |r| if r + 5 > DOWN { 0.7 } else { 0.3 }, |_| false),
        Cloth::tube(CAPE_ROUND, CAPE.len(), |c, r| neck + place * cape_at(c, r), |r| if r < 10 { 0.5 } else { 0.3 }, |r| CAPE[r].4 > 0.0),
        Cloth::tube(SLEEVE_ROUND, SLEEVE_ALONG, |c, r| top + place * sleeve_at(c, r, -1.0), |_| 0.35, |_| false),
        Cloth::tube(SLEEVE_ROUND, SLEEVE_ALONG, |c, r| top + place * sleeve_at(c, r, 1.0), |_| 0.35, |_| false),
    ]
}

/// The drape every bishop's cloth hangs in while it lives, in its own frame:
/// settled once, at the start, still, in the open (the user: settled where
/// each appeared, on a flight its floor rose into its hem and it formed
/// crushed; and each paid for settling).
#[derive(Resource)]
pub(super) struct Drapes([Vec<Vec3>; 4]);

/// Settles the drape: its cloth as cut, held as a living bishop holds it,
/// 1.5 s of hanging (in steps of 1/240 s), nothing under it.
fn settled() -> [Vec<Vec3>; 4] {
    let mut cloth = cut(Vec3::ZERO, Quat::IDENTITY);
    let holds = holds(Vec3::ZERO, Quat::IDENTITY);
    for _ in 0..360 {
        for (k, (held, bodies)) in holds.iter().enumerate() {
            cloth[k].step(held, bodies, -100.0, 0.9, 1.0 / 240.0);
        }
        keep_round(&mut cloth[0].points, ROUND, 2, Vec3::ZERO, Quat::IDENTITY, &|c| c as f32 / ROUND as f32 * std::f32::consts::TAU);
        keep_round(&mut cloth[1].points, CAPE_ROUND, 12, Vec3::ZERO, Quat::IDENTITY, &|c| c as f32 / CAPE_ROUND as f32 * std::f32::consts::TAU);
    }
    cloth.map(|c| c.points)
}

/// Where the settled drape is kept, as data (the user: settled once and
/// kept, not settled again every start): each cloth's number of points, then
/// its points, little-endian.
const DRAPE_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/bishop_drape.bin");

fn encode(drape: &[Vec<Vec3>; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    for cloth in drape {
        out.extend((cloth.len() as u32).to_le_bytes());
        for p in cloth {
            for v in p.to_array() {
                out.extend(v.to_le_bytes());
            }
        }
    }
    out
}

/// The drape the data holds, if it is one for these cloths (as many points
/// to each as they are cut with).
fn decode(bytes: &[u8]) -> Option<[Vec<Vec3>; 4]> {
    let counts = cut(Vec3::ZERO, Quat::IDENTITY).map(|c| c.points.len());
    let mut at = 0;
    let mut word = || {
        let w = bytes.get(at..at + 4)?;
        at += 4;
        Some(<[u8; 4]>::try_from(w).ok()?)
    };
    let mut drape: [Vec<Vec3>; 4] = Default::default();
    for (k, cloth) in drape.iter_mut().enumerate() {
        let n = u32::from_le_bytes(word()?) as usize;
        if n != counts[k] {
            return None;
        }
        for _ in 0..n {
            let mut v = [0.0; 3];
            for x in &mut v {
                *x = f32::from_le_bytes(word()?);
            }
            cloth.push(Vec3::from_array(v));
        }
    }
    Some(drape)
}

/// The drape: kept as data (`data/bishop_drape.bin`, built into the game;
/// made with `--opt bakedrape`, which settles it, writes it and leaves).
pub(super) fn settle(mut commands: Commands, args: Res<Args>, mut exit: MessageWriter<AppExit>) {
    if args.opt("bakedrape") {
        let drape = settled();
        match std::fs::write(DRAPE_FILE, encode(&drape)) {
            Ok(()) => info!("bakedrape: the bishop's drape kept in {DRAPE_FILE}"),
            Err(e) => error!("bakedrape: {DRAPE_FILE}: {e}"),
        }
        exit.write(AppExit::Success);
        commands.insert_resource(Drapes(drape));
        return;
    }
    // (Built into the game: nothing read or settled as it starts. Not for
    // its cloth as cut, it fails, as a missing asset would: no settling it
    // instead, the user.)
    let drape = decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/bishop_drape.bin")))
        .expect("data/bishop_drape.bin is not the drape of the bishop's cloth as it is cut (cut otherwise since?): keep it again with `--opt bakedrape`");
    commands.insert_resource(Drapes(drape));
}

/// A bishop, floating over `at` (on the floor), facing `heading`: its cloth
/// (robe, sleeves, hood and mantle), its horns, and the darkness in it.
#[allow(clippy::too_many_arguments)]
fn make(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, assets: &Assets3, drapes: &Drapes, grey: bool, at: Vec3, heading: f32, lab: Option<Lab>) -> Entity {
    let place = Quat::from_rotation_y(heading);
    let top = at + Vec3::Y * SHOULDERS;
    let shade = |v: f32| Color::srgb(v, v, v).to_linear().to_f32_array();
    // Its cloth in the drape, its meshes made in its own frame (carried with
    // it: see `carry`).
    let [mut cloth, mut cape, mut left, mut right] = cut(Vec3::ZERO, Quat::IDENTITY);
    for (c, d) in [&mut cloth, &mut cape, &mut left, &mut right].into_iter().zip(&drapes.0) {
        c.set(|i| d[i]);
    }
    let mut robe_mesh = cloth.mesh();
    if !grey {
        robe_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, (0..DOWN).flat_map(|r| (0..ROUND).map(move |c| shade(robe_shade(c, r)))).collect::<Vec<_>>());
    }
    let mesh = meshes.add(robe_mesh);
    // Heavy cloth, matte (a sheen off the bright ground made it grey), seen
    // from both sides; its shade in its vertices (the markings). (`--opt
    // bishopgrey`: all of it one flat grey, to see its outline.)
    let cloth_material = materials.add(StandardMaterial { base_color: if grey { Color::srgb(0.4, 0.4, 0.4) } else { Color::WHITE }, perceptual_roughness: 0.9, reflectance: 0.25, double_sided: true, cull_mode: None, ..default() });
    let owner = commands
        .spawn((Drape, Mesh3d(mesh.clone()), MeshMaterial3d(cloth_material.clone()), Transform::default(), bevy::camera::visibility::NoFrustumCulling))
        .id();
    // Its sleeves, hanging off hidden arms bent in to where its hands would
    // be, their cuffs drooping (the user: what makes the Dark Bishop's
    // outline is its arms).
    let mut sleeves = Vec::new();
    for sleeve in [left, right] {
        let mut sleeve_mesh = sleeve.mesh();
        if !grey {
            sleeve_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, (0..SLEEVE_ALONG).flat_map(|r| (0..SLEEVE_ROUND).map(move |_| shade(if r + 1 == SLEEVE_ALONG { PALE } else { DARK }))).collect::<Vec<_>>());
        }
        let handle = meshes.add(sleeve_mesh);
        commands.spawn((OfBishop(owner), Drape, Mesh3d(handle.clone()), MeshMaterial3d(cloth_material.clone()), Transform::default(), bevy::camera::visibility::NoFrustumCulling));
        sleeves.push((sleeve, handle));
    }
    // The hood and mantle (see `CAPE`).
    let mut cape_mesh = cape.mesh();
    if !grey {
        cape_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, (0..CAPE.len()).flat_map(|r| (0..CAPE_ROUND).map(move |c| shade(cape_shade(c, r)))).collect::<Vec<_>>());
    }
    let cape_mesh = meshes.add(cape_mesh);
    commands.spawn((OfBishop(owner), Drape, Mesh3d(cape_mesh.clone()), MeshMaterial3d(cloth_material.clone()), Transform::default(), bevy::camera::visibility::NoFrustumCulling));
    let stiff_part = |commands: &mut Commands, mesh: Handle<Mesh>, material: Handle<StandardMaterial>, offset: Vec3, turn: Quat, scale: Vec3, dark: bool| {
        commands.spawn((OfBishop(owner), Stiff { offset, turn, dark }, Mesh3d(mesh), MeshMaterial3d(material), Transform::from_scale(scale)));
    };
    // In the hood, its head: darkness (unlit: no light gets in), no eyes (a
    // void for a face, as the Dark Bishop's); the hood falls over it.
    let void = if grey { cloth_material.clone() } else { materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, ..default() }) };
    stiff_part(commands, meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), void.clone(), head(), Quat::from_rotation_x(BOW), Vec3::new(0.14, 0.16, 0.13), true);
    // Horns curving up off it, two to a shoulder (at the mantle's corners).
    let horn = if grey { cloth_material.clone() } else { materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.6, ..default() }) };
    for side in [-1.0f32, 1.0] {
        for (at, dir, length) in [(Vec3::new(0.42, -0.05, -0.06), Vec3::new(0.2, 1.0, -0.15), 0.55), (Vec3::new(0.52, -0.1, 0.03), Vec3::new(0.45, 0.9, 0.0), 0.38)] {
            let at = Vec3::new(at.x * side, at.y, at.z);
            let dir = Vec3::new(dir.x * side, dir.y, dir.z).normalize();
            stiff_part(commands, assets.shard.clone(), horn.clone(), at, Quat::from_rotation_arc(Vec3::Y, dir), Vec3::new(5.0, length / 0.7, 5.0), false);
        }
    }
    // Its light, where its hands would meet: none until it gathers (see
    // `gleam`).
    commands.spawn((
        OfBishop(owner),
        Gleam,
        Stiff { offset: LIGHT, turn: Quat::IDENTITY, dark: true },
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap())),
        MeshMaterial3d(assets.bright.clone()),
        Transform::from_scale(Vec3::ZERO),
        bevy::light::NotShadowCaster,
        // (Lighting its sleeves and hood from below as it gathers: seen
        // across a room.)
        PointLight { intensity: 0.0, range: GLEAM_LIGHT.1, shadow_maps_enabled: false, ..default() },
    ));
    // Inside its sleeves and its robe, darkness too (as in its hood): its
    // arms and its body, never seen (looking into a cuff, or up under it,
    // the cloth's hollow inside was, the user). No hands.
    let from_neck = Vec3::Y * 0.06;
    let mut dark = |commands: &mut Commands, a: Vec3, b: Vec3, radius: f32| {
        let mesh = meshes.add(Capsule3d::new(radius, a.distance(b)).mesh().rings(2).longitudes(10).latitudes(6));
        stiff_part(commands, mesh, void.clone(), (a + b) * 0.5 - from_neck, Quat::from_rotation_arc(Vec3::Y, (b - a).normalize_or(Vec3::Y)), Vec3::ONE, true);
    };
    for side in [-1.0f32, 1.0] {
        dark(commands, arm_at(0.0, side), arm_at(0.5, side), 0.07);
        dark(commands, arm_at(0.5, side), arm_at(1.0, side), 0.07);
    }
    dark(commands, Vec3::Y * -0.11, Vec3::Y * -1.65, 0.15);
    let bishop = Bishop {
        cloth,
        mesh,
        sleeves,
        cape,
        cape_mesh,
        drape: Some(drapes.0.clone()),
        at,
        heading,
        velocity: Vec3::ZERO,
        time: 0.0,
        top,
        place,
        lab,
        goal: at,
        next_goal: 1.0,
        next_blink: 0.3,
        aimed: 0.0,
        struck: false,
        pained: false,
        pain_in: 0.0,
        call_in: 2.0,
        health: BISHOP_HEALTH,
        broken: None,
        charge: None,
        next_cast: 2.5,
        casting: None,
    };
    commands.entity(owner).insert((OfBishop(owner), bishop));
    owner
}

/// Where a bishop may float, from `from` (the spot on the floor before you
/// it is asked for): in a built place the floor nearest there.
fn floor_near(ground: &crate::ground::Ground, nav: Option<&crate::chasm::nav::Nav>, from: Vec3) -> Vec3 {
    if let Some(nav) = nav
        && let Some(j) = nav.nearest(from, 16)
    {
        return nav.nodes[j as usize];
    }
    Vec3::new(from.x, ground.at(from), from.z)
}

/// Bishops arrive: the lab's (`--opt bishop`, once you are where you start;
/// `bishop off` at the console takes it away, `bishop on` hangs it before
/// you afresh), and those `spawn bishop [N]` asks for (hunting), 3 m before
/// you.
#[allow(clippy::too_many_arguments)]
pub(super) fn arrive(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    assets: Res<Assets3>,
    ground: crate::ground::Ground,
    camera: Single<(&Transform, &Player, &FlyCam)>,
    nav: Option<Res<crate::chasm::nav::Nav>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawns: ResMut<crate::console::Spawns>,
    drapes: Res<Drapes>,
    bishops: Query<(Entity, &Bishop)>,
    parts: Query<(Entity, &OfBishop)>,
) {
    let (camera, player, fly) = *camera;
    let forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::Z);
    let across = Vec3::new(-forward.z, 0.0, forward.x);
    let grey = args.opt("bishopgrey");
    let face = |at: Vec3| (camera.translation - at).x.atan2((camera.translation - at).z);
    // (`spawn bishop [N]`: hunting, side by side.)
    let mut asked = 0;
    spawns.creatures.retain(|(kind, n)| {
        if kind == "bishop" {
            asked += n;
        }
        kind != "bishop"
    });
    for n in 0..asked {
        let aside = (n as f32 - (asked - 1) as f32 * 0.5) * 1.8;
        let at = floor_near(&ground, nav.as_deref(), camera.translation + forward * super::hunter::AHEAD + across * aside);
        make(&mut commands, &mut meshes, &mut materials, &assets, &drapes, grey, at, face(at), None);
        info!("a bishop hangs at {:.1}", at);
    }
    // The lab's.
    let lab = bishops.iter().find(|(_, b)| b.lab.is_some()).map(|(e, _)| e);
    if !args.opt("bishop") {
        if let Some(e) = lab {
            for (part, of) in &parts {
                if of.0 == e {
                    commands.entity(part).despawn();
                }
            }
        }
        return;
    }
    // (Once you are where you start: in a region of the plan you are put on
    // its path a moment after it begins; flying, as in a capture, a second
    // in.)
    if lab.is_none() && (player.ready || (fly.noclip && time.elapsed_secs() > 1.0)) {
        let at = floor_near(&ground, nav.as_deref(), camera.translation + forward * super::hunter::AHEAD);
        let heading = face(at);
        let lab = Lab { home: at, side: Vec3::new(forward.z, 0.0, -forward.x), aside: 0.0, want_aside: 0.0, pace: 0.0, want_heading: heading, next: 1.0 };
        make(&mut commands, &mut meshes, &mut materials, &assets, &drapes, grey, at, heading, Some(lab));
        info!("a bishop hangs at {:.1}", at);
    }
}

/// Bishops float: the lab's moves in turn, eight seconds each (`--set
/// bishop_mode=N` holds one: 0 still, 1 drifting, 2 jerking aside, 3
/// turning, 4 hunting; `bishop_yaw=D` faces it D degrees off you); the others hunt
/// you: drifting, never quite steadily, from spot to spot a little way off
/// where you are in sight, always facing you; and blinking aside, gone and
/// there in an instant, afterimages left along the way, when you shoot, or
/// hold your aim on it, or it is hit, or now and then. Their cloth hangs on
/// them (see `drape`). Broken, the darkness in it bursts and its robes fall
/// empty.
#[allow(clippy::too_many_arguments)]
pub(super) fn float(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    nav: Option<Res<crate::chasm::nav::Nav>>,
    camera: Single<&Transform, With<FlyCam>>,
    gun: Res<Gun>,
    assets: Res<Assets3>,
    ichor: Res<ichor::Ichor>,
    mut director: ResMut<Director>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bishops: Query<(Entity, &mut Bishop)>,
    parts: Query<(Entity, &OfBishop, Option<&Stiff>, &Transform), Without<Bishop>>,
    mut heard: Local<u32>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    let shot = gun.shots != *heard;
    *heard = gun.shots;
    let eye = camera.translation;
    for (entity, mut b) in &mut bishops {
        let b = &mut *b;
        b.time += dt;
        let r = |k: i32| hash01(entity.index_u32() as i32, (b.time * 10.0) as i32, k, 0x8b2);
        // Its voice, from its hood: in pain when hit (not again for half a
        // second: a blast is one cry), and now and then a call.
        if b.broken.is_none() && b.health > 0.0 {
            b.pain_in -= dt;
            b.call_in -= dt;
            let hood = b.top + Vec3::Y * 0.3;
            if std::mem::take(&mut b.pained) && b.pain_in <= 0.0 {
                b.pain_in = 0.5;
                b.call_in = b.call_in.max(1.5);
                commands.spawn((AudioPlayer::new(assets.bishop_pain.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(2.0)), Transform::from_translation(hood)));
            } else if b.call_in <= 0.0 {
                b.call_in = 4.0 + 6.0 * r(60);
                commands.spawn((AudioPlayer::new(assets.bishop_call.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(2.0)), Transform::from_translation(hood)));
            }
        }
        let to = eye - b.at;
        let facing = to.x.atan2(to.z);
        // Broken: the darkness in it bursts, its horns fall, its robes fall
        // empty and lie, and are gone.
        if b.health <= 0.0 && b.broken.is_none() {
            b.broken = Some(0.0);
            let chest = b.top - Vec3::Y * 0.5;
            commands.spawn((AudioPlayer::new(assets.bishop_death.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(2.0)), Transform::from_translation(chest)));
            for k in 0..4 {
                let p = chest - Vec3::Y * 0.35 * k as f32;
                shatter_quiet(&mut commands, &assets, p, b.velocity, entity.index_u32() * 7 + k);
                ichor::spray(&mut commands, &ichor, p, Vec3::Y, 2.0, entity.index_u32() * 11 + k);
            }
            for (part, of, stiff, transform) in &parts {
                if of.0 != entity {
                    continue;
                }
                match stiff {
                    Some(s) if s.dark => commands.entity(part).despawn(),
                    Some(_) => {
                        let q = |j: i32| hash01(part.index_u32() as i32, j, 0, 0x8b3) - 0.5;
                        commands.entity(part).remove::<Stiff>().insert(Debris { velocity: Vec3::new(q(0), 1.0 + q(1), q(2)) * 3.0, spin: Vec3::new(q(3), q(4), q(5)) * 6.0, life: LIES, total: LIES, size: transform.scale });
                    }
                    None => {}
                }
            }
            // (Limp: nothing holding it, it falls in a heap, not standing.)
            b.cloth.limp = true;
            b.cape.limp = true;
            for (sleeve, _) in &mut b.sleeves {
                sleeve.limp = true;
            }
            director.kills += 1;
            info!("a bishop is broken");
        }
        if let Some(since) = b.broken.as_mut() {
            *since += dt;
            if *since > LIES {
                for (part, of, _, _) in &parts {
                    if of.0 == entity {
                        commands.entity(part).despawn();
                    }
                }
                commands.entity(entity).despawn();
                continue;
            }
        } else if let Some(lab) = b.lab.as_mut()
            && args.num("bishop_mode", -1.0) != 4.0
        {
            // (`--set bishop_mode=4`: the lab's hunts, as a spawned one does.)
            let mode = if args.num("bishop_mode", -1.0) >= 0.0 { args.num("bishop_mode", 0.0) as u32 } else { (b.time / 8.0) as u32 % 4 };
            // (`--set bishop_yaw=D`: it faces D degrees off you, to see it
            // from the side.)
            let facing = facing + args.num("bishop_yaw", 0.0).to_radians();
            match mode {
                1 => lab.want_aside = 2.2 * (b.time * 0.55).sin(),
                2 if b.time >= lab.next => {
                    lab.next = b.time + 0.6 + 1.1 * r(0);
                    lab.want_aside = (r(1) - 0.5) * 6.0;
                }
                3 if b.time >= lab.next => {
                    lab.next = b.time + 0.9 + 0.8 * r(2);
                    lab.want_heading = facing + (r(3) - 0.5) * 2.6;
                }
                0 => lab.want_aside = 0.0,
                _ => {}
            }
            if mode != 3 {
                lab.want_heading = facing;
            }
            // Drifting, it follows slowly; jerked aside, a lunge at up to 18
            // m/s, braked hard (snapped there at once, 66 m/s at first, it
            // flung its robe up over its head); turning, it snaps round.
            if mode == 2 {
                let want = ((lab.want_aside - lab.aside) * 12.0).clamp(-18.0, 18.0);
                lab.pace += (want - lab.pace).clamp(-200.0 * dt, 200.0 * dt);
                lab.aside += lab.pace * dt;
            } else {
                lab.pace = 0.0;
                lab.aside += (lab.want_aside - lab.aside) * (1.0 - (-dt * 3.0).exp());
            }
            let turn = (lab.want_heading - b.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            b.heading += turn * (1.0 - (-dt * if mode == 3 { 14.0 } else { 4.0 }).exp());
            let base = lab.home + lab.side * lab.aside;
            b.at = Vec3::new(base.x, ground.at(base), base.z);
        } else {
            // Hunting. It sees you (nothing built between its hood and your
            // eyes); you hold your aim on it (within 4 degrees), or shoot.
            let hood = b.top + Vec3::Y * 0.3;
            let sees = hood.distance(eye) < 40.0 && ground.clear(hood, eye);
            let at_it = (b.top - Vec3::Y * 0.6 - eye).normalize_or(Vec3::Z);
            b.aimed = if camera.forward().dot(at_it) > 4f32.to_radians().cos() { b.aimed + dt } else { 0.0 };
            let passes = |a: Vec3, z: Vec3| ground.passes(a + Vec3::Y * 1.2, z + Vec3::Y * 1.2, 0.4);
            // Blinking aside, gone and there at once. (Not while its
            // strands are out: blinking, it broke them, the user. It stands
            // to its cast; startled as it gathers, it blinks, and the
            // gathering is broken.)
            let startled = sees && b.casting.is_none() && (shot || b.aimed > 0.5 || b.struck || r(9) < dt * 0.15);
            if startled && b.time >= b.next_blink {
                let across = Vec3::new(-to.z, 0.0, to.x).normalize_or(Vec3::X);
                let to_spot = |k: i32| {
                    let side = if r(10 + k) < 0.5 { -1.0 } else { 1.0 };
                    let far = BLINK.0 + (BLINK.1 - BLINK.0) * r(20 + k);
                    floor_near(&ground, nav.as_deref(), b.at + across * side * far)
                };
                if let Some(to) = (0..6).map(to_spot).find(|&q| q.distance(b.at) > BLINK.0 * 0.7 && passes(b.at, q)) {
                    let by = to - b.at;
                    // Afterimages along the way, the nearest it left the
                    // longest.
                    // (Its meshes are in its own frame, carried.)
                    let frame = Transform::from_translation(b.top).with_rotation(b.place);
                    for k in 0..4 {
                        let f = (k as f32 + 0.5) / 4.0;
                        let material = materials.add(StandardMaterial { base_color: Color::srgba(0.03, 0.03, 0.03, 0.55), alpha_mode: AlphaMode::Blend, double_sided: true, cull_mode: None, unlit: true, ..default() });
                        let life = 0.18 + 0.3 * (1.0 - f);
                        for handle in [&b.mesh, &b.cape_mesh, &b.sleeves[0].1, &b.sleeves[1].1] {
                            if let Some(mesh) = meshes.get(handle).cloned() {
                                commands.spawn((Ghost { life, total: life, material: material.clone() }, Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), frame.with_translation(frame.translation + by * f), bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster));
                            }
                        }
                    }
                    commands.spawn((AudioPlayer::new(assets.bishop_blink.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.6)), Transform::from_translation(to + Vec3::Y * SHOULDERS)));
                    b.at = to;
                    b.goal = to;
                    if args.opt("huntlog") {
                        info!("huntlog: bishop {} blinks {:.1} m aside ({}), {:.1} m from you", entity.index_u32(), by.length(), if shot { "shot at" } else if b.struck { "hit" } else if b.aimed > 0.5 { "aimed at" } else { "of itself" }, Vec2::new(to.x - eye.x, to.z - eye.z).length());
                    }
                    b.next_blink = b.time + 1.2 + 1.3 * r(5);
                    // (Startled, its gathering broken: shoot it as it gathers.)
                    if b.charge.take().is_some() {
                        b.next_cast = b.time + 1.5;
                    }
                    b.aimed = 0.0;
                }
            }
            b.struck = false;
            // Drifting from spot to spot a little way off, you in sight from
            // there, its body getting there: suddenly another, now and then.
            if b.time >= b.next_goal || Vec2::new(b.goal.x - b.at.x, b.goal.z - b.at.z).length() < 0.6 {
                b.next_goal = b.time + 0.8 + 1.4 * r(6);
                let spot = |k: i32| {
                    let a = r(30 + k) * std::f32::consts::TAU;
                    let q = floor_near(&ground, nav.as_deref(), b.at + Vec3::new(a.cos(), 0.0, a.sin()) * (2.0 + 3.0 * r(40 + k)));
                    let far = Vec2::new(q.x - eye.x, q.z - eye.z).length();
                    (q, far)
                };
                // (Failing one in its range, you in sight from it, the one
                // it gets to that is nearest its range: any it got to, it
                // drifted off, 40 m and more.)
                let off = |far: f32| (RANGE.0 - far).max(far - RANGE.1).max(0.0);
                let ideal = (0..10).map(spot).find(|&(q, far)| (RANGE.0..RANGE.1).contains(&far) && passes(b.at, q) && ground.clear(q + Vec3::Y * 2.6, eye));
                let nearest = || (0..10).map(|k| spot(50 + k)).filter(|&(q, _)| passes(b.at, q)).min_by(|a, b| off(a.1).total_cmp(&off(b.1)));
                if let Some((q, _)) = ideal.or_else(nearest) {
                    b.goal = q;
                }
            }
            let want = (b.goal - b.at).with_y(0.0);
            let want = want.normalize_or(Vec3::ZERO) * DRIFT * (want.length() / 1.0).min(1.0);
            b.velocity += (want - b.velocity).clamp_length_max(8.0 * dt);
            let next = b.at + b.velocity * dt;
            if passes(b.at, next) && ground.under(next).is_some() {
                b.at = Vec3::new(next.x, ground.at(next), next.z);
            } else {
                b.velocity = Vec3::ZERO;
                b.next_goal = b.time;
            }
            let turn = (facing - b.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            b.heading += turn.clamp(-3.0 * dt, 3.0 * dt);
            // Its cast: seeing you, now and then, it stops still, its light
            // gathering at its hands; then three strands of lightning run
            // from it to you, joined to it till they strike (or it blinks).
            let far = Vec2::new(to.x, to.z).length();
            if let Some(left) = b.casting.as_mut() {
                b.velocity *= (-dt * 8.0).exp();
                *left -= dt;
                if *left <= 0.0 {
                    b.casting = None;
                }
            } else if let Some(c) = b.charge.as_mut() {
                *c += dt;
                b.velocity *= (-dt * 8.0).exp();
                if *c >= GATHER {
                    b.charge = None;
                    b.casting = Some(CASTING);
                    let from = b.top + Vec3::Y * 0.06 + Quat::from_rotation_y(b.heading) * LIGHT;
                    let dir = ((eye - Vec3::Y * 0.35 - from).normalize_or(Vec3::Z) + Vec3::Y * 0.15).normalize();
                    for k in 0..STRANDS {
                        let material = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(BOLT_GLOW, BOLT_GLOW, BOLT_GLOW), double_sided: true, cull_mode: None, ..default() });
                        let light = commands.spawn((PointLight { intensity: BOLT_LIGHT.0, range: BOLT_LIGHT.1, shadow_maps_enabled: false, ..default() }, Transform::from_translation(from))).id();
                        // (Never empty: an empty mesh upset the renderer.)
                        let (first, first_twin) = bolt_mesh(&[from, from + dir * 0.05], eye, 0, BOLT_WIDTH, 0.0);
                        let twin_mesh = meshes.add(first_twin);
                        let twin_material = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Blend, double_sided: true, cull_mode: None, ..default() });
                        let twin = commands.spawn((Mesh3d(twin_mesh.clone()), MeshMaterial3d(twin_material.clone()), Transform::IDENTITY, bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster)).id();
                        // Its own way: from somewhere on the orb's front
                        // half (the user: from anywhere, in a tunnel, they
                        // ran into its walls), straight out from it.
                        let q = |j: i32| hash01(entity.index_u32() as i32, (b.time * 10.0) as i32, 90 + k as i32 * 16 + j, 0x8b5);
                        let (z, a) = (2.0 * q(0) - 1.0, q(1) * std::f32::consts::TAU);
                        let out = Vec3::new((1.0 - z * z).sqrt() * a.cos(), z, (1.0 - z * z).sqrt() * a.sin());
                        let out = if out.dot(dir) < 0.0 { out - dir * 2.0 * out.dot(dir) } else { out };
                        let start = from + out * ORB;
                        let range = |(lo, hi): (f32, f32), j: i32| lo + (hi - lo) * q(j);
                        commands.spawn((
                            Bolt {
                                owner: entity,
                                passed: false,
                                hurt: false,
                                centre: start,
                                velocity: out,
                                speed: range(BOLT_SPEED, 2),
                                phase: q(3) * std::f32::consts::TAU,
                                spin: range(SPIN, 4) * if q(5) < 0.5 { -1.0 } else { 1.0 },
                                spiral: range(SPIRAL, 6),
                                wander: Vec3::new(range(WANDER.1, 7), range(WANDER.1, 8), range(WANDER.1, 9)),
                                wander_at: Vec3::new(q(10), q(11), q(12)) * std::f32::consts::TAU,
                                age: 0.0,
                                path: vec![from, start],
                                tip: start,
                                struck: None,
                                material: material.clone(),
                                light,
                                twin,
                                twin_mesh,
                                twin_material,
                                seed: entity.index_u32() as i32 * 7 + k as i32,
                                roll: 0,
                                next_roll: 0.0,
                            },
                            Mesh3d(meshes.add(first)),
                            MeshMaterial3d(material),
                            Transform::IDENTITY,
                            bevy::camera::visibility::NoFrustumCulling,
                            bevy::light::NotShadowCaster,
                        ));
                    }
                    b.next_cast = b.time + BETWEEN.0 + (BETWEEN.1 - BETWEEN.0) * r(70);
                    if args.opt("huntlog") {
                        info!("huntlog: bishop {} casts, {:.1} m from you", entity.index_u32(), far);
                    }
                }
            } else if sees && b.time >= b.next_cast && (CAST_RANGE.0..CAST_RANGE.1).contains(&far) {
                b.charge = Some(0.0);
                // (A swell, from the gathering to its peak as its motes are
                // loosed, a second on, and fading.)
                commands.spawn((AudioPlayer::new(assets.bishop_cast.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.76)), Transform::from_translation(b.top)));
            }
        }
        let place = Quat::from_rotation_y(b.heading);
        let broken = b.broken.is_some();
        let floor = b.at.y;
        // It floats, rising and falling a little.
        let top = Vec3::new(b.at.x, floor + SHOULDERS + 0.04 * (b.time * 1.3).sin(), b.at.z);
        b.top = top;
        b.place = place;
        // Alive, its cloth frozen in its drape, carried (see `carry`).
        // Broken: put where its drape is, simulated again, limp, to fall.
        if !broken {
            continue;
        }
        if let Some(drape) = b.drape.take() {
            b.cloth.set(|i| top + place * drape[0][i]);
            b.cape.set(|i| top + place * drape[1][i]);
            b.sleeves[0].0.set(|i| top + place * drape[2][i]);
            b.sleeves[1].0.set(|i| top + place * drape[3][i]);
        }
        let step = dt / 2.0;
        for (cloth, handle) in [(&mut b.cloth, &b.mesh), (&mut b.cape, &b.cape_mesh)] {
            for _ in 0..2 {
                cloth.step(&[], &[], floor + 0.02, top.y + 0.9, step);
            }
            if let Some(mut mesh) = meshes.get_mut(handle) {
                cloth.draw(&mut mesh);
            }
        }
        for (sleeve, handle) in &mut b.sleeves {
            for _ in 0..2 {
                sleeve.step(&[], &[], floor + 0.02, top.y + 0.9, step);
            }
            if let Some(mut mesh) = meshes.get_mut(&*handle) {
                sleeve.draw(&mut mesh);
            }
        }
    }
}

/// Its stiff parts follow its shoulders; so do its cloth meshes while its
/// cloth is frozen (else they are where the cloth is: where they stand).
pub(super) fn carry(bishops: Query<&Bishop>, mut stiff: Query<(&OfBishop, &Stiff, &mut Transform), Without<Drape>>, mut cloths: Query<(&OfBishop, &mut Transform), (With<Drape>, Without<Stiff>)>) {
    for (of, part, mut transform) in &mut stiff {
        let Ok(b) = bishops.get(of.0) else { continue };
        transform.translation = b.top + Vec3::Y * 0.06 + b.place * part.offset;
        transform.rotation = b.place * part.turn;
    }
    for (of, mut transform) in &mut cloths {
        let Ok(b) = bishops.get(of.0) else { continue };
        *transform = if b.drape.is_some() { Transform::from_translation(b.top).with_rotation(b.place) } else { Transform::default() };
    }
}

/// Its light grows as it gathers and holds as it casts, flickering; none
/// else.
pub(super) fn gleam(time: Res<Time>, bishops: Query<&Bishop>, mut lights: Query<(&OfBishop, &mut Transform, &mut PointLight), With<Gleam>>) {
    for (of, mut transform, mut light) in &mut lights {
        let Ok(b) = bishops.get(of.0) else { continue };
        let k = if b.casting.is_some() { 1.0 } else { b.charge.map_or(0.0, |c| (c / GATHER).min(1.0)) };
        let flicker = 1.0 + 0.15 * (time.elapsed_secs() * 37.0).sin() * k;
        transform.scale = Vec3::splat(0.13 * k.powf(0.7) * flicker);
        light.intensity = GLEAM_LIGHT.0 * k * k * flicker;
    }
}

/// Its strands run: each tip on, turning towards you (no faster than
/// `BOLT_TURN`: step aside and it passes), circling its way; all of it joined
/// to the orb, drawn anew now and then, kinked and forked, shimmering. On
/// you, it hurts you (and shoves, and jolts the view) and holds a moment; on
/// what is built, it bursts; then (or if its bishop blinks or breaks) it
/// fades.
#[allow(clippy::too_many_arguments)]
pub(super) fn bolts(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    assets: Res<Assets3>,
    mut director: ResMut<Director>,
    mut gun: ResMut<Gun>,
    mut player: Single<(&Transform, &mut Player), Without<Bolt>>,
    mut bishops: Query<(Entity, &mut Bishop)>,
    mut bolts: Query<(Entity, &mut Bolt, &Mesh3d)>,
    mut lights: Query<(&mut Transform, &mut PointLight), (Without<Bolt>, Without<Player>, Without<FlyCam>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs().min(0.05);
    let eye = player.0.translation;
    let chest = eye - Vec3::Y * 0.35;
    // (The eye is the player's: the camera and the player are one.)
    let view = eye;
    // (Which bishops still have strands running: the rest are done casting,
    // free to move again (the user: idle as its strands held and faded, a
    // sitting duck), their orbs out, the strands fading on their own.)
    let mut live: Vec<Entity> = Vec::new();
    for (entity, mut m, mesh) in &mut bolts {
        let m = &mut *m;
        m.age += dt;
        // Its orb, where it is now (its bishop gone or broken: it fades).
        let orb = bishops.get(m.owner).ok().map(|(_, b)| b).filter(|b| b.broken.is_none()).map(|b| b.top + Vec3::Y * 0.06 + Quat::from_rotation_y(b.heading) * LIGHT);
        if orb.is_none() && m.struck.is_none() {
            m.struck = Some(BOLT_HOLD);
        }
        if let Some(orb) = orb {
            m.path[0] = orb;
        }
        match m.struck.as_mut() {
            None => {
                // Its way turning towards you, wandering, no faster than it
                // can: curling out from the orb at first, slow, then on.
                let heading = m.velocity.normalize_or(Vec3::Z);
                let w = m.wander_at + m.wander * m.age;
                let wander = Vec3::new(w.x.sin(), w.y.sin(), w.z.sin());
                let wander = (wander - heading * wander.dot(heading)) * WANDER.0;
                // (Past you, straight on: a shot, not a reach.)
                if m.age > 0.3 && (chest - m.centre).dot(heading) < 0.0 {
                    m.passed = true;
                }
                let want = if m.passed { heading } else { ((chest - m.centre).normalize_or(heading) + wander).normalize_or(heading) };
                let angle = heading.angle_between(want);
                let early = (m.age / 0.6).min(1.0);
                let rate = BOLT_CURL + (BOLT_TURN - BOLT_CURL) * early;
                let turned = if angle > 1e-4 { Quat::IDENTITY.slerp(Quat::from_rotation_arc(heading, want), (rate * dt / angle).min(1.0)) * heading } else { heading };
                // (Slow at first, curling tight: fast, they curled wide and
                // struck the walls of a tunnel before they turned.)
                m.velocity = turned * m.speed * (0.15 + 0.85 * (m.age / 0.5).min(1.0));
                m.centre += m.velocity * dt;
                // Round its way, its own spiral: out over its first metres,
                // in again on you over its last.
                let across = turned.cross(Vec3::Y).normalize_or(Vec3::X);
                let up = across.cross(turned);
                let a = m.phase + m.age * m.spin;
                let radius = if m.passed { 0.0 } else { m.spiral * (m.age / 0.3).min(1.0) * (m.centre.distance(chest) / CLOSING).min(1.0) };
                let next = m.centre + (across * a.cos() + up * a.sin()) * radius;
                // (Over the floor, crawling along it if it dips that low:
                // dipping on its curls, it struck the steps of a stair.)
                let floor = ground.at(next) + 0.15;
                let next = next.with_y(next.y.max(floor));
                if m.centre.y < floor {
                    m.centre.y = floor;
                }
                let prev = m.tip;
                // Its tip's step (fast, a step can be a metre), or anywhere
                // along it behind (the user: the whole strand hurts), on
                // anywhere of your body.
                let (feet, head) = (eye - Vec3::Y * BODY.0, eye + Vec3::Y * BODY.1);
                let hit_you = between(prev, next, feet, head) < BODY.2 || touches(&m.path, prev, feet, head);
                // (Not at first: curling out, crawling over a wall near it.)
                let blocked = m.age > 0.4 && !ground.clear(prev, next);
                if hit_you || blocked || m.age > BOLT_LIFE {
                    m.struck = Some(0.0);
                    sparks(&mut commands, &assets, prev, -turned, entity.index_u32());
                    commands.spawn((Flash { life: FLASH.2 }, PointLight { intensity: FLASH.0, range: FLASH.1, shadow_maps_enabled: false, ..default() }, Transform::from_translation(prev)));
                    if args.opt("huntlog") {
                        info!("huntlog: a strand {} ({:.1} s, {:.1} m from you)", if hit_you { "strikes you" } else if blocked { "strikes what is built" } else { "spent" }, m.age, next.distance(chest));
                    }
                    if hit_you {
                        m.tip = chest;
                        m.hurt = true;
                        let right = *player.0.right();
                        shock(&mut commands, &assets, &mut player.1, right, &mut director, &mut gun, turned);
                    } else {
                        commands.spawn((AudioPlayer::new(assets.mote_burst.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.2)), Transform::from_translation(prev)));
                    }
                } else {
                    m.tip = next;
                    if m.path.last().is_none_or(|p| p.distance(next) > BOLT_KINK.0 * 1.5) {
                        m.path.push(next);
                    }
                }
            }
            Some(since) => {
                // Struck (or broken): holding on you, then fading. Struck
                // on what is built, it still hurts as it holds, bright (the
                // user crossed strands far from their tips, harmless: they
                // had struck a wall).
                *since += dt;
                if m.tip.distance(chest) < 0.8 {
                    m.tip = chest;
                }
                if !m.hurt && *since < BOLT_HOLD && touches(&m.path, m.tip, eye - Vec3::Y * BODY.0, eye + Vec3::Y * BODY.1) {
                    m.hurt = true;
                    let along = (m.tip - m.path[0]).normalize_or(Vec3::Z);
                    sparks(&mut commands, &assets, chest, -along, entity.index_u32());
                    if args.opt("huntlog") {
                        info!("huntlog: a strand, struck, holding, strikes you");
                    }
                    let right = *player.0.right();
                    shock(&mut commands, &assets, &mut player.1, right, &mut director, &mut gun, along);
                }
                if *since > BOLT_HOLD + BOLT_FADE {
                    commands.entity(m.light).despawn();
                    commands.entity(m.twin).despawn();
                    commands.entity(entity).despawn();
                    continue;
                }
            }
        }
        if m.struck.is_none() {
            live.push(m.owner);
        }
        let fade = m.struck.map_or(1.0, |s| 1.0 - ((s - BOLT_HOLD) / BOLT_FADE).clamp(0.0, 1.0));
        // Drawn anew now and then (shimmering), and flickering.
        m.next_roll -= dt;
        if m.next_roll <= 0.0 {
            m.next_roll += BOLT_EVERY;
            m.roll += 1;
            let mut points = m.path.clone();
            points.push(m.tip);
            let (strand, twin) = bolt_mesh(&points, view, m.seed * 7919 + m.roll, BOLT_WIDTH, m.seed as f32 + m.age * TWIN_TWIST.1);
            if let Some(mut built) = meshes.get_mut(&mesh.0) {
                *built = strand;
            }
            if let Some(mut built) = meshes.get_mut(&m.twin_mesh) {
                *built = twin;
            }
        }
        if let Some(mut black) = materials.get_mut(&m.twin_material) {
            black.base_color.set_alpha(fade);
        }
        let flicker = 0.6 + 0.8 * hash01(m.seed, m.roll, 0, 0x6d4);
        if let Some(mut glow) = materials.get_mut(&m.material) {
            glow.emissive = LinearRgba::rgb(BOLT_GLOW, BOLT_GLOW, BOLT_GLOW) * flicker * fade;
        }
        if let Ok((mut at, mut light)) = lights.get_mut(m.light) {
            at.translation = m.tip;
            light.intensity = BOLT_LIGHT.0 * flicker * fade;
        }
    }
    // (A cast just begun has no strands yet: they are spawned at its end.)
    for (entity, mut b) in &mut bishops {
        if b.casting.is_some_and(|left| left < CASTING - 0.2) && !live.contains(&entity) {
            b.casting = None;
        }
    }
}

/// Flashes fade fast and are gone.
pub(super) fn flashes(mut commands: Commands, time: Res<Time>, mut flashes: Query<(Entity, &mut Flash, &mut PointLight)>) {
    for (entity, mut flash, mut light) in &mut flashes {
        flash.life -= time.delta_secs();
        if flash.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        light.intensity = FLASH.0 * (flash.life / FLASH.2).powi(2);
    }
}

/// A burst's flash, fading.
#[derive(Component)]
pub(super) struct Flash {
    life: f32,
}

/// Afterimages fade and are gone.
pub(super) fn ghosts(mut commands: Commands, time: Res<Time>, mut materials: ResMut<Assets<StandardMaterial>>, mut ghosts: Query<(Entity, &mut Ghost)>) {
    for (entity, mut ghost) in &mut ghosts {
        ghost.life -= time.delta_secs();
        if ghost.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut m) = materials.get_mut(&ghost.material) {
            m.base_color.set_alpha(0.55 * ghost.life / ghost.total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How near two segments come: crossing apart, parallel, and end to
    /// end.
    #[test]
    fn segments_between() {
        let d = between(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.5), Vec3::new(0.0, 1.0, 0.5));
        assert!((d - 0.5).abs() < 1e-5, "{d}");
        let d = between(Vec3::ZERO, Vec3::X, Vec3::new(0.0, 0.3, 0.0), Vec3::new(1.0, 0.3, 0.0));
        assert!((d - 0.3).abs() < 1e-5, "{d}");
        let d = between(Vec3::ZERO, Vec3::X, Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0));
        assert!((d - 1.0).abs() < 1e-5, "{d}");
    }

    /// A strand touches you anywhere along it, the last stretch to its tip
    /// too; not a body's width and more off.
    #[test]
    fn strand_touches() {
        let path = [Vec3::new(0.0, 1.0, 0.0), Vec3::new(2.0, 1.0, 0.0), Vec3::new(4.0, 1.0, 0.0)];
        let tip = Vec3::new(6.0, 1.0, 0.0);
        let body = |x: f32, z: f32| (Vec3::new(x, 0.3, z), Vec3::new(x, 1.9, z));
        let (f, h) = body(1.0, 0.4);
        assert!(touches(&path, tip, f, h));
        let (f, h) = body(5.0, 0.4);
        assert!(touches(&path, tip, f, h));
        let (f, h) = body(3.0, 0.8);
        assert!(!touches(&path, tip, f, h));
    }

    /// The drape kept as data fits the bishop's cloth as it is cut (else:
    /// keep it again with `--opt bakedrape`), and reads back as it was kept.
    #[test]
    fn kept_drape_fits() {
        let kept = decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/bishop_drape.bin"))).expect("data/bishop_drape.bin does not fit the cloth as cut");
        assert_eq!(decode(&encode(&kept)), Some(kept));
    }
}
