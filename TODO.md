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

Last updated 2026-11-14.

## Current focus

**Combat, creatures and navigation, together.** Combat was the original
focus; working on it revealed how much it depends on the creatures and on
navigation, so the three are now one line of work.

- Navigation questions such as the Detour export are decided within this
  line, as combat and creatures need them.
- The gladiator's weirdness pass is creature work, but a tangent from the
  main line.

## Waiting on you

- **Parked** Harrow on the new mesh (2026-11-12). You, 2026-11-14: needs
  work overall (between good enough and not distinct enough), but better
  than before the mesh; its forelegs belong, its elbows as they are; no
  more time on it now. `spawn harrow mesh=1` (add
  `tame` to see it walk). The Planar Guillotine mesh, cut into rigid pieces
  on the harrow's bones and moved by its haul walk and haul attack,
  unchanged. Its forelegs stretched to the harrow's long ones (about 2.1
  times the mesh's) and thickened by the square root of that; its torso
  1.3 times the mesh's (`spine`, 2026-11-13: the body was short), all of it
  in the waist so the hind legs stay at the hind end, only longer; made
  symmetric by mirroring the mesh's +x half (`mirror`; its sides differed,
  posed mid-stride: -x's folded foreleg became a bar jutting ahead), its
  head (the blade, off to one side) kept whole, one; its forelegs in turn
  at every pace, the haul's heave and surge after each (twice a cycle;
  the harrow's own unchanged, both together, once; `fore=0` for together,
  to compare); its motion varied, each foot's timing (`vary_time`, 0.6 of
  a cycle, the user's) and each step's length (`vary_length`, 0.6; first 0.8); its
  joints marked in the creatures' pale (`marks = 0.15` m in harrow.pieces:
  shoulders, elbows, hips, knees, neck); a pale highlight along the blade where you drew,
  tapering to its tip, on both sides (`highlight` line: its corners, laid
  on every face under it; filled out to the faces' creases it was the
  whole of one side; not yet seen in a capture: the head turns to face
  the camera); its body level, hips as
  high as its chest (`level = 1`; slanting, its hind part looked an
  afterthought); its hind
  knees folded back as the mesh has them (the harrow's point forward and
  out, which stood them under the body; the feet and timing still the
  haul's); its hind legs, neck and blade the mesh's lengths; 2 m tall; in the
  creatures' black (the model has no material). Joints in
  `creatures/harrow.pieces` (the mesh is posed mid-stride, so they are
  estimates). To judge: the look and the walk; whether the stretched
  forelegs belong to it; whether the elbows should stand higher. Tuning
  lives in that file (`height`, `thicken`, joints).
- **Parked** Tumbler (2026-11-13; you, 2026-11-14: its dark cracks read
  well; shove, stagger and throw right; later passes to come), the X-shaped Compact Core mesh, cut into
  its two diagonals (rigid blades turning where they cross) and its core
  (a diamond body, its point a head). Two ways to move (you, 2026-11-13:
  the rest removed, kept in db34738: 1 cartwheeling, 3 both, 4 a thrown
  star, 5 a spear leaping); `spawn tumbler move=N`, 2 by default. Your
  defaults (2026-11-13): 4 m tall, rhombi (`attack=1`), two strands.
  2 striding like dividers: one diagonal planted, the other windmilling
  over the top to plant its far tip ahead (swung under, it would scrape
  and cut through the planted one), the X opening and shutting, its core
  upright (`speed`, 20 m/s, you 2026-11-13: an ingredient for move=6;
  turning as far per metre at any pace: per second, quick, it stepped to
  and fro past a goal off to its side for ever, tested to 30 m/s);
  6 as its shadow (your idea, 2026-11-13; storyboard first, which you
  approved): it turns its face to where it goes, a light (unseen) sinks
  from overhead to low behind it and its body collapses along the rays
  into its shadow, stretched up to 10 m ahead; there, the mirror, rising
  out of it. On uneven ground (you, 2026-11-13: "let's go"): each hop
  samples the ground along its way (128 heights) and finds the light whose
  shadow of its top reaches the far end; `fold.wgsl` moves each vertex
  along the light's ray to where it first meets that ground: it pours
  over ledges, climbs steps up to 1.9 m (taller: the hop stops a metre
  short); cast back at a wall too high, its light stops where the shadow
  reaches the wall's face. Seen on the broken district: a 3 m drop.
  Curved shadow (you, 2026-11-13: its ends not joined by a straight
  line, by stretching, not swimming; the swim still turned on its own
  axis, as any straight shadow following a curve must): its shadow sets
  off from its face (whichever side is nearer, no turning) and bends
  along a circle's arc to where it goes, the light's rays bending with
  it, each point laid along the arc as on a ribbon (`fold.wgsl`); it
  rises at the arc's end, facing back along it. How far: its light sinks
  no lower than `light=` degrees (13.5): its shadow at most its height over
  that angle's tangent, along the arc (10 m at `size=2.4`; twice as tall,
  twice as far; you, 2026-11-13). Far goals: 60% to all of that by
  chance, aimed a little aside; the far end no more than 75
  degrees round from its face (so the arc turns at most 150). Swim
  removed. In-game captures from above failed (the camera stays at
  standing height); Python top view at scratchpad core/curved_top.png.
  Tried and reverted (you, 2026-11-13): its body left standing where it
  was until it had risen where it went, only its shadow leaving, to
  avoid the falling-down look; it lost much of its character.
  Instead a quick collapse (you, 2026-11-13: the slow one gave the
  falling look): its body collapses in the first 30% of leaving (about
  0.5 s; 87% before), the light sinking on over the rest, its shadow
  stretching; arriving, the shadow draws in and it rises in the last
  30% (`collapse=`). `speed=` quickens all of it (times: 2 twice as
  quick; you, 2026-11-13, alongside the stride's, as ingredients).
  You prefer 1 or 2 so far. Its core's body (the rhombus under the head)
  made 1.4 times as tall, upwards, the head lifted with it (`body=`; 1 as
  modelled).
  Attack (your design), now a menace (you, 2026-11-13: never standing
  still doing nothing; it moves to fire, never twice running): it goes to
  a spot 8 to 16 m off you, swung 30 to 80 degrees round either way, from
  which it sees you and (as a shadow) one shadow reaches; there it winds
  up (its body glowing 0.45 s) and fires a burst of 1 to `shots=` (4)
  rhombuses, `gap=` (1 s) apart, fixed; then at once on to the next.
  The rhombus (you, 2026-11-13): a vortex on it (`vortex.wgsl`, from your
  tunnel picture, grayscale; inverted by default, dark streaks out of a
  white core, `vortex=1` the first), gathering speed (4 m/s, +24 m/s², to
  34), spinning slowly (1.5 rad/s), its trails the stalker's dark ray
  (`beam.rs` Ray::dark, 10 cm, darkening 0.85; they were never drawn at
  first: their points ran against the ray's facing, every face culled).
  The inverted vortex first looked plain white (you): it was at the light
  one's brightness, all but its darkest streaks past white; now a field
  just under white (over it, its bloom washed the streaks out), its
  streaks dark, thicker and reaching nearer the core (you: more
  prominent, same character). Its trails gone when it hits you,
  and never drawn within 1.5 m of your eye (left, they crossed your view
  as a dark wedge).
  Each shot its own wind-up glow (0.45 s, the last of its gap). `aside=`
  aims its shots that far to your side, to see them pass.
  Arriving where it cannot see you, it goes on (always logged, with why:
  out of range or something between). A shadow hop cut short (a step too
  high before its spot) counted as no move, and it hopped again without
  firing (you, 2026-11-13: it kept moving); now one hop is one move,
  wherever it lands, and standing unable to set off it fires after
  0.5 s (test one_move_then_there). Only from a steady
  stance. Each rhombus its body's size, spinning about its flight, a
  trail from each corner, 20 harm. `line`: its spots the
  ends of a line before you instead (to judge motion; `chase` gone, the
  hunt does it). (Cadence tried before: random waits, passes, lingers.)
  Black lightning through the ground (you, 2026-11-13: the rhombi weak;
  an experiment after its theme; `spawn tumbler move=6 attack=2`,
  `crack.rs`): a jagged black crack grows from its feet along the ground
  toward you, all one width (you, 2026-11-13: growing, not travelling;
  no head and tail), its tip 6 m/s, +30 m/s², to 18, bending after you at
  0.5 rad/s (`seek=`); 1.6 s after it set off all of it is gone at once
  (`lasts=`: in effect its range, about 26 m). Its tip crossing under
  your feet (1.2 m, and you no more than 0.7 m over it) hurts, 20, stops
  it, and flings forks round you.
  Tested: standing or walking across its line it strikes you; running
  across it, or over it, you get away. Climbs steps up to 1.9 m, stops at
  a wall or the void. Drawn multiplying the ground (black middle, a
  crisp soft edge: wider and softer, it read as a shadow's puddle), its
  zigzag and forks laid as it grows and kept (you, 2026-11-13: what it
  has grown must not change; flickering all along, it vibrated; a test
  holds it). Three thin strands (you, 2026-11-13: several thin, not one
  wide; `strands=`, 1 the wide one), 14 cm across, spread over 0.8 m,
  sharing the line's zigzag and each zigzagging its own way about its
  place, so they cross; all from one point under its feet. Kept to
  revisit (you, 2026-11-14: the rhombi the default for now).
  Far off as its shadow (you: it bobbed, rising between hops): its shadow
  goes on leg after leg, drawn into its end and stretched on, rising once
  where it fires; a far spot it sees you from is one move however many
  legs. It works (you, 2026-11-14).
  Shot (you, 2026-11-13: physical by default; `fall.rs`): its blades
  (stepped capsules tapering as the model does) and core are hit standing,
  never as its shadow; 50 health (`health=`). A shard rocks it back on its
  feet by its torque about them (face on), shoves it back sliding on its
  feet (0.6 m/s a shard, `shove=` times) and knocks its blade about the
  hinge (side on), springing back; it staggers 0.04 s a shard (0.4 at
  most). Killed (`death=`): 1 its hinge limp, it falls as one, blades
  swinging free, splaying open; 2 it comes apart, blades and core each
  falling (default: reads better, you); 3 it bursts as the others do.
  No burst on overkill, unlike the stalker (you, 2026-11-14: point blank
  hurls it apart instead). The fall: points per piece
  held rigid by shape matching, Verlet, Coulomb friction, sleeping when
  still; moving on as it was, thrown by the killing blast as one body
  (along the shots, spun about where they struck; the struck piece flung
  harder; a little up): you saw it break down in place, 2026-11-13; then
  too little point blank (the shards after the killing one passed through
  it): now pieces land 20 to 25 m back point blank.
  Ichor where it parts and where struck, and 40 splinters and flakes of it
  (7 big blade pieces, up to 1.8 m) thrown with its pieces, lying a few
  seconds (you: a few plain pieces read as a toy breaking; splinters, not
  cut meshes, for their cost). A hit recoils its core back from the shot
  (a flinch; turning it whole spun it like a toy, removed);
  the rock and knock tilt one way and ease back, never swinging past (you:
  rocking back and forth did not work).
  Lessons (tests): a lift out of the ground must not become speed (it rose
  into the air); a stiff friction held it up like a stepladder; its core
  free on the hinge swung for ever; its hinge points stuck out as feet.
  `--opt aim` (captures): `fight`'s shots at the nearest tumbler.
  To judge: the throw and the shove (`shove=`); the stagger.
  Damage shown (your idea, 2026-11-14; `scars.rs`, `scars.wgsl`): struck
  cracks, as in glass (jagged rays from where each shard struck, branching
  further out; cell edges tried first read as a mesh of rings), in each
  part's own units so they stay put; a shard within a crack's reach makes
  it reach further, else a new one (16 a part). The light inside it shows
  through its blades' cracks, faint at rest, bright winding up and when
  struck; on its glowing body they split the glow, dark (you had not seen
  them: shards at its body counted as striking its blades, whose hit
  shapes wrap round it about the crossing, so its body was never
  cracked; now its core is struck before its blades where both are; and
  its cracks' faint light at rest, left on as it glowed, was white on
  white). Struck, the light inside flares a moment (the flinch a rigid X
  cannot make). Dead, all dark. You: the light cracks read clearly, good
  as they are (their shape could be tuned). To judge: the dark ones.
  Not yet: navigation, sounds; breaking geometry (your idea).
- **Next** Prove district navigation (you, 2026-11-14: the item with most
  uncertainty): teleporting walkers (`--opt blink`) need testing across the
  districts to see whether they hold up. See Navigation for creatures.
- **You** Sounds you are finding:
  - stalker call and death;
  - bishop arrival;
  - cable-creak warning (swarm);
  - void low hums.
- **You** Chasm: you had a more ambitious answer to "the chasm has no floor",
  held back; and another decoration idea. (chasm notes)
- **You** The catalogue (https://ruescasd.github.io/null-catalogue/, repo
  `../null-catalogue`): your shots. F12 in the game, copy the chosen `.png`
  and `.txt` pairs into its `shots/`, `python build.py --publish`. The build
  lists the unused `shot_` places in places.txt, to remove if you want.

## Creatures and combat

The critical path is enemies (enemy-pipeline notes).

**Gladiator** (`docs/creatures.md`)
- **Parked** An animation pass over every creature, later (you, 2026-11-07).
  The gladiator's human swagger: its arm and trunk swings and its hips' sway
  now reversed by default (your pick, "better than what we had, but still
  work to do"); quieting them, a stoop and arms held in were tried and
  didn't work; the legs, the most human motion left, untouched. Judge with
  `spawn gladiator still`. Its drops and hops too (you, 2026-11-14: its hop
  "not so much"): clip creatures have no airborne pose, it rises upright
  in its walk, as if lifted (a hop and a fall clip, or a pose made for it).
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
- **Next** Your verdicts (2026-11-14), deciding the open questions below:
  - it never lands: flight is all it does, and it falls only when killed
    (nothing gives it a reason to land). Goes, listed and asked first: the
    crash landing and its ring, the slide, choosing a landing spot, staying
    down while you are in range. Its flights over obstacles stay;
  - damage shown in its flight: a flinch, its thrust cutting out more often
    the more it is hurt, flying lower;
  - body 4 (pale crests) the default, the other skins and `bodies` go;
  - its impaling shards go, the dimples where the spikes were cut domed;
  - a cable holding you only hurts (`latch=1` the default);
  - its cost: stop here; the shimmer as it is;
  - overall: needs work (the landing question above).
- Step 2 built: `spawn impaled` flies after you (14 m off,
  facing you, feet 1.5 m up); `spawn impaled course` flies a set course to
  judge it; its weight's numbers are options (`lift=`, `spool=`, `tilt=`,
  `climb=`, `speed=`, `hover=`). Your feel test. It flies through
  structures (step 4) and takes no shots yet.
- Step 3 built, the field (b), low and labouring (your picks):
  two beams from its soles to the ground, white-hot fading to black (your
  gradient picture), born 0.35 m below its soles, bent all along, in hard
  straight-edged shards (the other four fields removed, your say
  2026-11-10). They follow its thrust; a placeholder hum (pitch and
  loudness its thrust's).
- Feet: soles made four-pointed stars (an X, prongs splayed),
  flat-shaded, in `tools/impaled_mesh.py` (in place of the paw's toes).
- Its plume: `plume=1` curves, `plume=2` shards (default).
- Taking hits built: shards stick in it a moment, push and rock it,
  each hit cuts its thrust 0.12 s (it dips); health 80 (about six blasts at
  7 m; `health=`); dead it falls, lands hard (placeholder: the stalker's
  impact) and topples away from the killing shot, and lies. Open: under
  steady fire it is beaten down to the ground and drags (it can't climb
  back between blasts); keep that, or let it recover?
- Step 4, stage 1 built (your call 2026-11-10: it lands to attack,
  flight its only way to move): it chooses a spot 5-14 m from you (flat,
  in your sight, apart from others), flies there straight or up over what
  is in the way (ways out first when it is shut in, as in the arena's
  pens), crashes down (your call 2026-11-11: no climbing to 4 m and
  stopping first; coming in on the move it lets go short of its spot,
  shoved down, ~7 m/s, and slides in on its feet, grinding to a stop on
  it; where the ground there isn't flat and clear, it brakes, hangs over
  the spot and drops as before; falling level; impact, your view jolted
  near it, its body squashed
  down onto its feet ~14% and back in about 0.3 s), and stays down while
  you are within its grasp's range and in its sight (see its attacks);
  beaten down on its way, it stays landed. Checked: floor, stacks, spire,
  city and an arena round, every way clear (a warning if it is ever
  inside something). `keep=N` keeps the old hovering after you.
- Its attacks: the crash (sliding in, its letting go the tell;
  where it can't slide, it hangs 0.5 s over its spot first; sliding, it
  digs in at 16 m/s², about 2.5 m from 9 m/s; a ring sweeps
  out where it hits; within 6 m on the ground you are hurt, up to 25, and
  thrown; a jump spares you) and its grasp (your call 2026-11-10, in place
  of the lash: too slow to reposition, it needs reach; 2026-11-11:
  relentless; 2026-11-11 again: trivially escaped; harpoons tried,
  `cables=2`, no personality; so its own slow cables and an agile flight,
  and flung flying too, reaching far): flying or landed, each arm on its
  own, again and again (0.6-1.4 s apart, a 0.5 s tell of its shoulder
  drawn back), up to six cables out per arm, homing on you slowly (7 m/s
  of their own, you run 10; range 60 m), a cable flung on the move
  carrying its body's speed (losing it slowly, 0.1 a second), the cable
  behind dragged along by its body, the nearer the more (over ~10 m);
  their way sagging behind them, fed from its waist band; touching you one
  takes hold (8 each, then 6 a second) and slows you (to 4 m/s; your
  call 2026-11-12: for now it only slows and hurts, no reeling in;
  `latch=4` still reels, 4 m/s to 5 m from it); an arm holding you
  flings no more. Missed or let go, a cable
  is dropped: it falls, lies 6 s, thins away. Landed it stays down while
  you are within 21 m and in sight (2% a second after 15 s it leaves
  anyway); else it rises 3 m and comes after you, far off (over 26 m)
  giving chase in the open, as fast as it can: its first slow flight
  again (your call 2026-11-11, after trying it far more agile: twice its
  weight in thrust, 54° tilt, 12 m/s², 24 m/s, catching a runner from 40 m
  in under 5 s; a middle between to be found; its tilt and facing are
  now steered apart, which the agile one needed), choosing a spot
  near where you will be (1.5 s on) once it can land short of you,
  falling and sliding. Seen in a run with the agile flight (a runner from
  20 m behind, 20 s): held 9 times, the first in flight 5 s in, reeled
  back, then the crash; free after each 4 s hold, caught again. Slow, on
  a still player: 8 cables out on its way in, all taking hold within a
  moment (8 each: a burst of ~60). Open (yours): your feel test
  (flight, cables in flight; how the dragged cables look I could not
  capture); escape (hurt it by 20 and it lets go; or 4 s); `latch=1..3`
  to compare with the reel; weight for its slide (dust, a
  scrape). It lands 2-6 m from you (2.5 best), but coming in fast it
  slides at 13-15 m/s, its crash struck 8+ m from you (out of reach). Its arms and band are cables, not mesh
  (tools/impaled_mesh.py, creatures/impaled.cables); its cables could
  borrow the buds' leeching-cable look (combat/leech.rs), your note.
- Its cables' heads, and its body's skin (2026-11-12: the heads
  lost among the parts lagging behind; its black body hides its cables as
  its anatomy). Built: each flung cable's head a small pale barbed shard
  (its shards' material) clamped in the loop's bend, pointing the way it
  goes, lying with a dropped cable (dripping ichor was tried and
  removed, your call: it added nothing). Cables stay black (your
  call: grey and pale collars tried, removed). Body skins to compare,
  `body=1..5` (`spawn impaled 5 bodies`: one of each): 1 black (default
  for now), 2 grey, 3 pale ash, 4 pale crests along its fused cabling, 5
  pale bands, mirrored. Seen: 2 and 3 make the black band and arm cables
  read as its anatomy, but show Meshy's sculpt plainly (muscled legs) and
  3 swallows the pale shards; 4 mottled, a worn look; 5 zebra-like,
  competing with the band's coils. `shards=0` hides the shards it is
  impaled with (their own mesh; no remesh): black then nearly vanishes
  against the sky; grey a smooth hunched figure with small dark dimples
  where the spikes were cut and capped (to dome in tools/impaled_mesh.py
  if they go for good). Open (yours): a verdict on body and shards; then
  the losers and `bodies` go.
- Its cost (2026-11-12, your call: 1.25 ms of a 6.9 ms frame for
  one attacking brute is really bad). Traced: almost all of it is meshes
  and materials changed every frame, which Bevy then re-uploads and
  re-prepares (shadows included). Agreed order: 1 shaders animate the
  beams and the shimmer instead of edits each frame; 2 the body bent on
  the GPU by a skeleton; 3 cables made into tubes on the GPU from their
  points; 4 trim the grasp's logic. Step 1 done: beams and shimmer are
  meshes made once, what changes carried per object (`MeshTag`); the
  shimmer is its own material bending the screen behind it, fading to
  nothing at its outline and ends (your borders). Measured: no frame-time
  change (hovering +0.2-0.3 ms is drawing it: GPU; attacking +1.0-1.3 ms
  as before); the cost is in steps 2 and 3. Step 2 done: the body and
  its shards skinned to three joints (middle, left and right shoulder,
  weighted as the old bend), only the shoulder joints moved; checked
  exaggerated 5x. Idle machine: attacking +0.62-0.74 ms (was +1.0-1.3).
  Step 3 done: its cables (arms, and those dropped) are tubes laid on the
  GPU from a buffer of points, the mesh made once; only the points are
  written, the arms while they move, the dropped ones only while one falls
  or thins (they were rebuilt every frame); the heads are entities moved,
  not a mesh rebuilt. Ends are now closed. Attacking now +0.55-0.65 ms
  over an empty scene; of that about 0.2 is the brute being there
  (hovering), about 0.35 the attack: GPU drawing the cables close up
  (+0.15 ms main pass), the cables' points (`arms::draw` 0.09), the grasp
  (`impaled::fly` 0.07), and the renderer re-specializing things entering
  shadow views (~0.1-0.15: heads, its landing; the drips, now removed,
  were part of it). Next (yours to
  pick): step 4, the grasp and the cables' points (e.g. cables at rest
  skinned to the shoulders like the body, only flung ones written); or
  stop here. Open (yours): the shimmer's
  look (`BEND` in haze.wgsl, 0.12 m; stronger inside than before, where
  the old one showed mostly at its edges).
- **Agreed** Stage 3, dense places (a 3D search), only if wanted.
- Open: a choice costs 0.2-3 ms of sweeps (one a frame across all brutes).

**Stalker** (stalker notes)
- **Next** The hover approved (you, 2026-11-14): remove the
  gladiator-walk-on-stalker code (`hover=0`), listing what goes and asking
  first. Its air stride's pace to tune: `air` (now 0, 1 or 2 only) opened to
  any number.
- **Next** The searchlight stays pale (you, 2026-11-14): the black option goes.

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

**All creatures**
- **Agreed** Damage indicators for the others (you, 2026-11-14: they apply
  in general): the tumbler's techniques (marks fixed to a part, struck
  cracks accumulating where hit, light through them, a flare as the
  flinch; damage-ideas notes) wherever they suit a creature; and motion
  impaired by damage, e.g. a flyer's propulsion cutting out now and then,
  or flying lower. Breaking geometry (your idea) still open.
- Many passes on creatures to come (you, 2026-11-14): the tumbler too, later.

**Others**
- **Agreed** Hounds and harrows: real blocking. They only push one way now.
- **Idea** The cable mover: throws cables to the ground and hauls itself.
- **Idea** Wall crawlers for the depths.
- **Parked** Creatures pushed into a canal.

**Navigation for creatures** (`docs/outdoor-navigation.md`, navmesh notes)
- **Next** Prove district navigation (you, 2026-11-14: the most uncertain
  item): navigable nodes plus teleporting, tested across the districts.
  Correctness is shown (nodes exact; 300 of 300 chases in tiers, broken,
  stacks); unproven: how it plays in each district, and its runtime cost
  (node building, plans in stacks).
  Step 1 done (2026-11-14, all six districts; every node checked by
  `node_reach`, not sampled): walking within nodes and teleporting itself
  hold everywhere. Found: floor's pit (16 m deep) unreachable at a 10 m
  teleport (50 of 300 chases; fine at 20 m); a raised cluster in broken
  (~14 000 m², 50-62 m up, round 7070, 930) unreachable even at 20 m;
  highlands' long strip nodes make walks of 600 m where a teleport across
  would do (the planner only walks within a node); plans over 2 ms (stacks
  99th 5.4 ms, worst 8.5; broken worst 8.2; tiers worst 2.4). Your rules
  (2026-11-14, from the pit and the plateau): no terrain where a player
  without the hook can get stuck (a generation constraint; the pit breaks
  it); a few places walkers can't reach are fine (flyers cover them); all
  enemies able to drop down anywhere, one move for all (proposed). A walker
  with no plan must never just stand: gather below, fire, or give up the
  chase (which can mean leaving the area). Plan agreed: a player-reach
  check, the traps it finds, drop links in the planner, then a hop
  (~1.4 m, the player's) and the generator fixes. Known from the code: nodes
  ignore structures standing on district ground (walkers pass through
  them), and are cut for the biped only.
  Step 2 done: the `stacks_view` hang was a node of 8125 fragments (an
  outline of 134 615 points) through geo's triangulation, quadratic twice
  over; now spade directly with geo's snapping (`triangles`), every node's
  triangles checked to cover its outline; stacks builds in 4.2 s (12.7).
  Player traps found (2026-11-14, `player_traps`: the player's own walking
  space, linked one way by a running jump and a mantle, about 2.45 m up,
  `player::hookless_reach`, or a drop): floor's pit (13.4 m short of a way
  out), a 2 by 5 m slot in floor (`floor_slot`, 1.5 m short), a basin in
  stacks among its towers (`stacks_basin`, 3.5 m short); tiers, broken,
  uplands, highlands none. A report for now; it asserts none once the
  generator rules are in.
  Drops built (2026-11-14): one way links off every lip onto a lower node
  within 3 m across, any depth, the landing checked as the rig falls (its
  gravity, its ground, frames of 1/30 to 1/240 s); in the planner beside
  teleports; the walker leaps with the rig's leap (its airborne pose and
  landing crouch), its hop found from where it stands. All six districts:
  every drop landed where planned (deepest 23 m, stacks); 300 of 300 in
  tiers, broken, stacks, uplands; highlands 298 (its long strip walks);
  floor 276 (only walkers starting in the pit: the trap). Building drops
  0.7 to 1.7 s more. Seen in play: a gladiator on floor's rim drops into
  the pit after you. Walkers now reach every node but broken's plateau.
  Your decisions (2026-11-14): the hop only as one case first, to judge
  how hard its animation is (every walker would need it): a spawn option,
  off by default, the rig's leap as it is. Traps fixed by ways out: stairs,
  added freely (a scale cue too). Then step 3, nodes built as the world
  streams.
  Hop built as that one case (2026-11-14, `spawn KIND hop=2.45`, off by
  default): up onto ledges as high as you climb without the hook, taking
  off up to 2 m back (from the ledge's foot the rig's leap, going across
  at one speed, met the edge still rising), the gentlest leap that clears
  by 0.4 m. Walkers with it: 300 of 300 in stacks and broken, hops up to
  2.2 m. Seen in play (broken, `hop_spots` lists them): the harrow (its
  legs procedural) leaps up with its pounce pose; the gladiator (clips)
  rises upright in its walk, as if lifted: clip creatures have no leap
  pose. Hops cost the build ~1 s more in stacks. **You:** judge the look.
  Found on the way: a walker spawned before its nodes are built goes as
  in the open, up ledges (`--do` now waits for the nodes); in play, for
  step 3. The gladiator's dash set its pace in the air (now not in the air).
  Teleporting is a creature's own, case by case (you, 2026-11-14: applied
  to all it won't look good): only authored bodies, by their shade, have
  it (10 m); the rest range 0 (`blink_range` gives it, gliding as itself,
  no look). The white glows (the tell for creatures without a shade) are
  removed: they did not work.
  Without teleports (2026-11-14, `BLINK_RANGE=0`), chases reached, no hop /
  hop 2.45: floor 276/276 (the pit), tiers 167/300, stacks 56/218, broken
  156/299, uplands 274/300, highlands 236/298. The hop decides whether
  walkers without teleports reach most districts. Stacks with hops: plans
  up to 46-49 ms (many leap portals); off the frame since 2026-11-15, so
  now how quickly walkers react (standing up to ~5 frames a replan) and
  the other cores' load (shared with terrain and node building): a faster
  planner (fewer portals, flat arrays) before walkers without teleports
  hunt in stacks, after the stand-still fallback and nodes built as the
  world streams. Fixed: a walker with no way re-planned every
  frame (now waits 1 s), and a failed plan searched every portal (up to
  41 ms; now the nodes are searched first: 2 ms at most in tiers).
  The hop looked floaty to you: `--set leap_gravity=N` (25 unless given)
  to try others in play; drops and hops planned by it. Stances and dashes
  no longer steer a creature in the air on a drop or hop (the harrow's
  haul cut a hop short).
  Your verdicts (2026-11-14): leap gravity 60 (now the default; attack
  leaps and drop kicks keep their time in the air, scaled from 25); the
  harrow's hop looks good; the gladiator's not so much, part of the
  gladiator's animation work (below). The harrow's mesh is its default
  (`mesh=1`; `mesh=0` its shards).
  Pits' stairs (2026-11-14, `worldgen/src/plates.rs`, `Stair`): every pit
  has a flight cut along one edge, inside it, 4 m wide, steps of at most
  0.5 m up and 1 m on, from its floor to plain ground beyond (not a pit,
  mesa, pillar, canal or site's ground), climbing half its depth at least,
  a landing at its foot; its steps are plates, so walls and navigation
  have them as any. Floor's pit and stacks' basin (a pit too) are traps no
  more (`floor_pit_stair`); walkers with teleports 300 of 300 in floor now.
  Hidden slopes over their steps (2026-11-15, collider only,
  `ColumnMesh::walk`): walked smoothly (test `stairs_walked_smoothly`:
  0.033 m a frame walking, 0.5 without); shots hit the slope till the
  slopes are the player's alone (deferred, memory/stair-slopes).
  Floor's slot was no trap: the checks took the edge of the square for a
  wall (they now judge the edge by the big plates built). Fills of
  pockets tried and removed (you, 2026-11-15): their simple picture of
  movement (pillars solid, no body width, no jumps) told no pocket
  reliably. Stacks' last pocket was a well (a plate 6.5 m below its
  sibling, in a crack): wells raised (2026-11-15, `WELL_REACH` in
  plates.rs): a plate more than 2.45 m below every plate a body can pass
  to (a metre of border or more), by walkable tops, raised to 0.5 below
  the lowest; worked out once per parent and kept, siblings screened
  first (0.4 to 0.5 ms a parent; `wells_cost`). Six raised in stacks'
  square, none elsewhere. `player_traps` now asserts none: none in all
  six districts.
  Walkers' thinking off the frame (you, 2026-11-15: 9-14 ms a creature, a
  game breaker): plans, ways across a node and going-from-here searches
  are made on other cores (`Blink::plan_in_background`), the walker
  standing a frame or two, or going on along the way it has; the leap's
  check in play its root's landing alone, fewer hops tried. Steering in
  the frame now at most 0.2 to 0.45 ms a walker (was 7 to 14). Tried and
  dropped: one search from your node shared by all walkers: the same ways,
  but 10 to 50 times slower (no heading: it searched all round, 387 000
  portals in stacks).
  `keep N KIND...` at the console (2026-11-15, combat/hunter/keep.rs):
  creatures kept round you, dropped from the sky onto the walking space;
  left far behind, or gone after leaving, let go. Stranded walkers (no way
  to you) keep watch, wandering near the ground they reach nearest you,
  looking every 2 s for a way; hurt, they leave (~110 m off) and are gone
  (`district::Stranded`; stacks without teleports: 236 of 236 stranded
  keep watch). Your frame drops on moving away were the node build: its
  threads (half the cores) held the game's up for whole time slices
  (frames of 100 ms to 1.3 s; not the allocator, mimalloc tried, nor the
  tables' lock); now set below the game's priority: none past start-up,
  stacks' nodes in ~11 s. Slow frames logged ("hitch"): over 20 ms
  (you: 100 fps the floor, not the goal), `--set hitch=MS`, the first 10 s
  of play in one line. Your
  harrow in the air: creatures stood on a pillar's top over its whole
  plate, beside its narrow grown form; they stand now on the ground walked
  on (`walk_height_at`: the pillar's base).
  Nodes under 10 m² left out (2026-11-15, `Space::prune`): in the stacks
  a third of its nodes were slivers at its pillars' feet (1 148 of its 1 447
  small ones, under 1% of its ground; pillars' tops were never in), each
  with portals to every node 20 m round. Stacks: 3 738 nodes to 2 291,
  portals 398 000 to 250 000, links 2.9 M to 1.6 M; built in 5.7 s (7.1)
  headless, 7.8 s in play at the start (11.2); plans median 0.9 ms (1.2),
  99th 4.4 (5.3). Every chase reached as before. Next for the build: step
  3 (in tiles, as the world streams).
  The build against its essential cost (you, 2026-11-15: no complacency;
  memory/essential-cost), stacks' square, one thread, CPU: 7.7 s (more:
  see below) to 2.4 s (tiers 1.5, broken 0.6). Its stages "on one thread" had run on eight
  (a thread for every chunk, eight for each build thread, all at once: the
  game's build used every core, not half): now `nodes::par_chunks`, the
  build's threads exactly; and its ties broken by place, the same nodes
  whatever the cores. Teleports' hops and portals made only when a
  teleporting walker first plans (a third of the build, for the gladiator
  alone). Drops: falls exact by their curve, no stepping (5.0 s to 0.2),
  landing spots by where nine paths cross the node (0.9 s to 0.17), links'
  best per point before kept. Portals: a node's parts joined by Kruskal
  over its portals' Delaunay edges, and a node's portals weighed all at
  once where few (the nearest search ran its 64 rings to the end for every
  portal of a node with fewer than 7): 748 ms to 20; plans in play too,
  stacks without teleports median 465 to 43 µs, 99th 22 to 0.9 ms.
  Walking space: a convex piece cut by a straight line along each own edge
  closed all along where that is exact (a square corner or less at each
  end, or the next edge closed and long enough), the rest one boolean of
  what reaches into it: 1.3 s to 0.68. Hashing: Bevy's (foldhash), not
  std's SipHash. Triangulation's edge splitting by 1 m cells. Left, by gap
  (stacks; now, against the essential estimate): walking 680 ms (25-60),
  drops 520 (50-250), outlines 360 (100-150: geo's 2 mm buffer and union
  per fragment, then the triangulation), pieces 400 (50-100, not looked
  at), join 120 (a few), obstacles 94, edges 51. In play (12 cores, at
  the start): stacks' nodes in 1.1 s (7.8), no slow frame past the first
  10 s; pieces (460 ms) and join (140) run on one thread, now most of it.
  Then (same day): pieces 400 to 210 ms, two thirds of it the wells rule
  (worldgen): whether a site splits now kept per call (`Cache::splits`;
  asked again for every sample round a plate, its slope four landform
  samples), a parent's plates kept from its first pass. Join 120 to 31:
  each fragment's spans along an open stretch, pairs meeting 1 cm or more
  (`MEET`) one node (sampled every 10 cm before, a shorter meeting joined
  or not by where the points fell). Triangulation's corners snapped within
  1 mm (`TRIANGLE_SNAP`, 0.1 before): four nodes in the broken had a
  navmesh in two parts (corners half a millimetre apart either side of a
  narrow neck); `node_reach` now asserts none in all six districts, and
  judges triangles against outlines by the snapping's own bound. Stacks,
  one thread: 2.0 s (tiers 1.2, broken 0.5); in play 0.7 s, pieces (235
  ms, one thread) a third of it. Left (stacks, against essential):
  walking 670 (25-60), drops 520 (50-250), outlines 370 (100-150),
  pieces 210 (50-100; wells 86 of it), obstacles 89.
  Then: walking 670 to 510 ms (a pillar's hulls within another left out,
  `outermost`: 32 000 to 23 000 in the stacks; convex hulls tested against
  a piece by their edges, not geo's `intersects`). Pieces made in strips on
  the build's threads (the same pieces, whatever the threads). Tried and
  removed: half capsules and a neighbour's stretch along an edge dropped
  (no gain: geo's boolean costs by the shape, not the corner; in the tiers
  three capsules in four are neighbours' walls ending at a corner);
  triangulating a node's fragments together without geo's 2 mm growing and
  union (100 ms less, but nodes in two: each side of a stretch cuts its
  own walking space, their arcs ending mm apart, which the union glued).
  Both left gaps need one change: the walking space cut the same on both
  sides of a stretch (and a difference of our own for a convex piece and
  the capsules at its corners), not more trimming. Stacks, one thread:
  1.9 s; in play 0.7 s (pieces 137 ms of it, sharing the start with the
  terrain's loading).
  Then (you, 2026-11-15: the target is the techniques' essential cost, not
  a seed made faster): `seed_sweep` (ignored test), 8 seeds × the six
  districts, a square of each found on each seed's map, built on one
  thread, each stage's time against a written model of its least work
  (`MODEL`) priced by primitives timed in the same run, and checked. 65 s.
  Gaps stable across seeds (within a district, least to most mostly under
  1.5×) and alike across districts: the work was not fitted to a seed.
  Per stage, the big squares (stacks, broken, tiers), ours/essential:
  drops 318/5 ms (61×), walking 223/3.5 (64×), pieces 151/18 (8×),
  outlines 206/97 (2.1×: my earlier "outlines need the cut alike on both
  sides" is mostly wrong, the triangulation itself is most of it), edges
  32/1.4 (22×: a hash set made for every edge, every edge in its 8 m cells
  tried), join 20/1 (17×), navmeshes 23/2.4, obstacles 32/19, portals
  23/14. Whole squares 5-8× (floor 14×, small). Checks: no traps in all
  48, no navmesh in parts; seed 4's stacks had two outline specks 0.7 mm
  across left untriangulated, held to the coverage's own bound now.
  Then, by the sweep: walking 223 to 80 ms (big squares, one thread):
  geo's boolean (20 to 40 µs a piece: not its wrapper, not a fixed cost,
  the cutters' corners and overlaps) replaced by `nodes::convex_less`, a
  convex piece less convex cutters by its boundary (each edge kept where
  nothing covers it, linked into rings; geo's as the fallback, counted, none
  in 48 squares); exact where geo's put corners on a ~0.4 mm grid (some of
  the mm mismatches either side of a stretch were that), checked against
  geo's on every piece (`walking_difference`). Capsules from a table, plain
  rings. Drops 324 to 172 ms: what lies about each lip found once for its
  rays, spots and falls (`Space::about`), blocks' rings made once; and a
  bug: `along` grew a sliver's hull by moving its edges out, its sharp
  corner reaching a fall's path from 2.7 m off and failing good drops (37
  in 340 000), now kept within its bounds grown as much. Edges: a set per
  edge removed, no gain (28 candidates an edge from 8 m cells is the cost:
  a hash by line would do). Whole big squares 1035 to 752 ms; in play
  stacks' nodes in 0.5 s (0.7). Left: outlines 211 (100, 2.1×), drops 172
  (6: lips 65-110 ms, spots), pieces 156 (18), walking 80 (4), edges 33
  (1.5), join 20 (1).
  Then (all the same results, counts identical over the 48 squares): the
  lips' edge points in a grid counted out, by node, squared distances
  (about half the time); a lip's rays through its node's fragments found
  once, buffers kept (rays 119 to 73 ms in the broken); pieces 151 to 109:
  every piece was cut against all the world's canals and stations (40 ms
  in the broken), now by an index of those within reach (`cut_index_agrees`),
  sites further than 1.5 squares from a plate passed over
  (`far_cells_skipped_exactly`), a plate's sites found once and clipped
  into kept buffers, pillars grown on the build's threads. Big squares
  659 ms (752). In play, quiet machine, four runs each against the last
  commit: start 1 to 2 slow frames (worst 50 to 53 ms) both, no later
  hitches, nodes 0.4 s (0.5). (Hitches seen first were the machine's: busy,
  both versions showed them alike.) Next: pillars grown once for the
  mesher and the nodes both (each grows them now), outlines, edges by a
  hash of their lines.
- **Agreed** Structures: move from the patched Rust crates to C++ Detour
  (through bindings, staying on Bevy); the crates fail the worst-case frame
  cost (17 ms a long search, 122 ms a burst), Detour passes (0.6 ms).
- **Agreed** Walkers: a knee-high step-up (to 0.8 m), animated, timeboxed:
  stop if it drags (you, 2026-11-14: animation has led to goose chases).
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

- **Agreed** Engine decision, what's open (`docs/engine-decision.md`):
  - spike B, streaming dense places and far detail (you, 2026-11-14: run
    it); Unity left out (no sign-in);
  - the two shadow checks;
  - an even-handed navigation bench rerun;
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
