"""Procedural lattice head: egg shell + weighted surface Voronoi + chamfered ribbon struts, unioned, as GLB.

Recipe: an egg-shaped shell with the concept's proportions; seed points on it, two of them heavy
(the big side rings); a power diagram on the shell gives the cells; straight ribbons along the cell
edges with convex joints, all unioned; everything below a cut height removed so the jaw hangs open;
struts that point steeply down at the cut carry on to the floor as legs.
"""
import argparse, json, numpy as np, trimesh
from collections import defaultdict

DEFAULTS = dict(
    head_top=1.0, head_bottom=0.14,      # the egg spans these heights (floor = 0, crown = 1)
    width=0.386, depth=0.547,            # full width / depth of the egg
    taper=0.5,                           # > 0 narrows the bottom of the egg into a chin
    shear=0.06,                          # pushes the crown back, skull-like
    bump_n=6, bump_amp=0.05, bump_sigma=0.6,   # low-frequency lumps on the shell (count, radial amplitude, width in rad)
    cut=0.28,                            # struts below this height are removed (open jaw)
    n_cells=40, seed=1,
    big_dirs=[[1, 0.1, 0.0], [-1, 0.1, 0.0]],   # directions of the two big side cells
    big_weight=0.02, big_clear=0.75, min_sep=0.25,    # power weight; angular clearances (rad)
    weight_sigma=0.004,                  # normal-distributed power weights for the small cells: wide size range
    lloyd=2,
    strut_w=0.05, strut_t=0.025, chamfer=0.45,         # ribbon width / thickness; chamfer as a fraction of t
    ring_scale=1.3, leg_scale=1.2,       # width multipliers for struts bordering a big cell, and for legs
    size_power=0.35, size_clamp=[0.6, 1.5],   # strut width follows the size of the cells it borders
    swell=1.25,                          # ribbon ends widen at junctions
    simplify=0.02, prong_taper=0.35,
    leg_azimuths=[45, 135, 225, 315], leg_band=0.14, leg_sector_cos=0.5,   # legs: sectors, how far above the cut to look
    leg_splay=0.05, floor=0.0,
    union=True, subdiv=5,
)


def shell(d, P):
    """Map unit directions (N,3) to points on the egg."""
    a, b, c = P["width"] / 2, (P["head_top"] - P["head_bottom"]) / 2, P["depth"] / 2
    cy = (P["head_top"] + P["head_bottom"]) / 2
    x, y, z = d[:, 0] * a, d[:, 1] * b, d[:, 2] * c
    s = 1 + P["taper"] * d[:, 1]
    x, z = x * s, z * s
    if "_bumps" in P:                                  # lumps: radial scale from a few gaussian bumps
        centers, amps, sigma = P["_bumps"]
        ang = np.arccos(np.clip(d @ centers.T, -1, 1))
        r = 1 + (amps[None, :] * np.exp(-0.5 * (ang / sigma) ** 2)).sum(1)
        x, y, z = x * r, y * r, z * r
    z = z - P["shear"] * d[:, 1] * b
    return np.stack([x, y + cy, z], 1)


def to_dir(p, P):
    a, b, c = P["width"] / 2, (P["head_top"] - P["head_bottom"]) / 2, P["depth"] / 2
    cy = (P["head_top"] + P["head_bottom"]) / 2
    d = np.stack([p[:, 0] / a, (p[:, 1] - cy) / b, p[:, 2] / c], 1)
    return d / np.linalg.norm(d, axis=1, keepdims=True)


def unit(v):
    return v / np.linalg.norm(v)


def sample_seeds(P, rng):
    big = np.array([unit(np.array(d, float)) for d in P["big_dirs"]])
    pts = []
    for _ in range(50000):
        if len(pts) >= P["n_cells"]:
            break
        v = unit(rng.normal(size=3))
        if any(np.arccos(np.clip(v @ b, -1, 1)) < P["big_clear"] for b in big):
            continue
        if any(np.arccos(np.clip(v @ q, -1, 1)) < P["min_sep"] for q in pts):
            continue
        pts.append(v)
    if len(pts) < P["n_cells"]:
        print(f"warning: only {len(pts)} of {P['n_cells']} seeds fit")
    return np.vstack([big, np.array(pts)]), len(big)


def labels_for(verts, seeds, weights):
    d2 = ((verts[:, None, :] - seeds[None, :, :]) ** 2).sum(-1) - weights[None, :]
    return d2.argmin(1), d2


def douglas_peucker(pts, eps):
    if len(pts) < 3:
        return pts
    a, b = pts[0], pts[-1]
    ab = b - a
    L = np.linalg.norm(ab)
    if L < 1e-12:
        d = np.linalg.norm(pts - a, axis=1)
    else:
        d = np.linalg.norm(np.cross(pts - a, ab), axis=1) / L
    i = int(d.argmax())
    if d[i] > eps:
        return np.vstack([douglas_peucker(pts[:i + 1], eps)[:-1], douglas_peucker(pts[i:], eps)])
    return np.vstack([a, b])


def ribbon(p, q, n, w, t, ch, scale_p=1.0, scale_q=1.0):
    """A chamfered ribbon from p to q: width w along the surface, thickness t along the normal n."""
    u = unit(q - p)
    bn = unit(np.cross(n, u))
    nn = np.cross(u, bn)
    c = ch * t
    prof = [(-w / 2 + c, -t / 2), (w / 2 - c, -t / 2), (w / 2, -t / 2 + c), (w / 2, t / 2 - c),
            (w / 2 - c, t / 2), (-w / 2 + c, t / 2), (-w / 2, t / 2 - c), (-w / 2, -t / 2 + c)]

    def ring(cen, s):
        return np.array([cen + (bn * x + nn * y) * s for x, y in prof])
    rp, rq = ring(p, scale_p), ring(q, scale_q)
    return trimesh.convex.convex_hull(np.vstack([rp, rq])), rp, rq


def build(P):
    rng = np.random.default_rng(P["seed"])
    P = dict(P)
    if P["bump_n"] > 0:
        centers = rng.normal(size=(P["bump_n"], 3))
        centers /= np.linalg.norm(centers, axis=1, keepdims=True)
        P["_bumps"] = (centers, rng.uniform(-1, 1, P["bump_n"]) * P["bump_amp"], P["bump_sigma"])
    sph = trimesh.creation.icosphere(subdivisions=P["subdiv"])
    verts = shell(sph.vertices.copy(), P)
    faces = sph.faces
    shell_mesh = trimesh.Trimesh(verts, faces, process=False)
    vnorm = shell_mesh.vertex_normals
    seed_dirs, nbig = sample_seeds(P, rng)
    seeds = shell(seed_dirs, P)
    weights = np.zeros(len(seeds))
    weights[:nbig] = P["big_weight"]
    weights[nbig:] = rng.normal(0, P["weight_sigma"], len(seeds) - nbig)
    for _ in range(P["lloyd"]):                        # relax the small cells, big seeds fixed
        lab, _ = labels_for(verts, seeds, weights)
        for i in range(nbig, len(seeds)):
            m = lab == i
            if m.any():
                seeds[i] = shell(to_dir(verts[m].mean(0, keepdims=True), P), P)[0]
    lab, d2 = labels_for(verts, seeds, weights)
    counts = np.bincount(lab, minlength=len(seeds)).astype(float)   # cell sizes, as vertex counts
    small = counts[nbig:][counts[nbig:] > 0]
    median_size = np.median(small) if len(small) else 1.0

    # ---- march the cell boundaries over the shell triangles
    nodes, nodenorm, nodelab, node_id = [], [], {}, {}

    def node(key, pos, nrm, labpair=None):
        if key not in node_id:
            node_id[key] = len(nodes)
            nodes.append(pos)
            nodenorm.append(unit(nrm))
            if labpair is not None:
                nodelab[node_id[key]] = labpair
        return node_id[key]
    segs = set()
    for fi, (i0, i1, i2) in enumerate(faces):
        tri = (i0, i1, i2)
        L = (lab[i0], lab[i1], lab[i2])
        if L[0] == L[1] == L[2]:
            continue
        cross = []
        for a, b in ((0, 1), (1, 2), (2, 0)):
            va, vb = tri[a], tri[b]
            if L[a] == L[b]:
                continue
            g_a = d2[va, L[a]] - d2[va, L[b]]      # <= 0 at a
            g_b = d2[vb, L[a]] - d2[vb, L[b]]      # >= 0 at b
            t = g_a / (g_a - g_b) if g_a != g_b else 0.5
            key = (min(va, vb), max(va, vb))
            cross.append(node(key, verts[va] + t * (verts[vb] - verts[va]),
                              vnorm[va] + t * (vnorm[vb] - vnorm[va]), (min(L[a], L[b]), max(L[a], L[b]))))
        if len(cross) == 2:
            segs.add((min(cross), max(cross)))
        else:
            c = np.mean([nodes[k] for k in cross], 0)
            j = node(("f", fi), c, shell_mesh.face_normals[fi])
            for k in cross:
                segs.add((min(j, k), max(j, k)))
    nodes = np.array(nodes)
    nodenorm = np.array(nodenorm)
    adj = defaultdict(set)
    for a, b in segs:
        adj[a].add(b)
        adj[b].add(a)

    # ---- walk chains between junctions, straighten them
    chains, used = [], set()
    starts = [n for n in adj if len(adj[n]) != 2] or list(adj)[:1]
    for s in starts:
        for nb in list(adj[s]):
            e = (min(s, nb), max(s, nb))
            if e in used:
                continue
            path = [s, nb]
            used.add(e)
            while len(adj[path[-1]]) == 2:
                prev, cur = path[-2], path[-1]
                nxt = next(iter(adj[cur] - {prev}), None)
                if nxt is None:
                    break
                e = (min(cur, nxt), max(cur, nxt))
                if e in used:
                    break
                used.add(e)
                path.append(nxt)
                if nxt == s:
                    break
            pts = douglas_peucker(nodes[path], P["simplify"])
            nrm = nodenorm[path]
            keep = [int(np.argmin(np.linalg.norm(nodes[path] - q, axis=1))) for q in pts]
            labpair = next((nodelab[k] for k in path if k in nodelab), (-1, -1))
            chains.append(dict(pts=pts, nrm=nrm[keep], lab=labpair))

    # ---- cut below the jaw line; pieces ending at the cut dangle
    cut = P["cut"]
    pieces = []
    for ch in chains:
        pts, nrm = ch["pts"], ch["nrm"]
        cur_p, cur_n = [], []
        for i in range(len(pts)):
            p = pts[i]
            if i > 0:
                q = pts[i - 1]
                if (q[1] - cut) * (p[1] - cut) < 0:
                    t = (cut - q[1]) / (p[1] - q[1])
                    x = q + t * (p - q)
                    xn = unit(nrm[i - 1] + t * (nrm[i] - nrm[i - 1]))
                    if p[1] >= cut:
                        cur_p, cur_n = [x], [xn]
                    else:
                        cur_p.append(x)
                        cur_n.append(xn)
                        pieces.append(dict(pts=np.array(cur_p), nrm=np.array(cur_n), lab=ch["lab"], kind="prong"))
                        cur_p, cur_n = [], []
            if p[1] >= cut:
                cur_p.append(p)
                cur_n.append(nrm[i])
        if len(cur_p) > 1:
            pieces.append(dict(pts=np.array(cur_p), nrm=np.array(cur_n), lab=ch["lab"], kind="strut"))

    # ---- legs: pillars from the lowest lattice node in each requested azimuth sector down to the floor
    low = np.vstack([pc["pts"] for pc in pieces])
    low = low[low[:, 1] < cut + P["leg_band"]]
    taken = []
    for az in P["leg_azimuths"]:
        target = np.array([np.sin(np.radians(az)), 0, np.cos(np.radians(az))])
        hz = low[:, [0, 2]]
        score = (hz @ target[[0, 2]]) / (np.linalg.norm(hz, axis=1) + 1e-9)   # cosine to the sector direction
        order = np.argsort(-score)
        for i in order:
            if score[i] < P["leg_sector_cos"]:
                break
            if all(np.linalg.norm(low[i] - t) > 0.05 for t in taken):
                taken.append(low[i])
                break
    for n in taken:
        foot = np.array([n[0] * (1 + P["leg_splay"]), P["floor"], n[2] * (1 + P["leg_splay"])])
        hn = unit(np.array([n[0], 0, n[2]]))
        pieces.append(dict(pts=np.array([n, foot]), nrm=np.array([hn, hn]), lab=(-1, -1), kind="leg"))

    # ---- geometry: ribbons per segment, convex-hull joints, union
    meshes, joints = [], defaultdict(list)

    def key(p):
        return tuple(np.round(p / 1e-5).astype(int))
    for pc in pieces:
        pts, nrm = pc["pts"], pc["nrm"]
        w = P["strut_w"]
        if pc["kind"] == "leg":
            w *= P["leg_scale"]
        elif min(pc["lab"]) >= 0:
            if min(pc["lab"]) < nbig:
                w *= P["ring_scale"]
            else:
                rel = counts[list(pc["lab"])].mean() / median_size
                w *= float(np.clip(rel ** P["size_power"], *P["size_clamp"]))
        last = len(pts) - 2
        for i in range(len(pts) - 1):
            p, q = pts[i], pts[i + 1]
            if np.linalg.norm(q - p) < 1e-6:
                continue
            n = unit(nrm[i] + nrm[i + 1])
            sp = P["swell"] if (i == 0 and pc["kind"] != "leg") else 1.0
            if pc["kind"] == "prong" and i == last:
                sq = P["prong_taper"]
            elif pc["kind"] == "strut" and i == last:
                sq = P["swell"]
            else:
                sq = 1.0
            m, rp, rq = ribbon(p, q, n, w, P["strut_t"], P["chamfer"], sp, sq)
            meshes.append(m)
            joints[key(p)].append(rp)
            joints[key(q)].append(rq)
    failed = 0
    for k, rects in joints.items():
        if len(rects) >= 2:
            try:
                meshes.append(trimesh.convex.convex_hull(np.vstack(rects)))
            except Exception:
                failed += 1
    if P["union"]:
        try:
            out = trimesh.boolean.union(meshes, engine="manifold")
        except Exception as e:
            print("union failed, concatenating:", str(e).splitlines()[0][:100])
            out = trimesh.util.concatenate(meshes)
    else:
        out = trimesh.util.concatenate(meshes)
    kinds = defaultdict(int)
    for pc in pieces:
        kinds[pc["kind"]] += 1
    stats = dict(cells=int(len(seeds)), chains=len(chains), pieces=dict(kinds), struts=len(meshes),
                 joint_failures=failed, faces=int(len(out.faces)), watertight=bool(out.is_watertight),
                 extents=[round(float(v), 3) for v in out.extents])
    return out, stats


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--set", nargs="*", default=[], help="param=value (JSON values)")
    a = ap.parse_args()
    P = dict(DEFAULTS)
    for kv in a.set:
        k, v = kv.split("=", 1)
        if k not in P:
            raise SystemExit(f"unknown parameter {k}; known: {', '.join(P)}")
        P[k] = json.loads(v)
    mesh, stats = build(P)
    mesh.export(a.out)
    print(stats, "->", a.out)
