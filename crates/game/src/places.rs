//! Places to go to by name, for testing (the user, 2026-11-04): kept in
//! `places.txt` at the repository's root, one a line, `NAME
//! X,Y,Z,YAW,PITCH [{WORLD}] [note]`, the five numbers as `--cam` takes them
//! (Y above the ground, see `ground`), WORLD the flags that make the world
//! it is in (`World`; none: outdoors, seed 1). `tp NAME` at the console
//! goes there (in another world, said, with how to start there), `tp` alone
//! lists them; `savetp NAME [note]` keeps where you are (said, if it
//! replaces one); `--tp NAME` on the command line starts there, its world's
//! flags brought along. A name not there, or a line that does not read, is
//! said, never passed over.
//!
//! (A place kept only as numbers came back in the air (the user, 2026-11-09):
//! saved in one world and gone to in another, or its height measured from
//! the outdoor terrain when saved and from a ground lab's when started.)

use std::path::PathBuf;

/// A place: its name, where (as `--cam`), the world it is in, and what it
/// is.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub at: [f32; 5],
    pub world: World,
    pub note: String,
}

impl Place {
    /// As it is written in the file.
    fn line(&self) -> String {
        let [x, y, z, yaw, pitch] = self.at;
        let mut at = format!("{x:.2},{y:.2},{z:.2},{yaw:.2},{pitch:.2}");
        if self.world != World::default() {
            at = format!("{at} {{{}}}", self.world.flags());
        }
        if self.note.is_empty() { format!("{:<16} {at}", self.name) } else { format!("{:<16} {at:<42} {}", self.name, self.note) }
    }
}

/// The options that make what is built (the chasm's places, the labs), and
/// the settings: those a place's world is told by. (Another that changes
/// what is built belongs here, or its places come back wrong.)
const WORLD_OPTS: [&str; 12] = ["chasm", "arena", "region", "plan", "rooms", "junctions", "orb", "well", "woven", "fit", "noside", "inside"];
const WORLD_SETS: [&str; 3] = ["spine", "rooms", "kind"];

/// The world a place is in: the seed, the ground lab (`--ground`), and the
/// options and settings that make what is built (`WORLD_OPTS`, `WORLD_SETS`).
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub seed: u32,
    pub ground: Option<String>,
    pub opts: Vec<String>,
    pub sets: Vec<(String, f32)>,
}

impl Default for World {
    /// Outdoors, seed 1.
    fn default() -> Self {
        World { seed: 1, ground: None, opts: Vec::new(), sets: Vec::new() }
    }
}

impl World {
    /// The world the game was started in.
    pub fn of(args: &crate::Args) -> World {
        World {
            seed: args.seed,
            ground: args.ground.clone(),
            opts: WORLD_OPTS.iter().filter(|o| args.opt(o)).map(|o| o.to_string()).collect(),
            sets: WORLD_SETS.iter().filter(|k| args.sets.iter().any(|(s, _)| s == *k)).map(|k| (k.to_string(), args.num(k, 0.0))).collect(),
        }
    }

    /// As command-line flags (none for outdoors, seed 1).
    pub fn flags(&self) -> String {
        let mut words = Vec::new();
        if self.seed != 1 {
            words.push(format!("--seed {}", self.seed));
        }
        if let Some(g) = &self.ground {
            words.push(format!("--ground {g}"));
        }
        words.extend(self.opts.iter().map(|o| format!("--opt {o}")));
        words.extend(self.sets.iter().map(|(k, v)| format!("--set {k}={v}")));
        words.join(" ")
    }

    /// The world those flags make, or what does not read.
    fn parse(flags: &str) -> Result<World, String> {
        let mut world = World::default();
        let mut it = flags.split_whitespace();
        while let Some(flag) = it.next() {
            let value = it.next().ok_or_else(|| format!("{flag} without its value in {{{flags}}}"))?;
            match flag {
                "--seed" => world.seed = value.parse().map_err(|_| format!("--seed {value}: not a seed"))?,
                "--ground" => world.ground = Some(value.to_string()),
                "--opt" => world.opts.push(value.to_string()),
                "--set" => {
                    let (k, v) = value.split_once('=').ok_or_else(|| format!("--set {value}: not name=value"))?;
                    world.sets.push((k.to_string(), v.parse().map_err(|_| format!("--set {value}: not a number"))?));
                }
                other => return Err(format!("{other}: not a world's flag (--seed, --ground, --opt, --set)")),
            }
        }
        Ok(world)
    }

    /// `args` given what this world needs and they lack (the seed only if
    /// none was given, `seed_given`); what they say otherwise is left, to
    /// be found different (see `--tp`).
    pub fn bring(&self, args: &mut crate::Args, seed_given: bool) {
        if !seed_given {
            args.seed = self.seed;
        }
        if args.ground.is_none() {
            args.ground = self.ground.clone();
        }
        for o in &self.opts {
            if !args.opt(o) {
                args.opts.push(o.clone());
            }
        }
        for (k, v) in &self.sets {
            if !args.sets.iter().any(|(s, _)| s == k) {
                args.sets.push((k.clone(), *v));
            }
        }
    }
}

/// The ground a place's height is above, wherever it is measured (saving,
/// going there, starting there): a ground lab's top where there is one
/// (`--ground`; its flat ground under it with `--opt camfloor`, to look
/// from under its decks), and never below the world's terrain; else the
/// world's terrain.
pub fn ground(world: Option<&crate::terrain::WorldGen>, lab: Option<&crate::groundlab::LabGround>, camfloor: bool, x: f32, z: f32) -> f32 {
    // (Over a void, the world's level: its walls' foot is the shell's
    // thickness, a setting; measured from it, a place there moved with it,
    // the user's "blink", 2026-11-17.)
    let terrain = world.map_or(0.0, |w| if w.0.is_void(x, z) { 0.0 } else { w.ground_height(x, z) });
    match lab {
        Some(lab) if !camfloor => lab.height(x, z).max(terrain),
        _ => terrain,
    }
}

/// The file, at the repository's root.
pub fn file() -> PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).expect("the game crate is two folders into the repository");
    root.join("places.txt")
}

/// The place a line names, if it names one (not blank, not a `#` comment).
fn parse_line(line: &str) -> Result<Option<Place>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    // (A word, and what follows it.)
    let word = |s: &str| -> (String, String) {
        let s = s.trim_start();
        let end = s.find(char::is_whitespace).unwrap_or(s.len());
        (s[..end].to_string(), s[end..].trim().to_string())
    };
    let (name, rest) = word(line);
    let (at, rest) = word(&rest);
    if at.is_empty() {
        return Err(format!("{name}: no X,Y,Z,YAW,PITCH after the name"));
    }
    let v: Vec<f32> = at.split(',').map(|v| v.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|_| format!("{name}: {at} is not X,Y,Z,YAW,PITCH"))?;
    let at = <[f32; 5]>::try_from(v).map_err(|v| format!("{name}: {} numbers, not five (X,Y,Z,YAW,PITCH)", v.len()))?;
    // (Its world, in braces, if not outdoors at seed 1.)
    let (world, note) = match rest.strip_prefix('{') {
        Some(inner) => {
            let (flags, note) = inner.split_once('}').ok_or_else(|| format!("{name}: {{ without its }}"))?;
            (World::parse(flags).map_err(|e| format!("{name}: {e}"))?, note.trim().to_string())
        }
        None => (World::default(), rest),
    };
    Ok(Some(Place { name, at, world, note }))
}

/// Every place in `text`, or what does not read (with its line).
fn parse(text: &str) -> Result<Vec<Place>, String> {
    let mut places = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if let Some(place) = parse_line(line).map_err(|e| format!("{} line {}: {e}", file().display(), i + 1))? {
            places.push(place);
        }
    }
    Ok(places)
}

/// Every place kept.
pub fn read() -> Result<Vec<Place>, String> {
    let text = std::fs::read_to_string(file()).map_err(|e| format!("cannot read {}: {e}", file().display()))?;
    parse(&text)
}

/// The place `name`, or why not (the names there are, if it is not one).
pub fn find(name: &str) -> Result<Place, String> {
    let places = read()?;
    match places.iter().rev().find(|p| p.name == name) {
        Some(p) => Ok(p.clone()),
        None => Err(format!("no place {name} in {}; there are: {}", file().display(), names_of(&places))),
    }
}

/// The names kept, for listing and completing.
pub fn names() -> Vec<String> {
    read().map(|p| p.into_iter().map(|p| p.name).collect()).unwrap_or_default()
}

fn names_of(places: &[Place]) -> String {
    if places.is_empty() { "none".into() } else { places.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ") }
}

/// The places listed, one a line, for the console.
pub fn listed() -> Result<Vec<String>, String> {
    Ok(read()?.iter().map(Place::line).collect())
}

/// `text` with `place` in it: in the place of the one of its name (and
/// that one given back; its note kept if `place` has none), or added at the
/// end; every other line as it was.
fn saved(text: &str, place: &Place) -> (String, Option<Place>) {
    let mut out = Vec::new();
    let mut old = None;
    for line in text.lines() {
        match parse_line(line) {
            Ok(Some(p)) if p.name == place.name && old.is_none() => {
                let note = if place.note.is_empty() { p.note.clone() } else { place.note.clone() };
                out.push(Place { note, ..place.clone() }.line());
                old = Some(p);
            }
            _ => out.push(line.to_string()),
        }
    }
    if old.is_none() {
        out.push(place.line());
    }
    (out.join("\n") + "\n", old)
}

/// `place` kept in the file; the one it replaces, if any.
pub fn save(place: &Place) -> Result<Option<Place>, String> {
    if place.name.is_empty() || place.name.starts_with('#') || place.name.contains(char::is_whitespace) {
        return Err(format!("{:?}: a name is one word, not starting with #", place.name));
    }
    let text = match std::fs::read_to_string(file()) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", file().display())),
    };
    // (Not written over a file that does not read: it would lose what it
    // could not read.)
    parse(&text)?;
    let (text, old) = saved(&text, place);
    std::fs::write(file(), text).map_err(|e| format!("cannot write {}: {e}", file().display()))?;
    Ok(old)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_saves() {
        let text = "# places\n\ntiers   5248,2,640,0,0   the tiers' sample point\nbroken 7296,2,640,0,-10\n";
        let places = parse(text).unwrap();
        assert_eq!(places.len(), 2);
        assert_eq!(places[0], Place { name: "tiers".into(), at: [5248.0, 2.0, 640.0, 0.0, 0.0], world: World::default(), note: "the tiers' sample point".into() });
        assert_eq!(places[1].note, "");
        // Replaced in its line, the comment and the other kept.
        let new = Place { name: "tiers".into(), at: [1.0, 2.0, 3.0, 4.0, 5.0], world: World::default(), note: "moved".into() };
        let (out, old) = saved(text, &new);
        assert_eq!(old.unwrap().at, [5248.0, 2.0, 640.0, 0.0, 0.0]);
        let again = parse(&out).unwrap();
        assert_eq!(again[0], new);
        assert_eq!(again[1].name, "broken");
        assert!(out.starts_with("# places\n"));
        // Saved again with no note, its note kept.
        let (out, _) = saved(&out, &Place { note: String::new(), ..new.clone() });
        assert_eq!(parse(&out).unwrap()[0].note, "moved");
        // Added at the end.
        let (out, old) = saved(text, &Place { name: "arena".into(), at: [0.0; 5], world: World::default(), note: String::new() });
        assert!(old.is_none());
        assert_eq!(parse(&out).unwrap().last().unwrap().name, "arena");
    }

    /// A place's world: written in braces (not for outdoors at seed 1),
    /// read back the same, its note after it.
    #[test]
    fn keeps_its_world() {
        let world = World { seed: 5, ground: Some("void_inlay".into()), opts: vec!["chasm".into(), "rooms".into()], sets: vec![("rooms".into(), 1.0)] };
        let place = Place { name: "well".into(), at: [1.0, 2.0, 3.0, 4.0, 5.0], world: world.clone(), note: "the round well".into() };
        let line = place.line();
        assert!(line.contains("{--seed 5 --ground void_inlay --opt chasm --opt rooms --set rooms=1}"), "{line}");
        assert_eq!(parse(&line).unwrap()[0], place);
        assert!(!Place { world: World::default(), ..place.clone() }.line().contains('{'));
        assert!(parse("x 1,2,3,4,5 {--opt chasm").is_err());
        assert!(parse("x 1,2,3,4,5 {--fly high}").is_err());
    }

    /// `--tp`: a place's world brought to flags that lack it; flags that
    /// say otherwise left, to be found different.
    #[test]
    fn brings_its_world() {
        let world = World { seed: 5, ground: None, opts: vec!["chasm".into(), "rooms".into()], sets: vec![("rooms".into(), 1.0)] };
        let mut args = crate::Args { opts: vec!["mute".into()], sets: Vec::new(), cam: None, time: None, shot: None, seed: 1, focus: None, ground: None, run: Vec::new() };
        world.bring(&mut args, false);
        assert_eq!(World::of(&args), world);
        let mut given = crate::Args { seed: 3, ..args.clone() };
        world.bring(&mut given, true);
        assert_ne!(World::of(&given), world);
    }

    #[test]
    fn says_what_does_not_read() {
        assert!(parse("tiers").unwrap_err().contains("line 1"));
        assert!(parse("ok 1,2,3,4,5\ntiers 1,2,3").unwrap_err().contains("line 2"));
        assert!(parse("tiers 1,2,x,4,5").is_err());
    }

    /// The places kept all read (a line that does not would stop `tp`).
    #[test]
    fn the_file_reads() {
        let places = read().unwrap();
        assert!(!places.is_empty());
    }
}
