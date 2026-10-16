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
5. **Riding.** Now. The player does the moving, Quake-gliding style; the
   canal rewards it, never drives it. The idea chosen: the whole segment
   behaves differently while on (the pulses stay as visuals).
6. **Geometry tweaks**, to make the journey more interesting:
   - more vertical variation along a canal (the floors were lowered 4.5 m,
     `51cce99`, kept for now);
   - canals that turn somewhat, not dead straight.
7. **Activation.** How a canal or station is turned on is still open. A
   candidate: energy delivered to a station, such as a porter's load.

## Open

- **Stations need real architecture.** They are their own kind of place,
  a transit building, not every important place. Finding them must not be
  trivial.
- **The hand-placed spire example** at (2633, 2004) is squeezed out by a
  canal passing 60 m from it.
