# Outdoor navigation: the reframing and its validation

Opened 2026-11-02, written before anything is measured (step 0), revised
the same day with the observer's remarks and the gladiator's measured legs.
The thresholds are proposals until the user agrees them.

## Terms (the user, 2026-11-03)

- **District:** an area of the world's ground, made of plates by rules
  (floor, tiers, stacks, broken, uplands, highlands). One surface per spot,
  so navigation is built exactly in 2D: the node graph below.
- **Structure:** anything built in the world, at any scale. That includes
  the spire, hanging cities, the reactor city, the chasm, the region, the
  arena, the void's cylinders and spans, and the pillars standing on
  stacks' ground. Its surfaces stack in levels, so navigation is the
  typical kind every game does (rooms, hallways, stairs, floors): Recast/
  Detour, or built by construction where the generator is regular, plus
  generator rules.
- **Seam:** where a structure meets the ground or another structure.
  Always a designed link.

Everything is embedded in the one world (the user). The standalone
scenes the chasm, region and arena are today are only a stage of the
implementation, never a category.

## The reframing

- **Districts:** navigation is defined by the generator. A *navigable node*
  is a maximal group of plates merged where the traversal rules hold for a
  body: the step is within its climb, the passable border is at least its
  width, and each plate fits it after shrinking by its radius at drops and
  walls. Within a node, walking is correct by construction.
- **Between a district's nodes:** no drop, climb or jump for creatures.
  Walkers walk within a node and teleport across node boundaries,
  re-forming at a valid point in another node, with a tell and limits.
  Teleporting is a walker's capability (the user, 2026-11-02), not a
  separate creature. Flyers cross the open air. Only the player climbs,
  mantles and jumps.
- **Why teleporting exists** (the user, 2026-11-03): districts require
  it. Almost all of a district must be reachable, and walking can't manage
  that across its steps. Structures don't require it, but it isn't
  forbidden in them yet. It may end up restricted to districts; that's
  decided later.
- **Structures:** creatures use normal navigation and stay on the parts
  they can walk to. Each structure's generator decides where that is.
  Pillar perches are assumed unreachable for walkers for now; pillars are
  to be settled for stacks specifically.
- **Seams:** each one is a designed link between a district's nodes and a
  structure's walkable parts.

This track is independent of the engine and runs alongside the engine
evaluation. The engine evaluation's finish line is unchanged: the
even-handed rerun, the shadow checks, Unity, and the switching estimate.

## Measures used below

- **Node size:** a node's **area** (m²) is the main measure. It matches
  "share of walkable ground". Its **extent** (longest dimension) is
  reported alongside, because terraces make long, narrow nodes.
  - A **large node** has at least 250 000 m², the area of a 500 m square.
  - A **walker-sized node** has at least 10 000 m², the area of a 100 m
    square.
- **Per district:** every kill is judged per district (floor, tiers,
  stacks, broken, uplands, highlands), then summarised for the world. A
  mixed outcome is expected, since some step heights are designed per
  district.
- **Climbs a creature can visibly perform.** Measured from the
  gladiator's own mesh: hips 1.33 m, knees about 0.84 m, legs 1.11 to
  1.14 m from hip to ankle.
  - Its walk comes from clips with feet placed by IK. Its 0.65 m step-up is
    what its physics capsule steps over, not an animated move.
  - A human-like body can take a step up to about knee height (0.8 m) with
    a high step. Above that, it's a scramble that needs hands, which is a
    climbing move [assumed, a rule of thumb].
  - So climbs up to 0.8 m count as walking, if a step-up animation is made
    for them. Anything above is a climb capability, which is the quagmire.

## Kill criteria

Each is judged per district. Any one that triggers stops the reframing
there, for a rethink.

1. **"Forced" is false.** At a climb of at most 0.8 m, judged in bands by the
   share of walkable ground in large nodes:
   - **at least 80%: not forced.** Walking can carry the district.
   - **40 to 80%: mixed.** Walkers are viable with some help, so flyers and
     teleport are a choice there, recorded as such.
   - **under 40%: forced.**
2. **Retired as a kill (2026-11-02, after step 1).** It was: "walkers are
   pointless: at 0.6 m, under 50% of walkable ground in walker-sized
   nodes". The user's reason for retiring it: teleporting is a capability
   of walkers, not a separate creature. Walkers walk within a node and
   teleport across node boundaries, so a fragmented district means more
   teleports, not useless walkers. What remains is a measure, not a kill:
   per district, the node boundaries a walker crosses per 100 m of a chase
   (teleports per 100 m, once the teleport's range rule exists). It feeds
   the slice question on walkers in stacks and broken.
3. **Flying isn't safe by construction, or doesn't reach where it's
   needed.**
   - Any swept flight held to the clearance map touches geometry: zero
     tolerance.
   - Or flyers reach an attack position for under 95% of player positions.
   - **The sample of player positions** is half open ground and half
     **perches**: pillar tops, terrace edges within 2 m of a drop of 2 m or
     more, and structure tops.
   - **An attack position** is clear airspace where the flyer fits, with a
     line of sight to the player. Proposed: within 2.5 m for melee, or
     within 30 m for a ranged attack. Both are recorded until the flyer's
     weapon is designed.
4. **It doesn't play well.** In the slice (step 4), the user answers no to
   any of these, and tuning within the slice doesn't fix it:
   - Can I escape a walker by using the terrain?
   - Is the flyer threatening but avoidable?
   - Does the flyer feel heavy and dangerous in combat, not like the buds?
   - Can I trivialise an encounter by perching or running? (This must be
     no.)
   - Can I read the teleport's tell in time?
   - In stacks and broken, does the walker still read as a walker, not a
     blinker?
   - Does each creature feel like it belongs to this world?
   - Does crossing the seam between outdoors and a place feel seamless,
     for me and for the creatures?

## Steps

0. **These criteria,** agreed with the user before anything runs.
1. **Measure the premise.** The plate graph over a patch from every
   district. For each climb (0.6, 0.65, 0.8, then 0.9, 1.2, 1.75, 2.0 m as
   context), per district:
   - how many nodes there are;
   - their area and extent distributions;
   - the share of walkable ground in large and in walker-sized nodes;
   - the distribution of step heights between nodes.

   It also reports **the terrain lever:** per district, how much merging a
   smaller designed step would produce. The district is regenerated with
   its step rule (`quantum`, `quantum0`) scaled down to fit the walker's
   climb. That shows whether a problem lies in the creatures or the
   terrain. It doesn't change the kills.

   Settles kills 1 and 2 for the terrain as it is. Headless.
2. **Nodes by construction, cross-checked both ways.**
   - **Detour's limits first:** six vertices per polygon, 65535 vertices
     per tile, polygons per tile and reference bits, tile size. These
     decide how plates are split.
   - **Then build per body:** shrunk plates and links, with the
     definition's tests (convex, fits, links symmetric and wide enough).
   - **Then compare with the independent Recast/Detour navmesh** on the
     same patches:
     - reachability must agree for every pair, in both directions;
     - a walkability check on returned ways must pass.

     Where they disagree, either side may be wrong.
3. **Flyers and teleport, by construction.**
   - **Flyers:** a clearance map from the top surfaces plus the bounds of
     tall structures, then thousands of random flights over every district,
     swept against the real meshes. Measure how often a flyer reaches an
     attack position.
   - **Teleport:** check that every chosen arrival point is valid.
   - Settles kill 3.
4. **A playable slice,** judged by the user against kill 4's questions:
   - outdoors, on two contrasting districts;
   - one walker, one flyer, one teleporter with its tell;
   - **one designed seam** into a place: an entrance or a chasm edge.
5. **A fresh review** of the criteria, results and slice notes before
   committing to the direction.

## Step 1 results (2026-11-02)

Raw data: `target/navbench/nodes.json`, from `zz_nodes` in
`crates/game/src/navmesh/zz_nodes.rs`. One 1.6 km patch per district,
each 100% that district. Below, the share of walkable ground in large
nodes (at least 25 ha) and in walker-sized nodes (at least 1 ha).

| District (world share) | Large at 0.6 / 0.65 / 0.8 m | Kill 1 band | Walker-sized at 0.6 m | Kill 2 | Context: large at 1.2 / 1.75 / 2 m | Lever, steps capped at 0.6 m, large at 0.65 m |
|---|---|---|---|---|---|---|
| Floor (26%) | 98 / 98 / 98% | not forced | 100% | pass | 98 / 98 / 98% | 98% |
| Tiers (11%) | 0 / 0 / 0% | forced | 50% | at the bar | 0 / 0 / 99% | 84% |
| Stacks (26%) | 38 / 38 / 39% | forced, just under the mixed band | 43% | (retired) | 55 / 71 / 77% | 41% |
| Broken (19%) | 0 / 0 / 44% | forced; mixed only with a 0.8 m step | 27% (78% at 0.8 m) | (retired) | 68 / 86 / 87% | 52% |
| Uplands (12%) | 38 / 38 / 38% | forced, just under the mixed band | 96% | pass | 53 / 92 / 92% | 91% |
| Highlands (6%) | 67 / 70 / 72% | mixed | 91% | pass | 98 / 99 / 99% | 93% |

Reading it:
- **Kill 1 doesn't trigger as a whole.** "Forced" holds in tiers, stacks and
  uplands, and in broken at the walker's climb. Floor is not forced.
  Highlands is mixed, so flyers and teleport are a choice there. Broken
  becomes mixed only if a knee-height (0.8 m) step-up animation is made.
- **Room for walkers is low in stacks and broken:** at 0.6 m, 43% and 27%
  of the ground lies in walker-sized nodes. In broken, a 0.8 m step would
  lift that to 78%. Stacks stays low at any climb up to 2 m because of its
  pillars. Kill 2 was retired after this result (see above): with
  teleporting as a walker capability, this measures how often walkers
  teleport, not whether they're useful.
- **The terrain lever** (every step capped at 0.6 m) would make tiers,
  uplands and highlands walkable (84 to 93%), at the cost of their
  designed 2 m, 1.75 m and 1 m steps. It barely helps stacks or broken.
  Its 0.6 m row is distorted: the capped steps are exactly 0.6 m, so they
  sit right on the climb, which is why it's read at 0.65 m.
- **The steps where nodes part** follow each district's design: tiers 82%
  between 1.2 and 2 m; uplands 59% between 1.2 and 2 m; highlands 56%
  between 0.8 and 1.2 m; broken 44% between 0.6 and 0.8 m.

Caveats:
- One patch per district, so one sample of each.
- 25 cm cells, and the body's radius taken off approximately.
- Every large node runs into the patch's edge. Smaller nodes cut by the
  edge may really be larger, which leans towards "forced".
- **Found for step 2:** in stacks, the world's own height function is more
  than 50 cm off its mesh at 11% of points (elsewhere at most 1%). Nodes
  built by construction must follow the mesh's model of the plates, so
  this needs explaining first.

**Decision (the user, 2026-11-02): no terrain lever for tiers, uplands or
highlands.** Their designed steps (tiers' 2 m ledges; uplands' and
highlands' tilted plates with their 1.75 m and 1 m steps) are those
districts' identity: softened, they "don't really exist". This is not a
general rule. Flattening is judged case by case: where a district's
identity doesn't rest on its steps, and softening helps navigation or
anything else, it merits consideration.

## Step 2 (in progress)

**2a. The stacks discrepancy: explained, not a bug.** The world's height
function treats a pillar plate as a solid prism, its whole cell at the
pillar's top. At full detail the mesher instead grows the pillar from the
structure library's `pillar` form (`emit_pillar`): open, stacked slabs on
the plate's outline, scaled to the pillar's height. So the real pillar is
narrower than its cell, has ground round it, and has slabs overhanging gaps.
All 517 mismatched points in the stacks patch were pillar plates, about 11%
of its area. For nodes, a pillar is a grown structure, not a plate:
- its plate's base ground, with the pieces' footprints as obstacles;
- each piece's top as its own surface, with headroom checked against the
  pieces above. These are the perches, known exactly from the grower's
  prisms.

Nothing else grows on plates by default. Slabs and kerbs are off, and wall
panels sit on plate sides. Canals bring their own geometry.

**2b. Detour's limits for hand-made polygons** (`DetourNavMeshBuilder`,
`DetourNavMesh.h`):
- At most 6 vertices per polygon (`DT_VERTS_PER_POLYGON`), so plates are
  split into convex pieces.
- Vertices are unsigned 16-bit cell coordinates from the tile's corner, in
  steps of `cs` across and `ch` up, fewer than 65535 per tile. Choosing
  64 m tiles with `cs` = `ch` = 1/64 m makes tile edges exact binary
  numbers (4096 steps a side) and leaves 1 km of height a tile.
- 64-bit references: 2^20 polygons per tile, 2^28 tiles.
- Within a tile, polygons connect only through a shared edge with
  identical vertex indices. A step between two plates becomes a shared
  edge covering exactly the passable stretch. Both polygons are split so
  they share its two ends, the edge sits at one height, and each polygon's
  detail mesh carries its true surface.

  Detail meshes are optional: without one, Detour fans the polygon, which is
  exact for a planar polygon. Up to 255 detail triangles per polygon.
- Edges on a tile's border are flagged as portals (`0x8000` and the side),
  and Detour joins them to the neighbouring tile by its overlap rule, using
  `walkableClimb`.

**2c. Nodes by construction, first version (biped)** (`zz_plates` in
`crates/game/src/navmesh/zz_plates.rs`).
- **Method.** Outdoors the plates form a flat map seen from above, so
  walking space is exact in 2D: every plate piece, less a band of the
  body's radius round every edge it can't cross. Nodes are what's left,
  joined across the stretches it can cross.
  - The generator hands out its pieces through the same visitor the mesher
    uses (`PlateWorld::plate_pieces`, split out of `emit_split`), so the
    outlines are by construction the meshed ones. The world generator's 40
    tests still pass.
  - A step that changes along a shared edge (tilted plates) is cut exactly
    where it crosses the climb. Earthworks are sampled every 25 cm.
  - In this version, pillar plates are all obstacle (the ground round a
    grown pillar is left out), and canals' cuts are edges.
- **Agreement with step 1:** step 1 sampled the mesh; this is exact
  geometry from the plates. The two share nothing but the generator, and
  agree within 2 points in every district. Large nodes, step 1 against
  construction: floor 98 / 98%, tiers 0 / 0, stacks 38 / 40, broken 0 / 0,
  uplands 38 / 38, highlands 67 / 69. Walker room: 100 / 100, 50 / 49,
  43 / 44, 27 / 27, 96 / 95, 91 / 92.
- **Speed:** under 14 s per 1.4 km patch, under 1 s in most districts.
- **The definition's tests,** in all six districts:
  - every open stretch is met by the same stretch from the piece across:
    0 unmatched;
  - every stretch with nothing across, inside the patch, is a canal's cut
    or a hole: 0 unexplained.
- **Found and fixed (2026-11-03): holes through the ground.** Since the
  first prototype, the generator had skipped plate pieces under 0.5 m².
  Each one was a hole straight through to the void under the world: a ray
  down found nothing beneath any of them. Full patches had 274 (floor),
  1085 (broken) and 2424 (stacks). The user had seen them. Now every piece
  with any area is meshed (`LEAST_PIECE` in plates.rs), and the count is 0
  in all six districts. Node results are unchanged. The world generator's 40
  tests and the game's 50 pass.

Next in step 2:
- the ground round grown pillars, with pillar tops as surfaces (perches);
- canal floors;
- export to Detour tiles;
- 2d: reachability against Recast/Detour for every pair, and walkability of
  returned ways;
- the teleport-frequency measure.

**2c, pillars (2026-11-03).**
- **Method.**
  - The generator hands out a pillar's grown prisms through the code that
    meshes them (`PlateWorld::pillar_prisms`, split out of `emit_pillar`).
  - A pillar plate is ground at its base, less the cross-sections of its
    prisms within a body's height (2.6 m), grown by the radius.
  - Each prism's top is a perch: the top, less whatever of its pillar lies
    within a body's height above it, shrunk by the radius. That's computed
    as (top − above) shrunk, which is the same as (top shrunk) − (above
    grown) and about 300× cheaper.
- **Robustness.** Coordinates are snapped to 1/1024 m. That's exact in
  binary, and a whole number of Detour's 1/64 m cells. The boolean library
  (i_overlay 4.0.7, under geo) panicked on one pillar's near-degenerate
  slabs until then. Failures are now caught per pillar and counted: 0
  of 8308.
- **Stacks, full patch:** ground in walker-sized nodes 49% (44% with
  pillars all obstacle), in large nodes 42% (was 40%). 7416 perches of at
  least 0.5 m², 58 661 m², on 8308 pillars. Under 40 s in all.
- **Pillars can't be climbed by walking.** I first claimed they were
  walkable stairs, from 550 839 pairs of touching slab tops within a
  climb. That was an inference, and wrong. The user doubted it from
  memory: "the vertical differences seemed bigger". Measured properly
  (usable tops only, room for the body, each step within the climb, from
  the ground), no pillar in the full stacks patch is climbed above 0.6 m,
  out of 8308. The touching pairs are mostly slabs with others too close
  above them to stand on. Median pillar 14 m tall. So perches are separate
  nodes by the geometry, not by choice: reached only by teleporting or by
  the player's own movement.

**Canals (the user, 2026-11-03): creatures don't walk in canals, for now.**
Canals (frictionless half-pipes 14 m wide, rims about 3.5 m below the
ground) and their station courts stay out of walkers' space. Plates stop a
body's radius short of a canal's cut, as already built. Deferred: what
happens to a creature pushed into a canal.

**2d. Cross-check against Recast/Detour (2026-11-03).** The method:
- the exact nodes, against Recast/Detour built independently from the
  same patch's mesh (the identical set of plates);
- 4000 pairs per district: 3000 within 150 m, 1000 within 1000 m, at
  least 200 m inside the patch;
- every way Detour returns, followed every 10 cm over the exact walking
  space.

`zz_plates` (ZZ_PAIRS, ZZ_CHECK) and `navbench <dir> nodes`.

- **Two kinds of difference are Recast's own rounding, not errors.**
  - Recast keeps the climb in whole cells: floor(0.6 / 0.0875) = 6, which
    is **0.525 m, not 0.6**. So the game's Recast navmeshes deliver less
    climb than `navmesh::CLIMB` promises.
  - It rounds headroom up to 30 cells, 2.625 m.

  With our model given the same climb and headroom, as below:

  | District | Agree | Way only in our nodes | Way only in Detour | Off Detour's floor |
  |---|---|---|---|---|
  | Floor | 3998 | 0 | 0 | 2 |
  | Tiers | 3999 | 1 | 0 | 0 |
  | Stacks | 3969 | 16 | 0 | 15 |
  | Broken | 3997 | 0 | 0 | 3 |
  | Uplands | 4000 | 0 | 0 | 0 |
  | Highlands | 4000 | 0 | 0 | 0 |

- **Detour finds no way our model lacks, in any district.** The ways
  only our nodes have, and the points off Detour's floor, are gaps a body
  fits through but Recast's voxels close. They're mostly round pillars.
  A body one voxel wider (0.41 m) in our model leaves tiers at 4000/4000
  and stacks at 3 and 6.
- **Errors found and fixed in our model:**
  - **Low slabs as obstacles.** A pillar slab whose top is within a climb
    of the ground is stepped onto, not walked round. Detour walked over
    ground our model called blocked. In stacks, ways beyond the radius
    outside the walking space fell from 246 m to 24 m.
  - **Joining by midpoint only.** Fragments were joined only at the
    middle of each crossable stretch, so where one side's walking space
    was in several fragments, some stayed apart. Now tested every 10 cm.
  - **Different regions.** First, Detour's terrain reached past our pieces
    (20 m margin). Then whole pieces reached past Detour's terrain. Now both
    use the same column-aligned set of plates.
- **Detour's ways against the exact walking space:** 0.01% to 0.47% of
  their length runs more than 5 cm outside it, almost all within the
  body's radius. Highlands, for example: 1046 km walked, 2.6 km within
  15 cm to the radius, under 1 m beyond it. So Detour's ways run up to
  35 cm closer to drops than the body allows, mostly on tilted plates,
  where Recast only approximates where the step crosses the climb. The
  exact model has no such error by construction.

## The teleport measure (2026-11-03): how much teleporting each district needs

Written before it ran. It's a measure, not a test with a pass mark: it
feeds the teleport's design and the slice question "walker or blinker?".

- **Patches and body:** the same 1.4 km patch per district as before. The
  biped (radius 0.35 m, climb 0.6 m, headroom 2.6 m), on the nodes built by
  construction.
- **Nodes:** district ground only. Perches are left out, being unreachable
  for walkers for now.
- **Chases:** 2000 per district, each from a random point on the walking
  space to another 20 to 150 m away (flat distance). That's the range of a
  hunt, up to the stalker's 150 m.
- **The teleport rule, a parameter:** from anywhere in its node, a walker
  may teleport to any point of another node whose walking space comes
  within R metres (3D distance, closest points). R is 3, 5, 10 and 20 m.
  A variant allows only nodes of at least 10 m² as landing places.
- **Reported, per district and range:**
  - the share of chases needing no teleport (start and target on one node);
  - the fewest teleports per chase: median and the worst 10%;
  - teleports per 100 m of straight-line distance;
  - the share of chases the target can't be reached in at that range;
  - as a range-free proxy, the node boundaries the straight line from start
    to target crosses, per 100 m.
- **Limits:**
  - It counts what the geometry forces, not what a creature chooses.
  - Node distances are taken from points every metre along the nodes' edges,
    so they're good to about half a metre.

**Teleport measure results (2026-11-03)** (`ZZ_TELEPORT=1 zz_plates`). For
chases of 20 to 150 m, at a 5 and a 10 m range:

| District | No teleport needed | Teleports per chase, median (worst tenth) | Per 100 m | Unreachable |
|---|---|---|---|---|
| Floor | 96% | 0 (0) | 0 | 4%, reached at 20 m |
| Highlands | 73% | 0 (1) | 0.3 | 0% |
| Uplands | 63% | 0 (1) | 0.4 | 0% |
| Tiers | 8% | 2 (6) at 5 m; 2 (5) at 10 m | 2.8 / 2.4 | 0% |
| Broken | 14% | 2 (5) / 2 (4) | 2.6 / 2.2 | 2.6%, even at 20 m |
| Stacks | 24% | 3 (9) / 2 (6) | 3.9 / 2.9 | 2.9% at 5 m, 0 at 10 m |

- **Two groups.**
  - Floor, uplands and highlands rarely need a teleport.
  - Tiers, broken and stacks need one every 35 to 45 m of chase, about every
    7 to 9 s at a hunting pace.
- **Range matters little past 5 to 10 m.** Each terrace line costs one
  teleport whatever the range. At 3 m, stacks gets markedly worse: 5 per
  100 m, up to 47 in a chase.
- **Landing only on nodes of 10 m² or more** changes almost nothing.
- **A few targets in broken are unreachable even at 20 m** (2.6%), likely
  the mesas and pits; not checked.
- **These are the fewest teleports the geometry allows.** When and where
  they happen needs the chase maps.

## Teleport prototype (2026-11-03)

Built to feel teleporting in the game (`--opt blink`), after the measure.

- **Nodes at runtime.** The construction moved out of the test into
  `crates/game/src/navmesh/nodes.rs`; `zz_plates` now calls it and reproduces
  the recorded tiers numbers exactly. `--opt blink` builds the nodes over a
  600 m square round where you start, in the background (tiers 3.5 s, broken
  1 s, stacks 13 s); `--opt nodedraw` draws them, a colour each, slivers
  under 10 m² dark red.
- **Walking within a node.** Each node's outline (its fragments, each grown
  2 mm and joined, then simplified within 2 mm) is triangulated by
  constrained Delaunay and gets its own landmass 2D navmesh.
- **Teleporting** (`navmesh/district.rs`, `Blink` on every hunter). You on
  another node: it plans by A* over "portals" (both ends of every hop,
  linked by straight-line walking within their node), each teleport costing
  `blink_cost` metres of walking (8). It walks to where the first teleport
  leaves from, stands for its tell (`blink_tell`, 0.6 s: a glow where it
  stands and where it will land), is there (a flash at both), and rests
  (`blink_rest`, 1.5 s) before another. Range `blink_range` (10 m). Leaps and
  dashes only within its own node.
- **Test** (`blink_walks`, ignored, run by name; `BLINK_DISTRICT=`): a walker
  moved as its steering says chases 300 times from spot to spot (20 to
  150 m). All reached in tiers, stacks and broken; every teleport within
  range, onto the walking space; never more than 0.2 m off it (cutting
  corners, within the radius band).

**Findings.**

- **Fewest teleports is the wrong plan.** Tiers' nodes are long terraces:
  planned for the fewest teleports, a walker walked 220 m along one to make a
  single hop rather than blink twice where it stood (34 of 300 chases not
  reached in 120 s). Planned by time (walking plus 8 m a teleport), it walks
  far less and teleports about a third more than the fewest:

  | District | Teleports per chase | The fewest | Chases reached |
  |---|---|---|---|
  | Tiers | 3.1 | 2.3 | 300 of 300 |
  | Broken | 2.5 | 1.9 | 300 of 300 |
  | Stacks | 5.3 | 3.9 | 300 of 300 |

  So in practice a chaser in these districts teleports somewhat more often
  than the measure said.
- **A walker must commit.** Ways found anew every half second flipped round
  a pillar from one side to the other (landmass's search over triangles isn't
  exactly shortest), and plans found anew every second alternated between
  two about as good; both left walkers pacing for ever. Now a way is kept
  till the goal moves 1 m or the walker strays 1 m from it, and a teleport
  till the goal changes node or moves 8 m.
- **Edges need care.** Joined fragments miss each other by up to half a
  millimetre on the 1/1024 m grid (hence the 2 mm growth); fragment corners
  lie up to 0.6 mm outside their piece (lookups tolerate 1 mm); a walker
  landing on the edge of two nodes keeps the one it landed on.
- **Cost.** Steering is a few microseconds a frame; a new plan costs a median
  0.1 ms in tiers and broken (worst 2 to 8 ms), but 0.7 ms in stacks (worst
  11 ms). Stacks needs a cheaper plan (spread over frames, or a coarser
  graph) before it is played seriously.
- **In the game** (smoke run, tiers, a gladiator): it chased, and when you
  were shoved down a terrace it walked to the edge, told, teleported 1 m
  across and 2 m down, and went on. A step *down* of 2 m is a drop a
  creature could take; whether walkers drop rather than teleport down is a
  design question.
- **Kept on their nodes (2026-11-04).** The user's first feel test: with
  teleports held off (`blink_rest=1000`) a gladiator still walked up and
  down plates, and it often reached them and then teleported. Outdoors
  nothing stops a creature: its sweep passes through everything and its
  height is the ground's wherever it steps (`ground.rs`), so whatever moved
  it other than its steering (a shot's knock, its momentum, a straight run)
  took it across risers. Now a creature on district ground is held to its
  node's walking space in its own move (`hunt`'s sweep, `Nodes::keep_on`),
  and after anything else moves it (`keep_on_nodes`); it leaves its node
  only by a teleport or a fall through the air. Checked by shoving one
  sideways every 3 s: it slid along its node's edge and stayed on its 26 m
  plate; with only the after-the-fact keep it dipped 2 m over the edge for
  a frame first. To hold teleports off: `blink_range=0` (`blink_rest`
  counts only after a first teleport).
- **The shade (2026-11-04).** The user's design for the teleport's look,
  derived from the echo: its dark silhouette splits from it, goes to where
  it lands as a dash goes, in a straight line through whatever is in the
  way (so a lip in the way does not matter), and it reforms there. Built
  for authored bodies (`clip/shade.rs`): a black, unlit copy of the model
  posed as the body; it comes 0.6 m out of the body toward the landing as
  it tells, then the body is hidden and the creature glides along the
  teleport's line at `blink_speed` (20 m/s, the dash's), walking on as in a dash;
  landed, the body is back and the shade lingers on it 0.25 s. Gliding, it
  can still be shot. Other creatures glide too, in their own look, with the
  glows. Authored bodies now walk or dash, never run (`docs/creatures.md`).
  Then (the user, same day: it paused before going; the body was not left
  behind enough, it should remain until the shadow reaches its
  destination; the shade need not animate): with a shade, no tell (unless
  `blink_tell` is given); the shade leaves the body at once and goes along
  the line facing its way, still, in the pose the body had; the body stays
  where it left, held still in its stride, and is the one shots strike,
  till the shade is there; then the body is there and the shade lingers on
  it 0.25 s. Creatures without a shade still glide along the line
  themselves.
- **Where a teleport starts and lands (2026-11-04).** Hops are the nearest
  points of two nodes' edges, so every teleport went from the foot of a
  wall to its lip (the user). They are still what plans are made by, but a
  teleport now lands on the point of its node within range nearest where
  it goes next (the next takeoff, or 3 m short of you), at least 1 m in
  from the edges (`Nodes::land`); and a walker on its way to a takeoff
  goes from where it is as soon as it lands no worse off, all told
  (`Nodes::sooner`, looked at every 0.25 s). Measured on tiers and broken
  at the 10 m range: median teleport 8.4-8.7 m before, 9.8-9.9 m after (the
  shortest tenth 4-5 m before, 9.1 m after); landed 1 m or more inside 5-18%
  before, 64-69% after; 8% less walking; chases all reached.
