# Performance

The target is at least 100 frames a second on the development machine (RTX
5060, 24 cores, 1080p), at the worst time of day: both suns up. It is part of
the vision, not a nicety. This file says how we measure, what has been done,
and what is still open, the biggest first.

## Measuring

- `--opt bench`: holds still once everything in view has loaded, then logs
  the average and worst frame, the costliest render passes, and how many
  meshes are drawn. Use `--set width=1920 --set height=1080` for 1080p, and
  a `--time` with both suns up (250, for example; at 150 only one is).
- `--opt flybench`: flies straight on from `--cam` (`--set flyspeed`, 125 m/s
  by default, noclip's fast; `--set flytime`, 30 s) and logs every frame:
  the spread, the frames over 10, 16.7 and 33 ms, and what arrived in the
  worst ones. This is for stutter.
- `--opt viewbench`: a few of the chasm region's or room lab's views, each
  measured.
- `--opt startlog`: from launch, every half second: frames, the terrain
  patches still to generate, the shaders compiling.
- Traces: the profiling build
  (`CARGO_TARGET_DIR=target/profile cargo build --profile fast --features profile`)
  writes a Chrome trace of every system, per frame.
- Meshing: `cargo run -p worldgen --release --example plates_bench` times
  the terrain's generation per patch.

After any change, check a capture that it still draws everything before
believing a faster number. A black screen is very fast.

## Where a frame goes (2026-10-16)

At the user's spot among the pillars (2576, 1931), 1080p, both suns up, on
Vulkan: about 10.4 ms (96 fps). On DirectX 12, before the material fix, it
broke down as:

| Part | Cost |
|---|---|
| Base frame, no sun shadows | 3.9 ms |
| Main sun's shadows | 3.4 ms |
| Second sun's shadows | 2.3 ms |

The shadows' cost splits into shading every pixel with two suns' shadow
maps (about 1.8 ms at 1080p) and drawing their six cascades of dense
geometry. The GPU is the limit now; the CPU has room.

## Done (2026-10-16)

- **Stutter flying fast** (`1d0ef27`). Physics waited on a background task
  queued behind the world's generation: frames of up to 0.86 s. Worst frame
  at 125 m/s went from 609 to 42 ms.
- **Terrain patches twice as wide** (`2dfd298`). A third as many meshes to
  draw, each drawn again in every shadow view.
- **The terrain material written only when it changes** (`97e982c`). It
  was rewritten every frame, so every mesh was re-checked in every view.
  This saved 2.3 ms on Vulkan standing still.
- **Startup from about 35 s to about 3 s** (`d0030e5`, `a7c8c6c`): the
  station check in the terrain's generation, half the cores for the
  background, and the window waiting for shaders too.
- **Vulkan by default; DirectX 12 with `--opt dx12`.** DirectX 12 was about
  8% faster after the material fix, but its shader compiler makes startup
  about 12 s.

## Candidates

### 1. The suns' shadows: the big one

About 6 ms of a 10 ms frame by day. The options, from least to most work:

- **Shadow maps at 2048 instead of 4096.** Saves about 1.2 ms; far shadows a
  little softer. Show captures first.
- **A cheaper second sun.** One short cascade, or none. Up to 2.3 ms; it
  loses some of its broad soft shadows. Show captures first.
- **Fewer or shorter cascades for the main sun.** 1.5 km instead of 3 km
  saved 0.9 ms (measured with the old, smaller patches).
- **Far terrain casting no shadows.** Beyond 1.7 km saves about 0.6 ms,
  beyond 450 m about 1.4 ms. Hard to see in captures at low sun.
- **Precomputed far shadows (the user's idea).** The suns move on fixed
  paths, the same every cycle, and the world is static. So we can store,
  per vertex of the coarse terrain and the far structures, whether each sun
  is visible, at say 64 positions along its path, and blend at run time.
  Keep live, sharp shadows only near you, in one or two cascades, where
  moving things cast them. This removes the far cascades, which hold most
  of the shadow drawing. The second sun's broad soft shadows suit it
  especially. Start with a prototype on the coarse terrain and compare
  captures at low sun. Memory: about 16 bytes per vertex for two suns.
  Cost: tracing toward the sun for every vertex as each patch is generated,
  in the background.
- **Far cascades redrawn only occasionally.** The alternative to
  precomputing: redraw a far cascade only when the sun or the camera has
  moved enough to change it. It needs custom work inside Bevy's shadow
  pipeline.
- **Simplified shadow geometry for pillars and structures.** Coarse
  stand-ins cast the shadows instead of the full meshes.

### 2. The material while moving

The terrain material still changes every frame while the camera moves (the
camera's position, for the planet's curve), so all meshes are re-checked in
every view then. The camera's position belongs somewhere that is not the
material.

### 3. Structures

About 700 meshes besides the terrain: their shadows cost about 1.2 ms by
day, and each is prepared again for every view. Merge tiles, and give far
versions no shadows or simplified ones.

### 4. The canal stones

They roughly double the canal's triangles within 450 m (14.9k to 36.3k).
No cost measured, but build them only within about 150 m if they show up.

### 5. The fixed cost of a frame

About 3–4 ms at night with nothing special in view. The region's list
(2026-10-12) still applies:

- the ichor pool's simulation runs everywhere (0.26 ms);
- the terrain streamer runs under the chasm (0.15 ms);
- the sky's environment map is regenerated every frame (0.15–0.4 ms);
- the HUD text is measured every frame (0.08 ms);
- lamp shadows: ceiling lamps as spot lights would need one view each
  instead of six.

### 6. Smaller, noted

- **Hunting bishops:** sight checks and spot picks, about 0.09 ms each.
- **Cloth:** about 0.23 ms per cape; the optimisation candidates are in
  the cloth notes.
- **Input latency:** to be gone in the finished game. Suspects: pipelined
  rendering, the fixed physics step, the driver's frame queue.

## Rules of thumb

- Every new world feature: run `plates_bench` and `--opt startlog` before
  committing. The canal stations made the terrain's generation 4 to 20
  times slower, and nobody noticed until startup took 35 s.
- Every new look: a bench at the worst time of day.
- Nothing may `block_on` the background pool's tasks: they queue behind the
  world's generation.
