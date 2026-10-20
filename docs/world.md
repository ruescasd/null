# The world

What the open world is made of, as of 2026-10-17: the districts, the
landmarks placed in it, and the lab's catalogue of patterns not yet placed.
The chasm is a separate mode (`--opt chasm`) and is listed only briefly.

The world is a 16 km square that wraps at its edges. Its ground is
**plates**: flat-topped prisms in a Voronoi pattern at three sizes (128,
32 and 8 m apart). They follow a smooth underlying landform (a noise field
about 2 km across) in flat steps. Heights now run from -21 to 206 m: half
the land within a few metres of 8 m, 95% below 64 m.

**The ground's subdivision.** Below what the geometry draws, the terrain
shader carries on the plates' own rule: a plate splits into the pieces of a
finer Voronoi diagram that fall inside it (32 m, then 8 m; 80% of pieces
split again), every piece flush but its own tone and slight tilt (the canal
stones' numbers), with a grooved joint round it (lit as a 45-degree chamfer
on either side). It gives a sense of speed without a texture: the user
judged a pattern acceptable only where it describes how the thing was
built, which the noise grain, flecks and cracks did not. The finer pieces
(2 and 0.5 m) read as painted and are off. `--opt nosubdiv` for none;
`--set subdiv_smallest` (8; 32 for the 32 m pieces only, 2 or 0.5 for the
finer ones), `subdiv_split`, `subdiv_groove`, `subdiv_tone`, `subdiv_tilt`,
`subdiv_joint`. The smallest pieces drawn are a district's own lever (the
user was torn between 8 and 32 m): 32 m in the highlands, 8 elsewhere. No
measurable cost.

## Districts

The district map is a coarse Voronoi diagram (cells about 2 km), so
districts span 1.5–3 km. Each sets the plate generator's rules. Relief and
tone are blended across borders so districts meet without accidental
cliffs. Defined in `crates/worldgen/src/district.rs`.

| District | Share | Character | Relief | Ledges | Extras |
|---|---|---|---|---|---|
| **Floor** | 26% | Vast pale flat plates, like plazas: steps of a few centimetres | ×0.04 | 0.25 m | rare mesas and pits; the hanging cities stand here |
| **Tiers** | 11% | Terraced slopes of 2 m ledges: everything is a mantle away | ×1.3 | 2 m | big plates rarely split |
| **Stacks** | 26% | Dark, dense piers and pillars on small plates | ×0.6 | 1 / 0.5 m | pillars on 18% of the finest plates (5–28 m tall) |
| **Broken** | 19% | The original broken plate terrain, a bit of everything | ×1.0 | 2 / 0.75 m | small plates, nudged; rare pillars |
| **Uplands** | 12% | Gentle rolling hills of tilted plates, level crests | ×1.5 | 1.75 m | big plates, tilted to set gradients and eight headings |
| **Highlands** | 6% | High country: tilted plates in strata, lunar ridges | ×3.0 | 1 m | no level crests; the shader's pieces only 32 m |

Measured heights by district (median, 95% below, highest): Floor 1 / 8 /
87 m, Stacks 12 / 53 / 128 m, Broken 16 / 74 / 170 m, Tiers 18 / 92 /
206 m.

Within any district: **mesas** (raised big plates) and **pits** (sunken
ones), dark or light **marker** plates, and **pillars** grown from the
`pillar` form.

Shares are of the ground (seed 1). The district map picks floor, tiers,
stacks or broken by weight, then turns two fifths of the tiers into
highlands and of the broken into uplands. `--set district=N` makes one
district everywhere (0 floor, 1 tiers, 2 stacks, 3 broken, 4 uplands, 5
highlands). Uplands and highlands have no sites or colossi yet.

## Rolling terrain: uplands and highlands

Plates that tilt to follow a rolling landform (to set gradients, about
5–19 degrees, and eight headings), keeping their steps so the land reads as
built. They follow the landform without its ridges, which fade in across
their borders. They came from an experiment of three versions (big plates
with 1 m steps; 1.75 m steps and level crests; 2.5 m steps and level
crests).

**The user's findings (2026-10-17), which made the two districts:**
- Version 2 works better for reliefs 1 to 2 (now the uplands); version 1
  works better for reliefs 3 and up (now the highlands).
- These variants suit the terrain of biomes: kinds of land that districts
  are made from, alongside Floor, Tiers, Stacks and Broken.
- Small plates with big steps read as the Broken district, and distant
  coarse plates as blocks of houses: rejected.
- Taller hills than the steepest tilt (19 degrees) can follow turn into
  stacked walls; steeper tilts, up to about 40 degrees (still walkable),
  are untried.
- Full daylight washes the slopes out. Judge them at a low sun
  (`--time 60`) or darker (`--set ev=12`).

## Landmarks in the open world

Set in `data/structures.ron`.

**Hand-placed structures:**
- **The spire** at (1637, 791): the `spire` box style, 60 × 340 × 60 m.

**Sites**, which grow out of the ground, reshaping it into a core and
terraces. The site grid is 768 m with a 55% chance per cell, but for now
only the placed examples are used:
- **An ordered site** at (2633, 2004): a `spire` on square terraces with
  stairs on the axes, `towers` plates around it. A canal now passes 60 m
  from it, which squeezes it out.
- **Towers on a round mound** at (2698, 413), among the stacks' pillars.

**Colossi**, megastructures on a 2 km grid (85% chance per cell; 12 in the
world on seed 1):
- **Needles** from a square mound with ramps (`colossus_needles`), placed
  at (2801, 834).
- **Shattered shafts** from a low mound (`colossus_ruin`), placed at
  (4872, 3097).
- **Hanging cities** (`city_tall`) in the Floor district, on low square
  platforms with ramps. These are the rule; the rest of the colossi are
  these.

**The canal network:** 9 stations (placeholder courts) and 14 canals; see
`docs/canals.md`.

## The lab's catalogue, not yet placed

`--opt lab` shows these on flat ground; `--focus name` shows one. They
passed review as patterns.

| Pattern | What it is |
|---|---|
| `city_tall` | The hanging city: a sparse lattice of decks, each a composed unit (also placed, as colossi) |
| `reactor_square` | A square hanging city round a void, a reactor in it |
| `reactor_round` | The city in rings round the reactor |
| `complex_mixed` | A colossal core in a human-scale fabric, grand stairs |
| `mg_tower` | Yardstick: the Manifold Garden tower |
| `unit_plaza` | Stoas, a rotunda or tower, gateways |
| `unit_court` | Halls round a court; some with a giant loggia on top |
| `unit_terraces` | Terraces on the axis |
| `unit_gallery` | A hall, towers, a colonnade or vault |
| `unit_tower` | Towers on a podium |
| `unit_cascade` | Trays cantilevered off a solid core (after Fallingwater) |
| `unit_garden` | A pool on the axis, pergolas, pavilions, obelisks, a gateway |
| `back_plain` | Colossal wedges behind a human-scale field |
| `back_flight` | Wedges with a human-scale flight up the middle |

The building blocks behind them: box styles (`needle`, `tower_block`,
`slab_block`, `lattice`, `spire`) and forms (blocks, towers, slabs, courts,
daises, pillars, crowns, needles, ruins, terraces and the units above).

## The chasm (a separate mode)

`--opt chasm`: a floorless void with places in its walls, reached by paths
inside the rock. It has the region (`--opt region`), the room lab, and the
orb, paintings, planetarium rotundas and ichor pool among its places.

## Known issues

- **Earthwork rims: the mesh and the height function disagree.** Where a
  site's or colossus's earthwork meets the natural ground, a sloped plate's
  top is drawn as flat facets between its corners, while the height
  function (`height_at`, the walls' sampling, `--cam` placement, bots)
  follows the earthwork's smooth curve; where the curve bends the two
  differ by over 15 m (seen at the rim of the hanging city at (9413,
  11527), near (9550, 12190), in facets of 45 to 65 degrees). No holes
  come of it as far as a ray test from points on the mesh shows, but
  anything placed by height there stands inside or above the ground.
- **Holes onto the void (fixed 2026-10-18, `a4eda67`).** Walls between
  plates sampled the ground beyond them too sparsely: tilted neighbours
  and station courts were missed. A ray test (from every station's court,
  and down at a slant from 600 points standing on the mesh in the tiers,
  uplands and highlands) now finds none.
- **Holes beside pillars (fixed 2026-10-20).** A wall sampled a pillar's
  nominal top beyond it and was left out, but at full detail a pillar is
  its open form on a low base: the void showed between its slabs (the
  user's, at (16305, 16372), in a site at the world's corner; the corner
  was chance). Walls now sample a pillar's base (`PlateWorld::solid`). The
  probe: `cargo run -p worldgen --release --example holes -- X Y Z` (Y
  `auto` to stand on the ground), rays down from a spot through ground
  meshed as the game does; 462 rays through there before, none now; over
  40 spots scattered across the world, 3 rays at 2 spots remain.
- **Hairline cracks between plates.** Those 3 rays: skimming a flat top
  (at (2499, 14212), 7 cm over it), a ray slipped between two plates
  meeting at one height. Not seen in play.

## Removed, worth revisiting

See `docs/removed.md`: stepped ziggurat sites, racks and pipe networks,
curtains, the massif, and the stepped-mountain and walled-enclosure colossi.
