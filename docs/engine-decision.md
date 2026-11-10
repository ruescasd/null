# Engine decision: the two ledgers

Opened 2026-11-02 for the decision to stay on Bevy or switch. The user's
rule (2026-11-01): switching cost must not shade the feature verdicts.
So features go in one ledger, judged on evidence alone, and costs go in
the other. The cost of staying is counted as concretely as the cost of
switching, and the two meet only at the decision. A fresh agent reviews
both before any decision is made.

Each entry carries how well it's known:
- **[measured]**: we ran it here.
- **[source]**: read in the engine's own code.
- **[researched]**: from docs or reports, not run.
- **[assumed]**: judgement, unchecked.

Candidates: Bevy (current), Godot 4.7, Unity 6, Unreal 5. Consoles are
not a goal (the user: proper movement needs a mouse), so PC only.

## 1. Feature ledger

What each engine gives for the features the game needs, before any
thought of what moving costs.

### Navigation: navmesh core (build, paths, crowd)

Bench in `tools/navbench/` (CRITERIA.md fixed first, RESULTS.md).

- **C++ Recast/Detour** passes everything but one line: 10 detours at tile
  edges, 0.14% [measured], with options to fix them that haven't been tried
  [assumed]. It's portable:
  Bevy (Rust bindings, `recastnavigation-sys`, untried), Godot
  (GDExtension, plain C++), Unity (native plugin, but it ships its own
  derivative), Unreal (it is Unreal's core) [researched].
- **Bevy's current crates** (landmass + rerecast, patched in 7 places, plus
  4 conversion workarounds) fail on frame cost: 122 ms [measured]. There's
  no sliced search [source].
- **Godot's own server**: every query scans every polygon in the map and
  resets a corridor array sized to the map [source]. With queries on the
  main thread it fails frame cost [measured]. With its threaded queries it
  wasn't tried; the reviewer expects a latency and CPU failure instead.
  All tile edges merge, but 630 edge errors inside tiles leave some ways
  missing [measured]. A Detour-backed navigation server could also be
  registered in its place (`NavigationServer3DManager`) [researched].
- **Unity's and Unreal's own** weren't run. Both are Detour derivatives with
  time-sliced or async pathfinding and runtime building (Unreal: Navigation
  Invokers) [researched].
- Verdict: navigation doesn't separate Bevy and Godot once Detour goes in
  both. It may favour Unity or Unreal: if their built-in navigation passes,
  integration costs almost nothing there. That's untested (review,
  2026-11-02).

### Navigation: world-scale orchestration (which tiles, cache, wrap)

- Built by us in every engine except Unreal (World Partition with
  streamed navmesh tiles), and Unreal's doesn't wrap [researched].
  Detour's tile add/remove holds under streaming: 0.17 ms churn worst
  [measured].

### Shadows: many shadowed lamps without popping

- The user's reframe (2026-11-01): popping matters only for the
  conjunction of many shadowed lights, packed rooms and loose light
  volumes, which the vision doesn't need. Not decisive, but shadows
  themselves are essential.
- Bevy: no cached shadow maps, every spot rendered each frame. 8 spots in
  view cost 8.5 ms in the arena [measured]. That's a per-frame cost gap.
- Unreal: Virtual Shadow Maps, cached for static geometry [researched].
- Unity HDRP: cached punctual shadows. HDRP has been in maintenance since
  Unity's February 2026 strategy, and URP's equivalent is unverified
  [researched].
- Godot: its positional shadow atlas is said to redraw a light's shadow
  only when the light or a caster in its range changes, but without
  Unreal's static/dynamic split [researched, unchecked: check
  renderer_scene_cull.cpp or test]. My earlier "same budget as Bevy" was
  an unchecked guess.
- Not yet measured: the two agreed shadow checks (lamp density in the
  chasm, and shadow cost in a real chasm room at 2048/1024 map sizes).

### World streaming and far detail

- Bevy: terrain and sites already stream (terrain.rs, 3 LODs to 4.8 km,
  wrap-aware) [measured in-game]. Not built: streaming dense places, far
  detail for architecture. Spike B was planned for this, not run.
- Godot and Unity: built by us [researched].
- Unreal: World Partition and HLOD built in, no wrap [researched].

### Large-world precision

- Bevy: f32. 8 km gives millimetre steps, which already bit navigation
  (tiles moved to local frames) [measured]. Needs a floating origin.
- Godot: double precision with a custom engine build [researched].
- Unity: f32, floating origin by hand [researched].
- Unreal: Large World Coordinates (doubles) [researched].

### Occlusion

- Unity 6: GPU occlusion culling [researched].
- Unreal: built in [researched].
- Bevy 0.19: experimental GPU occlusion culling, unused here [researched].
- Godot: OccluderInstance3D, built in [researched].

### Sound

- Unity and Unreal: official Steam Audio and FMOD [researched].
- Bevy: integration ours [researched].
- Godot: Steam Audio via community plugins [researched, not deeply].

### Animation

- Even. Our pipeline is offline tools plus our own runtime layer, and
  every engine does glTF, skinning and clip blending (the user's
  correction, 2026-11-01). Bevy runs ten clip-driven robots at 123 fps
  [measured].

### Procedural geometry and custom shaders

- The world is procedural and static, so it could be baked [researched].
- Bevy: everything is ours and in code, with 1.4k lines of WGSL [measured].
- Godot: a simple text shading language [researched].
- Unity HDRP: Shader Graph (GUI), or fragile hand-written lit shaders
  [researched].
- Unreal: material graphs, or HLSL by hand [researched].

### Workflow for an agent doing the work

- Bevy: all code, headless, seconds per loop [measured].
- Godot: `--headless`, text resources, scripts from the command line. The
  bench ran there without a GUI [measured]. Gotchas found: async map
  sync, about 40,000 frames a second headless, crash-free only with
  bakes throttled.
- Unity: batch-mode editor launches, domain reloads, HDRP shader variant
  compiles [researched]. Needs a sign-in.
- Unreal: heaviest editor and build. The user's main concern is its weight
  [researched].

### Movement (Quake-style and our own)

- Ours in every engine; Bevy has it now [measured in-game].

## 2. Cost ledger

### Staying on Bevy: what we'd carry and build

- **Navigation core:** replace landmass + rerecast with C++ Detour through
  bindings. This means bindings (one crate exists), a C++ build in the
  workspace, Floors moved over, and the sliced long-route planning. It
  would retire 7 vendor patches and 4 workarounds. Detour's own open item:
  the tile-edge detours (one patch, or settings). Risk: the bindings crate
  is untried [assumed: days, not weeks; the surface is navmesh.rs, 508
  lines, plus 9 files that use it].
- **Navigation orchestration:** tiles around the player, cache, wrap.
  The same in Godot and Unity; Unreal has it, but without the wrap.
- **Shadows:** if the shadow checks show the per-frame cost matters,
  shadow caching or visible-lamp selection is renderer work in Bevy. It
  was parked as "a renderer project, not a seam patch". Size unknown
  until the checks run.
- **Streaming dense places and far detail:** ours (spike B). The same in
  Godot and Unity.
- **Floating origin:** ours.
- **Occlusion:** ours, if needed.
- **Sound middleware:** ours to integrate.
- **Bevy upgrades:** breaking changes every release, plus vendored crates
  to carry through each [measured over this project].

### Switching: what moving costs

- **Code:** about 53k lines of Rust. Of that, `worldgen` (15.5k) uses only
  glam, not Bevy, so it can move as a library: Rust GDExtension in Godot,
  or a native plugin in Unity and Unreal. The `game` crate (37.8k) uses
  Bevy in 68 of 69 files and would be rewritten, in GDScript or Rust
  (Godot), C# (Unity) or C++ (Unreal).
- **Shaders:** 1.4k lines of WGSL to port. Godot's language is close,
  while Unity HDRP and Unreal need their own material systems.
- **Tools:** offline Python tools (about 1k lines) carry over, and so does
  the bench. The in-game tooling lives in the game crate and would be
  rewritten: captures, review pages, `--opt` labs, the `--do` console,
  viewbench, rigtrace.
- **Knowledge:** what we learnt is mostly about design and is kept.
  Engine-specific lessons are lost.
- **Workflow loss:** depends on the engine (feature ledger, workflow).
- **Not yet estimated in time:** a rewrite's duration. Staying has a time
  estimate and switching has none, and both need one before they can meet.

### Features missing from this ledger (review, 2026-11-02)

- Physics: avian against Jolt, PhysX or Chaos, including cooking colliders
  at runtime for procedural terrain.
- Uploading procedural meshes at runtime in each engine.
- Dense far geometry (Nanite, HLOD) for spike B.
- Each engine's baseline frame cost at 100 fps on the RTX 5060 (Lumen,
  VSM and Nanite are a real risk).
- Clip retargeting and IK for the enemy pipeline, not just glTF and
  blending.
- Detour building tiles at runtime (about 0.1 core while streaming), which
  would remove the bake and its disk cost.

## 3. Open before the decision

- The two shadow checks: these say whether Bevy's shadow cost gap is real
  for our rooms.
- Spike B, or at least its plan: streaming dense places and far detail.
- The fresh agent's review of both ledgers. Above all it should check
  that each candidate was tested in its intended configuration: the
  Godot result was wrong twice before it was right.
