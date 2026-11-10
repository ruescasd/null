"""Measure a creature's walk as the shape of its steps, for another body to
walk with (the user, 2026-11-06: the stalker, fighting, sheds its struggling
walk for the gladiator's, energetic).

    python tools/step_shape.py creatures/NAME.glb [--write]

Not its joints' turns (on other legs those slide and dig in): what its feet
and body do, in metres of its own leg, so a body with legs of another length
can do the same, its own feet planted where they land. Over one cycle of its
walk, from its left foot coming down:

- when each foot lifts and lands (of the cycle);
- each foot's swing: how far along its way and how high, through the swing;
- where a foot lands from its hip (ahead, out), and the stride;
- its hips' rise and fall, sway and surge about their mean, their height;
- its pelvis's and shoulders' turn, and its trunk's lean.

Written to creatures/NAME.steps with --write.
"""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from creature_clips import Skeleton, clip_named, read_glb, read_kept  # noqa: E402
from gait_fit import sample, worlds  # noqa: E402

# Samples written for each curve over its span.
POINTS = 24


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("glb")
    p.add_argument("--write", action="store_true", help="write creatures/NAME.steps")
    a = p.parse_args()
    glb = Path(a.glb)
    gltf, blob = read_glb(glb)
    skel = Skeleton(gltf, blob)
    kept = read_kept(glb.with_suffix(".creature"))
    m = float(kept["height"]) / float(kept["model_height"])
    names = {skel.names[j]: j for j in skel.joints}
    legs = [[names[n] for n in side.split()] for side in kept["legs"].split(",")]
    arms = [[names[n] for n in side.split()] for side in kept["arms"].split(",")]
    spine = [names[n] for n in kept["spine"].split()]
    hips, neck = spine[0], spine[-1]
    clip = kept["walk"].split()[0] if kept.get("walk") else clip_named(skel, "walk", None)
    times, duration, locals_, _ = sample(skel, clip)
    # (Its toes: a foot is down from its heel landing till its toes push
    # off; by its ankle alone, down 45% of the cycle, both in the air at
    # once, a run's.)
    toes = [next((c for c, pa in skel.parent.items() if pa == legs[k][2]), None) for k in range(2)]
    if None in toes:
        sys.exit("its feet have no toe joints")
    order = [hips, neck, legs[0][0], legs[1][0], legs[0][2], legs[1][2], arms[0][0], arms[1][0], toes[0], toes[1]]
    w = worlds(skel, locals_, order) * m  # (points, samples, 3), metres
    n = w.shape[1]
    fwd = np.array([float(v) for v in kept["forward"].split()])
    fwd3 = np.array([fwd[0], 0.0, fwd[1]]) / np.linalg.norm(fwd)
    left3 = np.cross(np.array([0.0, 1.0, 0.0]), fwd3)
    # Into its own frame: x to its left, y up, z ahead.
    w = np.stack([w @ left3, w[..., 1], w @ fwd3], axis=-1)
    H, N, UL, UR, FL, FR, AL, AR, TL, TR = range(10)
    feet = [FL, FR]
    if w[UL, :, 0].mean() < w[UR, :, 0].mean():
        sys.exit("its left leg is not on its left: check `forward` in the .creature")
    # Its leg: hip joint to ankle, straight, at rest.
    def rest_world(j):
        chain = [j]
        while chain[-1] in skel.parent:
            chain.append(skel.parent[chain[-1]])
        mm = np.eye(4)
        for c in chain:
            mm = skel.rest(c) @ mm
        return mm[:3, 3] * m
    leg = (np.linalg.norm(rest_world(legs[0][1]) - rest_world(legs[0][0])) + np.linalg.norm(rest_world(legs[0][2]) - rest_world(legs[0][1])))
    # A foot down: still over the ground (the clip in place, the ground
    # going back under it at its speed), within a fifth of its speed. (By
    # its height, its heel rising, it was down a third of the cycle, not
    # the most of it.)
    speed = float(kept["walk"].split()[1]) * m
    ground = [w[f, :, 1].min() for f in feet]
    dt = duration / n
    still = lambda f: np.abs((np.roll(w[f, :, 2], -1) - np.roll(w[f, :, 2], 1)) / (2 * dt) + speed) < 0.2 * speed
    def edges(d):
        # (Without its blips: a few samples are no step.)
        for _ in range(2):
            d = np.where(np.roll(d, 1) == np.roll(d, -1), np.roll(d, 1), d)
        lift = [k for k in range(n) if d[k - 1] and not d[k]]
        land = [k for k in range(n) if not d[k - 1] and d[k]]
        if len(lift) != 1 or len(land) != 1:
            sys.exit(f"a joint stops {len(land)} and goes {len(lift)} times a cycle; expected once each")
        return lift[0], land[0]
    # Landing as its ankle stops; lifting as its toes go.
    (_, land_l), (_, land_r) = edges(still(FL)), edges(still(FR))
    # (Its toes go back a little as they push off, faster than the ground:
    # going only once they go on ahead.)
    stays = lambda t: (np.roll(w[t, :, 2], -1) - np.roll(w[t, :, 2], 1)) / (2 * dt) + speed < 0.2 * speed
    (lift_l, _), (lift_r, _) = edges(stays(TL)), edges(stays(TR))
    span = lambda land, lift: np.array([(k - land) % n < (lift - land) % n for k in range(n)])
    down = [span(land_l, lift_l), span(land_r, lift_r)]
    # The cycle from the left foot landing.
    start = land_l
    def of(k):
        return ((k - start) % n) / n
    stride = speed * duration
    print(f"{clip}: {duration:.3f} s, {speed:.2f} m/s, stride {stride:.2f} m, leg {leg:.2f} m (stride {stride / leg:.2f} legs)")
    duty = [d.mean() for d in down]
    print(f"each foot down {duty[0]:.0%} and {duty[1]:.0%} of the cycle; the left lifts at {of(lift_l):.3f}, the right lands at {of(land_r):.3f}, lifts at {of(lift_r):.3f}")
    # The hips about their mean (in place): rise, sway, surge.
    hip = w[H] - w[H].mean(axis=0)
    height = (w[UL, :, 1].mean() + w[UR, :, 1].mean()) / 2 - np.mean(ground)
    print(f"hip joints {height:.2f} m over the ankles ({height / leg:.2f} legs): rise and fall {np.ptp(hip[:, 1]) * 100:.1f} cm, sway {np.ptp(hip[:, 0]) * 100:.1f} cm, surge {np.ptp(hip[:, 2]) * 100:.1f} cm")
    def yaw(a_, b_):
        d = w[a_] - w[b_]
        return np.degrees(np.arctan2(-d[:, 2], d[:, 0]))
    pelvis, shoulders = yaw(UL, UR), yaw(AL, AR)
    trunk = w[N] - w[H]
    lean = np.degrees(np.arctan2(trunk[:, 2], trunk[:, 1]))
    roll = np.degrees(np.arctan2(-trunk[:, 0], trunk[:, 1]))
    print(f"pelvis turns {np.ptp(pelvis):.1f} deg, shoulders {np.ptp(shoulders):.1f} deg; trunk leans {lean.mean():.1f} deg forward ({np.ptp(lean):.1f} through), rolls {np.ptp(roll):.1f}")
    # Each foot's swing, from where it lifts to where it lands, relative to
    # its hip joint: how far along (0..1), how high over its ground (legs),
    # and out (legs).
    swings = []
    for f, u, (lift, land), g in ((FL, UL, (lift_l, land_l), ground[0]), (FR, UR, (lift_r, land_r), ground[1])):
        ks = [(lift + i) % n for i in range((land - lift) % n + 1)]
        # Over the ground: the clip is in place, the body going on at speed.
        t = np.arange(len(ks)) * duration / n
        z = w[f, ks, 2] + speed * t
        along = (z - z[0]) / (z[-1] - z[0])
        up = (w[f, ks, 1] - g) / leg
        ahead = (w[f, land % n, 2] - w[u, land % n, 2]) / leg
        out = (w[f, land % n, 0] - w[u, land % n, 0]) / leg
        swings.append((along, up, ahead, out))
        peak = int(np.argmax(up))
        print(f"  a swing: {len(ks) / n:.2f} of the cycle, highest {up.max():.2f} legs at {peak / (len(ks) - 1):.2f} of it; half its way by {np.interp(0.5, along, np.linspace(0, 1, len(ks))):.2f} of it; lands {ahead:+.2f} legs ahead of its hip, {out:+.2f} out")
    if not a.write:
        return
    def curve(values, span=None):
        xs = np.linspace(0.0, 1.0, len(values), endpoint=span is not None)
        q = np.linspace(0.0, 1.0, POINTS, endpoint=span is not None)
        return " ".join(f"{v:.4f}" for v in np.interp(q, xs, values))
    cyc = lambda arr: np.roll(arr, -start)
    ahead = np.mean([s[2] for s in swings])
    out = np.mean([abs(s[3]) for s in swings])
    lines = [
        f"# {glb.stem}.steps: the walk of {glb.name} ({clip}, {duration:.3f} s) as its steps' shape, measured by",
        "# tools/step_shape.py --write, for another body to walk with (see `rig::Steps`).",
        "# Lengths in its legs (hip joint to ankle). The cycle runs from its left foot landing.",
        "# Its own leg (m), the cycle (s), its speed (m/s); its stride (legs), how much of the",
        "# cycle a foot is down, where the left foot lifts and the right (of the cycle); where",
        "# a foot lands from its hip (legs, ahead and out); its hip joints' height (legs).",
        f"leg = {leg:.4f}",
        f"cycle = {duration:.4f}",
        f"speed = {speed:.4f}",
        f"stride = {stride / leg:.4f}",
        f"down = {np.mean(duty):.4f}",
        f"lifts = {of(lift_l):.4f} {of(lift_r):.4f}",
        f"land = {ahead:.4f} {out:.4f}",
        f"hips = {height / leg:.4f}",
        "# Through a swing (from lift to landing, evenly): how far along its way, how high.",
        f"along = {curve(np.mean([np.interp(np.linspace(0, 1, 64), np.linspace(0, 1, len(s[0])), s[0]) for s in swings], axis=0), 1)}",
        f"up = {curve(np.mean([np.interp(np.linspace(0, 1, 64), np.linspace(0, 1, len(s[1])), s[1]) for s in swings], axis=0), 1)}",
        "# Through the cycle (evenly, from its left foot landing): its hips' rise, sway (to its",
        "# left) and surge about their mean (legs); its pelvis's and shoulders' turn (degrees,",
        "# its left side back); its trunk's lean about its mean (degrees, forward) and roll.",
        f"rise = {curve(cyc(hip[:, 1] / leg))}",
        f"sway = {curve(cyc(hip[:, 0] / leg))}",
        f"surge = {curve(cyc(hip[:, 2] / leg))}",
        f"pelvis = {curve(cyc(pelvis - pelvis.mean()))}",
        f"shoulders = {curve(cyc(shoulders - shoulders.mean()))}",
        f"lean = {curve(cyc(lean - lean.mean()))}",
        f"roll = {curve(cyc(roll - roll.mean()))}",
    ]
    path = glb.with_suffix(".steps")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
