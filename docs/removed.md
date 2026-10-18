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
- **The ground's grain, flecks and cracks** (`terrain.wgsl`, removed
  2026-10-17; all three at `1127424`). Value-noise mottling and micro-relief
  on the ground, there for a sense of speed, read as camouflage or a bugged
  texture, even at its finest octave. Sparse flecks and fine cellular
  (Worley) cracks were better but still cosmetic; the plain ground looked
  best. The cellular cracks might suit somewhere specific (a crazed glaze,
  dried ground). Speed now has to come from built things: stones in the
  shader, matched to the canal's, or small built features.

## Movement

- **The thrust beam** (Mouse 1 before combat): recoil pushing the player
  away from where it aimed, energy recharging on the ground, drawn as a
  jittering bolt. Retired as propulsion for the shotgun (at `66b1785`); its
  bolt may come back as the lightning gun.

## The beasts' heads (review 56)

- **Two more skulls** (a cranium sweeping back and up; a heavy, domed,
  hyena-like one), **slit and six-eyed variants**, and **a few long crooked
  fangs**. Kept: the hound's narrow head with two rows of needles, and the
  angler's. All at `8691ac6`.
- **The angler's lure** (a stalk with a bulb that flared like the eyes):
  too on the nose.

## The stalker's head (2026-10-18)

- **The cage's other finishes** (all pale; the body's black rising to a pale
  crown; black with pale highlights only), **eyes** (upright rhombuses
  behind its front openings, flaring as it looks at you; round ones before),
  and **other necks** (shard tendons from the torso into its struts, taut
  cables, pale or dark). Chosen: inked, no eyes, its own struts as its neck.
  All at `38826c1` (`--set cagetone`, `--opt cageeyes`, `--opt shardneck`,
  `--opt palecables`).
- **The skull built by formula** (`skull.rs` before the cage): a sphere
  pushed in and out came out an egg. A head's shape is authored (concept,
  reconstruction, bake), not generated.

## Weapons

- **The shard shotgun with a turning cage** (a barrel whose middle was an
  open cage of six rails turning a sixth per shot, shards lying in it, its
  front and back shrouded): replaced by the pump-action, kept as a basis for
  a rotary cannon. Git tag `rotary-cage` (`git show rotary-cage:crates/game/src/combat/weapon.rs`).

## The creature

- **The hunched humanoid and the feral creature** (`--opt hunched`,
  `--opt creature`), and the alternative heads, hands and eyes. The chosen
  combination is the default. `--opt luminous` stays as a possible final boss.
