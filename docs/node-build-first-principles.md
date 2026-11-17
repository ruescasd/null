# The district node build from first principles

Written 2026-11-16 at the user's request: define what the build must
produce for navigation, the least work that takes, and what the code does
instead.

**Basis.** All times are measured on seed 2, on one thread, on the 600 m
square of district ground built round the player (`SIDE`). The stacks are
the example throughout: they are the worst of the six districts.

**Two kinds of number.** A *measured* number comes from a run. An
*estimate* is counts multiplied by operations timed on this machine. My
earlier estimates of this kind missed by 3 to 10×, so the estimates here
are a bar to test against, not results.

## What navigation needs from the build

Everything here is for one body. The walker is 0.35 m in radius, climbs
0.6 m, and is 2.6 m tall.

1. **The plates in the square.** These are convex polygons, each with its
   height. Pillars come with their slabs within the body's height.
   - Stacks: 10 418 pieces, 78 899 slab prisms.
2. **Which plate edges the body can cross.** An edge is crossable where the
   step across it is within the climb.
   - Stacks: about 54 000 edges.
3. **The walking space.** This is where the body's centre can be: each
   plate, less 0.35 m from the edges it can't cross and from pillar slabs.
   It is stored as convex parts, which are also the navmesh polygons Detour
   takes.
   - Stacks: 104 000 parts, 473 000 corners.
4. **Nodes.** Parts are joined where they touch, which gives the portals
   between parts (for paths within a node) and each node's rim.
5. **Drops.** These are one-way links from a node's rim down to a lower
   node, each with a fall checked clear.
   - Stacks: 11 855 lips.
6. **A planning graph** over nodes and drops.

## Stage by stage (stacks)

| stage | what it produces | measured (ms) | least work, estimate (ms) | how I got the estimate | analysed? |
|---|---|---|---|---|---|
| pieces | 1 | 272 | 2-30 (see below) | ~6 clips (25 ns each) per piece, plus each plate's lookup | split into steps, not to the bottom |
| edges | 2 | 22 | ~2 | one hash and three heights per edge | no |
| obstacles | keep-out regions for 3 | 103 | ~25 | one hull per region (16 points: 0.8 µs); 24 708 regions | no |
| walking | 3 | 289 | 10-20 | see below | yes |
| nodes | 4 | 120 | ~15 | one sort of 473 000 edges by line, then a walk | no |
| prune | removes nodes under 10 m² | 13 | ~2 | one pass over the parts | no |
| drops | 5 | 119 | ~3-10 | model in `seed_sweep` | no |
| portals | 6 | 19 | ~9 | one Delaunay point per portal | no |
| **all** | | **959** | **~70-110** | | |

In the estimates, ~ means about.

So the work should cost roughly 0.1 s of CPU per square. It costs about
1 s. The other big districts are close to the stacks: tiers 494 ms,
broken 969 ms.

## Where the code departs from the definition

### Pieces: the same work done twice

Its steps, measured:

| step | stacks | tiers | broken |
|---|---|---|---|
| splitting the plates (`plate_pieces`) | 180 ms (17 µs a piece) | 150 ms | 186 ms |
| pillar prisms | 37 ms | (no pillars) | 1 ms |
| stitching the corners together | 48 ms | 41 ms | 47 ms |

Two points:

- **It is duplicated.** The terrain mesher builds the very same pieces with
  the same function (`visit_split`, through `mesh_column` → `emit_split`),
  for every full-detail column round the player. Navigation builds them
  again from scratch.
- **The 17 µs a piece is unexplained.** Clipping a cell by its ~6
  neighbours is about 0.15 µs. What the rest is (canal cuts, plate lookups,
  splitting across levels) has not been measured.

### Walking: subtracting general regions one at a time

From first principles, a piece's walking space is three steps:

1. **The piece pulled in** by 0.35 m along each of its own edges the body
   can't cross. That is one straight cut per edge, and the result stays
   convex. This holds where the wall covers the whole edge; how often it
   covers only part was not counted.
2. **Less what intrudes:**
   - the round ends of walls near the piece;
   - the bands of other walls that reach into it;
   - the pillar feet in or near it.

   Each of these is a convex polygon of 14 to 16 sides.
3. **The result split into convex parts.**

What intrudes, counted per region tried against a piece (stacks):

| kind | count | what the definition needs |
|---|---|---|
| the band of the piece's own wall | 21 941 | one straight cut each |
| another wall's band, really overlapping | 18 670 | a subtraction |
| the piece's own pillar's feet | 16 534 | a subtraction |
| other pillars' feet, really overlapping | 8 674 | a subtraction |
| bands and feet whose box overlaps but whose shape doesn't | 27 443 | nothing |

That comes to about 22 000 straight cuts and 44 000 subtractions. Priced
at 25 ns a cut and 0.15-0.3 µs a subtraction, it is 10-20 ms.

What the code does instead:

- **Its own walls are subtracted as 14-sided bands**, not cut straight. A
  band crossing a piece cuts it along several of its edges.
- **The 27 443 regions that overlap only by box are tried in full.**
- **Every region is tried against every part made so far.** That is
  279 000 tries for 93 000 regions, and 343 000 cuts. It makes 437 000
  parts to keep 104 000: the rest are merged back after each region.
- **Corners are 112 bytes, with 64-byte names.** Every copy, hash and
  comparison pays for that size. This was improved today but is still
  there.

The work tracks the parts made and thrown away, not the walking space
produced. That is why it is about 1 µs per try and 289 ms in all.

### The same data problem elsewhere

The edges stage finds each edge's twin by hashing pairs of 64-byte
corner names. That is the same cost that dominated walking. It is a
suspicion, not yet measured.

## What is not known

- What `plate_pieces` spends its 17 µs a piece on.
- Breakdowns of obstacles, nodes and drops.
- **Behaviour in play.** Navigation has had two or three real runs in
  two districts, cut short by frame drops. Whether it works in the game
  has not been shown in any district.
- **Recast's build cost on this ground.** Its Rust port measured 93 ms per
  58 m tile per core (2026-11-01). A 600 m square is about 100 tiles, so
  roughly 10 s of CPU. The C++ library's build time was not measured.

## The choice this gives

- **Finish our build.**
  1. Change walking's shape to the three steps above, prototyped and timed
     on one district before anything else. It must reach about 20 ms in
     the stacks, or the approach is wrong.
  2. Take the pieces from the terrain mesher instead of building them
     again.
  3. Measure the stages not yet analysed the same way.
- **Use Recast for district ground.** First time C++ Recast on the same
  square. If it is far under 1 s with acceptable results, it is the
  routine option. The measurement we have, from its Rust port, says ~10 s.

Either way, the next real test is in play, in all six districts, with
frame times logged.

## Walking prototype: result and why (2026-11-16)

The user chose to finish ours, and asked for the reason if the target was
missed.

**What changed:**
- Corners are 24 bytes (place, point number, line number). Keys and lines
  live in tables per piece.
- A crossing is named by its two lines' numbers.
- Each region is tested against the whole piece before any part is tried.

The parts are the same (stacks 103 896) and every check passes.

**Result:** walking went from 289 to 233 ms in the stacks, 246 to 191 in
the broken, and 106 to 90 in the tiers. The target was about 20 ms:
**missed by more than 10×.**

**Why: both counts and costs were wrong in the estimate.** Measured with
cycle counters in every step (stacks, about 1 030 million cycles):

| step | how many | cycles each | share |
|---|---|---|---|
| a new crossing, named exactly and placed | 603 834 | 376 (placing 150) | 22% |
| a cut's side tests, other than the split | 164 000 cutting tries | ~1 040 | 17% |
| a region tested against a part | 235 712 (71 637 apart) | 480 | 11% |
| a split, other than its crossings | 342 926 | ~320 | 11% |
| merging | 80 977 joins | — | 9% |
| adding and removing parts | 437 087 | ~210 | 9% |
| a region tested against the whole piece | 91 386 | 810 | 7% |
| output as `Corner`s | 473 029 corners | ~120 | 6% |

1. **The counts were far higher than assumed.** The estimate assumed one
   or two cuts per region. The geometry gives about 3.7, and each cut makes
   about 1.8 new exact corners:
   - each region has 14-16 edges, because round ends are drawn in 12 steps;
   - a pillar's ~11 overlapping feet are taken out one after another. They
     alone make 174 000 of the 343 000 cuts.
2. **Each step costs what its own arithmetic needs.** The estimate priced
   everything at 25 ns, the cost of one plain clip. But testing a 16-edge
   region against a 6-corner part is about 120 ns of real side tests.
   Naming a crossing exactly (128-bit) and placing it is about 85 ns.

So at these counts, even ideal steps (about 20-50 ns each) give about
70-80 ms. Getting to 20 ms would take about 4× fewer steps, and that
means changing what is computed, not how fast:
- **Fewer round steps.** A geometry choice: ends become coarser.
- **A pillar's feet taken out together**, not one by one. Exactly, that is
  their union; as the convex hull, it would block more ground.
- **A different way to split the walking space into parts.**

## Pillars (decided by the user, 2026-11-16)

**What the "feet" were.** A pillar is the library's "pillar" form grown on
its plate's whole outline. It is bands of core blocks (the outline pulled
in by 0.5 m) with facade panels standing out up to 0.5 m. The code took
the core and each low panel as its own keep-out region: about 11
overlapping regions per pillar. All of that walking space was strips at
most 15 cm wide.

**Decided:**
- **The panels are inlaid.** They are sunk into faces flush with the plate
  (`Facade`'s `inlay`, now on for `pillar_band` in `data/structures.ron`).
  The user, having walked both versions: they already look better this
  way. `inlay: false` gives the old look back.
- **A pillar a walker would walk into is solid, its whole plate.** That is
  a pillar with a slab within its height, above its climb. Its edges are
  walls, like a hole's. A walker's centre now stays at most 15 cm out of
  the recesses, which are 0.1-0.5 m deep.
- **The trial switches (`--opt nopanels`, `--opt inlays`) are removed.**

**Result** (seed 2, one thread; every check passes, node reach and
walkers' tests as before):

| | walking (ms) | parts | whole build (ms) |
|---|---|---|---|
| stacks | 233 → 116 | 103 896 → 42 763 | 915 → 588 |
| broken | 191 → 173 | | 919 → 827 |
| tiers | 90 → 84 | | 489 → 473 |

**Walking now: wall bands only.** Stacks: 151 039 cuts, making 197 218
parts to keep 42 763. **57% of the cuts are along a band's round ends**
(85 499); the rest are along its long sides (65 540). The same share
holds in the tiers and broken.

From first principles, a round end matters only where a wall stops and
the edge goes on open, or where another plate's wall ends near this
plate. Where two walls meet at a plate's corner (plates are convex), the
walking space is exactly the two straight cuts. The bands' round ends are
cut there anyway.

**Wall ends counted** (seed 2):

| | wall ends at points | free (one wall) | cuts along round ends | A: at a corner with both edges walls | B: at a corner, one wall | C: not a corner of the plate | A, of all cuts |
|---|---|---|---|---|---|---|---|
| tiers | 6 315 | 166 | 62 838 | 13 931 | 46 521 | 2 386 | 13% |
| stacks | 10 875 | 2 294 | 85 499 | 29 636 | 52 901 | 2 962 | 20% |
| broken | 12 577 | 1 199 | 115 932 | 77 975 | 35 129 | 2 828 | 35% |

- **Few wall ends are free.** Almost all are points where walls on two
  lines meet.
- **Only A is redundant.**
  - B is needed: the plate goes on past the wall's end, so the rounding
    there is real walking-space boundary.
  - C is needed too: another plate's wall ending near this plate.
- **Strictly, A cuts are not zero work today.** A rounded end's corners
  sit up to about 1.2 cm outside the band's long sides (12 steps
  circumscribe the circle), so they trim slivers that size. Straight cuts
  at plate corners would be the tighter shape, still at least the radius
  from every wall.
- **The 12-step rounding is why B is large.** Every corner where a plate
  goes on past a wall end costs several cuts.

## Straight cuts for walls (built 2026-11-16)

**The rule.** A plate side that is wall along its whole length (the
maximal run of its edges on one line) is taken by one straight cut: the
band's side facing it, the band itself left out. This is exact where, at
each end of the side:
- the plate's edge there (on another line) is a wall too, because within
  a convex plate a point's nearest wall is across from it; or
- the wall goes on past the end as far as the plate reaches past it
  within the band.

Otherwise the band is kept.

**A bug on the way.** A first version reasoned per edge. Stitched
collinear edges then let a straight cut run along a side that was only
partly wall. The sweep's definition check caught it at 4-8 points per
square.

**Result** (one thread; all 48 sweep squares pass, node reach and
walkers as before):

| seed 2 | walking (ms) | parts |
|---|---|---|
| tiers | 84 → 76 | 36 402 → 35 616 |
| stacks | 116 → 84 | 42 763 → 39 317 |
| broken | 173 → 78 | 35 409 → 26 429 |

Medians over 8 seeds, whole build: tiers 355 ms, stacks 542, broken 502.

**Round steps, for the user to judge in game** (`--set round_steps=N`).
Measured on seed 2 at 6 steps instead of 12, all six squares passing:

| seed 2 | walking (ms), 6 steps | parts, 6 steps |
|---|---|---|
| tiers | 58 | 26 567 |
| stacks | 65 | 28 238 |
| broken | 66 | 20 139 |

At 6 steps a wall's round end sits up to r / cos 30° = 0.404 m from it,
instead of r / cos 15° = 0.362 m at 12. The definition check's bound now
follows the steps.
