# Navigation bench: criteria

Fixed 2026-11-01, before any candidate ran (the user: the same test on
every candidate, judged strictly; switching cost kept out of it).

Candidates: landmass + rerecast as patched (our current), C++ Recast/Detour
(recastnavigation, unpatched), Godot 4.7.2's navigation server, Unity's
navmesh (once signed in).

Shared input, written once by `zz_bench` (crates/game/src/navmesh/zz_bench.rs)
into `target/navbench/`:

- `terrain.bin`: A1's 2 × 2 km of the world's terrain around (8192, 8192),
  world coordinates, about 5M triangles.
- `settings.txt`: both bodies' build settings (cell sizes, radius, height,
  climb 0.6 m, slope 45°, region sizes, edge length and error, polygon
  size, detail sampling), and the tile grid (biped 58.33 m, beast 75 m,
  tiles on the world's grid from 0).
- `cases.txt`: the cases below, from the terrain alone, never from a
  navmesh.

Every candidate uses its own engine's navigation as shipped. Settings may
be set; source may not be changed. Each writes `results/<name>.json`;
`judge.py` reads them all and prints the verdicts.

## Pass or fail

A candidate passes only if it meets every line. The worst case counts,
never the mean.

1. **Straight ground truth.** All 300 pairs that the terrain lets you
   walk in a straight line (no step over 0.5 m on a 1 m lane, up to 40 m)
   get a way, no longer than 3 × the straight distance + 1 m.
2. **Flat floor across tile edges.** All pairs 1.6 m apart across a tile
   edge, on one flat floor, get a way: no cut, and none longer than
   3 × straight + 1 m.
3. **Real steps across tile edges.** Pairs across a tile edge with a step
   of over 0.7 m: no more than 1% joined straight across (way under
   straight + 1 m). Each one joined gets inspected.
4. **No hang, no crash, no stalled way**, through every run.
5. **Frame cost, worst frame**, in every run (50 agents with ways,
   50 with none, 50 all given far goals in one frame, and 30 following a
   moving point while tiles stream): the navigation's work on the game's
   thread is never over 2 ms in a frame. That's a fifth of the 10 ms frame
   that 100 fps allows. Work the engine moves to other threads doesn't
   count against this, but item 6 does.
6. **Latency.** In those runs, 99% of agents start moving within 0.5 s of
   being given a goal.
7. **Agents.** None stuck alone (more than 5 m from its goal, no
   neighbour within 1.5 m, not moved 0.5 m in 2 s) in any run.
8. **Streaming.** While the tiles within 450 m of the moving point are
   added and removed, items 5 and 7 still hold.

## Recorded, judged afterwards

- Build time per tile on one core, and the whole-world estimate. The bar
  is a bake under 1 hour on 11 workers.
- Memory per 4 km², both bodies.
- Single way queries, near (to 150 m) and far (to 1500 m): found counts
  and times. Where candidates disagree on whether a way exists, the pairs
  are listed and inspected.
- Followers keeping up with the moving point (within 30 m at the end).
- Workflow: can it all run headless, from text, and how long the setup
  took.

## The decision

These results go into the feature ledger. Costs (switching, and carrying
patches or building missing features) go into the cost ledger, only at
the decision point. Before deciding to switch engine or stay on Bevy, a
fresh agent that hasn't seen these conversations reviews the evidence and
challenges the verdicts (the user, 2026-11-01).
