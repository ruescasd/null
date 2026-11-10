"""Fit a creature's walk and run with sine waves, and say how closely they fit.

    python tools/gait_fit.py creatures/NAME.glb [--write WAVES]

A walk or run repeats, so over one cycle each joint's turning is a mean pose
plus a few sine waves, each with its own size and phase (a Fourier series;
Unuma et al. 1995). Fitted, a gait is a small table of numbers we can play at
any speed and bend in code (the user, 2026-10-31: the weirdness layer).

Each joint's rotation is taken about its mean over the cycle, as a rotation
vector (axis times angle), and each of its three numbers fitted; the hips'
movement likewise. This reports, for each number of waves, how far the fit
puts the feet, hands and head from the clip's (cm in the game), and how far
its joints turn from it; and the clip's own seam, where its end meets its
start (a fit can only smooth that).
"""

import argparse
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from creature_clips import CLIPS, Skeleton, clip_named, quat_matrix, read_glb, read_kept  # noqa: E402

SAMPLES = 256


def matrix_quat(m):
    """A rotation matrix's quaternion (x, y, z, w)."""
    t = np.trace(m)
    if t > 0:
        s = np.sqrt(t + 1.0) * 2
        q = [(m[2, 1] - m[1, 2]) / s, (m[0, 2] - m[2, 0]) / s, (m[1, 0] - m[0, 1]) / s, 0.25 * s]
    else:
        i = int(np.argmax(np.diag(m)))
        j, k = (i + 1) % 3, (i + 2) % 3
        s = np.sqrt(1.0 + m[i, i] - m[j, j] - m[k, k]) * 2
        q = [0.0, 0.0, 0.0, (m[k, j] - m[j, k]) / s]
        q[i] = 0.25 * s
        q[j] = (m[j, i] + m[i, j]) / s
        q[k] = (m[k, i] + m[i, k]) / s
    q = np.array(q)
    return q / np.linalg.norm(q)


def qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return np.array([aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz])


def qinv(q):
    return np.array([-q[0], -q[1], -q[2], q[3]])


def qlog(q):
    """A unit quaternion's rotation vector (axis times angle, radians)."""
    if q[3] < 0:
        q = -q
    v = q[:3]
    s = np.linalg.norm(v)
    if s < 1e-12:
        return v * 2.0
    return v / s * 2.0 * np.arctan2(s, q[3])


def qexp(r):
    a = np.linalg.norm(r)
    if a < 1e-12:
        return np.array([r[0] / 2, r[1] / 2, r[2] / 2, 1.0])
    h = a / 2
    return np.concatenate([r / a * np.sin(h), [np.cos(h)]])


def mean_quat(qs):
    """The mean of unit quaternions (the principal eigenvector of their outer
    products: sign-blind)."""
    m = np.einsum("ni,nj->ij", qs, qs)
    w, v = np.linalg.eigh(m)
    return v[:, -1]


def fit(values, waves):
    """Each column of `values` (sampled evenly over one cycle) as its mean and
    `waves` harmonics, rebuilt at the same samples: (rebuilt, coefficients)."""
    f = np.fft.rfft(values, axis=0)
    kept = np.zeros_like(f)
    kept[: waves + 1] = f[: waves + 1]
    return np.fft.irfft(kept, n=len(values), axis=0), f[: waves + 1] / len(values)


def sample(skel, clip):
    """Over one cycle of `clip`: each joint's local rotation (quaternions),
    the root joints' translations, and its parts' rest matrices."""
    tracks = skel.tracks(clip)
    duration = skel.duration(clip)
    # (Sampled over [0, duration): a looping clip's last key is its first.)
    times = np.linspace(0.0, duration, SAMPLES, endpoint=False)
    locals_ = {j: np.array([skel.local(j, tracks, t) for t in times]) for j in skel.joints}
    seam = {j: (skel.local(j, tracks, 0.0), skel.local(j, tracks, duration)) for j in skel.joints}
    return times, duration, locals_, seam


def rebuild(skel, locals_, waves):
    """The same joints' local matrices, from their fits."""
    out = {}
    for j, ms in locals_.items():
        qs = np.array([matrix_quat(m[:3, :3] / np.linalg.norm(m[:3, 0])) for m in ms])
        scale = np.linalg.norm(ms[0][:3, 0])
        mean = mean_quat(qs)
        vec = np.array([qlog(qmul(qinv(mean), q)) for q in qs])
        vfit, _ = fit(vec, waves)
        tfit, _ = fit(ms[:, :3, 3], waves)
        rebuilt = np.repeat(np.eye(4)[None], len(ms), axis=0)
        for k in range(len(ms)):
            rebuilt[k, :3, :3] = quat_matrix(qmul(mean, qexp(vfit[k]))) * scale
            rebuilt[k, :3, 3] = tfit[k]
        out[j] = rebuilt
    return out


def worlds(skel, locals_, points):
    """Each of `points`' world positions at every sample, from local
    matrices (joints without one at rest)."""
    n = len(next(iter(locals_.values())))
    out = []
    for p in points:
        chain = [p]
        while chain[-1] in skel.parent:
            chain.append(skel.parent[chain[-1]])
        pos = np.zeros((n, 3))
        for k in range(n):
            m = np.eye(4)
            for j in chain:
                m = (locals_[j][k] if j in locals_ else skel.rest(j)) @ m
            pos[k] = m[:3, 3]
        out.append(pos)
    return np.array(out)  # (points, samples, 3)


def angle(a, b):
    """The angle between rotation matrices' rotations (degrees)."""
    qa = matrix_quat(a[:3, :3] / np.linalg.norm(a[:3, 0]))
    qb = matrix_quat(b[:3, :3] / np.linalg.norm(b[:3, 0]))
    return np.degrees(2 * np.arccos(min(1.0, abs(float(np.dot(qa, qb))))))


def waves_of(values, waves):
    """Each column of `values` (sampled evenly over one cycle) as a constant
    and, for each wave k, its cosine and sine parts: value(u) = a0 +
    sum(c_k cos(2 pi k u) + s_k sin(2 pi k u)), u through the cycle (0..1).
    (rows: a0, c_1, s_1, c_2, s_2, ...)."""
    _, f = fit(values, waves)
    rows = [f[0].real]
    for k in range(1, waves + 1):
        rows += [2.0 * f[k].real, -2.0 * f[k].imag]
    return np.array(rows)


def evaluate(rows, u):
    """`waves_of`'s rows at `u` (as the game evaluates them)."""
    out = rows[0].copy()
    for k in range(1, (len(rows) - 1) // 2 + 1):
        a = 2.0 * np.pi * k * u
        out += rows[2 * k - 1] * np.cos(a) + rows[2 * k] * np.sin(a)
    return out


def write_gait(glb, skel, clip, duration, locals_, moving, waves):
    """`creatures/NAME.gait`: the walk's joints as waves, for the game to play
    in place of the clip (`walk=1`); and a few of the fit's own poses, for its
    test (the game's evaluation must give the same)."""
    out = glb.with_suffix(".gait")
    lines = [
        f"# {out.name}: the walk of {glb.name} ({clip}, {duration:.3f} s) as waves, fitted by",
        "# tools/gait_fit.py --write (Unuma et al. 1995). u runs through the clip, 0..1, as",
        "# the walk's line in the .creature times it. Each line: what, a joint, its numbers.",
        "# rotation: its mean turn (x y z w), then its turn from that, a rotation vector",
        "# (radians): the constant, then each wave's cosine and sine parts (3 numbers each).",
        "# translation: the same, without the mean (model units, its parent's frame).",
        "# check: a joint at u, its turn (x y z w) and place, as the fit gives them.",
        f"clip = {clip}",
        f"waves = {waves}",
    ]
    fitted = {}
    for j in moving:
        ms = locals_[j]
        qs = np.array([matrix_quat(m[:3, :3] / np.linalg.norm(m[:3, 0])) for m in ms])
        mean = mean_quat(qs)
        vec = np.array([qlog(qmul(qinv(mean), q)) for q in qs])
        rot = waves_of(vec, waves)
        name = skel.names[j]
        lines.append(f"rotation | {name} | " + " ".join(f"{v:.7g}" for v in list(mean) + list(rot.ravel())))
        moves = np.ptp(ms[:, :3, 3], axis=0).max() > 1e-6
        tra = waves_of(ms[:, :3, 3], waves) if moves else None
        if moves:
            lines.append(f"translation | {name} | " + " ".join(f"{v:.7g}" for v in tra.ravel()))
        fitted[j] = (mean, rot, tra)
        # (The formula, as written, against the fit it came from.)
        rebuilt, _ = fit(vec, waves)
        for k in range(0, len(ms), 17):
            assert np.allclose(evaluate(rot, k / len(ms)), rebuilt[k], atol=1e-6), "the waves do not give back the fit"
    for j in moving[:: max(1, len(moving) // 6)]:
        mean, rot, tra = fitted[j]
        for u in (0.0, 0.37, 0.81):
            q = qmul(mean, qexp(evaluate(rot, u)))
            t = evaluate(tra, u) if tra is not None else locals_[j][0][:3, 3]
            lines.append(f"check | {skel.names[j]} | {u} " + " ".join(f"{v:.7g}" for v in list(q) + list(t)))
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {out}: {len(moving)} joints, {waves} waves")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("glb")
    p.add_argument("--waves", type=int, default=8, help="up to how many waves to try (8)")
    p.add_argument("--write", type=int, metavar="WAVES", help="write the walk with this many waves to creatures/NAME.gait, for the game (walk=1)")
    a = p.parse_args()
    glb = Path(a.glb)
    gltf, blob = read_glb(glb)
    skel = Skeleton(gltf, blob)
    kept = read_kept(glb.with_suffix(".creature"))
    if "height" not in kept or "model_height" not in kept:
        sys.exit(f"run tools/creature_clips.py {glb} first: its height is needed")
    cm = float(kept["height"]) / float(kept["model_height"]) * 100.0
    find = lambda role, side=None: skel.find(role, side)
    points = {"feet": [find("foot", "L"), find("foot", "R")], "hands": [find("hand", "L"), find("hand", "R")], "head": [find("head")]}
    if any(j is None for js in points.values() for j in js):
        sys.exit("feet, hands or head not found: run tools/creature_clips.py first")
    order = [j for js in points.values() for j in js]

    for role in ("walk", "run"):
        clip = kept.get(role, "").split()[0] if kept.get(role) else clip_named(skel, role, None)
        times, duration, locals_, seam = sample(skel, clip)
        moving = [j for j in skel.joints if np.ptp(locals_[j].reshape(len(times), -1), axis=0).max() > 1e-6]
        if a.write and role == "walk":
            write_gait(glb, skel, clip, duration, locals_, moving, a.write)
        truth = worlds(skel, locals_, order)
        print(f"\n{role} ({clip}, {duration:.3f} s, {len(moving)} joints moving)")
        gap = max(angle(a_, b_) for a_, b_ in seam.values())
        jump = max(np.linalg.norm((b_ - a_)[:3, 3]) for a_, b_ in seam.values()) * cm
        print(f"  its seam (end against start): joints up to {gap:.1f} deg apart, moved up to {jump:.1f} cm")
        # How much of each joint's turning lies in each wave (its energy).
        energy = np.zeros(a.waves + 1)
        for j in moving:
            ms = locals_[j]
            qs = np.array([matrix_quat(m[:3, :3] / np.linalg.norm(m[:3, 0])) for m in ms])
            mean = mean_quat(qs)
            vec = np.array([qlog(qmul(qinv(mean), q)) for q in qs])
            f = np.fft.rfft(vec - vec.mean(axis=0), axis=0)
            e = (np.abs(f) ** 2).sum(axis=1)
            energy += e[: a.waves + 1]
        total = energy[1:].sum()
        print("  share of all turning in each wave: " + "  ".join(f"{k}:{energy[k] / total:.1%}" for k in range(1, a.waves + 1)))
        # Its feet's sliding while down, as `creature_clips.py` measures it:
        # played at its measured speed, how fast a planted foot moves (m/s).
        speed = float(kept[role].split()[1])
        way = np.array([float(v) for v in kept["forward"].split()])
        dt = duration / len(times)
        tall = float(kept["model_height"])
        def slide(pos):
            out = []
            for k in range(2):
                down = truth[k, :, 1] < truth[k, :, 1].min() + 0.03 * tall
                vel = (np.roll(pos[k], -1, axis=0) - np.roll(pos[k], 1, axis=0)) / (2 * dt)
                out.extend(np.linalg.norm(vel[down][:, [0, 2]] + way * speed, axis=1))
            return float(np.median(out)) * cm / 100.0
        print(f"  the clip's feet slide {slide(truth):.2f} m/s while down")
        print("  waves | feet cm max (mean) | hands cm max | head cm max | joints deg max (mean), worst | slide m/s")
        for waves in range(1, a.waves + 1):
            fitted = rebuild(skel, {j: locals_[j] for j in moving}, waves)
            full = {**locals_, **fitted}
            got = worlds(skel, full, order)
            err = np.linalg.norm(got - truth, axis=2) * cm  # (points, samples)
            feet, hands, head = err[0:2], err[2:4], err[4:5]
            degs = np.array([[angle(fitted[j][k], locals_[j][k]) for k in range(0, len(times), 4)] for j in moving])
            worst = skel.names[moving[int(np.argmax(degs.max(axis=1)))]].replace("mixamorig:", "")
            print(f"  {waves:5d} | {feet.max():6.1f} ({feet.mean():4.1f})     | {hands.max():6.1f}       | {head.max():6.1f}      | {degs.max():5.1f} ({degs.mean():4.2f}), {worst:12s} | {slide(got[0:2]):.2f}")


if __name__ == "__main__":
    main()
