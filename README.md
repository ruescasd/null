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
| E or Mouse 4/5 (hold) | tether: a spike flies out (60 m), roots where it hits and pulls you towards it; hold jump while pulled to lift a little over a lip in the way; release to keep the momentum. The crosshair grows when a surface is in reach |
| V | noclip fly: Space/E and Ctrl/Q up/down, Shift fast, wheel speed |
| T | pause / resume the suns |
| Left / Right | scrub time |
| Up / Down | time speed |
| P | soft / hard shadows |
| F2 | structures' surface panelling off / on |
| F3 | panelling / etched network |
| F4 | ink lines off / on |
| F12 | screenshot to `screenshots/` |

Command-line options, mostly for tuning and capturing reference frames:

    --cam x,height_above_ground,z,yaw,pitch   start position (degrees)
    --time seconds                            where the suns are
    --seed n                                  world seed
    --shot path.png                           render once the view has loaded, save, exit
    --opt bot                                 scripted movement test: logs speed/height and exits
    --opt stairbot                            walks up a flight of stairs near spawn, logs every frame
    --opt shotpair                            with --shot: a second capture 5 frames later (<path>_b.png), to compare for flicker
    --set burst=N                             with --shot: N captures 3 frames apart (<path>_1.png, _2...)
    --opt bench                               once everything in view has loaded: average and worst frame time, costliest render passes
    --opt noclip|beam|tether|spin             start flying / force the beam or tether on / turn the camera (for captures)
    --opt voxel                               the earlier organic voxel terrain
    --opt flat|hard|noao|nocontact|notaa|nograin|nosites   switch features off
    --opt ssao                                screen-space AO (off by default: its noise shimmers on detailed facades)
    --set ev=11.2 --set bounce=2 --set fill=6000 --set night_fill=2500 --set contrast=1.1
    --set grain=0.2 --set relief=2.5 --set soft0=1 --set soft1=1   tuning numbers
    --set fog=4500 --set fog_day=0.04 --set fog_night=0.01 --set fog_glow=0.15   haze (fog=0: none)

`tools\visit.ps1 <place>` opens the game at a place from the review pages
(without one it lists them); `tools\haze.ps1 none|light|dense|dark` runs a
haze variant.

Run only one capture at a time: two instances starting together have
crashed the GPU driver.

## Layout

- `crates/worldgen` — engine-agnostic generation. `plates.rs`: the plate
  world (hierarchical Voronoi, prisms meshed exactly, stylised AO).
  `district.rs`: districts and their generator rules. `canal.rs`: the canals.
  `ifs.rs` + `structure.rs`: structures from fractal rules and modules;
  `forms.rs`: buildings grown from plates; `sites.rs`: where they grow in
  the world. `world.rs` + `mesh.rs`: the earlier voxel
  density field and surface-nets mesher. `cargo test -p worldgen`;
  benchmarks in `examples/`; `cargo run -p worldgen --release --example map --
  map.png 8` draws a top-down map of the whole world (districts, canals,
  site footprints) at 8 m per pixel and lists the sites nearest the spawn
  point; `--example floating` reports pieces of structures with nothing
  under them.
- `crates/game` — Bevy app: column streaming with LOD (`terrain.rs`), curved
  horizon and AO shaders (`*.wgsl`), suns / stars / bounce and fill light and
  grading (`look.rs`), structures and streamed sites (`structures.rs`), kept
  at their nearest wrapped copy (`landmarks.rs`), Quake-style movement, thrust beam and HUD (`player.rs`,
  tuning constants at the top; `player/tether.rs` is the grappling tether's
  state and look; `player/bot.rs` is the scripted test pilot),
  mouse look / noclip / world wrap (`camera.rs`), screenshots (`capture.rs`).
  Collision uses Avian's move-and-slide against per-column triangle meshes.

Lighting favours drama over physics. Though the world is airless, distant
things sink into a dark haze (faintly lit by day, glowing a little towards
the suns), because layers fading with distance are what make the scale read. Besides the two suns (each with a
visible disc) and the light bounced off the sunlit ground, there is a
shadowless fill with no visible source: opposite the dominant sun by day,
scaled with how much sun is up, and a faint glow from overhead at night.
The ground gets procedural grain in the terrain shader (albedo mottling and
a faint micro-relief, fixed in the world, tiling with the wrap) so motion
reads even with nothing else in view. Structures are plain by default, so
their geometry can be judged; the shader can add procedural panelling to
them (`--set detail=1`, or F2 in game): every face layered in bands that
run its whole width, divided into bays by regular frames, with weathering
streaks below each band, each detail fading out before it reaches the
pixel size; `--set etch=1` (F3) an etched network instead;
`--set structure_grain=0.2 --set structure_relief=2.5` the ground's grain.
Ink lines are drawn after lighting where depth jumps (silhouettes) or the
surface turns (creases), never where only the light changes, fading into
the haze, off by default: `--set ink=0.85` turns them on at that strength, `ink_width` their
width in pixels, `ink_fade` the distance they fade over; F4 turns them off
and on.

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
and the test structures. A site reshapes the plates around it rather than
standing on a slab: the 32 m plates under and around the structure become
a flat core, raised above the surrounding ground (an acropolis), level with
it (a plaza) or sunk into it (a court), and rings of terraces step from the
core to the ground. The outline follows the plates, so it is ragged rather
than drawn.

What grows on a site comes from *forms* (`forms.rs`), which work on plates
rather than boxes: each plate of the core, and some of the terraces, is
extruded into prisms (straight, battered or drawn to a leaning point), set
back, cut into blocks along its own lines with streets between, divided into
smaller plates, walled with gates or raised on pillars. Every piece stays a
convex polygon prism, with one hull for collision. Forms are named in
`data/structures.ron` and call each other, so one plate becomes a block with
setbacks, the next a walled court, the next a field of shards; a site may
also keep a box-style centrepiece. Plates next to a higher one may get a
flight of stairs up to it instead (0.45 m steps, walkable without jumping).
A site may shape its ground as an earthwork instead of terraces: the plates
around its core tilt into crisp facets whose corners follow one continuous
slope from the core's edge down to the broad shape of the land, so
neighbours meet seamlessly. Raised cores become mounds, sunk ones bowls,
round or square, with ramps along their axes; the structures rise out of
the ground rather than standing on it.

**The pattern lab** (`--opt lab`, or `tools\visit.ps1 lab`): the data file's
`lab` candidates stand in a row on flat, empty ground, two samples (seeds)
of each, so a pattern is judged on its own rather than for whether it
rescues a place. `--opt lab --opt labshots` photographs every candidate
from the same four angles (the last one close up) into `screenshots/lab/` and exits; with `--focus name` only the candidates whose names contain it. Patterns that
pass join the catalogue the world draws on. The forms grammar has `Shift`
(cantilevers) and negative tapers (forms widening upwards) for them. Box styles can keep a `Massif` (columns falling from the centre at every
level: a mountain of mountains), and the `Rack` form lines a polygon with
open frames whose bays hold pipes, tanks, hoses and machinery.

Colossi are megastructures on the same rules over a much coarser grid,
about one per district and a few hundred metres tall, seen from 4.5 km:
a form grown on a whole footprint (the big plate there, scaled up),
with grooved bands, setbacks, slots and crowns so their size reads. For
now: needle clusters in the stacks, stepped mountains in the tiers, walled
enclosures on the floor and shattered fields of bent shafts in broken
districts. Ordinary sites keep clear of them.

Site ground keeps its full detail at every distance, and everything built
reaches a few metres into its plate, so nothing floats when seen from afar.
Box-style structures are settled too: blocks stack flush (grooves only
run sideways), cells spanning a gap reach their neighbours, and anything
still cut off from the ground reaches down to what is below it. The test
structures stand on a flat core with terraces, like sites. Structures are built in the background within 2.5 km of the
camera and dropped beyond 2.9 km; changing the site rules regenerates the
terrain.

An experiment in enemies (`figure.rs`): figures made of the same fractal
language. A procedural skeleton walks on the terrain (feet planted until too
far, then stepping on an arc; two-bone IK legs); each bone's volume is filled
by a fractal fill of fragments (cubes, wedges, shards) held to the bone by
springs, a dense core with a frayed edge, so the body is held together
rather than solid. A core of glowing shards and a lamp inside the chest
light it from within, through its own gaps. The default is a human of exact
proportions, about 2.4 m tall, upright and calm, heavy-limbed and fairly
solid, glowing only in its chest, drawn as line art (each piece ringed in
black, the rest of the world unchanged; `--opt nooutline` turns it off), with
a matte near-black faceted skull with two steady lit eyes, hard feet and two
razor prongs for hands. `--set hand=N` chooses its hands (0 hard human, 1
long three-digit, 2 a single blade, 3 two razor prongs), `--set head=N` its
head (0 fragments, 1 the faceted skull, 2 a long forward wedge, 3 a tall
crest, 4 a wide disc, 5 the first box head) and `--set eyes=N` its eyes (0
slits, 1 flat slanted rhombuses, 2 upright rhombuses); `--opt eyepulse` has
the eyes pulse with the chest. `--opt luminous` gives the same shape slimmer and finer-grained
with glowing fragments threaded through every part (strange only in its
substance). `--opt hunched` gives the earlier menacing humanoid (head forward
and low, shoulders raised), `--opt creature` a feral creature with
digitigrade legs. One stands about 20 m ahead of
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
