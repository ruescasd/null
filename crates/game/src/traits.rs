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
const HUNTERS: &[&str] = &["harrow", "angler", "sprawl", "biped", "stalker", "porter", "chained"];
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
    (BEASTS, "wiry", false, &[], "a body of fibres: thin strands bundled along its bones"),
    (BEASTS, "cables", true, &[], "cables on its body: 1 muscles, 2 fibres, 3 a cable waist"),
    (BEASTS, "pounce", false, &[], "the hound's leap at you, running"),
    (&["harrow", "angler"], "body", true, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0], "its body: 0 shards round its bones; with void between: 1 a skeleton, 2 hoops, 3 a spiral, 4 a truss, 5 arches, 6 a keel and a fan, 7 branches, joined, 8 its shards, a third as many, 9 plates in bands, 10 Voronoi plates"),
    (&["harrow"], "mesh", true, &[0.0, 1.0], "its body and build from an authored mesh in place of its own parts: 1 the shard beast (a crest of blades, an open ribcage, legs of stacked shards, a burst of spikes for a head)"),
    (&["harrow", "angler"], "ribs", true, &RIB_SIZES, "body=1: its ribs' thickness, 1, 1.7, 2.5 or 3.5"),
    (STALKER, "searchlight", true, &[], "its searchlight after a hit: 0 pale, 1 black"),
    (STALKER, "nolance", false, &[], "no lance"),
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
