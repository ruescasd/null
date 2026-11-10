# Shard Creature Locomotion: Spec for a Fresh Start

Oct 30, 2026

## Purpose and context

Goal: a quadruped made of floating shards that walks with motion caused by its legs, at one fixed speed first. This spec replaces a previous attempt that failed for structural reasons, not tuning.

- **The creature.** A four-legged body built from black shards suspended in mid-air around an invisible skeleton. There is no skinned mesh; shards follow anchor points on the skeleton.
- **The world.** Movement shooter with a monochrome look. Creatures are meant to be strange, but strangeness must come later and from causes (see the last section), never from errors.
- **What failed before.** The body moved first and legs were fitted under it, so motion looked like a train with legs as decoration. Step triggers driven by a stride clock and distance thresholds caused vibration. Fixes were added as patches and safety margins, each breaking something else. Forelegs bent like hind legs. Margins of 2% on leg length silently bent knees to about 144–157°.
- **The core lesson.** These were symptoms of one design error: the body decided and the legs adapted. Everything below inverts that.

## Definition of correct

Correct means causally consistent with the physics this world establishes: every visible motion is a consequence of something, never an added parameter.

- **No ad-hoc motion.** Body bob, sway, pitch and speed changes must result from what the planted legs are doing. A separate "bob amplitude" or "sway" parameter is forbidden.
- **Integrity constraints that may never be violated:**
  - Contact: a planted foot does not move in world space, not even by a millimetre.
  - Continuity: no jitter, no single-frame pops, no value flipping back and forth between frames.
  - Consistency: the same situation always produces the same behaviour.
  - Intent: every motion has a readable purpose (support, step, push).
- **No safety margins.** If a margin seems necessary, it signals a missing rule in the model. Fix the model instead, and state the reason in writing.

## Architecture: feet first

Planted feet are fixed points in the world, and the body's position is computed from them. The body never moves on its own and then asks legs to fit.

Think of a walking body as vaulting over stiff legs, like an inverted pendulum: it rises as it passes over a planted foot and falls forward past it. Speed surges and slows with each step because the strides cause the motion.

Each frame runs in this fixed order, with no layer reading values computed later in the same frame:

1. **Contacts.** Which feet are planted, and where (fixed world positions).
2. **Body from contacts.** Shoulder position from the planted forelegs, hip position from the planted hind legs, each at full leg length along the leg's current angle. Body pitch is the line from hip to shoulder.
3. **Progression.** Planted legs rotate about their feet; this rotation is what advances the body.
4. **Lift and land decisions.** Based on leg angles and neighbour states (see Gait).
5. **Swing feet.** Lifted feet travel along an arc to their landing point.
6. **IK.** Joint angles for each leg from hip or shoulder to foot.
7. **Shards.** Each shard follows its skeleton anchor with a spring-damper. This is the only place secondary motion lives.

If something looks wrong, identify which layer produced it before changing anything.

## Leg model and knee geometry

A planted leg is straight by construction: the body is placed at exactly full leg length from the foot, so the knee cannot bend by accident.

**Why this must be by construction.** With two-bone IK, the knee angle depends only on the hip-to-foot distance. Near full extension that relationship is extremely sensitive. With the current segments (0.56 m and 0.73 m, total 1.29 m), holding the body at 98% of leg length already bends the knee to about 157°. Any smoothing, averaging or margin that lowers the body even 1–2 cm visibly bends the legs.

```
d^2 = a^2 + b^2 - 2ab cos(theta)
```

Here d is hip-to-foot distance, a and b are segment lengths, and theta is the knee angle. Check every choice that touches body height against this formula.

**Knee bend by phase:**

- Touchdown: straight, knee at or above 175°, leg reaching ahead of its shoulder or hip.
- Stance: essentially straight while the leg rotates over the foot. A small cushion is allowed only if defined as a number (proposal: never below 165°).
- Swing: the knee folds freely to lift the foot clear, then extends before landing.

**Bend direction is per leg pair and fixed relative to the body.** Hind knees point forward with the hock behind. Forelegs bend the opposite way, so the foot folds backward. Use an explicit pole direction per pair; never derive it from the leg's own geometry.

**No flicker at full stretch.** Clamp the IK distance to leg length. With a clamped distance and a fixed pole, a straight leg is exactly 180° and cannot flip.

## Over-constraint: who gives when legs disagree

Two planted legs at different angles cannot both be straight at full length under one rigid attachment point, so the model must decide explicitly which leg yields.

A leg reaching forward at 20° holds its shoulder at about 94% of the height a vertical leg would. With both forelegs planted, a rigid shoulder cannot satisfy both. Margins were previously used to hide this; they are now forbidden.

**Rules:**

- Shoulder and hip heights are independent. Forelegs set the shoulder, hind legs set the hip, and the difference is body pitch. This removes conflict between front and back.
- Within a pair, the leg that most recently landed (the reaching leg) stays straight and sets the height.
- The other planted leg in the pair absorbs the difference with a small knee bend, within the stance limit defined above.
- If the required bend exceeds that limit, that leg should already have lifted. Treat it as a gait bug, not something to smooth over.
- Never compute body height as a smoothed minimum or average across legs.

## Gait

Feet lift because of leg geometry and neighbour states, never because of a clock or a distance threshold.

- **Order.** Walk uses a fixed lateral sequence: hind left, fore left, hind right, fore right, a quarter cycle apart. At least three feet are planted most of the time.
- **Lift rule.** A foot lifts when its leg has rotated back to the lift angle behind its hip or shoulder **and** its neighbours (the adjacent legs in the sequence and the leg on the same side) are planted. If neighbours are not planted, the foot waits and the body's progress slows slightly. This rule decides; the order is what results, not a separate clock.
- **Landing point.** Ahead of the shoulder or hip at the touchdown angle, with the leg straight. Account for body motion during swing so the leg is at the right angle when the foot arrives.
- **Swing.** A fixed duration at fixed speed, along an arc with enough height to clear the ground, knee folded.
- **Speed control.** A target speed is required. Speed is achieved by adjusting landing distance and stance-leg push, not by moving the body directly. Without a target, speed will drift.
- **Angles are tunable, not fixed truths.** Start around 20° at touchdown for a walk and check against reference footage of real quadrupeds walking side-on.

## Acceptance criteria and tests

A build is done only when every test below passes on logged data from at least 20 consecutive steps at the fixed walk speed.

| Test | Pass condition |
| --- | --- |
| Planted foot fixed | Zero world-space movement while planted (within float precision) |
| Knee at touchdown | ≥ 175° on every leg |
| Knee in stance | Never below the agreed stance limit (proposal: 165°) |
| Knee direction | Hind knees forward, forelegs opposite, on every frame |
| No jitter | No joint angle or foot position reverses direction on consecutive frames while planted |
| No double steps | A foot never lifts again within one swing duration of landing |
| Gait order | Lift sequence matches hind left, fore left, hind right, fore right |
| Support | At least three feet planted for the agreed share of the cycle |
| Causal body motion | Body height and pitch match values recomputed from planted legs on every frame |
| Frame-rate independence | Results match within tolerance at 60, 144 and 300 fps |

Tests are written and reviewed by the user before implementation starts, and are not changed by the implementing agent without approval. Every result includes a side-on capture alongside the logs.

## Build order and scope

Scope for the first build is a walk, in a straight line, on flat ground, at one fixed speed. Each step must pass its tests before the next starts.

1. One leg on a body moving in a straight line, IK off. Draw contacts and targets as dots, check feet stay fixed.
2. Same leg with IK on. Verify knee angles and bend direction.
3. Two hind legs alternating, hip height from the planted leg.
4. All four legs with the lift rule, shoulder and hip independent, body pitch.
5. Shards on springs.
6. Only then: speed range. Running needs a compressing leg (body lowest at mid-stance, the opposite of walking), so it is a separate model, not a faster walk.
7. Later still: turning, slopes, uneven ground, gait transitions.

During development, run at 10–25% time scale to see oscillations, and keep debug drawing for contacts, leg angles and phases.

## Working rules for the agent

The agent cannot see motion, so every claim must be backed by measured evidence the user can check.

- **Evidence, not claims.** Never say something works without test output, logged values or a capture. If it cannot be shown, it is not done.
- **Report failures plainly.** State what failed, what was measured, and what is unknown. Do not explain away a problem the user reports; measure it first.
- **Stopping rule.** If three fixes in a row each break something else, stop patching. Write a short diagnosis of whether the problem is design or tuning, and propose a change to the model.
- **No margins, offsets or smoothing that change geometry** without a written justification and a check of what they do to knee angles.
- **Change one layer at a time.** Name the layer before editing it.
- **Do not modify tests** to make them pass. Propose test changes to the user separately.
- **Ask when a requirement is ambiguous** rather than choosing an interpretation silently.

## Later: making it strange the right way

Strangeness comes after a correct walk, by changing causes rather than symptoms; most incorrect motion reads as a bug, not as weird.

- **Change causes.** Leg count, unequal leg lengths, unusual bend directions, a different lift order, mass in odd places, shards held by springs of uneven strength. The resulting motion stays causally consistent, so it reads as a strange creature.
- **Never add symptoms.** No extra twitch, lurch or sway parameters layered on top. Example: for a limp, shorten one leg and let the drop and timing emerge.
- **Keep the integrity constraints** (contact, continuity, consistency, intent) at every setting. The architecture guarantees them; the parameters explore.
- **Global lawful transforms are allowed sparingly**, applied to the whole creature: time reversal, holding frames, parts running at different tempos.
- **Shard construction should read at range.** Larger gaps at joints, shards that lag and drift on springs, hits that knock shards loose, death as the springs letting go.
