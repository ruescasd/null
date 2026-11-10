# Navigation bench: results (2026-11-01, revised 2026-11-02)

Judged against CRITERIA.md, fixed before any candidate ran. The input is
A1's 2 × 2 km of terrain (5.06M triangles), 1312 biped tiles of 58.33 m.
Re-run with `python -I tools/navbench/judge.py target/navbench`. Raw
results are in `target/navbench/results/`.

## Verdicts

| Criterion | landmass (patched, ours) | C++ Detour, unpatched | Godot 4.7.2, chunks method |
|---|---|---|---|
| 1 Straight ground truth (300) | pass | pass | **1** long way round |
| 2 Flat floor across tile edges (6946) | **1** long way round | **10** long ways round | **8** no way, **8** long way round |
| 3 Steps across tile edges (≤ 1% joined) | pass, 0/172 | pass, 0/172 | pass, 0/172 |
| 4 No stalled way | pass (counted) | not measured (written as 0), no hang seen | not measured, no hang seen |
| 5 Worst frame ≤ 2 ms | **fail**: 122 ms (far goals), 4.0, 3.0 (streaming) | pass: 0.61 ms worst (planned routes) | **fail with queries on the main thread**: 2.9 s (first frame), 184 ms (streaming); threaded queries not tried |
| 6 99% start within 0.5 s | pass | pass | pass |
| 7 None stuck alone | pass | pass when long routes are planned (0); **16** when the crowd alone gets far goals | **1** (far goals), **2** (streaming) |
| 8 Streaming holds | **fail** (frames) | pass | **fail** (frames) |

Every candidate fails at least one line. Detour's only failure under its
intended use is criterion 2.

Detour variants run, all in `results/`:
- `detour`: the crowd alone gets every goal.
- `detour-planned-1000` and `detour-planned-4000`: far goals' routes
  searched by Detour's own sliced path queue (`dtPathQueue`), 1000 or 4000
  steps a frame shared by all, 65535 nodes, and fed to the crowd a corner
  at a time. Public API, no patch.
- `detour-tuned`: two crowd constants raised; a patch, kept for reference.

## What each failure is

- **landmass, frame cost.** It runs each search to the end in one go. 50
  far goals given in one frame cost 122 ms. It has no time-sliced search
  and no path queue. Its crowd step is also about 8 times dearer than
  Detour's: 0.40 ms mean against 0.05 ms.
- **Detour, 10 long ways round** (6 to 62 m, 0.14% of crossings).
  Recast leaves a fan of sliver polygons along long, sloping tile-border
  edges (45 m). Detour joins the edge to one sliver, then prices the
  crossing at that edge's midpoint, about 20 m off, so a way round the tile
  corner looks cheaper. This is the same flaw as landmass fix 6, which was
  patched there. The any-angle option doesn't help. Options: patch the
  pricing, use bake settings or tile sizes that leave fewer slivers, or run
  a shortcut pass over finished paths. None of these has been tried yet.
- **Detour, far goals.** Given to the crowd alone, 16 agents get stuck:
  its built-in path queue does 100 search steps a frame with 4096 nodes,
  so long routes come back cut short, or late. Planned the usual way, the
  sliced queue at 4000 steps a frame gives 0 stuck, every route complete
  within 0.8 s, and a worst frame of 0.61 ms. At 1000 steps a frame,
  routes take up to 3.3 s and 11 agents stall on the crowd's short partial
  path while they wait. Plain one-shot queries cost up to 2.9 ms near and
  3.5 ms far, over budget, so long routes must use the sliced search.
- **Godot, frame cost: a design limit, confirmed in its source.** In
  4.7.2's `modules/navigation_3d/3d/nav_mesh_queries_3d.cpp`, the
  closest-point query (`map_iteration_get_closest_point_info`) and a path
  query's search for its start and end polygons both loop over every
  polygon of every region in the map, with no spatial index. One
  closest-point lookup costs 39 ms on this map, and a 0.5 m path 47 ms
  with or without a search limit. Query cost follows the whole map's
  polygon count, not the path, so tile size and border can't change it.
  Godot keeps every detail triangle as a polygon: 1.04M here against
  Detour's 354k. At the streaming working set (about 190 tiles) it's
  still about 6 ms a lookup, and 30 followers re-pathing gave frames of
  100 to 184 ms. A coarser cell size would cut polygons, but it coarsens
  the mesh against the other candidates. Fixing it would mean patching
  and rebuilding the engine, or putting Detour in through a GDExtension.
- **Godot, joins.** Following the chunks demo (edge connections off,
  vertices snapped to a tenth of the map cell, border fixed, see bench
  bug 5), every tile edge merges: 1.16M edges merged, 0 free. 8 flat
  tile-edge pairs still find no way, and 9 cases go a long way round.
  These weren't diagnosed, because the frame cost failure is decisive on
  its own.
- **Godot as shipped with edge connections on:** the map sync joins
  region edges by distance with a search that grows with the square of
  the free edges: 100 tiles 6 s, 200 tiles 29 s, 400 tiles 150 s. It
  rebuilds the whole map on any region change. With aligned tiles this
  setting isn't needed, which is what the chunks demo says.
- **Godot, not tried:** the demo's full one-tile border. It segfaulted
  twice during the bake, cause unknown and not pursued, since no border
  changes the query cost. Bigger tiles weren't tried either, for the same
  reason.

## Recorded

| | landmass | Detour | Godot |
|---|---|---|---|
| Build, biped, ms a tile a core | 68 to 85 | 65 to 79 | about 115 (12 threads) |
| Whole-world bake, both bodies, 11 workers | about 0.3 h | about 0.3 h | about 0.4 h |
| Memory, biped + beast, per 4 km² | 37 + 27 MB | 67 + 48 MB | 24 + 18 MB (no detail heights) |
| Whole world on disk, both bodies (from memory per 4 km²) | about 4 GB | about 8 GB | about 3 GB |
| Ways found near (400) / far (100) | 167, 24 | 167, 24 | 167, 24 |
| Near query worst | 1.96 ms | 2.92 ms | 120 ms |
| Far query worst | 7.9 ms | 3.5 ms | 140 ms |
| Streaming followers kept up | 20 of 30 | 30 of 30 | 19 of 30 |

All three agree on whether a way exists for every near and far pair,
which cross-checks the bench. Only about 42% of near pairs and 24% of
far pairs are connected at all. That comes from the terrain (terraces
with steps over the 0.6 m climb), not from any candidate, and the design
question it raises holds in every engine.

## Bench bugs found and fixed before trusting it

1. Ground truth heights came from the world's `height_at`, which is up to
   7 m off the mesh. They now come from rays on the terrain mesh.
2. The Detour floor lookup had its search box centred 2.5 m below the
   point, so it picked lower floors. Centred now.
3. Godot's bake box was 3 km tall. Recast's 13-bit span heights clamped the
   terrain, giving 35k polygons. It now spans the terrain's own height range.
4. Godot was queried before its regions had built and its map had synced.
   It now waits for every region, then for a map that has been steady for a
   second of real time.
5. Godot's `border_size` was 0.7 m, and Godot rounds it up to whole cells.
   In f32, 0.7 m ÷ 0.11667 m is just over 6, so it rounded to 7 cells, a
   cell more than the box was grown by. That left a gap along every tile
   edge, so no edge could merge (0 of 6946). This, not Godot, was the first
   "Godot can't join tiles" result. It's now set a hair under 6 cells.
6. The planned-route Detour run treated a request still waiting in the
   queue (status 0) as finished, and threw most routes away. It now waits.

## Fresh review (2026-11-02): corrections, and what is still open

An agent that hadn't seen the work reviewed it. Its main points:

- **Godot didn't get an equivalent of Detour's intended-use run.** Godot
  4.7.2 can run path queries on threads (`navigation/pathfinding/max_threads`,
  4 by default), take a callback on `query_path`, look up a closest point
  in one region only (`region_get_closest_point`), and limit a query with
  `included_regions`. Criterion 5 doesn't count work on other threads. So
  the frame-cost failure is the verdict for main-thread queries only. The
  reviewer expects Godot still to fail, but on latency and CPU: about 0.7 s
  for 50 queries on 4 threads over the whole map. Still to run.
- **A third whole-map cost:** every Godot path query resets a corridor
  array sized to the whole map (queries.cpp:331–334). `included_regions`
  can't avoid it.
- **Godot's joins are not complete.** All tile edges merge (0 free), but the
  log shows 630 region edge errors, which are joins inside tiles. Godot itself
  suggests lowering `navigation/3d/merge_rasterizer_cell_scale`, which wasn't
  tried. That probably explains its 8 "no way" pairs. "Chunks method" also
  overstates it: the scored run used Recast's minimum border, not the demo's
  full one, which segfaulted twice and should count against criterion 4 for
  that configuration.
- **Numbers not saved:** the 39 ms closest-point and 47 ms path figures came
  from a probe whose output wasn't kept.
- **Criteria applied unevenly:**
  - Criterion 4 was measured only on landmass.
  - "Stuck" differs between candidates. landmass counts only agents in its
    Moving state. Detour's planned run measures distance to the current
    corner. Godot's bodies move without collision. The "crowded" exemption
    hides stuck followers: landmass passes criterion 7 while streaming with
    25 jammed and only 20 of 30 keeping up.
  - The timed region differs too. landmass's floor sampling is outside its
    timer, so its crowd step is more than 8× Detour's. Godot's includes
    0.2–0.5 ms of bench script.
  - Godot's "6732 added" is a bookkeeping bug: empty tiles get counted again
    every frame.
- **Noisy worst cases:** Detour's worst near and far queries range from
  2.8 to 4.5 ms across its four runs. A single worst sample is about ±50%.
- **Patched against unpatched:** criterion 2 compares landmass after its
  patch for this same flaw with Detour unpatched. Detour's crowd may
  straighten such crossings for walking agents. Not checked.
- **Memory compares different things.** Detour's figure is runtime tile data,
  while the others are resource arrays. Detour can also build tiles at
  runtime, about 0.1 core while streaming, which would remove the bake
  and its disk cost.
- **What the bench can't show:** multi-level interiors, stairs, doorways,
  off-mesh links, the second body, the wrap, more than 50 agents.

What the review found holds:
- Detour was tested fairly. The settings matched, and `dtPathQueue` was used
  as documented.
- No further Godot rounding bugs. It checked every f32 conversion in the
  bake.
- landmass's failure is real.
- Godot's per-query cost follows the whole map's polygons, as its source
  confirms.

## Not run

Unity and Unreal. I first wrote that no result would change the decision.
The review showed that's wrong: in those engines you'd use the built-in
navigation, not add Detour. If one passes, integration costs almost
nothing there, against custom work in Bevy and Godot, and that changes the
cost ledger.

Next: the feature and cost ledgers, then a fresh agent's review of this
evidence (the user, 2026-11-01), checking above all that each candidate
was tested in its intended configuration.
