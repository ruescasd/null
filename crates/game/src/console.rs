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
//! (the camera, as `--cam`); `clear`. Up brings back the last command.

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardInput},
    },
    light::ShadowFilteringMethod,
    pbr::ContactShadows,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{Args, camera::FlyCam};

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Console>()
            .init_resource::<Spawns>()
            .add_systems(Startup, spawn)
            .add_systems(PreUpdate, (type_in, swallow).chain().after(InputSystems))
            .add_systems(Update, (show, follow, loudness));
    }
}

/// What `spawn` at the console asks for: creatures, their kind and how many.
#[derive(Resource, Default)]
pub struct Spawns {
    pub creatures: Vec<(String, u32)>,
}

/// The kinds `spawn` knows.
pub const KINDS: [&str; 7] = ["harrow", "angler", "biped", "sprawl", "bud", "grove", "bishop"];

/// The console: whether it is down, the line being typed, what it has
/// said, the commands run (for Up), how far down it is (0 away, 1 down).
#[derive(Resource, Default)]
pub struct Console {
    pub open: bool,
    line: String,
    lines: Vec<String>,
    history: Vec<String>,
    back: usize,
    down: f32,
    closing: bool,
}

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Lines;

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
            panel.spawn((Lines, Text::new(""), TextFont { font_size: FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.8, 0.8, 0.8))));
        });
}

/// Typing: the key under Esc puts it down or away; down, characters go on
/// the line, Backspace takes one off, Enter runs it, Up brings back the
/// last command, Esc puts it away.
#[allow(clippy::too_many_arguments)]
fn type_in(mut keys: MessageReader<KeyboardInput>, mut console: ResMut<Console>, mut args: ResMut<Args>, mut spawns: ResMut<Spawns>, mut cursor: Single<&mut CursorOptions>, camera: Single<(&Transform, &FlyCam)>, mut exit: MessageWriter<AppExit>) {
    for key in keys.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
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
        match &key.logical_key {
            Key::Enter => {
                let line = std::mem::take(&mut console.line);
                console.lines.push(format!("> {line}"));
                if !line.trim().is_empty() {
                    console.history.push(line.clone());
                }
                console.back = 0;
                if line.trim() == "clear" {
                    console.lines.clear();
                } else if line.trim() == "quit" || line.trim() == "exit" {
                    exit.write(AppExit::Success);
                } else if line.trim().starts_with("spawn") {
                    // `spawn KIND [N]`: N of them (1) before you.
                    let words: Vec<&str> = line.split_whitespace().collect();
                    let count = words.get(2).and_then(|n| n.parse::<u32>().ok()).unwrap_or(1).clamp(1, 20);
                    match words.get(1) {
                        Some(kind) if KINDS.contains(kind) => {
                            spawns.creatures.push((kind.to_string(), count));
                            console.lines.push(format!("{count} {kind} before you"));
                        }
                        _ => console.lines.push(format!("spawn what? {}", KINDS.join(", "))),
                    }
                } else if line.trim() == "where" {
                    let (t, fly) = *camera;
                    let p = t.translation;
                    console.lines.push(format!("--cam {:.2},{:.2},{:.2},{:.2},{:.2}", p.x, p.y, p.z, fly.yaw.to_degrees(), fly.pitch.to_degrees()));
                } else {
                    let said = run(&line, &mut args);
                    console.lines.extend(said);
                }
            }
            Key::Backspace => {
                console.line.pop();
            }
            Key::ArrowUp => {
                if console.back < console.history.len() {
                    console.back += 1;
                    console.line = console.history[console.history.len() - console.back].clone();
                }
            }
            Key::ArrowDown => {
                console.back = console.back.saturating_sub(1);
                console.line = if console.back == 0 { String::new() } else { console.history[console.history.len() - console.back].clone() };
            }
            _ => {
                if let Some(text) = &key.text {
                    console.line.extend(text.chars().filter(|c| !c.is_control()));
                }
            }
        }
    }
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
    ("god", "nothing hurts you (for testing)"),
    ("bishop", "the bishop lab: on, one hangs before you, moving in turn (off and on again: before you where you are now)"),
    ("fullbright", "an even light from everywhere, no shadows: to see the shapes"),
    ("nolampshadows", "no shadows from lamps"),
    ("nocontact", "no contact shadows (screen space)"),
    ("notaa", "no temporal anti-aliasing (nor temporal shadow filtering)"),
];

/// The numbers that take effect as the game runs, what each does, and
/// what it is unless set.
fn numbers(args: &Args) -> [(&'static str, &'static str, f32); 12] {
    [
        ("tame_pace", "with tame on: how fast it walks (m/s; 0, its own: the porter 0.7, else changing every 8 s)", 0.0),
        ("tame_at", "with tame on: how far from you it keeps (m)", 14.0),
        ("porter_step", "how long the porter's steps take (times its own: longer, slower and further at the same pace)", 2.0),
        ("stilt_vault", "how far the stalker rises over the stilt it stands on (of all the way: 0 level, 1 all of it)", crate::rig::STILT_VAULT),
        ("stilt_creep", "how much of its pace the stalker keeps through its beat after each step (0 stopped, 1 all)", crate::rig::STILT_GATHER.0),
        ("stilt_beat", "how long the stalker's beat after each step (times its own: 0 none)", crate::rig::STILT_BEAT_SCALE),
        ("volume", "how loud (1 as made, 0 silent)", 1.0),
        ("huntpace", "how fast hunters move (1 as made)", 1.0),
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
            "spawn KIND [N]       N creatures 3 m before you: harrow, angler, biped, sprawl, bud, grove (7 asleep), bishop".into(),
            "clear                clears this".into(),
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
        [name, "off"] => {
            args.opts.retain(|o| o != name);
            vec![value(args, name)]
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
    let v = if args.unattended() || args.opt("mute") { 0.0 } else { args.num("volume", 1.0).max(0.0) };
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
fn show(time: Res<Time>, mut console: ResMut<Console>, panel: Single<(&mut Node, &mut Visibility), With<Panel>>, mut text: Single<&mut Text, With<Lines>>) {
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
        out.push_str(&format!("> {}_", console.line));
        if text.0 != out {
            text.0 = out;
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
    fn unknown_names_said_so() {
        let mut args = Args { opts: Vec::new(), sets: Vec::new(), cam: None, time: None, shot: None, seed: 0, focus: None };
        let said = run("shadowmaps", &mut args);
        assert!(said[0].contains("not a setting") && said[0].contains("shadowed"), "{said:?}");
        assert_eq!(run("shadowed", &mut args)[0], "shadowed 8 (default)");
        assert_eq!(run("nolampshadows", &mut args)[0], "nolampshadows off");
        let set = run("shadowmap on", &mut args);
        assert!(set.len() == 2 && set[1].contains("did you mean shadowed"), "{set:?}");
        assert_eq!(run("shadowmap", &mut args)[0], "shadowmap on");
        assert_eq!(distance("shadowmaps", "shadowed"), 4);
        assert_eq!(run("volume", &mut args)[0], "volume 1 (default)");
        assert_eq!(run("volume 0.3", &mut args)[0], "volume 0.3");
        assert_eq!(run("mute on", &mut args), vec!["mute on".to_string()]);
    }
}
