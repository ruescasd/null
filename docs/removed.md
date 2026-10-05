# Removed, worth revisiting

Pruned on 2026-10-05 to keep the project lean. All of it is at commit
`bda964f` (`git show bda964f:<path>`). Only what might still earn a place is
listed. The rest was judged in review and dropped.

## World content

- **Stepped mounds of the tiers** (ziggurat sites). Terraces of mantle-height
  ledges suit the movement. The earthwork and terrace code is still there;
  only the site rules and the ziggurat style went.
- **Racks and pipe networks** (`rack.rs`, `Roots`, `Rack`). Pipes lying on
  the ground and spreading from structures were "much better, not there yet".
  They are the first candidate for infrastructure roots, a city's transition
  into open land.
- **Curtains** (`curtain.rs`). Walls round an open shaft with sheaves of
  cables across the void. A reference for chasm walls.
- **Massif** (`Keep::Massif`). Columns falling from the centre at every
  level, a mountain of mountains. Liked early, never developed.
- **Stepped mountain and walled enclosure colossi.** These are colossus
  forms, removed in favour of the needles, the shattered shafts and the
  cities.

## Look

- **Ink lines** (`ink.rs`). Lines at silhouettes and creases after lighting,
  parked as "can't tell yet". The creature's own outline (inverted hulls)
  stays.
- **Procedural panelling and etched network** (`terrain.wgsl`). Detail in the
  shading on structures. Geometry comes first, so this would only return with
  restraint (finest octave only).

## Movement

- **The thrust beam** (Mouse 1 before combat): recoil pushing the player
  away from where it aimed, energy recharging on the ground, drawn as a
  jittering bolt. Retired as propulsion for the shotgun (at `66b1785`); its
  bolt may come back as the lightning gun.

## The creature

- **The hunched humanoid and the feral creature** (`--opt hunched`,
  `--opt creature`), and the alternative heads, hands and eyes. The chosen
  combination is the default. `--opt luminous` stays as a possible final boss.
