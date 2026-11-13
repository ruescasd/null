//! A tumbler's other shot (`attack=2`; the user, 2026-11-13: its rhombi
//! weak, an experiment after its theme): black lightning through the
//! ground. A thick jagged crack grows out from its feet along the ground
//! toward you, all one width, quicker as it goes, bending after you a
//! little (`seek=`); its tip crossing under your feet, it hurts you. Run
//! across its line and it misses you; walk, and it is on you; in the air
//! over it, it passes under you. It climbs steps, stops at a wall or an
//! edge over the void. A few moments after it set off (`lasts=`: in
//! effect its range), all of it is gone at once (the user, 2026-11-13:
//! growing, not travelling, uniform, not a head and a tail). Drawn
//! darkening the ground (multiplied: black at its middle, nothing at its
//! edge), its zigzag and its forks laid as it grows and kept: what it
//! has grown never changes (the user, 2026-11-13: flickering all along,
//! it vibrated).

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::camera::FlyCam;

/// How fast its tip grows as it sets off and at most (m/s), how quickly it
/// gathers (m/s²).
const START: f32 = 6.0;
const GATHER: f32 = 30.0;
const TOP: f32 = 18.0;
/// How long it lasts, by default (s): growing all the while, then gone
/// (about 26 m).
pub(super) const LASTS: f32 = 1.6;
/// How quickly it bends after you, by default (rad/s): slower than your
/// bearing swings running across it 15 m off (0.67), quicker than walking.
pub(super) const SEEK: f32 = 0.5;
/// How near its tip your feet hurt (m, across: its own half width and
/// yours), and how high over it they are clear (m).
const HIT: f32 = 1.2;
const CLEAR: f32 = 0.7;
const HARM: f32 = 20.0;
/// Its marks along the ground (m apart: its zigzag's corners), how far off
/// its line the ground's slope across it is felt (m), the most it rises
/// from one to the next (m: more, a wall).
const MARK: f32 = 1.2;
const SLOPE: f32 = 0.8;
const CLIMB: f32 = 1.9;
/// Its width: black at its middle (half width, m), darkening the ground
/// out to its edge (m); how high over the ground it is drawn (m).
/// (Its edge crisp: wider, soft, it read as a shadow's puddle, not
/// lightning.)
const CORE: f32 = 0.3;
const EDGE: f32 = 0.42;
const LIFT: f32 = 0.05;
/// Its strands, by default (the user, 2026-11-13: several thin ones, not one
/// wide one; 1 that one): each its width (as above), spread evenly across
/// (m off its line, the outermost), each zigzagging its own way about the
/// line they share (m off their place at most), crossing.
pub(super) const STRANDS: u32 = 2;
const STRAND: (f32, f32) = (0.07, 0.12);
const SPREAD: f32 = 0.4;
const STRAND_JAG: f32 = 0.4;
/// Its zigzag (m off its line at most), its forks (chance at a mark, for
/// each strand of one, length (m), width (half, core and edge, m)).
const JAG: f32 = 0.6;
const FORK: f32 = 0.3;
const FORK_LENGTH: (f32, f32) = (1.2, 3.0);
const FORK_WIDTH: (f32, f32) = (0.06, 0.13);
/// Striking you: its forks flung all round (how many).
const STRIKE_FORKS: u32 = 8;

/// A mark of its way on the ground: where (its line), across it (level),
/// the ground's slope across (rise per metre).
#[derive(Clone, Copy, Debug)]
struct Mark {
    at: Vec3,
    side: Vec3,
    slope: f32,
}

/// A crack growing through the ground: its tip (on the ground), which way
/// (level), how fast, how quickly it bends after you; its marks; how far it
/// has grown; its strands; when it set off and how long it lasts (s); whether it has
/// stopped growing (struck you, met a wall or the void, its time up) and
/// whether it struck you; its seed, its mesh and the entity drawing it (at
/// `origin`).
#[derive(Component)]
pub(in crate::combat) struct Crack {
    head: Vec3,
    way: Vec3,
    speed: f32,
    seek: f32,
    strands: u32,
    marks: Vec<Mark>,
    run: f32,
    born: f32,
    lasts: f32,
    stopped: bool,
    struck: bool,
    seed: u32,
    origin: Vec3,
    mesh: Handle<Mesh>,
}

/// What a crack's frame came to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ran {
    On,
    Struck,
    Ended,
}

impl Crack {
    fn new(from: Vec3, to: Vec3, (seek, lasts, strands): (f32, f32, u32), seed: u32, now: f32, ground: &dyn Fn(Vec3) -> Option<f32>) -> Crack {
        let way = (to - from).with_y(0.0).normalize_or(Vec3::Z);
        let mut c = Crack { head: from, way, speed: START, seek, strands: strands.max(1), marks: Vec::new(), run: 0.0, born: now, lasts, stopped: false, struck: false, seed: seed | 1, origin: from, mesh: Handle::default() };
        c.mark(from, ground);
        c
    }

    fn mark(&mut self, at: Vec3, ground: &dyn Fn(Vec3) -> Option<f32>) {
        let side = Vec3::new(-self.way.z, 0.0, self.way.x);
        let (l, r) = (ground((at - side * SLOPE).with_y(at.y + 1.0)), ground((at + side * SLOPE).with_y(at.y + 1.0)));
        // (A side over the void, or far off its line's height (a wall, a
        // drop), felt as level.)
        let slope = match (l, r) {
            (Some(l), Some(r)) if (l - at.y).abs() < 0.5 && (r - at.y).abs() < 0.5 => (r - l) / (2.0 * SLOPE),
            _ => 0.0,
        };
        self.marks.push(Mark { at, side, slope });
    }

    /// Grow a frame (`dt`): bent after your feet (`feet`), gathering speed,
    /// marking the ground as it goes; its tip under your feet, it struck
    /// you.
    fn step(&mut self, dt: f32, now: f32, feet: Vec3, ground: &dyn Fn(Vec3) -> Option<f32>) -> Ran {
        if self.stopped || now - self.born >= self.lasts {
            self.stopped = true;
            return Ran::Ended;
        }
        let to = (feet - self.head).with_y(0.0);
        if to.length() > 1e-3 {
            let want = to.normalize();
            let off = self.way.cross(want).y.atan2(self.way.dot(want));
            let turn = off.clamp(-self.seek * dt, self.seek * dt);
            self.way = (Quat::from_rotation_y(turn) * self.way).with_y(0.0).normalize_or(want);
        }
        self.speed = (self.speed + GATHER * dt).min(TOP);
        let from = self.head;
        let step = self.way * self.speed * dt;
        // Its marks every `MARK` along the way, each on the ground: past a
        // wall or over the void, it stops there.
        let mut next = (self.run / MARK).floor() * MARK + MARK;
        let mut last = from;
        let mut ended = false;
        while next <= self.run + step.length() {
            let p = from + step * ((next - self.run) / step.length());
            match ground(p.with_y(last.y + CLIMB)) {
                Some(h) if h - last.y <= CLIMB => {
                    let at = p.with_y(h);
                    self.mark(at, ground);
                    last = at;
                }
                _ => {
                    ended = true;
                    break;
                }
            }
            next += MARK;
        }
        let head = (from + step).with_y(last.y);
        let head = match ground(head.with_y(last.y + CLIMB)) {
            Some(h) if !ended && h - last.y <= CLIMB => head.with_y(h),
            _ => {
                ended = true;
                last
            }
        };
        // (Your feet: nearest the way it grew this frame, level.)
        let (a, b) = (from.with_y(0.0), head.with_y(0.0));
        let ab = b - a;
        let along = ((feet.with_y(0.0) - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
        let near = (a + ab * along).distance(feet.with_y(0.0));
        let over = feet.y - from.y.lerp(head.y, along);
        self.run += (head - from).with_y(0.0).length();
        self.head = head;
        if near < HIT && over > -1.0 && over < CLEAR {
            self.stopped = true;
            self.struck = true;
            return Ran::Struck;
        }
        if ended {
            self.stopped = true;
            return Ran::Ended;
        }
        Ran::On
    }

    /// Whether its time is up: all of it gone at once.
    fn gone(&self, now: f32) -> bool {
        now - self.born >= self.lasts
    }

    /// Drawn now: its strands, zigzagging about the line they share, each
    /// all one width, black at its middle, darkening out to its edge, from
    /// one point under its feet; their forks; struck, forks flung all round
    /// its tip. (Positions from `origin`; colours to multiply the ground
    /// by; never empty.) Each zigzag and fork as it was laid: what it has
    /// grown never changes.
    fn mesh(&self) -> Mesh {
        let mut m = Draw::default();
        let n = self.marks.len();
        let last = self.marks.last().expect("its first mark");
        let across = Vec3::new(-self.way.z, 0.0, self.way.x);
        let many = self.strands > 1;
        let (width, fork) = if many { (STRAND, FORK / self.strands as f32) } else { ((CORE, EDGE), FORK) };
        for strand in 0..self.strands {
            let seed = self.seed.wrapping_add(strand.wrapping_mul(0x9e37_79b9));
            let place = if many { SPREAD * (2.0 * strand as f32 / (self.strands - 1) as f32 - 1.0) } else { 0.0 };
            // Its line: the marks, each but the first set off the line by
            // chance (theirs and its own), and its tip.
            let mut line: Vec<(Vec3, f32, f32, Vec3)> = Vec::with_capacity(n + 1);
            for (k, mk) in self.marks.iter().enumerate() {
                let own = if many { STRAND_JAG * (2.0 * hash3(seed, k as u32, 1) - 1.0) } else { 0.0 };
                let off = if k == 0 { 0.0 } else { JAG * (2.0 * hash3(self.seed, k as u32, 0) - 1.0) + place + own };
                line.push((mk.at + mk.side * off + Vec3::Y * (mk.slope * off), 1.0, mk.slope, mk.side));
            }
            if self.head.distance(last.at) > 0.05 {
                line.push((self.head + across * place, 1.0, last.slope, across));
            }
            m.strip(&line, width, self.origin);
            // Its forks: from some of its marks, slanting off ahead,
            // zigzagging, narrowing to their tips.
            for (k, &(p, _, slope, side)) in line.iter().enumerate().take(n).skip(2) {
                if hash3(seed ^ 0x9e37, k as u32, 0) > fork {
                    continue;
                }
                let ahead = Vec3::new(side.z, 0.0, -side.x);
                let way = if hash3(seed, k as u32, 7) < 0.5 { -1.0 } else { 1.0 };
                let angle = way * (0.45 + 0.6 * hash3(seed, k as u32, 11));
                let length = FORK_LENGTH.0 + (FORK_LENGTH.1 - FORK_LENGTH.0) * hash3(seed, k as u32, 13);
                m.fork(p, Quat::from_rotation_y(angle) * ahead, length, side, slope, 1.0, (seed, k as u32), self.origin);
            }
        }
        if self.struck {
            for k in 0..STRIKE_FORKS {
                let a = (k as f32 + hash3(self.seed, k + 100, 0)) / STRIKE_FORKS as f32 * std::f32::consts::TAU;
                let out = Vec3::new(a.cos(), 0.0, a.sin());
                let length = 1.5 + 2.0 * hash3(self.seed, k + 200, 0);
                m.fork(self.head, out, length, across, last.slope, 1.0, (self.seed, k + 300), self.origin);
            }
        }
        m.mesh()
    }
}

/// A number in 0..1 from three.
fn hash3(a: u32, b: u32, c: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b) ^ c.wrapping_mul(0xc2b2_ae35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// A mesh being drawn: places (from an origin), colours (what the ground
/// is multiplied by: black at a crack's middle, white at its edge).
#[derive(Default)]
struct Draw {
    place: Vec<[f32; 3]>,
    colour: Vec<[f32; 4]>,
    index: Vec<u32>,
}

impl Draw {
    fn point(&mut self, p: Vec3, origin: Vec3, shade: f32) -> u32 {
        self.place.push((p - origin).to_array());
        self.colour.push([shade, shade, shade, 1.0]);
        self.place.len() as u32 - 1
    }

    /// A strip along `line` (each point: where, how much of its width,
    /// the ground's slope across, across), black within `core` (half width,
    /// m), darkening to `edge`; each point across its line's own turn.
    fn strip(&mut self, line: &[(Vec3, f32, f32, Vec3)], (core, edge): (f32, f32), origin: Vec3) {
        if line.len() < 2 {
            return;
        }
        let first = self.place.len() as u32;
        let n = line.len();
        for (i, &(p, f, slope, side)) in line.iter().enumerate() {
            let along = (line[(i + 1).min(n - 1)].0 - line[i.saturating_sub(1)].0).with_y(0.0);
            let across = if along.length() > 1e-4 { Vec3::new(-along.z, 0.0, along.x).normalize() } else { side };
            let rise = |d: f32| Vec3::Y * (slope * d * across.dot(side) + LIFT);
            for (d, shade) in [(-edge, 1.0), (-core, 0.0), (core, 0.0), (edge, 1.0)] {
                let d = d * f;
                self.point(p + across * d + rise(d), origin, shade);
            }
        }
        for k in 0..(n as u32 - 1) {
            let v = first + 4 * k;
            for s in 0..3 {
                let (a, b) = (v + s, v + s + 4);
                self.index.extend([a, a + 1, b + 1, a, b + 1, b]);
            }
        }
    }

    /// A fork from `p` along `out`, `length` long, zigzagging, narrowing to
    /// its tip; laid on the ground's slope across (`side`, `slope`) where it
    /// sets off.
    #[allow(clippy::too_many_arguments)]
    fn fork(&mut self, p: Vec3, out: Vec3, length: f32, side: Vec3, slope: f32, f: f32, (seed, k): (u32, u32), origin: Vec3) {
        const STEPS: u32 = 4;
        let mut line = Vec::with_capacity(STEPS as usize + 1);
        let mut at = p;
        let mut way = out;
        for s in 0..=STEPS {
            let off = (at - p).dot(side);
            line.push((at.with_y(p.y + slope * off), f * (1.0 - s as f32 / STEPS as f32), 0.0, side));
            let bend = 0.7 * (2.0 * hash3(seed, k * 31 + s, 1) - 1.0);
            way = Quat::from_rotation_y(bend) * way;
            at += way * length / STEPS as f32;
        }
        self.strip(&line, FORK_WIDTH, origin);
    }

    /// Its mesh (never empty: a mesh of nothing upset the renderer's
    /// allocator).
    fn mesh(mut self) -> Mesh {
        if self.index.is_empty() {
            self.place = vec![[0.0; 3]; 3];
            self.colour = vec![[1.0; 4]; 3];
            self.index = vec![0, 1, 2];
        }
        let normal = vec![[0.0, 1.0, 0.0]; self.place.len()];
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.place)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colour)
            .with_inserted_indices(Indices::U32(self.index))
    }
}

/// Its look: the ground multiplied by its colours (unlit, both faces,
/// unfogged: white leaves the ground as it is).
pub(super) fn material() -> StandardMaterial {
    StandardMaterial { base_color: Color::WHITE, unlit: true, alpha_mode: AlphaMode::Multiply, cull_mode: None, fog_enabled: false, double_sided: true, ..default() }
}

/// A crack set growing from under `from` (its feet, on the ground) at
/// `to` (your feet).
pub(super) fn fire(commands: &mut Commands, meshes: &mut Assets<Mesh>, look: Handle<StandardMaterial>, from: Vec3, to: Vec3, (seek, lasts, strands): (f32, f32, u32), seed: u32, now: f32, ground: &crate::ground::Ground) {
    let mut crack = Crack::new(from, to, (seek, lasts, strands), seed, now, &|p| ground.under(p));
    crack.mesh = meshes.add(crack.mesh());
    let drawn = (Mesh3d(crack.mesh.clone()), MeshMaterial3d(look), Transform::from_translation(from), Visibility::default(), bevy::camera::visibility::NoFrustumCulling, bevy::light::NotShadowCaster, bevy::light::NotShadowReceiver);
    commands.spawn((crack, drawn));
}

/// Each crack grown a frame; its tip under your feet, it hurts you;
/// drawn anew; its time up, gone.
#[allow(clippy::too_many_arguments)]
pub(in crate::combat) fn cracks(
    mut commands: Commands,
    time: Res<Time>,
    ground: crate::ground::Ground,
    assets: Res<super::super::Assets3>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut director: ResMut<super::super::Director>,
    mut gun: ResMut<super::super::Gun>,
    camera: Single<(&Transform, &mut crate::player::Player), (With<FlyCam>, Without<Crack>)>,
    mut cracks: Query<(Entity, &mut Crack)>,
) {
    let dt = time.delta_secs().min(0.05);
    let now = time.elapsed_secs();
    let (camera, mut player) = camera.into_inner();
    let feet = camera.translation - Vec3::Y * crate::player::EYE;
    for (entity, mut c) in &mut cracks {
        let c = &mut *c;
        if c.step(dt, now, feet, &|p| ground.under(p)) == Ran::Struck {
            player.health -= HARM;
            director.hurt = 1.0;
            gun.jolt += Vec2::new(0.05, 0.0);
            commands.spawn((AudioPlayer::new(assets.player_pain.clone()), PlaybackSettings::DESPAWN));
            info!("tumbler: its crack struck you after {:.1} m", c.run);
        }
        if c.gone(now) {
            meshes.remove(&c.mesh);
            commands.entity(entity).despawn();
            continue;
        }
        let fresh = meshes.add(c.mesh());
        let old = std::mem::replace(&mut c.mesh, fresh);
        meshes.remove(&old);
        commands.entity(entity).insert(Mesh3d(c.mesh.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(_: Vec3) -> Option<f32> {
        Some(0.0)
    }

    /// Run until it ends, you moved each frame by `you`; whether it struck.
    fn runs(from: Vec3, mut feet: Vec3, you: Vec3, seek: f32) -> bool {
        let mut c = Crack::new(from, feet, (seek, LASTS, STRANDS), 7, 0.0, &flat);
        let dt = 1.0 / 60.0;
        for i in 0..600 {
            feet += you * dt;
            match c.step(dt, i as f32 * dt, feet, &flat) {
                Ran::Struck => return true,
                Ran::Ended => return false,
                Ran::On => {}
            }
        }
        panic!("it never ended")
    }

    /// Standing or walking across its line, it gets you; running across
    /// it, you get away; over it, it passes under you.
    #[test]
    fn struck_unless_you_run_or_leap() {
        let feet = Vec3::new(0.0, 0.0, 15.0);
        assert!(runs(Vec3::ZERO, feet, Vec3::ZERO, SEEK), "standing");
        assert!(runs(Vec3::ZERO, feet, Vec3::X * crate::player::WALK_SPEED, SEEK), "walking across");
        assert!(!runs(Vec3::ZERO, feet, Vec3::X * 10.24, SEEK), "running across");
        assert!(!runs(Vec3::ZERO, feet + Vec3::Y * 1.2, Vec3::ZERO, SEEK), "over it");
    }

    /// A wall stops it; a step it climbs.
    #[test]
    fn climbs_steps_not_walls() {
        let step = |p: Vec3| Some(if p.z > 5.0 { 1.0 } else { 0.0 });
        let wall = |p: Vec3| Some(if p.z > 5.0 { 4.0 } else { 0.0 });
        let feet = Vec3::new(0.0, 1.0, 15.0);
        let mut c = Crack::new(Vec3::ZERO, feet, (SEEK, LASTS, STRANDS), 7, 0.0, &step);
        let ran = (0..600).map(|i| c.step(1.0 / 60.0, i as f32 / 60.0, feet, &step)).find(|r| *r != Ran::On);
        assert_eq!(ran, Some(Ran::Struck), "up a step");
        let mut c = Crack::new(Vec3::ZERO, feet, (SEEK, LASTS, STRANDS), 7, 0.0, &wall);
        let ran = (0..600).map(|i| c.step(1.0 / 60.0, i as f32 / 60.0, feet.with_y(4.0), &wall)).find(|r| *r != Ran::On);
        let head = c.head;
        assert_eq!(ran, Some(Ran::Ended), "at a wall");
        assert!(head.z <= 5.0 + MARK, "stopped at it: {head}");
    }

    /// What it has grown never changes as it grows on (the user,
    /// 2026-11-13): its line's points up to its last but one mark the same
    /// from frame to frame.
    #[test]
    fn grown_stays() {
        let mut c = Crack::new(Vec3::ZERO, Vec3::Z * 30.0, (SEEK, LASTS, STRANDS), 7, 0.0, &flat);
        let places = |c: &Crack| match c.mesh().attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) => p.clone(),
            _ => panic!("no places"),
        };
        for i in 0..30 {
            c.step(1.0 / 60.0, i as f32 / 60.0, Vec3::Z * 30.0, &flat);
        }
        let (before, laid) = (places(&c), c.marks.len());
        assert!(laid > 4, "grown: {laid} marks");
        for i in 30..50 {
            c.step(1.0 / 60.0, i as f32 / 60.0, Vec3::Z * 30.0, &flat);
        }
        let after = places(&c);
        let kept = 4 * (laid - 1);
        assert_eq!(before[..kept], after[..kept], "its grown line changed");
    }

    /// Its mesh: black at its middle, the ground as it is at its edges;
    /// some of it drawn as it runs and as it fades.
    #[test]
    fn darkens_the_ground() {
        let mut c = Crack::new(Vec3::ZERO, Vec3::Z * 30.0, (SEEK, LASTS, STRANDS), 7, 0.0, &flat);
        for i in 0..40 {
            c.step(1.0 / 60.0, i as f32 / 60.0, Vec3::Z * 30.0, &flat);
        }
        let mesh = c.mesh();
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colour)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR) else { panic!("no colours") };
        assert!(colour.iter().any(|c| c[0] == 0.0) && colour.iter().any(|c| c[0] == 1.0));
        assert!(mesh.indices().expect("indexed").len() > 100, "drawn");
    }
}
