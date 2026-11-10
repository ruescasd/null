//! Places to go to by name, for testing (the user, 2026-11-04): kept in
//! `places.txt` at the repository's root, one a line, `NAME
//! X,Y,Z,YAW,PITCH [note]`, the five numbers as `--cam` takes them (Y above
//! the ground). `tp NAME` at the console goes there, `tp` alone lists them;
//! `savetp NAME [note]` keeps where you are (said, if it replaces one);
//! `--tp NAME` on the command line starts there. A name not there, or a
//! line that does not read, is said, never passed over.

use std::path::PathBuf;

/// A place: its name, where (as `--cam`), and what it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub at: [f32; 5],
    pub note: String,
}

impl Place {
    /// As it is written in the file.
    fn line(&self) -> String {
        let [x, y, z, yaw, pitch] = self.at;
        let at = format!("{x:.2},{y:.2},{z:.2},{yaw:.2},{pitch:.2}");
        if self.note.is_empty() { format!("{:<16} {at}", self.name) } else { format!("{:<16} {at:<42} {}", self.name, self.note) }
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
    let (at, note) = word(&rest);
    if at.is_empty() {
        return Err(format!("{name}: no X,Y,Z,YAW,PITCH after the name"));
    }
    let v: Vec<f32> = at.split(',').map(|v| v.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|_| format!("{name}: {at} is not X,Y,Z,YAW,PITCH"))?;
    let at = <[f32; 5]>::try_from(v).map_err(|v| format!("{name}: {} numbers, not five (X,Y,Z,YAW,PITCH)", v.len()))?;
    Ok(Some(Place { name, at, note }))
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

/// Where `name` is, or why not (the names there are, if it is not one).
pub fn find(name: &str) -> Result<[f32; 5], String> {
    let places = read()?;
    match places.iter().rev().find(|p| p.name == name) {
        Some(p) => Ok(p.at),
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
        assert_eq!(places[0], Place { name: "tiers".into(), at: [5248.0, 2.0, 640.0, 0.0, 0.0], note: "the tiers' sample point".into() });
        assert_eq!(places[1].note, "");
        // Replaced in its line, the comment and the other kept.
        let new = Place { name: "tiers".into(), at: [1.0, 2.0, 3.0, 4.0, 5.0], note: "moved".into() };
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
        let (out, old) = saved(text, &Place { name: "arena".into(), at: [0.0; 5], note: String::new() });
        assert!(old.is_none());
        assert_eq!(parse(&out).unwrap().last().unwrap().name, "arena");
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
