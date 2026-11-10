# TODO: pending work and future directions

Everything open, agreed or exploratory, in one place for review. Kept up to
date as work moves on. Details live in the notes named in brackets: docs in
`docs/`, or my memory notes (I can read any of them out on request).

Each item is marked by what it waits on:

| Mark | Meaning |
|---|---|
| **You** | waiting on your look, feel test, decision or a sample |
| **Next** | agreed, to do next |
| **Agreed** | agreed, not yet scheduled |
| **Parked** | set aside on purpose, to come back to |
| **Idea** | exploratory, not decided |

Last updated 2026-11-09.

## Current focus

**Combat, creatures and navigation, together.** Combat was the original
focus; working on it revealed how much it depends on the creatures and on
navigation, so the three are now one line of work.

- Navigation questions such as the Detour export are decided within this
  line, as combat and creatures need them.
- The gladiator's weirdness pass is creature work, but a tangent from the
  main line.

## Waiting on you

- **You** Stalker: the latest hover. Darts and holds, legs walking the air,
  landing to fire, strike shown as a black ray on a miss. (stalker notes)
- **You** Stalker: searchlight pale or black, never judged.
  `spawn stalker searchlight=1`.
- **You** Teleporting walkers: feel test (`--opt blink`). (blink notes)
- **You** Walkers: choose step-up or mantle for small ledges.
- **You** Sounds you are finding:
  - stalker call and death;
  - bishop arrival;
  - cable-creak warning (swarm);
  - void low hums.
- **You** Engine decision: whether spike B (streaming, far detail) goes in
  the timebox, and whether to sign in to a Unity ID for its navigation
  test. (`docs/engine-decision.md`)
- **You** Chasm: you had a more ambitious answer to "the chasm has no floor",
  held back; and another decoration idea. (chasm notes)
- **You** The catalogue (https://ruescasd.github.io/null-catalogue/, repo
  `../null-catalogue`): your shots. F12 in the game, copy the chosen `.png`
  and `.txt` pairs into its `shots/`, `python build.py --publish`. The build
  lists the unused `shot_` places in places.txt, to remove if you want.
- **You** Commit: everything since 3066c55 is uncommitted (navigation,
  gladiator waves, stalker hover and mesh, show, places, and more).

## Creatures and combat

The critical path is enemies (enemy-pipeline notes).

**Gladiator** (`docs/creatures.md`)
- **Parked** An animation pass over every creature, later (you, 2026-11-07).
  The gladiator's human swagger: its arm and trunk swings and its hips' sway
  now reversed by default (your pick, "better than what we had, but still
  work to do"); quieting them, a stoop and arms held in were tried and
  didn't work; the legs, the most human motion left, untouched. Judge with
  `spawn gladiator still`.
- **Parked** The weirdness pass, part of that animation pass, one effect at
  a time, each an option to compare:
  1. freezes mid-motion (your idea);
  2. upper body lagging the legs;
  3. a hitch in the stride;
  4. uneven time within each stride;
  5. one limb's swing inverted.
- **Agreed** Idle-to-walk snap from the 2026-10-31 plan: check whether it is
  still there (the run slide is moot now it walks or dashes).
- **Agreed** Echo double, later candidates:
  - its own will (head, flinch);
  - shots passing through it;
  - what happens at death;
  - a whole-body trail instead of a pose lag.

**Impaled brute** (impaled notes): too heavy to walk, held up by a
propulsion; the flyer of the navigation plan, and a test of flight for
creatures and craft. Its mesh is `creatures/impaled.glb`, made by
`tools/impaled_mesh.py`: back flattened, shards in place of the spikes.
- **You** Step 2 built: `spawn impaled` flies after you (14 m off,
  facing you, feet 1.5 m up); `spawn impaled course` flies a set course to
  judge it; its weight's numbers are options (`lift=`, `spool=`, `tilt=`,
  `climb=`, `speed=`, `hover=`). Your feel test. It flies through
  structures (step 4) and takes no shots yet.
- **You** Step 3 built, the field (b), low and labouring (your picks): five
  variants to judge, `spawn impaled 5 fields` (one of each) or `field=N`:
  1 haze (bent air under it), 2 glow and grit drawn up off the ground,
  3 rings pressed into the ground, 4 two thick dark beams from its soles
  to the ground, white streaks running down them, flaring at the ground,
  5 the same beams white-hot fading to black (your gradient picture),
  born white-hot 0.35 m below its soles, bent all along (bent air round
  it, the beam itself rippling), in hard straight-edged shards: a white
  crystal core in a faceted black body; `spawn impaled 5 fields` for all five (your calls, after
  tendrils, dark lightning, a dark cage, fuzzy soft beams and tyre-like
  pulses didn't work). All follow its thrust; a
  placeholder hum (pitch and loudness its thrust's).
- **You** Feet: soles made four-pointed stars (an X, prongs splayed),
  flat-shaded, in `tools/impaled_mesh.py` (in place of the paw's toes).
- Its plume: `plume=1` curves, `plume=2` shards (default); fields 1 to 4
  removed (your say, 2026-11-10).
- **You** Taking hits built: shards stick in it a moment, push and rock it,
  each hit cuts its thrust 0.12 s (it dips); health 80 (about six blasts at
  7 m; `health=`); dead it falls, lands hard (placeholder: the stalker's
  impact) and topples away from the killing shot, and lies. Open: under
  steady fire it is beaten down to the ground and drags (it can't climb
  back between blasts); keep that, or let it recover?
- **You** Step 4, stage 1 built (your call 2026-11-10: it lands to attack,
  flight its only way to move): it chooses a spot 6-11 m from you (flat,
  in your sight, apart from others), flies there straight or up over what
  is in the way (ways out first when it is shut in, as in the arena's
  pens), crashes down (thrust cut 4 m over the spot, shoved down, ~8 m/s:
  falling level; impact, your view jolted near it, its body squashed
  down onto its feet ~14% and back in about 0.3 s), stays 5 s (where your attack will be), rises and chooses
  again; takes off sooner if you get 16 m away or out of its sight;
  beaten down on its way, it stays landed. Checked: floor, stacks, spire,
  city and an arena round, every way clear (a warning if it is ever
  inside something). `keep=N` keeps the old hovering after you.
- **You** Stage 2 built, its attacks (your picks, 2026-11-10): the crash
  (it hangs 0.5 s over its spot, the tell; a ring sweeps out; within 6 m
  on the ground you are hurt, up to 25, and thrown; a jump spares you) and
  lashes from its waist (cables uncoil from the band on your side, curl
  back 0.45 s, the tell, whip out to where you were, 8 m reach, 20 a hit,
  shoved; out of reach, missed). It lands 3.5-8 m from you. Standing still
  through one landing cost 71 of 100: tune by feel.
- **Agreed** Stage 3, dense places (a 3D search), only if wanted.
- Open: a choice costs 0.2-3 ms of sweeps (one a frame across all brutes).

**Stalker** (stalker notes)
- **Agreed** Once the hover is approved: remove the gladiator-walk-on-stalker
  code (`hover=0`), listing what goes and asking first.

**Bishop** (bishop notes)
- **Agreed** Beyond 30 m it doesn't search for you.
- **Agreed** Move it to the new cloth (`fabric.rs`) from the frozen drape.
- **Idea** Blink mid-cast, its strands staying attached and stretching.

**Porter and chained pair** (porter notes)
- **Agreed** Behaviours: the guard attacks on sight; the freed porter
  charges and detonates.
- **Agreed** Energy and draining.
- **Agreed** The chain's hold shown in their bodies.
- **Agreed** Load kinds: caged, seams.

**Swarm** (swarm notes)
- **Agreed** Buds visibly becoming the hunter's solid parts (chest, hips,
  head), bound by the weave's cables.
- **Idea** The hair; looking drains health; a hound pack that flanks you.

**Others**
- **Agreed** Hounds and harrows: real blocking. They only push one way now.
- **Idea** The cable mover: throws cables to the ground and hauls itself.
- **Idea** Wall crawlers for the depths.
- **Parked** Creatures pushed into a canal.

**Navigation for creatures** (`docs/outdoor-navigation.md`, navmesh notes)
- **Agreed** Exporting our plate polygons into Detour tiles: still open, to
  be decided as the combat and creature work needs it (the teleport
  prototype walks each node on its own small navmesh meanwhile).
- **Agreed** Flyers (step 3), a playable slice with walker, flyer, teleporter
  and a seam (step 4), and a fresh review (step 5).
- **Agreed** Broken district: 2.6% of chase targets unreachable even at a
  20 m teleport, likely mesas and pits, not checked.
- **Agreed** Pillar tops (perches) are unreachable for walkers for now.
- **Agreed** Blink's nodes are the CPU load and stutter seen when flying
  (measured 2026-11-07): a 600 m square rebuilt every 150 m travelled, 4-5 s
  each, ~5 million walking links. Flying the broken with it: 29 frames over
  33 ms (worst 121 ms) against 1 without. Needs incremental building (by
  tile, as the world streams), part of the navmesh streaming item below.
- **Agreed** Bug: blink's node build never finishes at `stacks_view` (still
  building after 120 s, one core spinning the whole time).
- **Agreed** Navmeshes:
  - built near the player as the world streams;
  - the stalker using them;
  - one beast-sized jump in the chasm region that isn't walkable.

## Weapons, items, player

- **Agreed** Shotgun weapon-feel pass. Substantial: damage, knockback,
  impact effects, creature pain, sounds. (shotgun and item notes)
- **Agreed** Shotgun:
  - drawn with its own camera, since it clips into walls;
  - a hand or forearm in view;
  - damage per shot against a slower rate.
- **Agreed** Lightning gun.
- **Agreed** Tether, for when we return to it: camera kick, debris at the
  bite, sound, stretch under load, a hand or forearm in view.
- **Idea** Item progression: tether tiers, shotgun variants, armour and
  boots, a beacon. Not before world structure, death and the health economy
  are settled. (item notes)

## World and places

**Chasm** (chasm notes)
- **Agreed** Stage 2: proper exposures, the stepwell route in the bot,
  tuning.
- **Agreed** Room variety, deferred while creatures come first:
  - the fitted Ring (both variants);
  - the fitted Overlook;
  - lattice and light screens across all rooms (a quick win);
  - a stair tower;
  - the Shrine;
  - crossing-arch corbels (you were unsure).
- **Agreed** What interiors are for waits on encounter design.

**Canals** (`docs/canals.md`)
- **Agreed** Step 6, geometry: vertical variation and slight turns.
- **Agreed** Step 7, activation, last.
- **Agreed** Stations must not be trivial to find.
- **Parked** In the void, a canal's beam climbing a cylinder as its station.

**Other places**
- **Agreed** Arena:
  - faces turned from the fill are pitch black at `--time 420`;
  - joints catch light as thin lines;
  - a play pass on each floor.
- **Agreed** Void district: rare lightning crawling on cores. **Parked**: the
  groove as a turning bearing.
- **Agreed** Wall panels: your side-marking idea (on, off or arched per side).
- **Agreed** Streetlights: a deliberate light sequence, not random noise.
- **Agreed** Intrusions: make them rarer, and a tweak pass.
- **Parked** Ichor pool: more strangeness.
- **Parked** Reactor city and the hanging city: next steps on reviews 35, 41
  and 46.
- **You** Literature review 33: your verdicts on the four ideas.
- **Agreed** World verticality as a whole, an open question, before canal
  rises.

## Rendering, performance and tech

- **You/Agreed** Engine decision, what's open (`docs/engine-decision.md`):
  - the two shadow checks;
  - an even-handed navigation bench rerun;
  - Unity's navigation, if you sign in;
  - a switching estimate;
  - a fresh review.
- **Parked** Lamp shadows, behind combat fun:
  - a visible-lamp pass;
  - static shadow caching;
  - per-light shadow resolution;
  - narrower cones as a look test;
  - lamps for the arena's top storeys.
- **Agreed** Performance: audit the input path; the hunting bishop's extra
  cost.
- **Agreed** Shadows: the dim sun's cascades.
- **Parked** Pop-in (you, 2026-11-07: not now). Detail levels switch in one
  frame: past 450 m the smallest plates are merged into their parent and
  the stacks' pillars aren't grown at all; structures switch at 350, 400 and
  900 m. Proposed: coarse versions that keep tall silhouettes (pillars as
  boxes), plus a short dithered fade between versions.
- **Watch** The stacks by day: median frame 9.5-9.8 ms flying, right at the
  100 fps line (a third of frames over 10 ms). Terrain meshing is not the
  cause (0.7 cores at 30 m/s); it is render time.
- **Agreed** Stair slopes:
  - colliding with the player only, so shots and splats hit the visible
    steps;
  - a pushbot check up the fitted rooms' flights.
- **Agreed** Options registry: every option declared once, replacing
  `build.rs`'s name scan.
- **Watch** Rare crashes (access violations): one in a test run on
  2026-11-06, none since. Dumps go to `target\crashdumps`.

## Ideas to explore

Not scheduled; each has a memory note with more.

- **Craters** (new): a short and medium-range indicator. First, investigate
  changing geometry at runtime, so meteors can fall and leave craters during
  play, not only at world generation: re-meshing, collision, navmesh tiles,
  100 fps.
- **Portals** (new): the accidental holes in our geometry looked like
  portals into an alternate world. As a feature: a very rare secret, or a
  way into an even more warped version of the world.
- **Hollow cores** (new): some void cylinders' cores hollow, as places
  to enter; best on cylinders whose groove has few piers, where the groove
  already reads as a room.
- **Glitched architecture** (new): the chasm's and hanging cities' stairs,
  balustrades, walkways and columns, buried or fused into the outdoors as
  if glitched there: near scale cues and strangeness. Intact and crisp,
  wrongly placed, never ruins.
- Tectonic chasms: district boundaries as chasms with bridges and structures
  on the walls.
- Ground infrastructure spreading from cities into the wilderness.
- A mega backdrop: off-kilter colossal elements behind small-scale fabric.
- Scale and paving: paving as a ruler and a speed cue; structural detail on
  wilderness slabs.
- Inset borders on plates (the nodedraw accident you liked).
- Room windows onto the chasm: rare and shaped.
- The marble material (`samples/shaders/marble.wgsl`) for architecture.
- Shafts: slots and slanting fins cut into masses, after Half-Life 2.
- Assembly seams: interlocking joints as building vocabulary.
- Tilted-slab biomes.
- BLAME! industrial surfaces and Giger biomechanics, "for later
  consideration".
- Freezing mid-motion as creature behaviour (also the gladiator's first weird
  effect).
