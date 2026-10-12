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
            .add_systems(Startup, spawn)
            .add_systems(PreUpdate, (type_in, swallow).chain().after(InputSystems))
            .add_systems(Update, (show, follow));
    }
}

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
fn type_in(mut keys: MessageReader<KeyboardInput>, mut console: ResMut<Console>, mut args: ResMut<Args>, mut cursor: Single<&mut CursorOptions>, camera: Single<(&Transform, &FlyCam)>) {
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
const SWITCHES: [(&str, &str); 4] = [
    ("fullbright", "an even light from everywhere, no shadows: to see the shapes"),
    ("nolampshadows", "no shadows from lamps"),
    ("nocontact", "no contact shadows (screen space)"),
    ("notaa", "no temporal anti-aliasing (nor temporal shadow filtering)"),
];

/// The numbers that take effect as the game runs, what each does, and
/// what it is unless set.
fn numbers(args: &Args) -> [(&'static str, &'static str, f32); 4] {
    [
        ("shadowed", "how many lamps cast shadows at once (0-8; each about 1.3 ms)", 2.0),
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
            "clear                clears this".into(),
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
            if !args.opt(name) {
                args.opts.push(name.to_string());
            }
            vec![value(args, name)]
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
                args.sets.retain(|(k, _)| k != name);
                args.sets.push((name.to_string(), v));
                vec![value(args, name)]
            }
            Err(_) => vec![format!("{name}: on, off, a number or default, not {v}")],
        },
        [name] => vec![value(args, name)],
        _ => vec![format!("not understood: {line} (help)")],
    }
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
