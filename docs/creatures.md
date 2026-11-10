# Creatures with authored bodies

The enemy pipeline under test since 2026-10-30. A generated mesh is rigged,
given animation clips from a library, and dropped into `creatures/`. The game
moves it with the same hunting mind, navigation and shots as the other
creatures, and plays its clips. The test's question: can we make convincing
"meat" enemies this way, quickly and repeatably?

## What the game needs

One binary glTF per creature, `creatures/NAME.glb`, holding:

- **A mesh rigged to a humanoid skeleton.** Mixamo, Meshy, Quaternius and
  most auto-riggers name joints in ways the tool recognises (`Hips`, `Neck`,
  `Head`, `LeftUpLeg` or `UpperLeg.L`, and so on).
- **Clips.** `walk` is required; `idle`, `attack`, `hit` and `death` are
  optional, and any left out are simply not played. (A `run` is no longer
  played: authored bodies walk or dash, see below.)
- **Walk and run in place.** The game moves the body; a clip that moves it
  too is refused by the tool.

## Making one

1. **Generate the mesh** in a neutral pose (T or A pose), with humanoid
   proportions. Stretched or extra limbs may defeat auto-riggers.
2. **Rig it and pick clips.** Two routes:
   - **Meshy:** pose the mesh A-Pose when generating; its rig step adds a
     walk and a run, and its Animate step offers more. Download each clip
     as GLB (Mixamo skeleton, the default), then merge them into one:

     ```
     python tools/merge_clips.py creatures/NAME.glb WALK.glb RUN.glb ...
     ```

     Texture the mesh before rigging it: each clip's GLB then carries the
     whole textured, rigged body, and nothing else is needed. To add a clip
     later, merge it into the creature's own GLB (`merge_clips.py
     creatures/NAME.glb creatures/NAME.glb NEW.glb`). Tried 2026-10-30 (the
     gladiator): it works, but its library's walks are all human.
   - **Mixamo** (free, royalty-free in games): upload the mesh, auto-rig,
     then download each clip as FBX, ticking **In Place** for walk and run.
     Download the first one with skin and the rest without. In Blender,
     import them onto one armature, rename each action (`walk`, `run`,
     `idle`, `attack`, `hit`, `death`: Mixamo names them all
     `mixamo.com`), and export as glTF Binary with animations.
3. **Measure it:**

   ```
   python tools/creature_clips.py creatures/NAME.glb --height 2.6
   ```

   This writes `creatures/NAME.creature`. It finds the joints and clips by
   name; if it can't, it says which and lists what the model has, and
   `--walk NAME`, `--run NAME` and so on name the clips. It measures:
   - how tall the model is (the game scales it to `--height`, in metres);
   - which way it faces;
   - each walking clip's ground speed, from its feet;
   - when the attack lands (its hands fastest);
   - **how much the clip's own feet slide** while down. The game can't
     mend that, so prefer clips where it is low.
4. **Run the game.** Each creature is spawned by its file's name
   (`spawn NAME`); the log at start lists them. A name can't be one the
   console already spawns (`harrow`, `bishop`, ...). The robot is a CC0
   test model.

You may edit `NAME.creature` by hand: `height`, the clips' names, the joints,
the hit radii, `shards` and `weapon`; the tool keeps them when run again. Run the tool again after changing the GLB.

## Judging it

| Console command | What for |
|---|---|
| `spawn NAME` | it hunts you |
| `spawn NAME tame tame_line tame_pace=2` | walks back and forth across your view, to judge the walk side-on |
| `spawn NAME still` | in one place, side-on to you (`still_turn=0` facing you), its head not turned to you: walks in place 6 s, stands 2.5 s, attacks, stands 1.5 s, over and over |
| `spawn NAME health=20` | dies quickly, to see the death |
| `round NAME 3` | three through the arena's gates, timed |
| `spawn NAME --help` | the options it takes |
| `show FILE.glb height=2.6 spin` | any model in `creatures/` stood before you as it is, unrigged, turning slowly (`turn=90` side-on, `ahead=M`; `show off`) |
| `spawn NAME echo echo_lag=0.07 echo_off=0.085` | a second body beside it, a moment behind (see below) |

Add `rigtrace` to log its feet every frame to `target/rigtrace-*.log` for
measuring sliding.

**The test's criteria (fixed 2026-10-30):**

- no visible foot sliding at chase speed;
- no pops between clips;
- it reads as a threat when charging head-on;
- a second, different biped takes less than a day of your time;
- ten of them on screen keep 100 fps.

## How it moves (for changing it)

`crates/game/src/combat/hunter/clip.rs`. An unseen biped hunter carries the
mind, navigation and collisions. The model stands on it:

- **It walks or dashes; it never runs** (the user, 2026-11-04: running at
  you it was slower than you walking backwards). Its walk is played at its
  measured ground speed, so a foot down stays down (as far as the clip's own
  foot does). Hunting you and more than its reach and 3 m off, it walks on
  to its next step and, as a foot leaves the ground (`TOE_OFF` in its
  stride, measured on the gladiator within 0.04 s), dashes along its way,
  its walk playing on at walking pace: a whoosh going, a thud and kicked-up
  chips landing. Then it walks till it may dash again (spawn options
  `dash_speed`, `dash_time`, `dash_rest`, `dash_rest_close`).
  - **Not like a machine** (the user, 2026-11-05: its dashes came at regular
    intervals). At a set 0.5 s rest it dashed at the first push-off after
    each, every 1.0 to 1.6 s in a 45 m chase. Now each rest is drawn, 0.4
    to 1.6 of its mean: coming at you 3 s (`dash_rest`); within 12 m,
    fighting, 0.8 s (`dash_rest_close`), and a quarter of the time none,
    two dashes running. In three 45 m chases the gaps came to 2.5 to 6 s.
    And no two steps alike: as each foot comes down its next step's pace is
    drawn, 0.85 to 1.15 of its walk, now and then a short slow step at 0.6
    (`stride_vary`, 0.15; 0, a machine's). Its walk plays at its ground
    speed, so its feet stay down whatever the pace.

- **Its walk as waves** (2026-11-05; agreed 2026-10-31, then left while
  navigation came first). `python tools/gait_fit.py creatures/NAME.glb
  --write 6` fits each joint of the walk clip as a mean turn plus six
  sine waves through the stride (Unuma et al. 1995) and writes
  `creatures/NAME.gait`; the game reads it beside the `.creature` (failing
  loudly if it was fitted to another clip) and plays it over the clip, in
  the walk's share of what is played, at the same point of the stride: by
  default since it passed blind (the user, 2026-11-05: "I couldn't tell
  them apart"); `walk=0` for the clip as it is. The fit: feet within 1.1 cm of the clip at worst, joints
  within 4°. In the game, in a 45 m chase, clip against waves: head over
  the root 2.234 / 2.234 m, its rise and fall 2.188 to 2.294 / 2.190 to
  2.295, foot lift 0.547 / 0.550 m, planted feet sliding 0.18 / 0.20 m/s.
  (Played with one wave on purpose, they slid 1.19 m/s, as the fit tool
  predicts, 1.24: the waves are what is drawn.) To compare blind: `spawn
  NAME 2 blind tame tame_line ahead=8` puts two side-on, walking back and
  forth, one 5 m behind and 6 m aside of the other; which is which is in
  `blind.txt` beside the game. (`tame` now walks an authored body at its
  walk's pace, not a run's.)
- **Each step its own shape** (`clip/gait.rs`, `Steps`; the user,
  2026-11-05). Its arms swing 0.6 of its clip's (`arm_swing`; the user:
  too large). And each step draws, either way:
  - **its reach** (`step_reach`, 0.1): its thighs swing that much further
    or less, its stride as much longer, so the same ground is covered in
    fewer strides and a foot down stays down;
  - **its arms' swing** (`step_arms`, 0.25);
  - **each foot's lift** (`step_lift`, 0.25): its knee bent that much more
    or less through its swing, from the line between how it stands where
    its swing begins and ends, so it is unchanged there;
  - **its trunk's lean** (`step_lean`, 3°): forward, sideways and twisted,
    eased to as each foot comes down.

  Reach and arms change only about the two moments of its stride when both
  thighs pass their middle (its feet under it, its arms near theirs), eased
  over 0.06 of a stride either side: found from its waves at load, each
  walk's own (the gladiator's 0.41 and 0.88, logged at start); its knees'
  swings likewise. All four 0 and `arm_swing=1`: the fit as it is (test
  `unshaped_is_as_fitted`). Measured in a 45 m chase, planted feet sliding
  (median): the clip 0.18 m/s, the waves unshaped 0.18, shaped as default
  0.20, every amount doubled 0.24; the higher foot up to 0.55 m, 0.55,
  0.66, 0.80. (Its lean was at first lost: the waves and `aim`, which
  turns its head to you and flinches it, were not run in order, and the
  waves came last; with the waves its head did not turn to you. Ordered
  2026-11-05.) Next: the weirdness layer.

- **Its arms and trunk quiet** (the user, 2026-11-07: the most human of it,
  and worse, a conceited, in-your-face walk, out of place in our world).
  Measured in its waves: its arms swung ±20° (forearms ±31°), and its
  spine's three joints and its hips' turn each about ±5.5°, the chest
  twisting against the hips; the hips rolled ±4°, the head turned ±7°.
  Now by default its arms hang still (`arm_swing`, 0; was 0.6), its trunk
  (hips' turn and roll, spine, neck, head) keeps its walk's mean pose
  without its swing (`torso`, 0; 1 as fitted), and no lean is drawn each
  step (`step_lean`, 0; was 3°). Its legs go as fitted: the hips' lost turn
  is given back to its thighs, so its feet land where they did (test
  `quiet_trunk`; the chest within 1° through its stride). Its hips still
  rise as fitted. `spawn gladiator torso=1 arm_swing=0.6 step_lean=3
  sway=1 hunch=0 arms_in=0` walks as before.

  Not enough (the user: "doesn't work", and nor did the same motions
  reversed, `torso=-1 arm_swing=-1`). Its clips' pose, not their swing,
  carries the swagger too: drawn as stick figures, its upper arms are held
  16 to 19° out from hanging, its hands 0.36 to 0.43 m from its middle with
  its shoulders 0.27 m (flared arms); its trunk and neck upright (2°
  forward); its hips swaying 8.8 cm and bobbing 10.6 cm. So also, walking
  and standing (not in its attack's or a hit's share):
  - its trunk stoops forward (`hunch`, 15°), shared among its spine's
    joints, its neck turned back by half so it looks ahead;
  - its arms turned in toward its sides about its forward (`arms_in`, 15°);
  - its hips' sway sideways gone (`sway`, 0; 1 as fitted): each thigh
    rolled out by as much, over its leg's height from hip to foot as it
    stood last frame, so its feet stay where they were (checked on the
    clip in Python: the planted feet moved 0.00 cm; a fixed lever, the hips'
    height, left 2 cm). Its bob is left: it is its legs' bend.

  Judged in place (`still`), the user chose (2026-11-07) the swings
  reversed and the pose as its clips': `hunch=0 arms_in=0 sway=-1 torso=-1
  arm_swing=-1 step_lean=3`, now the defaults. Each arm swings with its own
  side's leg, the chest twists with the hips, each hip rises as its leg
  swings, its hips sway the other way (its feet still planted: 0.02 cm, a
  Python check on the clip). Reversed, the left elbow bends back about 7°
  at one point of its stride. "Better than what we had, but still work to
  do": left there, for an animation pass over every creature later.

  A low arc, its look alone, was
  tried and taken out (the user: it did not work). A `run` clip in its file
  is no longer played.
- **Its attack** starts when it winds up, and the blow lands at the clip's
  measured moment.
- **Over the clips,** its head turns to you (up to 70°). Shot, it is shoved
  and staggered as every hunter is; its trunk leans with the reel spring every
  hunter's body has, and a struck limb, neck or head flinches the way the
  shards flew, sprung back (`FLINCH`).
- **A weapon** (`weapon = mace` in its `.creature`): the mace of
  `combat/hunter/mace.rs` in its right hand, on the hand joint, so it
  follows every clip and falls with it; its head blooms, spikes thrust out,
  as its attack clip's blow lands. Its grip: `--set grip_x/grip_y/grip_z`
  (degrees, in the hand joint's frame), `grip_out` (m up the hand) and
  `grip_palm` (m out of its palm). A fifth of its haft's shards are pale.
- **Floating shards** (`shards = left` or `right` in its `.creature`): the
  shard bodies' sprung pieces off that arm's shoulder, trailing its swing,
  scattered by a shot.
- **Its echo** (`echo`, `clip/echo.rs`): its model again, standing
  `echo_off` m off to one side and behind, drifting, posed every frame as
  the body was `echo_lag` s before: it walks, strikes, flinches and falls
  as the body did, that much later. Only a look: shots strike the body,
  and it dies with it. It casts no shadow.
- **Others of its kind** it does not walk through: every living hunter is
  a column (0.35 m round, a beast's 0.45) that pushes the others out
  (`apart` in `hunter.rs`). Its arms and weapon still pass through.
- **Shots** are taken on capsules laid along its joints as drawn.
- **Killed, it falls limp:** the stalker's ragdoll (`ragdoll.rs`) on its
  bones as drawn, pushed by the blow and carried on by its pace, each joint
  turned with its bone. It lies there 90 s and can be shot about. It falls
  faster than the stalker: at 20 m/s², its knees hardly holding (spawn
  options `fall_gravity`, `fall_tone`; with the stalker's 9.8 and 0.2 its
  hips came down in 0.76 s, now 0.40 s). (Its fall looked slowed down, the
  user, 2026-11-04: out in the world, thousands of metres from its middle,
  the ragdoll took every point for stopped by a wall, a float's rounding,
  and took its speed away every frame; it sank to the ground over 3 to 6 s.
  Fixed in `Ragdoll::step`; the stalker's fall out there too.) A `death`
  clip is not needed (it plays only if its hunter is taken away unkilled).

## Our own walk on a generated mesh

A mesh with no rig can wear one of our procedural creatures' walks instead
(2026-11-06, the stalker's first: `combat/pieces.rs`). `creatures/stalker.pieces`
names the model and its joints, measured in metres with the model stood at
its height: hips, chest, head, and each leg's hip, knee and foot and each
arm's shoulder, elbow and palm. The game:

- builds the stalker's skeleton to those joints (its legs, arms, torso
  and neck the mesh's lengths; how it stands, hunches and walks its own),
  so no piece is stretched;
- cuts the model's triangles, each to the bone it is nearest (its head:
  everything from its neck's top up), keeping its textures;
- rides each piece rigidly on its bone (`spawn stalker mesh=1`; `head=1`
  puts its cage head on instead of the model's).

The slender shard mesh, measured: hips 1.36 m, thighs 0.85 m over shins
0.47 m (the stalker's own: 0.65 over 0.95, stilts), so its knees are
higher than the stalker's. `mesh=2` keeps the stalker's own build (the
user: its proportions and height were better) and stretches each piece
along its bone to fit: shins 2 times, thighs 0.76, torso 1.2, arms about
1.1; across, unchanged. To fit another model, write its joints (a plot
of its vertices from the front and side shows them) and check the cut in
the game.

Measured on the robot (2026-10-30): in game, its feet slide as much as its
clips' own feet do, and no more (walk 0.9 m/s, run 0.15 m/s; a stylised
clip). Ten robots in the arena: 123 fps average, worst frame 9.5 ms.

## The stalker hovers in combat (`rig::HOVER`)

(Its defaults, the user, 2026-11-07: `air=2`, and `mesh=2`, its own build
in the slender mesh's pieces.)

Hunting, it hovers (the user, 2026-11-06: its walk would not come right
with our means):
- its feet hang 25 cm over the ground, each stilt trailing its way on a
  spring of its own, so they swing apart;
- its body bobs slowly, upright, moving as one piece, and it gets going
  quickly (leaning 6° into its way, it looked as if skiing, the user). (Its chest sprung after its hips as in its
  struggling walk, and its stilts on slow springs, its waist drove it, the
  rest dragged after, its trunk bending near 45° at a turn, comic, the
  user. In the test its trunk now bends at most 2.5°, its stilts out
  to 21°;
  before, 25° and 36°.)
- to fire it comes down on its stilts in about 0.4 s, wide-set, with a
  thud; only anchored can it lock (`Rig::anchored`), the landing the tell.
  (A crouch as it touched down, its head dropping 29 cm more in a frame,
  read as a second motion, the user: the descent is its anchoring.)

Hovering, it walks the air (`rig::AIR`, `air`; the user, 2026-11-06: leg
movement related to what walking legs would do, untethered from the
ground, strange):
- its legs go through the gladiator's walk as at its pace, touching
  nothing: a foot "down" goes back under it as on a floor that is not there,
  then swings on;
- each foot on an arc round its hip, rising as it swings out;
- aside, each leg in its own plane, passing the other;
- holding still, they hang straight again.

`air=2` takes half the steps, twice as long; `air=0` hangs still. In the
test its feet keep 24 cm up, its legs swing 1 to 2 m fore and aft and stay
21 cm apart or more; in the game, out to 30°.

After its shot it lifts off again. Wandering, before it has seen you, it
walks its own struggling walk. In the game its feet were off the ground 80%
of the time, and its shots came 4.9 s apart (4.6 walking: the landing).
`hover=0` walks the gladiator's walk instead (below); tame, `hover=1`
hovers.

## A walk measured for another body (`tools/step_shape.py`)

The stalker hunting walks the gladiator's walk (the user, 2026-11-06: in
combat it sheds its struggling nature; everything it does energetic). Not
the gladiator's joint turns, which on legs of other proportions slide and
dig in, but the shape of its steps, in lengths of its own leg:

- when each foot lifts and lands;
- its swing, along and up;
- where it lands from its hip;
- the hips' rise, sway and surge;
- the pelvis's and shoulders' turn, and the trunk's lean and roll.

`python tools/step_shape.py creatures/gladiator.glb --write` measures it
into `creatures/gladiator.steps`. A foot is down from its ankle stopping to
its toes going on ahead; timed by the ankle alone it was down 45% of the
cycle, which is a run. The rig (`rig/steps.rs`, `Intent::walk`) steps its
own feet with it, planted where they land, at the gladiator's cadence,
with no struggling beats and sharp acceleration. Its hips follow the
measured rise, never higher than its stilts reach.

Its legs drive its body (the user, 2026-11-06: it floated in a direction,
its feet stepping under it). At an even pace its body glided while its feet
kept up. Now its pace pulses through each step, the same on the whole:
- walking on ahead, as its hips' surge did, 0.8 to 1.2 of its pace;
- aside, thrown with each step, still as each foot lands, twice its pace
  between.

At 1.6 m/s its body goes 1.3 to 1.95 m/s ahead and 0 to 3.2 aside. Its hips
are never held higher than its planted stilts reach. A lifting foot starts
from the ground: the measured swing begins at the gladiator's ankle, a
tenth of a leg up as its toes push off, and the heelless stilt tip leapt
17 cm in a frame.

Its upper body is not the walk's (`rig::WALK_CHEST`). The gladiator's
shoulders twist 24° through each stride against its swinging arms, and its
trunk leans and rolls. On the stalker, its lance in both hands, that read
as wiggling (the user, 2026-11-06): with the stilt walk's lurch at each
step, its chest went 20 to 28 cm side to side. Now it walks as one carrying
a rifle does. Its shoulders stay square, its chest follows 30% of its hips'
sway, its chest rides on its hips, not sprung after them, and the lurch
is off. (Sprung, its chest trailed 29 cm behind its hips, thrown aside.) Its chest goes 2 to 6 cm side to side and does
not turn; its hips still rise and fall 10 to 15 cm.

Walking aside, its feet never cross: the leading one steps out and the
other closes up to it. Crossing, one stilt went through the other.

Measured on the stalker at 1.6 m/s:

| | Ahead | Aside |
|---|---|---|
| Steps in 4 s | 9 | 9 |
| Planted stilts sliding | none | none |
| Hips rising and falling | 15 cm | 10 cm |
| Facing | — | where it looks, 0° off |

The stalker uses it hunting and searching; wandering, it keeps its own
walk. Between shots, hovering, it adjusts where it is all the while, gun on
you (`stalker.rs`, `SHIFT`), and comes down only to fire:
- darts of 0.12 to 0.4 s at about 3.75 m/s (`shift_pace`, 0.4 either side);
- holds between, still in the air, mostly short, now and then up to a
  second; now and then two darts running;
- any way, towards you only half as far (never, nearer than 15 m), back
  towards where it settled once 2.5 m from it.

(Back and forth at a steady pace it read as a dance from side to side,
predictable and smooth; it should read as small adjustments to where it is,
the user.) In the game: one landing to each shot, its darts 0.08 to 0.38 s,
its shots 4.9 s apart. Its height keeps as it darts: the rig's body
drops as it runs, its legs to reach, and hovering its head dipped 27 cm at
each dart (the user); not hovering, now.

Its steps quicken with its pace, its stride never longer for its legs than
the walk's on ahead (1.8 legs), nor 1.0 aside. At 4 m/s it takes 12 steps
in 4 s ahead and 21 aside. Faster than about 2.5 m/s it is past what a walk
can do: its hips drop with each step, 46 cm aside and 68 ahead at 4 m/s.
Running would need a run (the gladiator's is there to measure).

Its stopping is the tell that it is about to fire (the user, 2026-11-06:
the shifting the norm, not the exception). In the game it moved 89% of the
time, turning back about once a second, and stood 0.5 s before every shot.
Its shots stayed 4.6 s apart. Stepping aside only now and then, it moved
13% of the time, then 45%: rarely, the user.

To judge the walk side-on: `spawn stalker tame tame_line
walk=1 tame_pace=1.6`; `walk=0` is its own. Shifting between shots:
`spawn stalker`, against `spawn stalker shift=0` (it stands to fire).
