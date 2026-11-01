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
- **Clips.** `walk` and `run` are required; `idle`, `attack`, `hit` and
  `death` are optional, and any left out are simply not played.
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
| `spawn NAME health=20` | dies quickly, to see the death |
| `round NAME 3` | three through the arena's gates, timed |
| `spawn NAME --help` | the options it takes |
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

- **Walk and run** share one phase. Each is played at its own measured ground
  speed, blended by pace, so a foot down stays down (as far as the clip's own
  foot does).
- **Its pace** is capped at its run clip's speed. Its attack starts when it
  winds up, and the blow lands at the clip's measured moment.
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
  turned with its bone. It lies there 90 s and can be shot about. A `death`
  clip is not needed (it plays only if its hunter is taken away unkilled).

Measured on the robot (2026-10-30): in game, its feet slide as much as its
clips' own feet do, and no more (walk 0.9 m/s, run 0.15 m/s; a stylised
clip). Ten robots in the arena: 123 fps average, worst frame 9.5 ms.
