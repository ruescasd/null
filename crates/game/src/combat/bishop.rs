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
/// near and far you may be; its motes: how many, how fast (m/s), how fast
/// they turn towards you (rad/s), how long they fly (s), what each does to
/// you, how big (m).
const GATHER: f32 = 1.0;
const BETWEEN: (f32, f32) = (4.0, 6.0);
const CAST_RANGE: (f32, f32) = (4.0, 24.0);
const MOTES: usize = 3;
const MOTE_SPEED: f32 = 10.0;
const MOTE_TURN: f32 = 1.8;
const MOTE_LIFE: f32 = 5.0;
const MOTE_HARM: f32 = 12.0;
const MOTE_SIZE: f32 = 0.12;
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
    /// Gathering its light (s into it), and when it may cast again.
    charge: Option<f32>,
    next_cast: f32,
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
        let base = at - dir * NEEDLE_OUT * NEEDLE_SCALE;
        commands.spawn((
            OfBishop(owner),
            Stiff { offset: to * (base - self.top - Vec3::Y * 0.06), turn: to * Quat::from_rotation_arc(Vec3::Y, dir), dark: true },
            Needle { delay, life: NEEDLE_LIFE, size: NEEDLE_SIZE * NEEDLE_SCALE },
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

/// A mote it loosed: how fast it flies, how long it has left; shot (to
/// burst).
#[derive(Component)]
pub(super) struct Mote {
    velocity: Vec3,
    life: f32,
    pub(super) popped: bool,
}

/// A mote's trail, shrinking away.
#[derive(Component)]
pub(super) struct Ember {
    life: f32,
}

/// The motes' mesh and their glow.
#[derive(Resource)]
pub(super) struct MoteKit {
    mesh: Handle<Mesh>,
    glow: Handle<StandardMaterial>,
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
pub(super) fn settle(mut commands: Commands, args: Res<Args>, mut exit: MessageWriter<AppExit>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // (Its motes' glow: the shards' white heat.)
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(60.0, 60.0, 60.0), ..default() });
    commands.insert_resource(MoteKit { mesh: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), glow });
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
    commands.spawn((OfBishop(owner), Gleam, Stiff { offset: LIGHT, turn: Quat::IDENTITY, dark: true }, Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap())), MeshMaterial3d(assets.bright.clone()), Transform::from_scale(Vec3::ZERO), bevy::light::NotShadowCaster));
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
    kit: Res<MoteKit>,
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
            // Blinking aside, gone and there at once.
            let startled = sees && (shot || b.aimed > 0.5 || b.struck || r(9) < dt * 0.15);
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
            // gathering at its chest; then its motes are loosed at you,
            // fanned out, to home in.
            let far = Vec2::new(to.x, to.z).length();
            if let Some(c) = b.charge.as_mut() {
                *c += dt;
                b.velocity *= (-dt * 8.0).exp();
                if *c >= GATHER {
                    b.charge = None;
                    b.next_cast = b.time + BETWEEN.0 + (BETWEEN.1 - BETWEEN.0) * r(70);
                    let from = b.top + Vec3::Y * 0.06 + Quat::from_rotation_y(b.heading) * LIGHT;
                    let aim = (eye - Vec3::Y * 0.35 - from).normalize_or(Vec3::Z);
                    for k in 0..MOTES {
                        let fan = (k as f32 - (MOTES - 1) as f32 * 0.5) * 0.55;
                        let dir = (Quat::from_rotation_y(fan) * aim + Vec3::Y * 0.3).normalize();
                        commands.spawn((Mote { velocity: dir * MOTE_SPEED, life: MOTE_LIFE, popped: false }, Mesh3d(kit.mesh.clone()), MeshMaterial3d(kit.glow.clone()), Transform::from_translation(from).with_scale(Vec3::splat(MOTE_SIZE)), bevy::light::NotShadowCaster));
                    }
                    if args.opt("huntlog") {
                        info!("huntlog: bishop {} casts, {:.1} m from you", entity.index_u32(), far);
                    }
                }
            } else if sees && b.time >= b.next_cast && (CAST_RANGE.0..CAST_RANGE.1).contains(&far) {
                b.charge = Some(0.0);
                // (A swell, from the gathering to its peak as its motes are
                // loosed, a second on, and fading.)
                commands.spawn((AudioPlayer::new(assets.bishop_cast.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.6)), Transform::from_translation(b.top)));
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

/// Its light grows as it gathers, flickering; none else.
pub(super) fn gleam(time: Res<Time>, bishops: Query<&Bishop>, mut lights: Query<(&OfBishop, &mut Transform), With<Gleam>>) {
    for (of, mut transform) in &mut lights {
        let Ok(b) = bishops.get(of.0) else { continue };
        let k = b.charge.map_or(0.0, |c| (c / GATHER).min(1.0));
        let flicker = 1.0 + 0.15 * (time.elapsed_secs() * 37.0).sin() * k;
        transform.scale = Vec3::splat(0.13 * k.powf(0.7) * flicker);
    }
}

/// Motes fly, turning towards you (no faster than `MOTE_TURN`: step aside
/// and they pass), leaving a trail; on what is built, or shot, or spent,
/// they burst; on you, they hurt you (and shove, and jolt the view).
#[allow(clippy::too_many_arguments)]
pub(super) fn motes(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    assets: Res<Assets3>,
    kit: Res<MoteKit>,
    mut director: ResMut<Director>,
    mut gun: ResMut<Gun>,
    mut player: Single<(&Transform, &mut Player), Without<Mote>>,
    mut motes: Query<(Entity, &mut Mote, &mut Transform), (Without<Player>, Without<Ember>)>,
    mut embers: Query<(Entity, &mut Ember, &mut Transform), (Without<Player>, Without<Mote>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let eye = player.0.translation;
    let chest = eye - Vec3::Y * 0.35;
    for (entity, mut m, mut transform) in &mut motes {
        let p = transform.translation;
        m.life -= dt;
        // Turning towards you, no faster than it can.
        let speed = m.velocity.length();
        let heading = m.velocity / speed.max(1e-4);
        let want = (chest - p).normalize_or(heading);
        let angle = heading.angle_between(want);
        let turned = if angle > 1e-4 { Quat::IDENTITY.slerp(Quat::from_rotation_arc(heading, want), (MOTE_TURN * dt / angle).min(1.0)) * heading } else { heading };
        m.velocity = turned * MOTE_SPEED;
        let next = p + m.velocity * dt;
        let hit_you = p.distance(chest) < 0.6 || next.distance(chest) < 0.6;
        let blocked = !ground.clear(p, next) || next.y < ground.at(next);
        if hit_you || blocked || m.popped || m.life <= 0.0 {
            sparks(&mut commands, &assets, p, -heading, entity.index_u32());
            if args.opt("huntlog") {
                info!("huntlog: a mote {}", if hit_you { "strikes you" } else if m.popped { "shot down" } else if blocked { "bursts on what is built" } else { "spent" });
            }
            if hit_you {
                player.1.health -= MOTE_HARM;
                player.1.velocity += heading.with_y(0.0).normalize_or(Vec3::ZERO) * 4.0 + Vec3::Y * 1.5;
                director.hurt = 1.0;
                gun.jolt += Vec2::new(0.06, -heading.dot(*player.0.right()) * 0.08);
                commands.spawn((AudioPlayer::new(assets.shatter.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.7)).with_speed(1.4)));
            } else {
                commands.spawn((AudioPlayer::new(assets.shatter.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(0.6)).with_speed(1.5), Transform::from_translation(p)));
            }
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = next;
        // Its trail.
        commands.spawn((Ember { life: 0.25 }, Mesh3d(kit.mesh.clone()), MeshMaterial3d(kit.glow.clone()), Transform::from_translation(p).with_scale(Vec3::splat(MOTE_SIZE * 0.7)), bevy::light::NotShadowCaster));
    }
    for (entity, mut e, mut transform) in &mut embers {
        e.life -= dt;
        if e.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = Vec3::splat(MOTE_SIZE * 0.7 * e.life / 0.25);
    }
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

    /// The drape kept as data fits the bishop's cloth as it is cut (else:
    /// keep it again with `--opt bakedrape`), and reads back as it was kept.
    #[test]
    fn kept_drape_fits() {
        let kept = decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/bishop_drape.bin"))).expect("data/bishop_drape.bin does not fit the cloth as cut");
        assert_eq!(decode(&encode(&kept)), Some(kept));
    }
}
