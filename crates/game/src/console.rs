//! A console, the Quake kind: the key under Esc (`~` or `` ` ``, by where it
//! is, whatever the layout) drops it down from the top of the screen; type
//! a command, Enter runs it; Esc or the same key puts it away. It reads and
//! sets the same switches the command line does (`--opt NAME`, `--set
//! NAME=VALUE`) as the game runs: whatever reads them each frame follows at
//! once (`nolampshadows`, `ev`, `fog`, ...); the camera's contact shadows
//! and temporal anti-aliasing, set up at the start, follow here
//! (`nocontact`, `notaa`). While it is down the game sees no keys or
//! clicks, and the mouse is free.
//!
//! `--opt console` starts with it down, listing what acts at once. A
//! setting is its name then its value: `NAME on`, `NAME off` (a switch),
//! `NAME VALUE`, `NAME default` (a number), `NAME` alone its value. And
//! `help`; `list` (what acts at once, then all else on or set); `where`
//! (the camera, as `--cam`; and copies it); `tp NAME` and `savetp NAME`
//! (places kept by name, see `places.rs`); `clear`. Up brings back the
//! last command (kept between runs); Left, Right, Home, End, Delete and
//! Ctrl with Left, Right or Backspace edit the line; Tab completes a word;
//! Ctrl+V pastes (a line at a time run), Ctrl+C copies the line and clears it.
//! `--do COMMAND` on the command line runs one at the start.

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardInput},
    },
    light::ShadowFilteringMethod,
    clipboard::Clipboard,
    pbr::ContactShadows,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{Args, camera::FlyCam};

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Console { history: remembered(), ..default() })
            .init_resource::<Spawns>()
            .add_systems(Startup, spawn)
            .add_systems(PreUpdate, (type_in, swallow).chain().after(InputSystems))
            .add_systems(Update, (show, follow, loudness, clock, given.run_if(crate::loading::playing)))
            .add_systems(PostUpdate, goto.before(TransformSystems::Propagate));
    }
}

/// What `spawn` at the console asks for: creatures, their kind, how many,
/// their options (see `traits.rs`), and where (each in turn at one of
/// these; none, before you). And a round (`round`: what it is, see
/// `combat::round`), and `reset` (every creature gone, your health back).
#[derive(Resource, Default)]
pub struct Spawns {
    pub creatures: Vec<(String, u32, crate::traits::Traits, Vec<Vec3>)>,
    pub round: Option<String>,
    pub reset: bool,
    /// Where `cam` moves the camera to (as `--cam`), at the frame's end:
    /// after what was spawned before it in the same breath.
    pub goto: Option<[f32; 5]>,
    /// Models `show` asks to stand before you (see `show.rs`), and whether
    /// those shown go.
    pub shows: Vec<crate::show::Asked>,
    pub unshow: bool,
}

/// The kinds `spawn` knows, besides the creatures of `creatures/` (each by
/// its name, see `options_kind`).
pub const KINDS: [&str; 12] = ["harrow", "angler", "biped", "sprawl", "stalker", "porter", "chained", "bud", "grove", "bishop", "impaled", "tumbler"];

/// Every kind `spawn` knows: `KINDS` and the creatures of `creatures/`.
fn kinds() -> Vec<String> {
    KINDS.iter().map(|k| k.to_string()).chain(crate::combat::creatures().iter().cloned()).collect()
}

/// The kind whose options `kind` takes: a creature of `creatures/` takes
/// those of an authored body (`clip`), the rest their own.
fn options_kind(kind: &str) -> &str {
    if crate::combat::creatures().iter().any(|c| c == kind) { "clip" } else { kind }
}

/// A line about the options of `kind`, said of it by its own name.
fn named(kind: &str, line: String) -> String {
    if options_kind(kind) == "clip" { line.replacen("clip", kind, 1) } else { line }
}

/// The console: whether it is down, the line being typed and where in it
/// the cursor is (in characters), what it has said, the commands run (for
/// Up), how far down it is (0 away, 1 down); what to copy to the clipboard.
#[derive(Resource, Default)]
pub struct Console {
    pub open: bool,
    line: String,
    at: usize,
    lines: Vec<String>,
    history: Vec<String>,
    back: usize,
    down: f32,
    closing: bool,
    copy: Option<String>,
    /// The commands run this session that made the scene (`spawn`, `show`,
    /// `round`, `reset`), in order: what a screenshot needs to be seen
    /// again (F12, see `capture.rs`).
    pub scene: Vec<String>,
    /// `shot`: a screenshot asked for, as F12 (see `capture.rs`).
    pub shoot: bool,
}

/// The commands that make the scene, kept for screenshots (see
/// `Console::scene`).
const SCENE: [&str; 4] = ["spawn", "show", "round", "reset"];

/// How many commands are kept between runs.
const REMEMBERED: usize = 500;

/// Where the commands run are kept between runs: beside the game.
fn history_file() -> Option<std::path::PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("console_history.txt"))
}

/// The commands run before, the last `REMEMBERED`.
fn remembered() -> Vec<String> {
    let text = history_file().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let lines: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
    lines[lines.len().saturating_sub(REMEMBERED)..].to_vec()
}

/// A command run, kept for the next run (said if it cannot be).
fn remember(line: &str) {
    use std::io::Write;
    let Some(path) = history_file() else { return };
    let kept = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| writeln!(f, "{line}"));
    if let Err(e) = kept {
        warn!("console: could not keep {line:?} in {}: {e}", path.display());
    }
}

/// The byte where character `at` of `line` starts.
fn byte(line: &str, at: usize) -> usize {
    line.char_indices().nth(at).map_or(line.len(), |(i, _)| i)
}

/// Where the word before (`-1`) or after (`1`) character `at` ends.
fn word(line: &str, at: usize, way: i32) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let mut i = at.min(chars.len());
    if way < 0 {
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        while i > 0 && chars[i - 1] != ' ' {
            i -= 1;
        }
    } else {
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
        while i < chars.len() && chars[i] != ' ' {
            i += 1;
        }
    }
    i
}

impl Console {
    /// `text` put in the line at the cursor, the cursor after it.
    fn insert(&mut self, text: &str) {
        let b = byte(&self.line, self.at);
        self.line.insert_str(b, text);
        self.at += text.chars().count();
    }

    /// The characters from `from` to `to` taken out of the line, the
    /// cursor at `from`.
    fn cut(&mut self, from: usize, to: usize) {
        let (a, b) = (byte(&self.line, from), byte(&self.line, to));
        self.line.replace_range(a..b, "");
        self.at = from;
    }

    /// The line from the history, the cursor at its end.
    fn recall(&mut self, line: String) {
        self.at = line.chars().count();
        self.line = line;
    }
}

impl Console {
    /// A line said in it (a round's result).
    pub fn say(&mut self, line: String) {
        self.lines.push(line);
    }
}

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Lines;

/// The line's letter at the cursor (lit), and the rest of the line after it.
#[derive(Component)]
struct AtCursor;
#[derive(Component)]
struct AfterCursor;

/// The colour the letter at the cursor is shown in.
const CURSOR: Color = Color::srgb(1.0, 0.85, 0.3);

/// How much of the screen it takes when down.
const HEIGHT: f32 = 45.0;
/// How many lines of what it has said it shows.
const SHOWN: usize = 22;

fn spawn(mut commands: Commands, mut args: ResMut<Args>, mut console: ResMut<Console>) {
    // (`--opt console`: down from the start, listing what acts at once.)
    if args.opt("console") {
        console.open = true;
        console.lines.push("console: help for the commands".into());
        let said = run("list", args.bypass_change_detection());
        console.lines.extend(said);
    }
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(-HEIGHT),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(HEIGHT),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.88)),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            let font = TextFont { font_size: FontSize::Px(14.0), ..default() };
            panel.spawn((Lines, Text::new(""), font.clone(), TextColor(Color::srgb(0.8, 0.8, 0.8)))).with_children(|lines| {
                lines.spawn((AtCursor, TextSpan::new(""), font.clone(), TextColor(CURSOR)));
                lines.spawn((AfterCursor, TextSpan::new(""), font.clone(), TextColor(Color::srgb(0.8, 0.8, 0.8))));
            });
        });
}

/// Typing: the key under Esc puts it down or away; down, characters go on
/// the line, Backspace takes one off, Enter runs it, Up brings back the
/// last command, Esc puts it away.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn type_in(
    mut keys: MessageReader<KeyboardInput>,
    mut ctrl: Local<bool>,
    mut clipboard: Option<ResMut<Clipboard>>,
    mut console: ResMut<Console>,
    mut args: ResMut<Args>,
    mut spawns: ResMut<Spawns>,
    mut cursor: Single<&mut CursorOptions>,
    camera: Single<(&Transform, &FlyCam)>,
    world: Option<Res<crate::terrain::WorldGen>>,
    lab: Option<Res<crate::groundlab::LabGround>>,
    mut exit: MessageWriter<AppExit>,
) {
    let camfloor = args.opt("camfloor");
    let ground = |x: f32, z: f32| crate::places::ground(world.as_deref(), lab.as_deref(), camfloor, x, z);
    for key in keys.read() {
        // (Ctrl held, from its own presses and releases: the keys held are
        // cleared each frame while the console is down (see `swallow`), so
        // Ctrl was let go by the time V came, and a v was typed, the user.)
        if matches!(key.key_code, KeyCode::ControlLeft | KeyCode::ControlRight) {
            *ctrl = key.state == ButtonState::Pressed;
            continue;
        }
        if key.state != ButtonState::Pressed {
            continue;
        }
        let ctrl = *ctrl;
        if key.key_code == KeyCode::Backquote || (console.open && key.logical_key == Key::Escape) {
            console.open = !console.open;
            // (The keys of the frame it goes away in are the console's too:
            // Esc would free the mouse again.)
            console.closing = !console.open;
            // (Down, the mouse free; away, captured again for looking.)
            cursor.visible = console.open;
            cursor.grab_mode = if console.open { CursorGrabMode::None } else { CursorGrabMode::Locked };
            continue;
        }
        if !console.open {
            continue;
        }
        let length = console.line.chars().count();
        let at = console.at.min(length);
        match &key.logical_key {
            Key::Enter => enter(&mut console, &mut args, &mut spawns, *camera, &ground, &mut exit),
            Key::Backspace if ctrl => {
                let from = word(&console.line, at, -1);
                console.cut(from, at);
            }
            Key::Backspace if at > 0 => console.cut(at - 1, at),
            Key::Delete if at < length => console.cut(at, at + 1),
            Key::ArrowLeft => console.at = if ctrl { word(&console.line, at, -1) } else { at.saturating_sub(1) },
            Key::ArrowRight => console.at = if ctrl { word(&console.line, at, 1) } else { (at + 1).min(length) },
            Key::Home => console.at = 0,
            Key::End => console.at = length,
            Key::Tab => complete(&mut console, &args),
            Key::ArrowUp => {
                if console.back < console.history.len() {
                    console.back += 1;
                    let line = console.history[console.history.len() - console.back].clone();
                    console.recall(line);
                }
            }
            Key::ArrowDown => {
                console.back = console.back.saturating_sub(1);
                let line = if console.back == 0 { String::new() } else { console.history[console.history.len() - console.back].clone() };
                console.recall(line);
            }
            // Pasted: at the cursor; each line ended in it run, as if typed
            // and Enter pressed (a whole setting-up pasted at once).
            _ if ctrl && key.key_code == KeyCode::KeyV => {
                let Some(board) = clipboard.as_deref_mut() else { continue };
                let Some(Ok(text)) = board.fetch_text().poll_result() else { continue };
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                let mut parts = text.split('\n').peekable();
                while let Some(part) = parts.next() {
                    console.at = console.at.min(console.line.chars().count());
                    console.insert(&part.chars().filter(|c| !c.is_control()).collect::<String>());
                    if parts.peek().is_some() {
                        enter(&mut console, &mut args, &mut spawns, *camera, &ground, &mut exit);
                    }
                }
            }
            // Copied, and the line cleared, as a terminal's Ctrl+C gives it
            // up (the user, 2026-11-07). (An empty line copies nothing: what
            // is on the clipboard kept.)
            _ if ctrl && key.key_code == KeyCode::KeyC => {
                console.copy = (!console.line.is_empty()).then(|| console.line.clone());
                console.recall(String::new());
                console.back = 0;
            }
            _ if ctrl => {}
            _ => {
                if let Some(text) = &key.text {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    console.at = at;
                    console.insert(&text);
                }
            }
        }
    }
    // (What `where` said, or the line, to the clipboard.)
    if let Some(text) = console.copy.take()
        && let Some(board) = clipboard.as_deref_mut()
        && let Err(e) = board.set_text(text)
    {
        console.lines.push(format!("(not copied: {e:?})"));
    }
}

/// The line run, as Enter: said back, kept in the history (and for the next
/// run), done.
fn enter(console: &mut Console, args: &mut Args, spawns: &mut Spawns, camera: (&Transform, &FlyCam), ground: &dyn Fn(f32, f32) -> f32, exit: &mut MessageWriter<AppExit>) {
    let line = std::mem::take(&mut console.line);
    console.at = 0;
    console.lines.push(format!("> {line}"));
    if !line.trim().is_empty() {
        console.history.push(line.clone());
        remember(&line);
    }
    console.back = 0;
    execute(&line, console, args, spawns, camera, ground, exit);
}

/// Tab: the word at the cursor completed, from what can be said there: a
/// command or a setting first; a creature's kind after `spawn` or `round`;
/// then its options. One, completed; several, as far as they agree, and
/// listed.
fn complete(console: &mut Console, args: &Args) {
    let at = console.at.min(console.line.chars().count());
    console.at = at;
    let before: String = console.line.chars().take(at).collect();
    let words: Vec<&str> = before.split(' ').collect();
    let (done, partial) = (&words[..words.len() - 1], words[words.len() - 1]);
    let done: Vec<&str> = done.iter().copied().filter(|w| !w.is_empty()).collect();
    let mut choices: Vec<String> = match done.as_slice() {
        [] => ["spawn", "round", "reset", "cam", "tp", "savetp", "show", "shot", "where", "list", "help", "clear", "quit"]
            .iter()
            .map(|c| c.to_string())
            .chain(SWITCHES.iter().map(|s| s.0.to_string()))
            .chain(numbers(args).iter().map(|n| n.0.to_string()))
            .collect(),
        [command] if *command == "spawn" || *command == "round" => kinds(),
        [command, kind, ..] if *command == "spawn" || *command == "round" => crate::traits::OPTIONS
            .iter()
            .filter(|(kinds, ..)| kinds.contains(&options_kind(kind)))
            .map(|(_, name, number, ..)| if *number { format!("{name}=") } else { name.to_string() })
            .collect(),
        [command] if *command == "tp" || *command == "savetp" => crate::places::names(),
        [command] if *command == "show" => crate::show::files().into_iter().chain(["off".to_string()]).collect(),
        [command, _, ..] if *command == "show" => ["height=", "ahead=", "turn=", "spin"].iter().map(|s| s.to_string()).collect(),
        [name] if SWITCHES.iter().any(|s| s.0 == *name) => vec!["on".into(), "off".into()],
        _ => Vec::new(),
    };
    choices.retain(|c| c.starts_with(partial));
    choices.sort();
    choices.dedup();
    let Some(first) = choices.first() else { return };
    // (As far as all of them agree.)
    let common: String = first.chars().enumerate().take_while(|&(i, c)| choices.iter().all(|o| o.chars().nth(i) == Some(c))).map(|(_, c)| c).collect();
    let rest: String = common.chars().skip(partial.chars().count()).collect();
    console.insert(&rest);
    if choices.len() == 1 {
        if !common.ends_with('=') {
            console.insert(" ");
        }
    } else {
        console.lines.push(choices.join("  "));
    }
}

/// A command run: typed (Enter), or given on the command line (`--do`, see
/// `given`).
#[allow(clippy::too_many_arguments)]
fn execute(line: &str, console: &mut Console, args: &mut Args, spawns: &mut Spawns, camera: (&Transform, &FlyCam), ground: &dyn Fn(f32, f32) -> f32, exit: &mut MessageWriter<AppExit>) {
    if line.split_whitespace().next().is_some_and(|w| SCENE.contains(&w)) {
        console.scene.push(line.trim().to_string());
    }
    if line.trim() == "clear" {
        console.lines.clear();
    } else if line.trim() == "shot" {
        console.shoot = true;
    } else if line.trim() == "quit" || line.trim() == "exit" {
        exit.write(AppExit::Success);
    } else if let Some(rest) = line.trim().strip_prefix("cam ") {
        // (`where`'s answer back: the camera there, so a creature spawned
        // can be seen round, `--do` after `--do`.)
        let v: Vec<f32> = rest.split([',', ' ']).filter(|w| !w.is_empty()).filter_map(|w| w.parse().ok()).collect();
        match <[f32; 5]>::try_from(v) {
            Ok(at) => {
                spawns.goto = Some(at);
                console.lines.push(format!("the camera to {rest}"));
            }
            Err(_) => console.lines.push("cam X,Y,Z,YAW,PITCH (as `where` says it)".into()),
        }
    } else if line.trim() == "tp" || line.trim().starts_with("tp ") {
        // (A place kept by name, see `places.rs`: there, as `cam`; alone,
        // the places listed.)
        match line.split_whitespace().nth(1) {
            None => match crate::places::listed() {
                Ok(listed) => console.lines.extend(listed),
                Err(e) => console.lines.push(e),
            },
            Some(name) => match crate::places::find(name) {
                // (In this world: there. In another: said, and how to
                // start there.)
                Ok(place) if place.world == crate::places::World::of(args) => {
                    spawns.goto = Some(place.at);
                    console.lines.push(format!("to {name}"));
                }
                Ok(place) => console.lines.push(format!("{name} is in another world: start with --tp {name} (it brings {})", if place.world.flags().is_empty() { "outdoors, seed 1".to_string() } else { place.world.flags() })),
                Err(e) => console.lines.push(e),
            },
        }
    } else if line.trim() == "savetp" || line.trim().starts_with("savetp ") {
        // (Where you are and look, as `where` says it, kept by name; the
        // place it replaces said, so that it is not lost unseen.)
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.get(1) {
            None => console.lines.push("savetp NAME [note]".into()),
            Some(name) => {
                let (t, fly) = camera;
                let p = t.translation;
                let above = p.y - ground(p.x, p.z);
                let place = crate::places::Place { name: name.to_string(), at: [p.x, above, p.z, fly.yaw.to_degrees(), fly.pitch.to_degrees()], world: crate::places::World::of(args), note: words[2..].join(" ") };
                match crate::places::save(&place) {
                    Ok(None) => console.lines.push(format!("{name} kept in {}", crate::places::file().display())),
                    Ok(Some(old)) => {
                        let [x, y, z, yaw, pitch] = old.at;
                        console.lines.push(format!("{name} kept, in place of {x},{y},{z},{yaw},{pitch} {}", old.note));
                    }
                    Err(e) => console.lines.push(e),
                }
            }
        }
    } else if line.trim() == "show" || line.trim().starts_with("show ") {
        // (A model of `creatures/` stood before you, to see it, see
        // `show.rs`: `height=M` tall (2.6), `ahead=M` before you (4),
        // `turn=DEG` from facing you, `spin` turning slowly round; `show
        // off`, those shown gone; alone, the files listed.)
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.get(1) {
            None => console.lines.push(format!("show FILE [height=M] [ahead=M] [turn=DEG] [spin]  (show off: gone): {}", crate::show::files().join("  "))),
            Some(&"off") => {
                spawns.unshow = true;
                console.lines.push("those shown gone".into());
            }
            Some(file) if !crate::show::files().iter().any(|f| f == file) => console.lines.push(format!("{file}: not in {}; there are {}", crate::combat::creatures_folder().display(), crate::show::files().join("  "))),
            Some(file) => {
                let mut asked = crate::show::Asked { file: file.to_string(), height: 2.6, at: Vec3::ZERO, facing: 0.0, spin: false };
                let (mut ahead, mut turn, mut wrong) = (4.0, 0.0, None);
                for w in &words[2..] {
                    let number = |v: &str| v.parse::<f32>().ok();
                    match w.split_once('=') {
                        Some(("height", v)) if number(v).is_some_and(|h| h > 0.0) => asked.height = number(v).expect("checked"),
                        Some(("ahead", v)) if number(v).is_some() => ahead = number(v).expect("checked"),
                        Some(("turn", v)) if number(v).is_some() => turn = number(v).expect("checked"),
                        None if *w == "spin" => asked.spin = true,
                        _ => wrong = Some(*w),
                    }
                }
                match wrong {
                    Some(w) => console.lines.push(format!("{w}: show takes height=M, ahead=M, turn=DEG and spin")),
                    None => {
                        let t = camera.0;
                        let forward = Vec3::new(t.forward().x, 0.0, t.forward().z).normalize_or(Vec3::Z);
                        asked.at = t.translation + forward * ahead;
                        // (Its +Z, a generated model's front, towards you.)
                        asked.facing = (-forward.x).atan2(-forward.z) + turn.to_radians();
                        console.lines.push(format!("{file}, {} m tall, {ahead} m before you", asked.height));
                        spawns.shows.push(asked);
                    }
                }
            }
        }
    } else if line.trim() == "mark" {
        // (A mark in the rig's trace, see the `rigtrace` option: now, what
        // you saw.)
        crate::combat::trace("rigtrace: MARK");
        console.lines.push(format!("marked in {}", crate::combat::trace_file()));
    } else if line.trim() == "reset" {
        spawns.reset = true;
        console.lines.push("every creature gone, your health back".into());
    } else if line.trim().starts_with("spawn") || line.trim().starts_with("round") {
        // `spawn KIND [N] [option ...]`: N of them (1) before you,
        // with their options; `spawn KIND --help` (or `help`, `?`),
        // what it takes. `round` the same, timed (see
        // `combat::round`), in the arena through its gates.
        let words: Vec<&str> = line.split_whitespace().collect();
        let round = words[0] == "round";
        let gates = round && args.opt("arena");
        let count = words.get(2).and_then(|n| n.parse::<u32>().ok());
        // (`ahead=M`: M metres before you, where you are now, not 3 m
        // before wherever you are when it forms: so a chase can be set up
        // from the command line, `--do "spawn gladiator ahead=40"`.)
        let ahead = words.iter().find_map(|w| w.strip_prefix("ahead=")).map(|m| m.parse::<f32>());
        let words: Vec<&str> = words.iter().copied().filter(|w| !w.starts_with("ahead=")).collect();
        let options = &words[(2 + count.is_some() as usize).min(words.len())..];
        match words.get(1) {
            _ if matches!(ahead, Some(Err(_))) => console.lines.push("ahead=M: how many metres before you, a number".into()),
            Some(kind) if kinds().iter().any(|k| k == kind) && matches!(options, ["?"] | ["help"] | ["--help"]) => console.lines.push(named(kind, crate::traits::help(options_kind(kind)))),
            Some(kind) if kinds().iter().any(|k| k == kind) => match crate::traits::Traits::parse(options_kind(kind), options) {
                Ok(traits) => {
                    let count = count.unwrap_or(1).clamp(1, 20);
                    let there = ahead.and_then(Result::ok).map(|m| {
                        let t = camera.0;
                        let forward = Vec3::new(t.forward().x, 0.0, t.forward().z).normalize_or(Vec3::Z);
                        (m, t.translation + forward * m)
                    });
                    let places = match there {
                        Some((_, at)) => vec![at],
                        None if gates => crate::chasm::arena::pens(),
                        None => Vec::new(),
                    };
                    spawns.creatures.push((kind.to_string(), count, traits, places));
                    let with = if options.is_empty() { String::new() } else { format!(": {}", options.join(" ")) };
                    if round {
                        spawns.round = Some(format!("{count} {kind}{with}"));
                    }
                    let place = match there {
                        Some((m, _)) => format!("{m} m before you"),
                        None if gates => "through the gates".into(),
                        None => "before you".into(),
                    };
                    console.lines.push(format!("{count} {kind} {place}{with}"));
                }
                Err(e) => console.lines.push(named(kind, e)),
            },
            _ => console.lines.push(format!("{} what? {}", words[0], kinds().join(", "))),
        }
    } else if line.trim() == "where" {
        // (Its height above the ground, as `--cam` takes it; also
        // in the log, where it can be copied, the user.)
        let (t, fly) = camera;
        let p = t.translation;
        let above = p.y - ground(p.x, p.z);
        let cam = format!("--cam {:.2},{:.2},{:.2},{:.2},{:.2}", p.x, above, p.z, fly.yaw.to_degrees(), fly.pitch.to_degrees());
        info!("where: {cam}");
        console.copy = Some(cam.clone());
        console.lines.push(format!("{cam}  (copied)"));
    } else {
        let said = run(line, args);
        console.lines.extend(said);
    }
}

/// `--do COMMAND` (repeatable): run as if typed, once the game is on (so a
/// fight can be set up the same way again, from the command line); what
/// they say also in the log. With `--opt blink`, once the district nodes
/// are built too (spawned before, a walker went as in the open, up a 2 m
/// ledge it was to hop).
#[allow(clippy::too_many_arguments)]
fn given(mut done: Local<bool>, mut console: ResMut<Console>, mut args: ResMut<Args>, mut spawns: ResMut<Spawns>, camera: Single<(&Transform, &FlyCam)>, world: Option<Res<crate::terrain::WorldGen>>, lab: Option<Res<crate::groundlab::LabGround>>, districts: Option<Res<crate::navmesh::district::Districts>>, mut exit: MessageWriter<AppExit>) {
    if *done || (args.opt("blink") && districts.is_none_or(|d| d.nodes.is_none())) {
        return;
    }
    *done = true;
    let camfloor = args.opt("camfloor");
    let ground = |x: f32, z: f32| crate::places::ground(world.as_deref(), lab.as_deref(), camfloor, x, z);
    for line in args.run.clone() {
        let before = console.lines.len();
        console.lines.push(format!("> {line}"));
        execute(&line, &mut console, &mut args, &mut spawns, *camera, &ground, &mut exit);
        for said in &console.lines[before..] {
            info!("console: {said}");
        }
    }
}

/// `cam` or `tp` at the console: the camera moved there (its height above
/// the ground, as `--cam` and `where`, see `places::ground`), at the end of
/// the frame.
fn goto(mut spawns: ResMut<Spawns>, args: Res<Args>, world: Option<Res<crate::terrain::WorldGen>>, lab: Option<Res<crate::groundlab::LabGround>>, camera: Single<(&mut Transform, &mut FlyCam)>) {
    let Some([x, above, z, yaw, pitch]) = spawns.goto.take() else { return };
    let (mut t, mut fly) = camera.into_inner();
    t.translation = Vec3::new(x, above + crate::places::ground(world.as_deref(), lab.as_deref(), args.opt("camfloor"), x, z), z);
    fly.yaw = yaw.to_radians();
    fly.pitch = pitch.to_radians();
    t.rotation = fly.rotation();
}

/// While it is down, the game sees no keys or clicks.
fn swallow(mut console: ResMut<Console>, mut keys: ResMut<ButtonInput<KeyCode>>, mut mouse: ResMut<ButtonInput<MouseButton>>) {
    if console.open || console.closing {
        keys.reset_all();
        mouse.reset_all();
        console.closing = false;
    }
}

/// The switches that take effect as the game runs, and what each does.
const SWITCHES: [(&str, &str); 7] = [
    ("mute", "no sound"),
    ("still", "the time of day held where it is (T in the game too)"),
    ("god", "nothing hurts you (for testing)"),
    ("fullbright", "an even light from everywhere, no shadows: to see the shapes"),
    ("nolampshadows", "no shadows from lamps"),
    ("nocontact", "no contact shadows (screen space)"),
    ("notaa", "no temporal anti-aliasing (nor temporal shadow filtering)"),
];

/// How loud unless set (of as made: the user, a tenth, 2026-10-31).
const VOLUME: f32 = 0.1;

/// The numbers that take effect as the game runs, what each does, and
/// what it is unless set.
fn numbers(args: &Args) -> [(&'static str, &'static str, f32); 6] {
    [
        ("volume", "how loud (1 as made, 0 silent)", VOLUME),
        ("timescale", "how fast the game's time runs (1 real time; 4 four times as fast, 0.25 slow)", 1.0),
        ("shadowed", "how many lamps cast shadows at once (0-8; each about 0.5 ms)", 8.0),
        ("ev", "exposure (EV100; higher is darker)", 11.2),
        ("fog", "how far the haze lets you see (m)", if args.opt("chasm") { 600.0 } else { 4500.0 }),
        ("fog_glow", "the haze's glow towards the suns", 0.15),
    ]
}

/// Runs a command on the switches, and what it says back. A setting is
/// its name then its value, as it is written back: `fullbright on`,
/// `fullbright off`, `ev 9`, `ev default`; its name alone, its value.
fn run(line: &str, args: &mut Args) -> Vec<String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    // (A name the console knows: one that acts at once, or one given at
    // the start or set since. Any other, said so, not read back as off as
    // though it were one (the user, mistyping `shadowed`), with the
    // nearest it knows.)
    let known = |args: &Args, name: &str| SWITCHES.iter().any(|s| s.0 == name) || numbers(args).iter().any(|n| n.0 == name) || args.opts.iter().any(|o| o == name) || args.sets.iter().any(|(k, _)| k == name);
    let hint = |args: &Args, name: &str| {
        let names: Vec<&str> = SWITCHES.iter().map(|s| s.0).chain(numbers(args).iter().map(|n| n.0)).collect();
        let near = names.into_iter().map(|n| (distance(n, name), n)).filter(|(d, n)| *d <= 3 || n.chars().zip(name.chars()).take_while(|(a, b)| a == b).count() >= 5).min();
        near.map_or("; list shows those that do".into(), |(_, n)| format!("; did you mean {n}?"))
    };
    let unknown = |args: &Args, name: &str| format!("{name}: not a setting that acts as the game runs{}", hint(args, name));
    let warned = |args: &Args, name: &str| format!("  (not a setting that acts as the game runs: it may do nothing now{})", hint(args, name));
    // (A number known to act at once shows its default when not set.)
    let value = |args: &Args, name: &str| {
        if let Some((_, _, default)) = numbers(args).into_iter().find(|n| n.0 == name) {
            let v = args.num(name, default);
            return format!("{name} {v}{}", if v == default { " (default)" } else { "" });
        }
        match args.sets.iter().rev().find(|(k, _)| k == name) {
            Some((_, v)) => format!("{name} {v}"),
            None => format!("{name} {}", if args.opt(name) { "on" } else { "off" }),
        }
    };
    match words.as_slice() {
        [] => Vec::new(),
        ["help"] => vec![
            "NAME                 its value".into(),
            "NAME on, NAME off    a switch (--opt NAME)".into(),
            "NAME VALUE           a number (--set NAME=VALUE); NAME default, back to its default".into(),
            "list                 what acts at once, then all else on or set".into(),
            "where                where you are and look, as --cam X,Y,Z,YAW,PITCH (Y as above a ground at 0)".into(),
            "spawn KIND [N] [OPTION ...]  N creatures 3 m before you (ahead=M: M m), with their options (`spawn KIND --help`".into(),
            "                     lists them): harrow, angler, biped, sprawl, stalker, porter, chained (a".into(),
            "                     stalker keeping a porter on a chain), bud, grove (7 asleep), bishop".into(),
            "round KIND [N] [OPTION ...]  the same, as a round: timed, and what it took said at its end (in".into(),
            "                     the arena, through its gates)".into(),
            "reset                every creature gone, your health back".into(),
            "cam X,Y,Z,YAW,PITCH  the camera there and that way (as `where` says it)".into(),
            "tp [NAME]            to a place kept by name in places.txt (--tp NAME at the start); alone, them listed".into(),
            "savetp NAME [note]   where you are kept by that name in places.txt (one there before: replaced, said)".into(),
            "show FILE [height=M] [ahead=M] [turn=DEG] [spin]  a model of creatures/ stood before you, M tall (2.6),".into(),
            "                     facing you, still (no rig); show off, gone; show alone, the files".into(),
            "shot                 a screenshot, as F12: screenshots/STAMP.png, the place shot_STAMP kept, the way there in STAMP.txt".into(),
            "clear                clears this".into(),
            "Keys: Up/Down the commands before (kept between runs); Left, Right, Home, End, Delete,".into(),
            "Ctrl+Left/Right a word, Ctrl+Backspace a word back; Tab completes; Ctrl+V pastes (each".into(),
            "line run); Ctrl+C copies the line and clears it; `where` copies what it says.".into(),
            "quit                 leaves the game".into(),
            "Others act if read each frame; many only count at the start.".into(),
        ],
        ["list"] => {
            // (Those that take effect as the game runs, always; then
            // anything else on or set, which may only have counted at the
            // start.)
            let mut out = vec!["live:".to_string()];
            for (name, what) in SWITCHES {
                out.push(format!("  {}    {what}", value(args, name)));
            }
            for (name, what, _) in numbers(args) {
                out.push(format!("  {}    {what}", value(args, name)));
            }
            let others: Vec<String> = args.opts.iter().filter(|o| !SWITCHES.iter().any(|s| s.0 == o.as_str())).cloned().collect();
            out.push(format!("on: {}", if others.is_empty() { "nothing else".into() } else { others.join(", ") }));
            let mut sets: Vec<String> = Vec::new();
            for (k, v) in args.sets.iter().rev() {
                if numbers(args).iter().any(|n| n.0 == k) {
                    continue;
                }
                if !sets.iter().any(|s| s.starts_with(&format!("{k} "))) {
                    sets.push(format!("{k} {v}"));
                }
            }
            sets.reverse();
            out.push(format!("set: {}", if sets.is_empty() { "nothing else".into() } else { sets.join(", ") }));
            out
        }
        [name, "on"] => {
            let was = known(args, name);
            if !args.opt(name) {
                args.opts.push(name.to_string());
            }
            if was { vec![value(args, name)] } else { vec![value(args, name), warned(args, name)] }
        }
        // (Off, a name it does not know, said so: the user's `time off`
        // answered as though it had done something.)
        [name, "off"] => {
            let was = known(args, name);
            args.opts.retain(|o| o != name);
            if was { vec![value(args, name)] } else { vec![unknown(args, name)] }
        }
        [name, "default"] => {
            args.sets.retain(|(k, _)| k != name);
            vec![value(args, name)]
        }
        [name, v] => match v.parse::<f32>() {
            Ok(v) => {
                let was = known(args, name);
                args.sets.retain(|(k, _)| k != name);
                args.sets.push((name.to_string(), v));
                if was { vec![value(args, name)] } else { vec![value(args, name), warned(args, name)] }
            }
            Err(_) => vec![format!("{name}: on, off, a number or default, not {v}")],
        },
        [name] if known(args, name) => vec![value(args, name)],
        [name] => vec![unknown(args, name)],
        _ => vec![format!("not understood: {line} (help)")],
    }
}

/// How many letters must change to make one word the other (Levenshtein).
pub(crate) fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (row[j + 1] + 1).min(row[j] + 1).min(prev + usize::from(ca != *cb));
            prev = here;
        }
    }
    row[b.len()]
}

/// The game's time runs as fast as `timescale` says (the user: to wait less
/// for what moves slowly, or watch it slowed). Not with the walk bot, which
/// runs it as fast as it can (`walkspeed`). (A system that steps no more than
/// so far in a frame, as the creatures and the cloth do, keeps to that: at
/// a high scale and a low frame rate, it lags behind.)
fn clock(args: Res<Args>, mut time: ResMut<Time<Virtual>>, mut last: Local<f32>) {
    if args.opt("walkbot") {
        return;
    }
    let scale = args.num("timescale", 1.0).max(0.01);
    if scale != *last {
        time.set_relative_speed(scale);
        *last = scale;
    }
}

/// The sound follows `volume` and `mute` (and is silent when nobody is
/// playing): what starts from now on (the global volume) and what is
/// already sounding (each playing sound's own, rescaled; muted at 0).
fn loudness(
    args: Res<Args>,
    mut global: ResMut<bevy::audio::GlobalVolume>,
    mut sinks: Query<&mut bevy::audio::AudioSink>,
    mut spatial: Query<&mut bevy::audio::SpatialAudioSink>,
    mut last: Local<Option<(f32, f32)>>,
) {
    use bevy::audio::{AudioSinkPlayback, Volume};
    let v = if args.unattended() || args.opt("mute") { 0.0 } else { args.num("volume", VOLUME).max(0.0) };
    // (What it was, and what it was last when not silent.)
    let (was, heard) = *last.get_or_insert((global.volume.to_linear(), global.volume.to_linear().max(1e-3)));
    if v == was {
        return;
    }
    global.volume = Volume::Linear(v);
    let scale = if v > 0.0 { v / heard } else { 1.0 };
    let each = |sink: &mut dyn AudioSinkPlayback| {
        if v == 0.0 {
            sink.mute();
        } else {
            sink.unmute();
            sink.set_volume(Volume::Linear(sink.volume().to_linear() * scale));
        }
    };
    for mut sink in &mut sinks {
        each(&mut *sink);
    }
    for mut sink in &mut spatial {
        each(&mut *sink);
    }
    *last = Some((v, if v > 0.0 { v } else { heard }));
}

/// Slides it down or up, and shows what it has said and the line.
#[allow(clippy::type_complexity)]
fn show(
    time: Res<Time>,
    mut console: ResMut<Console>,
    panel: Single<(&mut Node, &mut Visibility), With<Panel>>,
    mut text: Single<&mut Text, With<Lines>>,
    mut at: Single<&mut TextSpan, (With<AtCursor>, Without<AfterCursor>)>,
    mut after: Single<&mut TextSpan, (With<AfterCursor>, Without<AtCursor>)>,
) {
    let target = if console.open { 1.0 } else { 0.0 };
    let step = time.delta_secs() * 6.0;
    let down = console.down + (target - console.down).clamp(-step, step);
    if down != console.down {
        console.down = down;
    }
    let (mut node, mut visibility) = panel.into_inner();
    node.top = Val::Percent(-HEIGHT * (1.0 - down));
    let seen = if down > 0.0 { Visibility::Visible } else { Visibility::Hidden };
    if *visibility != seen {
        *visibility = seen;
    }
    if down > 0.0 {
        let from = console.lines.len().saturating_sub(SHOWN);
        let mut out = console.lines[from..].join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        // (The cursor: its letter lit, a space as `_`, past the end a `_`;
        // a bar put in before it hid the letter, the user.)
        let b = byte(&console.line, console.at);
        let rest = &console.line[b..];
        let (here, then) = match rest.chars().next() {
            Some(c) => (if c == ' ' { "_".to_string() } else { c.to_string() }, rest[c.len_utf8()..].to_string()),
            None => ("_".to_string(), String::new()),
        };
        out.push_str(&format!("> {}", &console.line[..b]));
        if text.0 != out {
            text.0 = out;
        }
        if at.0 != here {
            at.0 = here;
        }
        if after.0 != then {
            after.0 = then;
        }
    }
}

/// How bright fullbright is (the ambient light's brightness;
/// `--set fullbright_level` to try others).
const FULLBRIGHT: f32 = 1000.0;

/// The camera follows the switches set up at the start: contact shadows
/// (`nocontact`), temporal anti-aliasing (`notaa`; without it, shadows
/// filtered without it too); the ambient light follows `fullbright`.
fn follow(mut commands: Commands, args: Res<Args>, camera: Single<(Entity, Has<ContactShadows>, Has<TemporalAntiAliasing>), With<FlyCam>>, mut ambient: ResMut<GlobalAmbientLight>) {
    if !args.is_changed() {
        return;
    }
    // (Fullbright: the ambient light, otherwise none, raised; the lamps'
    // shadows go too, see `chasm::near_shadows`.)
    let level = if args.opt("fullbright") { args.num("fullbright_level", FULLBRIGHT) } else { 0.0 };
    if ambient.brightness != level {
        *ambient = GlobalAmbientLight { color: Color::WHITE, brightness: level, ..GlobalAmbientLight::NONE };
    }
    let (camera, contact, taa) = *camera;
    let mut cam = commands.entity(camera);
    match (!args.opt("nocontact"), contact) {
        (true, false) => {
            cam.insert(ContactShadows::default());
        }
        (false, true) => {
            cam.remove::<ContactShadows>();
        }
        _ => {}
    }
    match (!args.opt("notaa"), taa) {
        (true, false) => {
            cam.insert((TemporalAntiAliasing::default(), ShadowFilteringMethod::Temporal));
        }
        (false, true) => {
            cam.remove::<TemporalAntiAliasing>();
            cam.insert(ShadowFilteringMethod::Gaussian);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creatures_spawned_by_name() {
        assert!(kinds().iter().any(|k| k == "gladiator") && !kinds().iter().any(|k| k == "clip"), "{:?}", kinds());
        assert_eq!(options_kind("gladiator"), "clip");
        assert_eq!(options_kind("harrow"), "harrow");
        assert!(crate::traits::Traits::parse(options_kind("gladiator"), &["tame", "health=20"]).is_ok());
        let wrong = crate::traits::Traits::parse(options_kind("gladiator"), &["model=0"]).unwrap_err();
        assert!(named("gladiator", wrong.clone()).starts_with("gladiator has no option"), "{wrong}");
    }

    #[test]
    fn unknown_names_said_so() {
        let mut args = Args { opts: Vec::new(), sets: Vec::new(), cam: None, time: None, shot: None, seed: 0, focus: None, ground: None, run: Vec::new() };
        let said = run("shadowmaps", &mut args);
        assert!(said[0].contains("not a setting") && said[0].contains("shadowed"), "{said:?}");
        assert_eq!(run("shadowed", &mut args)[0], "shadowed 8 (default)");
        assert_eq!(run("nolampshadows", &mut args)[0], "nolampshadows off");
        let set = run("shadowmap on", &mut args);
        assert!(set.len() == 2 && set[1].contains("did you mean shadowed"), "{set:?}");
        assert_eq!(run("shadowmap", &mut args)[0], "shadowmap on");
        assert_eq!(distance("shadowmaps", "shadowed"), 4);
        assert_eq!(run("volume", &mut args)[0], "volume 0.1 (default)");
        assert_eq!(run("volume 0.3", &mut args)[0], "volume 0.3");
        assert_eq!(run("mute on", &mut args), vec!["mute on".to_string()]);
    }

    /// The line edited at its cursor, and words completed (the user: the
    /// console cumbersome, a spawn command typed out again at every launch).
    #[test]
    fn editing_and_completing() {
        let mut c = Console::default();
        c.insert("spawn harrow 3");
        c.at = word(&c.line, c.at, -1);
        assert_eq!(c.at, 13);
        c.cut(word(&c.line, c.at, -1), c.at);
        assert_eq!((c.line.as_str(), c.at), ("spawn 3", 6));
        c.insert("angler ");
        assert_eq!(c.line, "spawn angler 3");
        assert_eq!(word("spawn angler 3", 0, 1), 5);
        let args = Args { opts: Vec::new(), sets: Vec::new(), cam: None, time: None, shot: None, seed: 0, focus: None, ground: None, run: Vec::new() };
        let typed = |text: &str| {
            let mut c = Console::default();
            c.insert(text);
            complete(&mut c, &args);
            (c.line, c.lines)
        };
        assert_eq!(typed("spa").0, "spawn ");
        assert_eq!(typed("spawn har").0, "spawn harrow ");
        assert_eq!(typed("round harrow 3 bod").0, "round harrow 3 body=");
        assert!(typed("round harrow 3 bo").1.last().is_some_and(|l| l.contains("body=") && l.contains("bones")));
        assert_eq!(typed("mute o").1.last().map(String::as_str), Some("off  on"));
        let (line, said) = typed("spawn s");
        assert_eq!(line, "spawn s");
        assert!(said.last().is_some_and(|l| l.contains("sprawl") && l.contains("stalker")), "{said:?}");
    }
}
