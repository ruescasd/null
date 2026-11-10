//! A creature's own options, given when it is spawned (`spawn KIND [N]
//! [option ...]` at the console; the user: what varies a creature belongs to
//! it, not to a switch for the whole game, so two can differ side by side).
//! An option is a name (`cape`) or a name and a number (`slab=2`). Each
//! kind takes its own (`OPTIONS`); any other is refused, with the list
//! (`spawn KIND --help`, `help` or `?` shows it).
//!
//! (Read with `has` and `value`, not `opt` and `num`: these are not the
//! command line's, and `build.rs` gathers only those.)

use bevy::prelude::*;

/// The hunters, the beasts among them, and those that are (or bring) a
/// stalker or a porter.
const HUNTERS: &[&str] = &["harrow", "angler", "sprawl", "biped", "stalker", "porter", "chained", "clip"];
const BEASTS: &[&str] = &["harrow", "angler", "sprawl"];
const STALKER: &[&str] = &["stalker", "chained"];
const PORTER: &[&str] = &["porter", "chained"];

/// The skeleton's rib thicknesses (its `ribs` option, see `skeleton.rs`).
pub const RIB_SIZES: [f32; 4] = [1.0, 1.7, 2.5, 3.5];

/// Every option: the kinds that take it, its name, whether it takes a
/// number, the numbers it takes (none listed, any), and what it does. (A
/// number not among them refused as it is typed, said so: taken, it made
/// the creature panic as it was built, and the game with it, the user.)
pub const OPTIONS: &[(&[&str], &str, bool, &[f32], &str)] = &[
    (HUNTERS, "health", true, &[], "its health (45)"),
    (HUNTERS, "tough", false, &[], "ten times its health"),
    (HUNTERS, "tame", false, &[], "circles you at a distance, side on, its pace changing every 8 s"),
    (HUNTERS, "tame_at", true, &[], "tame: how far off (m, 14)"),
    (HUNTERS, "tame_pace", true, &[], "tame: a fixed pace (m/s)"),
    (HUNTERS, "tame_line", false, &[], "back and forth along a line across before you, to judge a walk"),
    (HUNTERS, "tame_span", true, &[], "tame_line: how long the line (m, 16)"),
    (HUNTERS, "still", false, &[], "its body frozen as it first stands, to see what moves on it"),
    (HUNTERS, "bones", false, &[], "its pieces hidden, its bones drawn as lines"),
    (HUNTERS, "rigtrace", false, &[], "its rig logged every frame (rigtrace: lines), to study its motion"),
    (HUNTERS, "blink_range", true, &[], "on district ground (--opt blink): how far a teleport goes (m, 10; 20 at most)"),
    (HUNTERS, "blink_tell", true, &[], "on district ground: how long it stands, glowing, before it teleports (s, 0.6)"),
    (HUNTERS, "blink_rest", true, &[], "on district ground: how long after a teleport before another (s, 1.5)"),
    (HUNTERS, "blink_cost", true, &[], "on district ground: how many metres of walking a teleport is worth, planning its way (8)"),
    (HUNTERS, "blink_speed", true, &[], "on district ground: how fast it glides along a teleport's line, through what is in the way (m/s, 20)"),
    (BEASTS, "wiry", false, &[], "a body of fibres: thin strands bundled along its bones"),
    (BEASTS, "cables", true, &[], "cables on its body: 1 muscles, 2 fibres, 3 a cable waist"),
    (BEASTS, "pounce", false, &[], "the hound's leap at you, running"),
    (&["harrow", "angler"], "body", true, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0], "its body: 0 shards round its bones; with void between: 1 a skeleton, 2 hoops, 3 a spiral, 4 a truss, 5 arches, 6 a keel and a fan, 7 branches, joined, 8 its shards, a third as many, 9 plates in bands, 10 Voronoi plates"),
    (&["harrow", "angler"], "ribs", true, &RIB_SIZES, "body=1: its ribs' thickness, 1, 1.7, 2.5 or 3.5"),
    (STALKER, "searchlight", true, &[], "its searchlight after a hit: 0 pale, 1 black"),
    (STALKER, "shift", true, &[0.0, 1.0], "between shots: 1 hovering, it adjusts where it is, darts and holds at no beat, its gun on you, and comes down only to fire; 0 it stands to gather and fire"),
    (STALKER, "hover", true, &[0.0, 1.0], "hunting: 1 it hovers, its stilts hanging straight down, coming down on them to fire; 0 it walks the gladiator's walk (tame: 1 hovers as it goes, else its own walk or walk=1)"),
    (STALKER, "air", true, &[0.0, 1.0, 2.0], "hovering: its legs walk the air, as they would at its pace, touching nothing: 2 (its own, the user, 2026-11-07) half the steps, twice as long, 1 at its cadence; 0 hanging still"),
    (STALKER, "shift_pace", true, &[], "how fast it darts, adjusting where it is (m/s, 3.75; each dart up to 0.4 of it either side)"),
    (STALKER, "walk", true, &[0.0, 1.0], "tame or tame_line: its walk, 0 its own, struggling (wandering), 1 its hunting one, the gladiator's measured (creatures/gladiator.steps)"),
    (STALKER, "nolance", false, &[], "no lance"),
    (STALKER, "mesh", true, &[0.0, 1.0, 2.0], "its body: cut from the model in creatures/stalker.pieces, rigid pieces on its bones, 2 (its own, the user, 2026-11-07) its own build, the pieces stretched to it, 1 its build fitted to the model; 0 its shards"),
    (STALKER, "head", true, &[0.0, 1.0], "mesh=1 or 2: its head, 0 the model's, 1 its cage"),
    (STALKER, "shatter", false, &[], "always bursts when killed, never falls"),
    (STALKER, "overkill", true, &[], "shards in a blow that burst it (10)"),
    (PORTER, "cape", false, &[], "wears a cape"),
    (PORTER, "slab", true, &[0.0, 2.0, 3.0], "its load (0 the plain slab, 2 caged, ...)"),
    (PORTER, "piece", true, &[0.0, 4.0], "its pieces: 0 shards, 4 rods"),
    (PORTER, "step", true, &[], "how long its steps take (s, 2)"),
    (&["bud"], "asleep", false, &[], "a plant: asleep, rooted where it stands, till it wakes"),
    (&["bud"], "hair", false, &[], "thin black strands from round its back"),
    (&["bud"], "leech", false, &[], "may shoot a cable into you and drink"),
    (&["bud"], "gaze", true, &[], "its eye held at one point, 0 closed to 1 open"),
    (&["grove"], "hair", false, &[], "its buds' strands, standing up stiff asleep"),
    (&["grove"], "lonewake", false, &[], "now and then one wakes alone"),
    (&["bishop"], "grey", false, &[], "all of it flat grey (its outline)"),
    (&["impaled"], "course", false, &[], "flies a set course across before you, side on, to judge its flight: across low, up, back high (its thrust stuttering midway), down"),
    (&["impaled"], "log", false, &[], "what it does, each second, in the log"),
    (&["impaled"], "lift", true, &[], "its full thrust over its weight (1.45): nearer 1, heavier"),
    (&["impaled"], "spool", true, &[], "how long its thrust takes to follow a change, s (0.35)"),
    (&["impaled"], "tilt", true, &[], "how far it tilts to move, degrees (20)"),
    (&["impaled"], "climb", true, &[], "the climb at which its thrust only holds its weight, m/s (2.5)"),
    (&["impaled"], "speed", true, &[], "its fastest, m/s (7)"),
    (&["impaled"], "hover", true, &[], "how high its feet keep above the ground, m (1.5)"),
    (&["impaled"], "keep", true, &[], "it hovers after you this far off (m) instead of landing near you: to judge its field by"),
    (&["impaled"], "health", true, &[], "how much it takes (80)"),
    (&["impaled"], "plume", true, &[1.0, 2.0], "its beams: 1 in curves, 2 in hard straight-edged shards (2)"),
        (&["clip"], "echo", false, &[], "a second body beside it, a moment behind: one creature, two identities"),
    (&["clip"], "echo_lag", true, &[], "echo: how far behind (s, 0.07)"),
    (&["clip"], "echo_off", true, &[], "echo: how far off to one side (m, 0.085)"),
    (&["clip"], "dash_speed", true, &[], "how fast it dashes (m/s, 20; it walks or dashes, never runs)"),
    (&["clip"], "dash_time", true, &[], "how long a dash lasts (s, 0.4)"),
    (&["clip"], "dash_rest", true, &[], "how long after a dash before another, on average, coming at you (s, 3; drawn each time, 0.4 to 1.6 of it)"),
    (&["clip"], "dash_rest_close", true, &[], "the same within 12 m, fighting (s, 0.8; a quarter of the time none: twice running)"),
    (&["clip"], "stride_vary", true, &[], "how much its steps' pace varies, either way (of its walk, 0.15; now and then a slow step); 0, a machine's"),
    (&["clip"], "walk", true, &[0.0, 1.0], "its walk: 1 (if it has creatures/NAME.gait, from tools/gait_fit.py) its clip as waves, code of our own, each step shaped (below); 0 its clip as it is"),
    (&["clip"], "arm_swing", true, &[], "walk=1: how far its arms swing (of its clip's, -1: reversed, each with its own side's leg, the user 2026-11-07; was 0.6)"),
    (&["clip"], "still", false, &[], "to judge its animations: in one place, facing one way, its walk played in place 6 s, standing 2.5 s, its attack, standing 1.5 s, over and over; its head not turned to you"),
    (&["clip"], "still_turn", true, &[], "still: which way it faces, degrees from you as it first stood (90, side on; 0 facing you)"),
    (&["clip"], "hunch", true, &[], "how far its trunk stoops forward (degrees, 0, its clips' upright chest; tried 15), its neck turned back by half"),
    (&["clip"], "arms_in", true, &[], "how far its arms turn in toward its sides (degrees, 0, its clips' flared arms; tried 15)"),
    (&["clip"], "sway", true, &[], "walk=1: how far its hips sway sideways (of its clip's, -1: reversed, the user 2026-11-07; 1 as fitted, 0 none); its thighs rolled out by as much, its feet planted"),
    (&["clip"], "torso", true, &[], "walk=1: how far its trunk swings, the hips' turn, spine, neck and head (of its clip's, -1: reversed, the user 2026-11-07; 1 as fitted, 0 none); its legs go as fitted"),
    (&["clip"], "step_reach", true, &[], "walk=1: how much each step's reach varies, either way (of its walk's, 0.1); its feet stay planted"),
    (&["clip"], "step_lift", true, &[], "walk=1: how much each foot's lift varies, either way (of its walk's, 0.25)"),
    (&["clip"], "step_arms", true, &[], "walk=1: how much each step's arm swing varies, either way (of arm_swing, 0.25)"),
    (&["clip"], "step_lean", true, &[], "walk=1: how far its trunk leans each step, forward, sideways and twisted, either way (degrees, 3); all four 0, every step alike"),
    (&["clip"], "blind", false, &[], "with 2 or more: one behind the other, 5 m apart, half walk=1 and half the clip, which at random, said in blind.txt beside the game (to compare blind)"),
    (&["clip"], "fall_gravity", true, &[], "killed, how fast it falls limp (m/s², 20; the stalker's 9.8)"),
    (&["clip"], "fall_tone", true, &[], "killed, how much its knees and elbows hold at first (0..1, 0.05; the stalker's 0.2)"),
];

/// A creature's options.
#[derive(Component, Clone, Default, Debug)]
pub struct Traits(Vec<(String, Option<f32>)>);

impl Traits {
    /// Whether it was given `name`.
    pub fn has(&self, name: &str) -> bool {
        self.0.iter().any(|(n, _)| n == name)
    }

    /// Its number for `name`, else `default`.
    pub fn value(&self, name: &str, default: f32) -> f32 {
        self.0.iter().rev().find(|(n, _)| n == name).and_then(|(_, v)| *v).unwrap_or(default)
    }

    /// The same, with `name` set to `value` (over any given).
    pub fn with(mut self, name: &str, value: f32) -> Self {
        self.0.push((name.to_string(), Some(value)));
        self
    }

    /// The options in `words` for `kind`; or what is wrong, with what it
    /// takes.
    pub fn parse(kind: &str, words: &[&str]) -> Result<Self, String> {
        let mut out = Vec::new();
        for word in words {
            let (name, value) = match word.split_once('=') {
                Some((n, v)) => (n, Some(v)),
                None => (*word, None),
            };
            let Some(&(_, _, number, choices, _)) = OPTIONS.iter().find(|(kinds, n, ..)| kinds.contains(&kind) && *n == name) else {
                return Err(format!("{kind} has no option {name:?}; {}", help(kind)));
            };
            match (number, value.map(|v| v.parse::<f32>())) {
                (true, Some(Ok(v))) if !choices.is_empty() && !choices.contains(&v) => {
                    let listed: Vec<String> = choices.iter().map(|c| c.to_string()).collect();
                    return Err(format!("{kind}: {name}={v}: one of {}", listed.join(", ")));
                }
                (true, Some(Ok(v))) => out.push((name.to_string(), Some(v))),
                (false, None) => out.push((name.to_string(), None)),
                (true, _) => return Err(format!("{kind}: {name} takes a number, {name}=N")),
                (false, Some(_)) => return Err(format!("{kind}: {name} takes no number")),
            }
        }
        Ok(Self(out))
    }
}

/// What `kind` takes, as a line.
pub fn help(kind: &str) -> String {
    let takes: Vec<String> = OPTIONS.iter().filter(|(kinds, ..)| kinds.contains(&kind)).map(|(_, n, number, _, what)| format!("{n}{} ({what})", if *number { "=N" } else { "" })).collect();
    if takes.is_empty() { format!("{kind} takes no options") } else { format!("{kind} takes: {}", takes.join(", ")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_parse() {
        let t = Traits::parse("porter", &["cape", "slab=2"]).unwrap();
        assert!(t.has("cape") && t.value("slab", 0.0) == 2.0 && t.value("piece", 7.0) == 7.0);
        assert!(Traits::parse("harrow", &["cape"]).unwrap_err().contains("harrow takes:"));
        assert!(Traits::parse("porter", &["slab"]).is_err());
        assert!(Traits::parse("porter", &["cape=1"]).is_err());
        assert!(Traits::parse("chained", &["searchlight=1", "cape"]).is_ok());
        // (A number not among those it takes, refused, with them.)
        assert!(Traits::parse("harrow", &["ribs=1.7"]).is_ok());
        assert!(Traits::parse("harrow", &["ribs=3"]).unwrap_err().contains("one of 1, 1.7, 2.5, 3.5"));
        assert!(Traits::parse("porter", &["slab=1"]).is_err());
    }
}
