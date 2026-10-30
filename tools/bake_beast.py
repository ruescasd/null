"""Bake an authored creature mesh into a harrow's body and build:
data/harrow_mesh.bin (see crates/game/src/combat/beast_mesh.rs).

    python tools/bake_beast.py SOURCE.glb [data/harrow_mesh.bin]

The source: one creature, standing, y up, its head towards +z, four legs
(a stray piece not joined to the body, such as the shard beast's fifth
limb, is left out). Needs numpy, scipy, trimesh, fast-simplification.

The creature is rigged to the mesh, not the mesh to a rig (fitted onto the
harrow's own build, its forelegs stretched 2.6 times into slabs, its thighs
left on its rump, the user, 2026-10-29): its build measured from the mesh,
its body at SCALE, its anchors in the body's core, each leg from where it
meets the body (its root, below the core: `drop`), its length traced foot
to root.
Each face is given to a bone: a leg's upper or lower half (cut from the body
at one height for both of a pair), else the torso's three bones, the neck or
the head by how far along the body it lies. Written: the build (the game's
`Plan` from it), each bone's segment as the mesh has it, and each bone's
triangles where the mesh has them (with normals smooth within CREASE), all
in the body's frame (m: x right, y up from the ground, z forward from the
middle between hips and chest). The game stands its rig up and moves each
piece from its segment onto its bone."""
import struct
import sys

import fast_simplification
import numpy as np
import trimesh
from scipy.cluster.hierarchy import fcluster, linkage

TARGET = 40000              # triangles kept (of 1.6 million; 15000 lost the thin blades)
SCALE = 1.5                 # metres a mesh unit
LEG_REACH = 0.14            # how near a leg's traced line a face is the leg's
CREASE = 35.0               # degrees: an edge sharper stays hard
SLACK = 0.9                 # the legs' reach the body stands at (room to heave and step)
ISLAND = 400                # faces: a smaller island off its piece is a stray
# This source's own: where its neck and its head begin, and the head's
# thickness (mesh units, along z).
NECK_Z, HEAD_Z, HEAD_RADIUS = 0.45, 0.62, 0.15
TORSO_RADIUS = 0.3          # its torso's thickness about its core (mesh units)


def legs_of(v):
    """Each leg's line, foot to root: its tip the lowest point of a cluster
    of low vertices, one per quarter (left/right, fore/hind)."""
    low = v[v[:, 1] < v[:, 1].min() + 0.35]
    labels = fcluster(linkage(low[:, [0, 2]], 'single'), 0.08, 'distance')
    tips = {}
    for k in np.unique(labels):
        p = low[labels == k]
        tip = p[np.argmin(p[:, 1])]
        name = ('fore_' if tip[2] > -0.1 else 'hind_') + ('left' if tip[0] < 0 else 'right')
        if name not in tips or tip[1] < tips[name][1]:
            tips[name] = tip
    assert len(tips) == 4, f'found the feet {sorted(tips)}: four legs wanted, one a quarter'
    lines = {}
    for name, tip in tips.items():
        centre, line = tip[[0, 2]], [tip]
        for y in np.arange(tip[1] + 0.04, 0.45, 0.04):
            band = v[(np.abs(v[:, 1] - y) < 0.02) & (np.linalg.norm(v[:, [0, 2]] - centre, axis=1) < 0.16)]
            if len(band) < 20:
                break
            c = band[:, [0, 2]].mean(0)
            line.append(np.array([c[0], y, c[1]]))
            # (Where the leg meets the body its slice suddenly widens.)
            if np.linalg.norm(band[:, [0, 2]] - c, axis=1).max() > 0.15 and y > -0.2:
                break
            centre = c
        lines[name] = np.array(line)
    return lines


def crease_normals(V, F):
    """Each corner's normal: its face's, smoothed with the faces round its
    vertex turned less than CREASE from it (by area)."""
    a, b, c = V[F[:, 0]], V[F[:, 1]], V[F[:, 2]]
    n = np.cross(b - a, c - a)
    area = np.linalg.norm(n, axis=1, keepdims=True)
    unit = n / np.maximum(area, 1e-12)
    vf = trimesh.Trimesh(V, F, process=False).vertex_faces
    out = np.zeros((len(F), 3, 3))
    cos = np.cos(np.radians(CREASE))
    for k in range(3):
        around = vf[F[:, k]]
        ok = around >= 0
        near = unit[np.where(ok, around, 0)]
        keep = ok & (np.einsum('fjd,fd->fj', near, unit) > cos)
        s = (n[np.where(ok, around, 0)] * keep[..., None]).sum(1)
        out[:, k] = s / np.maximum(np.linalg.norm(s, axis=1, keepdims=True), 1e-12)
    return out


def islands(mesh, owner):
    """Each bone's stray islands (faces not joined to the rest of its piece,
    fewer than ISLAND of them) given to the bone they border most: left on
    the torso, a shard of a shoulder hung in the air under the chest once
    the leg swung away from it."""
    from scipy.sparse import coo_matrix
    from scipy.sparse.csgraph import connected_components
    pairs = mesh.face_adjacency
    for _ in range(3):
        same = pairs[owner[pairs[:, 0]] == owner[pairs[:, 1]]]
        n = len(owner)
        graph = coo_matrix((np.ones(len(same)), (same[:, 0], same[:, 1])), shape=(n, n))
        _, label = connected_components(graph, directed=False)
        sizes = np.bincount(label)
        biggest = {}
        for bone in np.unique(owner):
            ls = np.unique(label[owner == bone])
            biggest[bone] = ls[np.argmax(sizes[ls])]
        moved = 0
        for l in np.where(sizes < ISLAND)[0]:
            faces = np.where(label == l)[0]
            bone = owner[faces[0]]
            if biggest[bone] == l:
                continue
            edge = pairs[np.isin(pairs[:, 0], faces) ^ np.isin(pairs[:, 1], faces)]
            other = owner[edge.ravel()]
            other = other[other != bone]
            if len(other):
                owner[faces] = np.bincount(other).argmax()
                moved += len(faces)
        print(f'islands: {moved} faces given to the bone they border')
        if not moved:
            break


def lerp_neck(t):
    """The plan's neck direction (forward, up) for the neck to rest along
    `t`: the rig turns it 0.4 of the way to where the head looks (level)."""
    k = 0.4 * t[0] + np.sqrt(0.16 * t[0] ** 2 + 0.2)
    b = (k * t - np.array([0.4, 0.0])) / 0.6
    return b / np.linalg.norm(b)


def bake(source, target):
    full = trimesh.load(source, force='mesh')
    centre = full.bounding_box.centroid
    body = max(full.split(only_watertight=False), key=lambda p: len(p.faces))
    lines = legs_of(body.vertices - centre)
    v, f = fast_simplification.simplify(body.vertices.astype(np.float32), body.faces.astype(np.int32), target_count=TARGET)
    mesh = trimesh.Trimesh(v, f, process=True)
    V, F = mesh.vertices - centre, mesh.faces
    mid = V[F].mean(1)
    ground = np.mean([l[0][1] for l in lines.values()])
    pairs = {'fore': ('fore_left', 'fore_right'), 'hind': ('hind_left', 'hind_right')}
    root = {p: np.mean([lines[n][-1] for n in names], 0) for p, names in pairs.items()}

    def core(z):
        """The body's core height at `z`: the middle of its faces near the
        midline there."""
        s = (np.abs(mid[:, 2] - z) < 0.05) & (np.abs(mid[:, 0]) < 0.12) & (mid[:, 1] > root['fore'][1])
        return np.median(mid[s, 1])

    # The build, in mesh units: hips and chest in the core over the legs'
    # roots; each leg from its root (at its pair's side, as high as the
    # pair's, where the mesh's leg meets the body) to its own knee and foot.
    # (From the core, a leg's bone ran its first 0.85 m inside the body, no
    # leg on it: swung out, the leg hung off the body, the user.)
    anchor = {p: np.array([0.0, core(root[p][2]), root[p][2]]) for p in pairs}
    drop = {p: anchor[p][1] - root[p][1] for p in pairs}
    z_mid = (anchor['fore'][2] + anchor['hind'][2]) / 2
    knee, legs = {}, {}
    for p, names in pairs.items():
        side = np.mean([abs(lines[n][-1][0]) for n in names])
        for n in names:
            line = lines[n]
            cum = np.concatenate([[0], np.cumsum(np.linalg.norm(line[1:] - line[:-1], axis=1))])
            knee[n] = line[np.searchsorted(cum, cum[-1] / 2)]
            hip = anchor[p] + np.array([side if line[0][0] > 0 else -side, -drop[p], 0.0])
            legs[n] = (hip, knee[n], line[0])
    upper = {p: np.mean([np.linalg.norm(legs[n][1] - legs[n][0]) for n in names]) for p, names in pairs.items()}
    lower = {p: np.mean([np.linalg.norm(legs[n][2] - legs[n][1]) for n in names]) for p, names in pairs.items()}
    side = {p: abs(legs[names[1]][0][0]) for p, names in pairs.items()}
    spread = {p: np.mean([abs(legs[n][2][0]) for n in names]) - side[p] for p, names in pairs.items()}
    # (Standing a little lower than the mesh, its knees bent: at full reach it
    # could neither heave nor step.)
    height = {p: np.sqrt((SLACK * (upper[p] + lower[p])) ** 2 - spread[p] ** 2) + drop[p] for p in pairs}
    rump_z = np.percentile(mid[(np.abs(mid[:, 0]) < 0.12) & (mid[:, 1] > root['hind'][1] + 0.05), 2], 1)
    rump = anchor['hind'][2] - rump_z
    head_start = np.array([0.0, core(HEAD_Z), HEAD_Z])
    head_len = V[:, 2].max() - HEAD_Z
    neck = head_start - anchor['fore']
    neck_dir = lerp_neck(np.array([neck[2], neck[1]]) / np.linalg.norm(neck[[2, 1]]))

    # Each bone's segment as the mesh has it (the rig's formulas: its rump
    # behind the hips and a little up; its torso three segments to the chest).
    rump_at = anchor['hind'] + np.array([0.0, rump * 0.35, -rump])
    segs = {k: (rump_at + (anchor['fore'] - rump_at) * k / 3, rump_at + (anchor['fore'] - rump_at) * (k + 1) / 3) for k in range(3)}
    segs[3] = (anchor['fore'], head_start)
    segs[4] = (head_start, head_start + np.array([0.0, 0.0, head_len]))
    bones = {'fore_left': (5, 6), 'fore_right': (7, 8), 'hind_left': (9, 10), 'hind_right': (11, 12)}
    for n, (up, lo) in bones.items():
        segs[up], segs[lo] = legs[n][:2], legs[n][1:]

    # Each face's bone: a leg's below its pair's root, near its line; the
    # rest along the body.
    owner, best = np.full(len(F), -1), np.full(len(F), np.inf)
    for p, names in pairs.items():
        for n in names:
            line, (up, lo) = lines[n], bones[n]
            lengths = np.linalg.norm(line[1:] - line[:-1], axis=1)
            cum = np.concatenate([[0], np.cumsum(lengths)])
            dist, at = np.full(len(F), np.inf), np.zeros(len(F))
            for k in range(len(line) - 1):
                e = line[k + 1] - line[k]
                t = np.clip(((mid - line[k]) @ e) / max(e @ e, 1e-9), 0, 1)
                dd = np.linalg.norm(mid - (line[k] + np.outer(t, e)), axis=1)
                nearer = dd < dist
                dist[nearer], at[nearer] = dd[nearer], cum[k] + t[nearer] * lengths[k]
            take = (dist < LEG_REACH) & (mid[:, 1] < root[p][1] + 0.02) & (dist < best)
            best[take] = dist[take]
            owner[take & (at <= cum[-1] / 2)], owner[take & (at > cum[-1] / 2)] = lo, up
    cuts = [-np.inf, segs[0][1][2], segs[1][1][2], NECK_Z, HEAD_Z, np.inf]
    axial = owner < 0
    for bone in range(5):
        owner[axial & (mid[:, 2] >= cuts[bone]) & (mid[:, 2] < cuts[bone + 1])] = bone
    islands(mesh, owner)

    to_body = lambda p: (np.asarray(p) - np.array([0.0, ground, z_mid])) * SCALE
    normals = crease_normals(V, F)
    plan = [
        (anchor['hind'][2] - z_mid) * SCALE, height['hind'] * SCALE,
        (anchor['fore'][2] - z_mid) * SCALE, height['fore'] * SCALE,
        rump * SCALE, TORSO_RADIUS * SCALE,
        np.linalg.norm(neck) * SCALE, neck_dir[0], neck_dir[1],
        head_len * SCALE, HEAD_RADIUS * SCALE,
        side['fore'] * SCALE, upper['fore'] * SCALE, lower['fore'] * SCALE, spread['fore'] * SCALE,
        side['hind'] * SCALE, upper['hind'] * SCALE, lower['hind'] * SCALE, spread['hind'] * SCALE,
        drop['fore'] * SCALE, drop['hind'] * SCALE,
    ]
    with open(target, 'wb') as fh:
        fh.write(b'BEAST3\0\0')
        fh.write(struct.pack(f'<{len(plan)}f', *plan))
        for bone in range(13):
            fh.write(np.concatenate([to_body(segs[bone][0]), to_body(segs[bone][1])]).astype('<f4').tobytes())
        for bone in range(13):
            pick = owner == bone
            assert pick.any(), f'bone {bone} got no piece of the mesh'
            corners = np.concatenate([to_body(V[F[pick]].reshape(-1, 3)), normals[pick].reshape(-1, 3)], 1)
            fh.write(struct.pack('<I', int(pick.sum())))
            fh.write(corners.astype('<f4').tobytes())
            print(f'bone {bone:2}: {pick.sum()} triangles')
    print('build:', ', '.join(f'{x:.3f}' for x in plan))
    print(f'baked {len(F)} triangles into {target}')


if __name__ == '__main__':
    bake(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else 'data/harrow_mesh.bin')
