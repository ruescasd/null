# null

Experimental FPS prototype: a procedurally generated, airless, greyscale world
of flat polygonal plates and megastructures, lit by two distant suns.
Rust + Bevy 0.19.

## Run

    cargo run -p game

Click to capture the mouse, Esc to release. F1 toggles the help overlay.

| Keys | |
|---|---|
| WASD | move (Quake 3 physics: strafe jumping gains speed) |
| Space or Mouse 2 (hold) | jump; holding it bunny hops on landing |
| (automatic) | mantle: in the air, push into a ledge whose top is within ~1.1 m of your feet to climb onto it (with a jump: ledges up to ~2.5 m). Small lips are stepped up even mid-air, and clipping an edge nudges you past it |
| Mouse 1 | thrust beam: its recoil pushes you away from where you aim. Aim at your feet to lift off. Energy recharges on the ground |
| E or Mouse 4/5 (hold) | tether: a spike flies out (60 m), roots where it hits and pulls you towards it; release to keep the momentum. The crosshair grows when a surface is in reach |
| V | noclip fly: Space/E and Ctrl/Q up/down, Shift fast, wheel speed |
| T | pause / resume the suns |
| Left / Right | scrub time |
| Up / Down | time speed |
| P | soft / hard shadows |
| F12 | screenshot to `screenshots/` |

Command-line options, mostly for tuning and capturing reference frames:

    --cam x,height_above_ground,z,yaw,pitch   start position (degrees)
    --time seconds                            where the suns are
    --seed n                                  world seed
    --shot path.png                           render once the view has loaded, save, exit
    --opt bot                                 scripted movement test: logs speed/height and exits
    --opt noclip|beam|tether                  start flying / force the beam or tether on (for captures)
    --opt voxel                               the earlier organic voxel terrain
    --opt flat|hard|noao|nossao|nocontact|notaa|nolandmarks   switch features off
    --set ev=11.2 --set bounce=2 --set fill=6000 --set soft0=1 --set soft1=1   tuning numbers

Run only one capture at a time: two instances starting together have
crashed the GPU driver.

## Layout

- `crates/worldgen` — engine-agnostic generation. `plates.rs`: the plate
  world (hierarchical Voronoi, prisms meshed exactly, stylised AO).
  `landmarks.rs`: megastructures (bridges, twisted towers, hovering slabs,
  needle fields). `world.rs` + `mesh.rs`: the earlier voxel density field and
  surface-nets mesher. `cargo test -p worldgen`; benchmarks in `examples/`.
- `crates/game` — Bevy app: column streaming with LOD (`terrain.rs`), curved
  horizon and AO shaders (`*.wgsl`), suns / stars / bounce and fill light and
  grading (`look.rs`), landmarks kept at their nearest wrapped copy
  (`landmarks.rs`), Quake-style movement, thrust beam and HUD (`player.rs`,
  tuning constants at the top; `player/tether.rs` is the grappling tether's
  state and look; `player/bot.rs` is the scripted test pilot),
  mouse look / noclip / world wrap (`camera.rs`), screenshots (`capture.rs`).
  Collision uses Avian's move-and-slide against per-column triangle meshes.

Lighting favours drama over physics: besides the two suns and the light
bounced off the ground there is a shadowless fill kept opposite the main sun.

The world is a torus (16 km, wrapping in x and z) rendered as if it were a
planet of radius 40 km: geometry is bent down by d²/2R around the camera.
