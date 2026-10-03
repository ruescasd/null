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
    --opt noclip|beam|tether|spin             start flying / force the beam or tether on / turn the camera (for captures)
    --opt voxel                               the earlier organic voxel terrain
    --opt flat|hard|noao|nossao|nocontact|notaa|nograin|nosites   switch features off
    --set ev=11.2 --set bounce=2 --set fill=6000 --set night_fill=2500 --set contrast=1.1
    --set grain=0.2 --set relief=2.5 --set soft0=1 --set soft1=1   tuning numbers

Run only one capture at a time: two instances starting together have
crashed the GPU driver.

## Layout

- `crates/worldgen` — engine-agnostic generation. `plates.rs`: the plate
  world (hierarchical Voronoi, prisms meshed exactly, stylised AO).
  `district.rs`: districts and their generator rules. `canal.rs`: the canals.
  `ifs.rs` + `structure.rs`: structures from fractal rules and modules;
  `sites.rs`: where they grow in the world. `world.rs` + `mesh.rs`: the earlier voxel
  density field and surface-nets mesher. `cargo test -p worldgen`;
  benchmarks in `examples/`; `cargo run -p worldgen --release --example map --
  map.png 8` draws a top-down map of the whole world (districts, canals,
  site footprints) at 8 m per pixel and lists the sites nearest the spawn
  point.
- `crates/game` — Bevy app: column streaming with LOD (`terrain.rs`), curved
  horizon and AO shaders (`*.wgsl`), suns / stars / bounce and fill light and
  grading (`look.rs`), structures and streamed sites (`structures.rs`), kept
  at their nearest wrapped copy (`landmarks.rs`), Quake-style movement, thrust beam and HUD (`player.rs`,
  tuning constants at the top; `player/tether.rs` is the grappling tether's
  state and look; `player/bot.rs` is the scripted test pilot),
  mouse look / noclip / world wrap (`camera.rs`), screenshots (`capture.rs`).
  Collision uses Avian's move-and-slide against per-column triangle meshes.

Lighting favours drama over physics. Besides the two suns (each with a
visible disc) and the light bounced off the sunlit ground, there is a
shadowless fill with no visible source: opposite the dominant sun by day,
scaled with how much sun is up, and a faint glow from overhead at night.
Surfaces get procedural grain in the terrain shader (albedo mottling and a
faint micro-relief, fixed in the world, tiling with the wrap) so motion reads
even with nothing else in view.

## The world

The planet was once a single built surface; what is left are its plates,
grouped into districts whose purpose is lost, each with its own rules and
its own structures:

| District | Ground | Sites (for now) |
|---|---|---|
| floor | vast pale flat plates, steps of a few cm | halls, screens of fins, whole quarters |
| tiers | terraces of 2 m (mantle-height) ledges | ziggurats, lattices |
| stacks | dark small plates, many tall pillars | spires, tables |
| broken | the original mixed terrain | lattices, halls, ziggurats |

Structures come from fractal *rules* (`ifs.rs`) filled with
hand-designed modules (`structure.rs`). A style splits a block into a grid,
keeps some cells and repeats; each final cell is filled by a module (box,
slab, column, fin, frame, ramp, stairs, or a group of parts), chosen by where
it sat in its parent. Everything is boxes and wedges: hard edges, box and
wedge colliders.

**`data/structures.ron`** holds the modules, styles, test placements and
site rules, and documents its own format. The game watches it: save and the structures are
rebuilt within a second; mistakes show at the top right while the last good
version stays up. Six test structures stand in an arc about 450 m ahead of the
default spawn point (`--opt nofractals` leaves them out). The earlier
distance-field fractals (`fractal.rs`: soft edges, millions of triangles,
fidgety collision) remain behind `--opt sdf_fractals`.

Sites are structures that grow out of the world by the data file's rules
(`sites.rs`): the planet is cut into cells of about 770 m, each holding a
site with some chance; the district there picks a style by weight and a
size within a range, and everything is decided by hashing the cell, so the
world is the same every time. Sites keep clear of each other, the canals
and the test structures. Each stands on a podium whose top clears most of
the ground under it (the odd pillar pokes through) and whose sides reach
below the lowest, so nothing floats or is half buried. They are built in
the background within 2.5 km of the camera and dropped beyond 2.9 km.

An experiment in enemies (`figure.rs`): figures made of the same fractal
language. A procedural skeleton walks on the terrain (feet planted until too
far, then stepping on an arc; two-bone IK legs); each bone's volume is filled
by a fractal fill of fragments (cubes, wedges, shards) held to the bone by
springs, a dense core with a frayed edge, so the body is held together
rather than solid. A core of glowing shards and a lamp inside the chest
light it from within, through its own gaps. The default is a menacing
humanoid (hunched, head forward and low, shoulders raised); `--opt creature`
gives a feral creature with digitigrade legs. One stands about 20 m ahead of
the spawn point and walks towards you; `--opt statue` keeps it still,
`--opt nofigures` removes it.

Canals are huge smooth half-pipes running dead straight across the planet,
each closing on itself around the torus (for now three parallel loops, so
they never cross). They cut trenches through high ground and ride
embankments over low ground. Their surface is slick, as in Quake 3 / Defrag:
no friction, air-strength acceleration and gravity always acting, and inside
the pipe the player rides the exact cylinder rather than its triangles, so
it works like a half-pipe: speed carries you up the walls and out over the
lip.

The world is a torus (16 km, wrapping in x and z) rendered as if it were a
planet of radius 40 km: geometry is bent down by d²/2R around the camera.
