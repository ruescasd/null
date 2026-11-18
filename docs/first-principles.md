# null from first principles

Written 2026-11-18 from a conversation in which the user and Claude
reassessed the whole project with fresh eyes (the earlier memory notes set
aside on purpose). It records what the game is meant to be, what has been
decided, what is still open, and what the technology must therefore do. It
is a reference for decisions, not a plan.

Each item is marked:

| Mark | Meaning |
|---|---|
| **Decided** | the user decided it |
| **Leaning** | agreed in direction, not yet fixed |
| **Open** | undecided |
| **Idea** | proposed, not decided (who proposed it is said) |

The project is experimental: the design is made up as we go, and many
questions are ours to answer by trying things, not facts waiting to be
found.

## 1. The game in one paragraph

You appear in a strange, dark world about which you know nothing. It was
built by an intelligence whose purpose is lost to you. You explore it,
lighting up the canals that cross it, finding things that make you
stronger, and uncovering its secrets, until the prey that arrived becomes
the menace, a demon flying at speed. However strong you become, there is
always somewhere deeper and worse.

## 2. Identity

### The juxtaposition

The identity is a juxtaposition (the user): De Chirico's classical world
(arcades, colonnades, stairs, empty squares, long shadows) against
megastructure and cosmic horror. Its effect is the unsettling kind that
good cosmic horror has, familiar forms in a wrong scale, wrong light and
wrong emptiness, not the monster tropes of *Alien*. BLAME! alone is not
original enough: it is an ingredient, never a whole picture.

Influences (the user's): De Chirico; Piranesi (the *Carceri*: the chasm,
its stairs and bridges); BLAME!; Moebius and his huge
landscapes; Iain M. Banks's *Feersum Endjinn* (megastructures, strange
beings, the Serehfa Fastness: rooms kilometres wide holding towns and
their own weather, a royal palace built into a chandelier, castle walls,
stairs and battlements at geological scale, all of it decoration over the
computation inside its walls; per a summary of the novel the user found,
2026-11-18); Manifold Garden;
the Half-Life 2 Citadel (surface language, `half-life-*.png`); Quake 3 and
Defrag (movement). Suggested by Claude as related, not the user's own:
Boullée (the starred rotunda is close to his Cenotaph for Newton; his
"architecture of shadows"), Hugh Ferriss, Superstudio's Continuous
Monument (the canals), Magritte (wrong-scale objects), Shadow of the
Colossus.

### Light and shadow on greyscale

**Decided.** Chosen for its power, not forced by resources (the user):
little art is as dramatic as lighting, shadow and geometry on a greyscale
palette. So shadows are the medium the game is made in, and their
technology is first-class work, not a workaround.

### Rules drawn from what works

From the user's review of the catalogue
(https://ruescasd.github.io/null-catalogue/, `../null-catalogue`):

1. **Construction with a method you can sense but not decode.** The spire;
   fractals as building material.
2. **Classical form meets alien presence or logic.** Shards in a
   colonnade (`1794091727`), a fractal painting nobody explains
   (`1794094199`), arched bridges and screens on a humming machine
   (`1794083946`), Italianate towers on a fractal lattice (the hanging
   cities).
3. **Light and shadow are the medium.** The hypostyle room (`1794092034`).
4. **Loneliness, with one human-scale object** that gives the size and
   asks a question: the lamp before the spire (`1794095359`).
5. **Every place holds an unanswered question.** What is it for, who made
   it, is it watching you?
6. **BLAME! is never a whole picture on its own** (`1794103496` and the
   other chasm exteriors: good to look at, not original enough).
7. **Self-similarity across scales: a place's smallest objects are made by
   its own rule.** The void's `Echo` lamp is the cylinder field in small
   (`crates/worldgen/src/grounds/void.rs`, `Lamp::Echo`): seen from below,
   a cylinder with two walkways at right angles is a streetlight. It came
   by accident, looking at the game while designing lamps.
8. **Each view holds a known measure and something that breaks it.**
   Classical forms are scale rulers everyone reads (a riser, a column, an
   arch); the uncanny comes from breaking them: by scale (an arch a
   kilometre wide holding up the world), by context (a classical form
   where it cannot be), by logic (classical detail on alien construction),
   and by emptiness and wrong light.

### What procedural generation is for

The user's purposes, in order of discovery:

1. A large world with no resources.
2. **Mining randomness for creative output:** forms nobody would design,
   such as the hanging cities. Randomness gives the widest reach outside
   the box and also unusable results; curation is the step that keeps the
   first and drops the second (the lab's catalogue does this).
3. An endless descent (section 4).

## 3. The player's experience

### Three arcs, one spine

| Arc | From | To | Carried by |
|---|---|---|---|
| Light | a dark, unknown, threatening world | a world lit up, with refuges and beacons | the canals, turned on |
| Power | weak weapons, movement and grapple | a demon flying at speed | items found in dangerous places |
| Knowledge | knowing nothing | the world's secrets revealed | mysteries |

**Leaning.** The canals carry all three: lit, they are refuge and beacon
(light); they are a movement whose mastery grows (power); their lamps
carry codes, and their ramps launch you to places always in view
(knowledge).

The user's examples, not yet designs:
- **Power:** no grapple at first (it lies deep in the chasm); movement
  toned down until boots are found; the shotgun from single barrel to
  double to one that makes enemies bleed ichor; the grapple's range,
  pull and cooldown improved with rare filaments.
- **Knowledge:** a hidden room with a map marking canals by symbols, and a
  panel taking a number for each, read from the pattern of lit and dark
  lamps along that canal. A structure hanging high in the sky, reached at
  last by launching off a canal's upturned end at speeds only mastery
  gives; up there, perhaps, where the world's designers lived (after The
  Witness).

### Two kinds of gate

Items gate progress (Metroid). Knowledge and skill gate it too (Outer
Wilds, The Witness): the sky object is always visible and reached by
mastery, not a key. Both suit the game; the second suits a skill-based
movement game especially, and places visible but unreachable double as
orientation and goals.

### Prey to predator

The tension between fast skilled movement and horror resolves over time:
prey becomes predator. The horror must not evaporate as power grows:
threats scale with depth and darkness, so the lit world is yours and the
dark is still theirs. The swarm already brings darkness.

### Movement

What works now (the user): cruising the surface for no purpose is fun in
itself, because it is flat and open; the canals, weaving in and out and
up and down the half-pipe, are fun and may teach the movement. Believed
to have potential: platforming spaces with empty space and height, where
grapple and strafe jumping must be done right or you fall. The hanging
cities have some of this but were not built for it (ceilings you bump
your head on). Interiors are untried; Doom Eternal shows vertical indoor
combat can work.

Principles that follow:
- **Keep the running surface open.** What makes the surface samey (flat,
  open) is what makes it fun. Put the variety where it does not block the
  runs: overhead (colossi, spans, vaults, decks casting shade), on the
  horizon, low and movement-friendly (lips, kerbs, ramps, bowls), in light
  and tone. De Chirico's empty square is both the look and the arena.
- **Build to the movement envelope.** As navigation has its body (radius
  0.35 m, climb 0.6 m), movement gets its numbers, given to every
  generator: mantle reach (1.1 m, 2.5 m with a jump), jump distance at a
  few speeds, grapple range (60 m now) and pull, clearance to swing. Then
  headroom over decks, anchors within reach, gaps sized to what can be
  crossed; gap sizes tied to upgrade levels are gates at no design cost.
- **Make speed legible.** Gaining speed should be audible and visible, so
  newcomers discover strafe jumping.

## 4. The world

**Decided.** The plates read as built, by an intelligence, not as ruin.

**Decided.** The surface is finite (16 km now; its size is a parameter,
and may grow). Strangeness and danger rise with distance from the start.

**Leaning: drop the wrap** (the user, on reflection). A torus works
against two things decided above: a gradient of strangeness (every
direction comes back round) and a sense of place (no edge to be near or
far from). A world with a border has a far side and an edge, and the
border may have something to offer in itself. Costs to weigh: the
canals are loops closing round the torus (three parallel ones), so they
would need ends (which the canal-ramp launch to the sky object wants
anyway); streaming, landmarks' nearest wrapped copy and the plates'
tiling all assume the wrap.

**Idea** (the user): a crack in the world to a stranger place. Inspired
by a real hole in the geometry, whose hall of mirrors against the starfield
gave it a strong sense of mystery (see TODO, Portals). Hidden somewhere,
perhaps at the world's border. That place, being utterly wrong, could be
the endless dungeon itself, free of the practical constraints on the
rest of the world.

**Idea** (Claude): the border as the shell's end. If the world is the top
of a shell on vaults (`--opt vaults`), its border is where the shell ends:
a cliff into the void, the vaults' flank in view. At the edge the world
shows itself as an artefact, the plainest statement that it was built by
an intelligence. A crack in that flank is a natural way into the stranger
place.

**Leaning.** The surface is fixed and curated: one world, generation used
as an authoring tool (generate, review, keep), so secrets, the canal code
and landmarks stay put. Deterministic generation and sites placed with
`at` already support it.

**Leaning.** Infinity lives in an endless dungeon apart from the surface:
the chasm, already floorless, or the stranger place beyond the crack
(above). As the chasm, an endless descent, stranger and more menacing deeper down, perhaps
with one unique creature very far below. Its generation runs unreviewed,
so its rules must guarantee a floor of quality. Very deep descent needs a
floating origin (f32). Path of Exile's Delve is a reference: an endless
descent where light is the path and darkness the danger.

### Large-scale structure

The surface's problem (the user): samey, little vertical variety, no
large-scale form, no orientation. The places work (the spire, the
cylinder region, the rooms, the cities); what is between them fails.

**Leaning: locally flat, globally varied** (the user). Fast traversal
needs ground that is flat where you run; a region's identity comes from
large-scale structure: mountains, valleys, canyons. Today there are hills,
not mountains; depressions, not valleys; and the canyon experiment reads
as city streets, not a grand canyon.

**Leaning.** Voids as the world's water (the user's idea): seas, lakes and
rivers of nothing that shape the land, guide the way and gate progress (a
rift crossable only with the improved grapple), with canals bridging
them. Vaults beneath (`--opt vaults`) are a step toward it. Large voids are
large-scale structure in themselves.

**Idea** (the user): ichor lakes, a more literal ocean. Ichor already
recurs (creatures' blood, the chasm rooms' fountains, the dark obsidian
of the shard creatures), so lakes of it tie the world to its beings.
Toxic, giving off fumes: haze, which would be the lakes' own and so
striking for being rare. A tension to resolve: the haze is global now
(`look.rs`, `haze`, 4.5 km), and the README credits it with making scale
read; if haze belongs to the lakes, distance must read some other way
elsewhere.

**Leaning: many more small scale indicators outdoors** (the user), as the
lamps are. Glitched architecture (TODO, Ideas): De Chirico's stairs,
arcades and columns glitched into the ground, half buried, a stair out of
a wall, an arcade from nowhere; intact and crisp, never ruins. They carry
their own size, so they measure the world, and they are the juxtaposition
in the open. In balance: the stark alien outdoors must stay, for them to
collide with.

The plates' look today reads as nature: jittered Voronoi (`JITTER = 0.6`
in `plates.rs`) is the signature of mud cracks and basalt; the three
levels of plates look alike; heights follow a noise landform. **Idea**
(Claude), directions to compare in the lab: less jitter; joints that
differ by level (a deep cut at 128 m, a groove at 32, a hairline at 8);
plates aligned to the nearest canal or station, so lighting a canal
reveals an order that was always there; heights set by rule (terraces,
courts, channels) more than by noise.

### Orientation

[computed] With R = 40 km, the ground is hidden behind the curve beyond
about 400 m from standing height; the spire (340 m) drops out of sight
beyond about 5.6 km; to be seen across half the world (8 km) a structure
must be about 720 m tall. The canals' station beams rise without limit
and are seen from anywhere, but alike. Places with their own signature
(rule 7), colossal forms, distinct beacons and voids as geography are
the means.

### Where the darkness comes from

**Open.** The arc of light needs darkness, but the suns light the world
most of the time. [computed from `SUNS` in `look.rs`] Both suns up 25% of
the time, the main only 25%, the dim only 25%, both down 25%; a full night
lasts about 156 s (174 s at most) and comes every 8 minutes or so: a
passing dusk, not a state.

**Idea** (Claude): darkness as the unknown (no map, no beams) carries the
main arc; literal darkness lives in places (under the shell, the descent,
voids, interiors, the shade of colossal structures); and real nights,
rarer and longer (5-8 minutes in a 20-minute cycle, say), make the
surface hostile, when the lit canals matter most (Dying Light: a movement
game where night brings the hunters and light marks safety). The orbits
are parameters, so long nights can be tried and judged in play.

## 5. What the technology must do

From the user's verdicts, the shadows and levels of detail must:

- never show distant geometry as blocks, nor pop in ("the flight
  simulator effect");
- hold in fast motion over the open surface (cruising is the favourite
  activity and the hardest case), at a low sun (the long shadows are part
  of the look);
- cause no hitches as the world generates;
- leave room for the world's lights changing as canals are lit;
- run at 100 frames a second or more at 1080p on an RTX 5060, with low
  input latency.

Known so far:
- The root of most recent shadow problems is that the levels of detail
  disagree: each coarse level built by its own code path, drawn lowered,
  two copies overlapping during each 0.4 s cross-fade
  (`HANDOFF-2026-11-18.md`).
- The traced-shadow probe (`--opt rt`) was cheaper and exact when still;
  it failed only where the copies met. Whatever the shadow technique, it
  needs levels that agree.
- Geomorphing remains a possibility; the work done on it was not useful
  and was scrapped (the user).
- The handoff's "the user's decision on the bend: display only" was not
  the user's decision; the user did not recognise it.

Also open: creature navigation on the surface and in the cities, the
world's wrap, generation hitches.

## 6. How we work

- **Serious about screwing around** (the user): a hobby project with no
  deadline or fixed aim, worked on professionally, with ambition and high
  standards.
- **Unusual results need unusual ways of working.** Wacko ideas are
  welcome, each assessed honestly; accidents are kept when good (the
  lamp).
- **Something real is not out of the question.** Hobby projects that
  become real games usually get there by narrowing at some point: one
  slice finished in all its parts. **Idea** (Claude), a slice for null:
  one surface region with a void and its rims, one canal to light, a place
  with a question in it, one creature, the movement with one upgrade
  found, shadows that hold at cruising speed. Not scheduled.
- **Probes end with a verdict** (keep, park, remove) rather than staying
  half alive behind a switch; `docs/removed.md` keeps the parked.

## 7. Ideas on the table

Not decided; kept so they are not lost.

- **Colossal furniture** (Claude; the user: the Serehfa Fastness): the
  world's own objects at landscape scale, such as the canals' streetlight
  800 m tall, dark until its canal is lit. Landmarks with recognisable
  shapes, tied to the light arc. Risk: whimsy instead of dread; the
  world's own vocabulary only, never household props, and rare.
- **A presence at landscape scale** (Claude): something alive glimpsed on
  the horizon or among the colossi, too big to fight. Creatures are all
  at human scale now.
- **Starting in the dark beneath** (Claude): beginning under the shell and
  climbing into the light.
- **The name.** "null" was picked for a repository, by chance; it fits
  the void and cosmic horror, and a room where the world's designers
  lived would explain it. Hard to search for if the game is ever shown.

## 8. Open questions

- Where the darkness comes from (section 4), partly by experiment.
- The plates' rework toward reading as built (section 4).
- What a session is: the goal's form (story, a creature far below, the
  sky object, all of these).
- What the hanging cities are for, and how players and creatures move
  through them.
- How movement and the grapple work indoors.
- The shadow and level-of-detail approach, against section 5.
