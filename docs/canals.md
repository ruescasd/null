# Canals

The canals are the world's transport: straight half-pipes between stations,
off until the player turns them on. An on canal can be ridden and has a beam
of light in the sky over it. An off one has neither: off the lit network you
should feel lost, even afraid, and each lit station feels safer for it.

## Plan

1. **Network and map.** Done (`bf3a3b6`). Nine stations on a jittered 3×3
   grid round the wrapping world, linked only to true neighbours, so canals
   cross nowhere but at stations: 14 canals of 4–7 km. The map example
   draws it.
2. **Canals as pipes between stations, on or off.** Done (`bf3a3b6`). Each
   ends on a station's court, a placeholder: a flat round court at the
   pipes' floor level. `--opt canals_on` turns them all on.
3. **Beams.** Done (`bf3a3b6`). A ray of light from above one station to
   above the next, shown only while its canal is on.
4. **Surface and light.** Done (`12c469d`, `a0b6f6d`). Grooves and joint
   rings in the pipe, light in the grooves while on (a faint glow and
   pulses along the flow), and stone blocks lining the pipe near you, so
   you can judge your speed.
5. **Riding.** The user's own movement, from first principles, in place
   of Quake's (2026-10-17). The player does the moving; the canal rewards
   it, never drives it, and the whole segment behaves differently while on.
   - Off: the pipe is stone, with friction and walking.
   - On: slick, with slightly weaker gravity and a slow damping of the swing
     across the pipe, and these rules:
     - Each key pushes its own way: W where you look, A and D square to it.
     - A push's part across the pipe works until your speed in the key's
       direction reaches a run (320 ups). The wall resets that, so
       pressing into a wall never runs out, and steering across the pipe
       always works.
     - A push's part along the pipe works until your speed along the pipe
       reaches a run. Holding keys gets you a run's worth and no more.
     - Turning against the strafe (D while turning left, A while turning
       right) adds speed along the pipe in the way you move. That
       counter-rotation aims the strafe down the pipe. It counts while the
       view is within `turn_window` of the way you move, and only for turns
       no faster than `turn_max`.
     - The pump: coming down a wall while strafing the way you swing gives
       extra speed. It works with the turning.
   - Why not Quake: its air acceleration let the wall reset your speed into
     it every step, so holding one strafe key gave speed along the canal
     without end (the surf exploit). Each guard tried killed the fun or
     the keys: a remembered push (a strafe key dead after hugging a wall),
     caps scaled by the view's angle (strafing either steered nothing or
     gained nothing past a run), counting only turns back towards the way
     you move (broke the turning). Spinning the view gained without end
     until too fast a turn counted for nothing.
   - The settings (the user's "modest" values):

     | Setting | Default | What it sets |
     |---|---|---|
     | `turn_gain` | 2.2 ups per degree | Speed from turning against the strafe |
     | `turn_window` | 70 degrees | How far off the way you move a turn counts |
     | `turn_max` | 350 degrees/s | Faster turns count for nothing |
     | `pipe_accel` | 1 run/s | How hard each key pushes |
     | `canal_pump` | 0.25 | Reward for the swing's rhythm |
     | `canal_glide` | 0.85 | Gravity in the pipe |
     | `canal_damp` | 0.06 | How fast a swing dies |
     | `canal_cap` | 90 m/s | Top speed (was 60; raised for more progression) |

   - How it plays (the user, 2026-10-17): the pump and the turning
     reinforce each other, with a trade-off between them. Too much turning
     at the bottom of the pipe loses the pump's gains; too many long swings
     up the walls means fewer turning boosts. The best balance shifts with
     speed: from slow, swing more; when fast, turn tighter. `turn_gain` 2.2
     reaches the top speed within a reasonable share of a canal's length.
   - Items could raise them: better boots, say, `turn_gain` 3, and the top
     speed (see item progression).
   - Tests: the movement bot holds one strafe for a set time and weaves
     (`--opt bot --opt strafeonly --opt canals_on`, `--set strafe_time`,
     `strafe_yaw`); it neither turns nor times, so it says little about
     the new movement. `--opt ridelog` writes `target/ridelog.csv`, frame
     by frame, with the turn rate.
6. **Geometry tweaks**, to make the journey more interesting:
   - **More vertical variation along a canal.** Built and switched off: long
     rises and dips over the land's shape (`--set canal_rise=30`,
     `canal_wave=1800`; the `canal_profile` example prints a canal's floor
     against the ground). Waiting on the world's own verticality, still to
     decide: how a canal rises and falls depends on what it crosses, and
     its rises should perhaps come from the land, not their own wave. (The
     floors were also lowered 4.5 m, `51cce99`, kept for now.)
   - **Turning: smooth curves set aside.** They need every piece of canal
     code to follow a curve, with a real risk of geometry glitches. Bends
     inside buildings (two straight pipes meeting at a "bend house", like a
     two-pipe station) would be cheap, but stopping and crossing a floor
     kills the ride's rhythm and momentum. Crests from the vertical
     variation hide what lies ahead instead.
7. **Activation.** How a canal or station is turned on is still open. A
   candidate: energy delivered to a station, such as a porter's load.

## Open

- **Riding.** Whether steep ordinary ground (earthworks, slanted plates)
  allows Quake's surf trick: not checked.

- **Stations need real architecture.** They are their own kind of place,
  a transit building, not every important place. Finding them must not be
  trivial.
- **The hand-placed spire example** at (2633, 2004) is squeezed out by a
  canal passing 60 m from it.
