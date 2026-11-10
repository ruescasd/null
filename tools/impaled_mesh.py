"""The impaled brute's mesh, made from Meshy's (the user, 2026-11-09: "a
better mesh, keep it as default"): the back flattened (the buttocks out of
place in this world: one smooth surface from the belt to the thighs, the
normal map's relief there faded flat), then the spikes cut off, their holes
capped, and faceted shards grown in their place (shorter, broken off, some
with a splinter), in their own pale material; and the feet's soles made
four-pointed stars, flat-shaded in facets (the paw's toes were too cute).

    python tools/impaled_mesh.py [IN.glb] [OUT.glb] [--seed N]

Defaults: creatures/Meshy_AI_Refine_the_impaled_cr_1105151647_texture.glb to
creatures/impaled.glb. The numbers (where the back is, the shards' sizes)
are this mesh's; a regenerated mesh needs them looked at again."""
import sys, os, tempfile
from collections import defaultdict
import numpy as np, trimesh


# The back.
def flatten(inp, out):
    s = trimesh.load(inp, force="scene", process=False)
    name, g = next(iter(s.geometry.items()))
    V0 = np.asarray(g.vertices); F = np.asarray(g.faces); N0 = np.asarray(g.vertex_normals).copy()
    P, inv = np.unique(V0.round(6), axis=0, return_inverse=True)
    inv = inv.reshape(-1)
    m = trimesh.Trimesh(P, inv[F], process=False)
    x, y, z = P[:, 0], P[:, 1], P[:, 2]
    def smooth(e0, e1, t):
        u = np.clip((t - e0) / (e1 - e0), 0, 1); return u * u * (3 - 2 * u)
    target = -0.365 + 1.0 * x * x
    w = smooth(0.32, 0.2, np.abs(x)) * smooth(0.08, 0.0, y) * smooth(-0.5, -0.34, y)
    w *= smooth(-0.02, -0.12, z)
    crotch = y < -0.26
    w[crotch] *= smooth(0.03, 0.09, np.abs(x[crotch])) * smooth(-0.15, -0.25, z[crotch])
    Q = P.copy()
    Q[:, 2] = z + w * (target - z)
    nb = m.vertex_neighbors
    moved = np.where(w > 0.02)[0]
    for _ in range(8):
        Qn = Q.copy()
        for i in moved:
            Qn[i] = Q[i] + 0.6 * w[i] * (Q[nb[i]].mean(axis=0) - Q[i])
        Q = Qn
    n0 = trimesh.Trimesh(P, inv[F], process=False).face_normals
    n1 = trimesh.Trimesh(Q, inv[F], process=False).face_normals
    flipped = ((n0 * n1).sum(1) < 0).sum()
    # Normals: the file's own where nothing moved; where it did, one normal per
    # position (all its seam copies), from the faces round it.
    Nq = trimesh.Trimesh(Q, inv[F], process=False).vertex_normals
    N = N0.copy()
    changed = np.abs(Q - P).max(axis=1)[inv] > 1e-7
    # (Within a ring of what moved too, so the shading joins.)
    ring = np.zeros(len(P), bool); ring[moved] = True
    for _ in range(2):
        ring[np.unique(np.concatenate([nb[i] for i in np.where(ring)[0]]))] = True
    fix = ring[inv]
    N[fix] = Nq[inv[fix]]
    g.vertices = Q[inv]
    g.vertex_normals = N
    # The normal map's relief there (the old curves) faded to flat: each face's
    # patch of the map blended to the neutral normal by how far it was moved.
    from PIL import Image, ImageDraw, ImageFilter
    mat = g.visual.material
    nmap = mat.normalTexture.convert("RGB")
    W, H = nmap.size
    mask = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(mask)
    uv = np.asarray(g.visual.uv)
    fw = w[inv[F]].max(axis=1)
    for f in np.where(fw > 0.02)[0]:
        pts = [(uv[i, 0] * W, (1 - uv[i, 1]) * H) for i in F[f]]
        draw.polygon(pts, fill=int(255 * min(1.0, fw[f] * 1.5)))
    mask = mask.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(3))
    flat = Image.new("RGB", (W, H), (128, 128, 255))
    mat.normalTexture = Image.composite(flat, nmap, mask)
    print("normal map: faded over", int((np.asarray(mask) > 8).sum()), "texels")
    s.export(out)
    print("moved", len(moved), "positions; max shift", np.abs(Q - P).max().round(3), "; faces turned over:", flipped)


# The spikes, found by their pale texture.
def measure_spikes(inp):
    s = trimesh.load(inp, force="scene", process=False)
    g = next(iter(s.geometry.values()))
    V0, F = np.asarray(g.vertices), np.asarray(g.faces)
    P, inv = np.unique(V0.round(6), axis=0, return_inverse=True); inv = inv.reshape(-1)
    Fp = inv[F]
    uv = np.asarray(g.visual.uv)
    tex = np.asarray(g.visual.material.baseColorTexture.convert("RGB")).astype(float) / 255
    th, tw = tex.shape[:2]
    c = uv[F].mean(axis=1)
    bright = tex[np.clip(((1 - c[:, 1]) * th).astype(int), 0, th - 1), np.clip((c[:, 0] * tw).astype(int), 0, tw - 1)].mean(axis=1) > 0.25
    m = trimesh.Trimesh(P, Fp, process=False)
    adj = m.face_adjacency
    both = bright[adj[:, 0]] & bright[adj[:, 1]]
    groups = trimesh.graph.connected_components(adj[both], nodes=np.where(bright)[0], min_len=1)
    body_pos = set(np.unique(Fp[~bright]).tolist())
    out = []
    for gi, fs in enumerate(sorted(groups, key=len, reverse=True)):
        vs = np.unique(Fp[fs])
        pts = P[vs]
        base = np.array([v for v in vs if v in body_pos])
        if len(base) < 3 or len(fs) < 6:
            out.append(dict(faces=int(len(fs)), base=int(len(base)), note="small or loose")); continue
        b = P[base].mean(axis=0)
        tip = pts[np.argmax(np.linalg.norm(pts - b, axis=1))]
        ax = tip - b; L = np.linalg.norm(ax); ax /= L
        r = np.linalg.norm((P[base] - b) - np.outer((P[base] - b) @ ax, ax), axis=1).mean()
        out.append(dict(faces=int(len(fs)), base=b.round(3).tolist(), dir=ax.round(3).tolist(), length=round(float(L), 3), radius=round(float(r), 4)))
    good = [o for o in out if "length" in o]
    print(len(groups), "groups,", len(good), "spikes; faces in spikes", int(bright.sum()))
    L = np.array([o["length"] for o in good]); R = np.array([o["radius"] for o in good])
    print("length m (mesh units) min/median/max", L.min().round(3), np.median(L).round(3), L.max().round(3), " base radius median", np.median(R).round(4))
    print("odd:", [o for o in out if "length" not in o][:8])
    return good


# Cut off, capped, shards in their place.
def shards(inp, spikes, out, seed):
    rng = np.random.default_rng(seed)
    s = trimesh.load(inp, force="scene", process=False)
    name, g = next(iter(s.geometry.items()))
    V, F = np.asarray(g.vertices), np.asarray(g.faces)
    N = np.asarray(g.vertex_normals)
    uv = np.asarray(g.visual.uv)
    P, inv = np.unique(V.round(6), axis=0, return_inverse=True); inv = inv.reshape(-1)
    tex = np.asarray(g.visual.material.baseColorTexture.convert("RGB")).astype(float) / 255
    th, tw = tex.shape[:2]
    c = uv[F].mean(axis=1)
    bright = tex[np.clip(((1 - c[:, 1]) * th).astype(int), 0, th - 1), np.clip((c[:, 0] * tw).astype(int), 0, tw - 1)].mean(axis=1) > 0.25
    # Only the spikes' faces go (not the pale specks elsewhere): the faces whose
    # positions lie on a measured spike's axis, within its length.
    cen = P[inv[F]].mean(axis=1)
    cut = np.zeros(len(F), bool)
    for sp in spikes:
        b, d, L = np.array(sp["base"]), np.array(sp["dir"]), sp["length"]
        t = (cen - b) @ d
        r = np.linalg.norm((cen - b) - np.outer(t, d), axis=1)
        cut |= bright & (t > -0.01) & (t < L + 0.01) & (r < max(sp["radius"] * 2.5, 0.03))
    keep = ~cut
    # The holes: edges (by position) used by one kept face; their loops capped.
    Fk = F[keep]
    edges = defaultdict(list)
    for fi, f in enumerate(Fk):
        for a, b in ((f[0], f[1]), (f[1], f[2]), (f[2], f[0])):
            edges[tuple(sorted((inv[a], inv[b])))].append((fi, a, b))
    open_e = [v[0] for k, v in edges.items() if len(v) == 1]
    nxt = {}
    for fi, a, b in open_e:
        nxt[inv[b]] = (inv[a], b, a)  # walk the loop against the face's winding
    newV, newUV, newN, capF = [], [], [], []
    seen = set(); loops = 0
    for start in list(nxt):
        if start in seen: continue
        loop = []; p = start
        while p in nxt and p not in seen:
            seen.add(p); q, vi_p, vi_q = nxt[p]; loop.append(vi_p); p = q
        if len(loop) < 3 or p != start: continue
        loops += 1
        pts = V[loop]; ctr = pts.mean(axis=0)
        ci = len(V) + len(newV); newV.append(ctr); newUV.append(uv[loop[0]]); newN.append(np.zeros(3))
        for i in range(len(loop)):
            capF.append([loop[i], loop[(i + 1) % len(loop)], ci])
    V2 = np.vstack([V, np.array(newV)]) if newV else V
    UV2 = np.vstack([uv, np.array(newUV)]) if newV else uv
    F2 = np.vstack([Fk, np.array(capF, int)]) if capF else Fk
    N2 = np.vstack([N, np.array(newN)]) if newV else N
    for ci_ in range(len(V), len(V2)):
        tris = [f for f in capF if f[2] == ci_]
        n = sum(np.cross(V2[t[1]] - V2[t[0]], V2[t[2]] - V2[t[0]]) for t in tris)
        N2[ci_] = n / (np.linalg.norm(n) + 1e-12)
    used = np.unique(F2); remap = -np.ones(len(V2), int); remap[used] = np.arange(len(used))
    body = trimesh.Trimesh(V2[used], remap[F2], process=False)
    body.visual = trimesh.visual.TextureVisuals(uv=UV2[used], material=g.visual.material)
    body.vertex_normals = N2[used]
    print(f"cut {cut.sum()} faces of {len(spikes)} spikes; {loops} holes capped; pale specks left: {(bright & keep).sum()} faces")

    # Shards: faceted (each face its own vertices), irregular, broken off.
    def frame(d):
        a = np.array([0, 1, 0]) if abs(d[1]) < 0.9 else np.array([1, 0, 0])
        u = np.cross(d, a); u /= np.linalg.norm(u); return u, np.cross(d, u)
    def shard(b, d, L, r):
        u, v = frame(d)
        n = rng.choice([3, 4, 4, 5])
        ang = np.arange(n) * 2 * np.pi / n + rng.uniform(-0.35, 0.35, n) * 2 * np.pi / n + rng.uniform(0, 6.28)
        rad = r * rng.uniform(0.75, 1.15, n)
        ring = lambda t, k, off: [b + d * t + off + (u * np.cos(a) + v * np.sin(a)) * rr * k for a, rr in zip(ang, rad)]
        kink = (u * rng.normal() + v * rng.normal()) * r * 0.4
        rings = [ring(-0.04, 1.0, 0), ring(L * rng.uniform(0.35, 0.6), rng.uniform(0.75, 1.05), kink)]
        tri = []
        if rng.random() < 0.7:  # a point off the axis: a chisel break
            tip = b + d * L + (u * rng.normal() + v * rng.normal()) * r * 0.3
            top = None
        else:  # broken off square-ish, at a slant
            k = rng.uniform(0.3, 0.45); tilt = (u * rng.normal() + v * rng.normal()) * 0.25
            top = [p + d * (L * 0.85 + np.dot(p - b, tilt)) - d * np.dot(p - b, d) for p in ring(0, k, kink * 1.4)]
            tip = None
        for i in range(n):
            j = (i + 1) % n
            a0, a1, b0, b1 = rings[0][i], rings[0][j], rings[1][i], rings[1][j]
            tri += [(a0, a1, b1), (a0, b1, b0)]
            if tip is not None: tri.append((b0, b1, tip))
            else: tri += [(b0, b1, top[j]), (b0, top[j], top[i])]
        if top is not None:
            tri += [(top[0], top[i], top[i + 1]) for i in range(1, n - 1)]
        tri = [t if np.dot(np.cross(t[1] - t[0], t[2] - t[0]), (t[0] + t[1] + t[2]) / 3 - (b + d * L * 0.4)) > 0 else (t[0], t[2], t[1]) for t in tri]
        return tri
    tris = []
    for sp in spikes:
        b, d, L, r = np.array(sp["base"]), np.array(sp["dir"]), sp["length"], sp["radius"]
        if L < 0.06: continue
        sl = L * rng.uniform(0.45, 0.65); sr = max(r * 1.15, 0.014) * rng.uniform(0.85, 1.15)
        tris += shard(b, d, sl, sr)
        if rng.random() < 0.4:  # a splinter beside it, from the same wound
            u, v = frame(d); side = u * np.cos(rng.uniform(0, 6.3)) + v * np.sin(rng.uniform(0, 6.3))
            d2 = d + side * rng.uniform(0.2, 0.45); d2 /= np.linalg.norm(d2)
            tris += shard(b + side * sr * 0.8, d2, sl * rng.uniform(0.4, 0.7), sr * 0.55)
    T = np.array(tris).reshape(-1, 3)
    shards = trimesh.Trimesh(T, np.arange(len(T)).reshape(-1, 3), process=False)
    shards.visual = trimesh.visual.TextureVisuals(material=trimesh.visual.material.PBRMaterial(name="Shard", baseColorFactor=[200, 200, 205, 255], metallicFactor=0.0, roughnessFactor=0.35, doubleSided=True))
    print(f"{len(T) // 3} shard faces")
    scene = trimesh.Scene(); scene.add_geometry(body, geom_name="body"); scene.add_geometry(shards, geom_name="shards")
    scene.export(out)


# The feet.
FOOT_FULL = 0.10   # how far up from the sole the foot is all star (mesh units)
FOOT_TOP = 0.22    # how far up it blends back into the leg
FOOT_OUT = 0.34    # the star's points, from the foot's middle
FOOT_IN = 0.11     # its notches between them
FOOT_SPLAY = 0.25  # how much narrower the star is at FOOT_FULL than at the sole


def star_radius(theta, points, turn):
    """How far from the middle a star polygon's outline is along `theta`:
    `points` points at FOOT_OUT, notches at FOOT_IN between, a point at
    `turn` (radians from +x)."""
    k = 2 * points
    corners = [(turn + np.pi * i / points, FOOT_OUT if i % 2 == 0 else FOOT_IN) for i in range(k)]
    xy = np.array([[r * np.cos(a), r * np.sin(a)] for a, r in corners])
    d = np.stack([np.cos(theta), np.sin(theta)], axis=1)
    best = np.full(len(theta), np.inf)
    for i in range(k):
        a, b = xy[i], xy[(i + 1) % k]
        e = b - a
        # Ray t*d meets a + s*e.
        den = d[:, 0] * (-e[1]) - d[:, 1] * (-e[0])
        with np.errstate(divide="ignore", invalid="ignore"):
            t = (a[0] * (-e[1]) - a[1] * (-e[0])) / den
            s = (d[:, 0] * a[1] - d[:, 1] * a[0]) / den
        ok = (np.abs(den) > 1e-12) & (s >= -1e-6) & (s <= 1 + 1e-6) & (t > 0)
        best = np.where(ok & (t < best), t, best)
    return best


def feet(inp, out, points=4, turn_deg=45.0):
    """Each foot's sole outline made a star of `points` points (one at
    `turn_deg` from +x: 45, four as an X, two splayed ahead and two behind;
    90 puts one straight ahead, +z), symmetric (the user, 2026-11-10: the
    paw's toes too cute; jagged, a star like the grappling hook's, few
    points; five read as a cartoon star): every vertex
    keeps its bearing from the foot's middle, its distance from it scaled
    so the outline at its height lies on the star, fully up to FOOT_FULL
    above the sole, blending back to the leg by FOOT_TOP; the foot then
    flat-shaded in facets and the normal map's relief there faded flat."""
    s = trimesh.load(inp, force="scene", process=False)
    g = s.geometry["body"]
    V0 = np.asarray(g.vertices); F = np.asarray(g.faces); N0 = np.asarray(g.vertex_normals).copy()
    uv0 = np.asarray(g.visual.uv)
    P, inv = np.unique(V0.round(6), axis=0, return_inverse=True)
    inv = inv.reshape(-1)
    lo = P[:, 1].min()
    h = P[:, 1] - lo
    Q = P.copy()
    weight = np.zeros(len(P))
    for side in (-1, 1):
        foot = np.where((h < FOOT_TOP) & (np.sign(P[:, 0]) == side))[0]
        sole = foot[h[foot] < 0.03]
        cx = (P[sole, 0].min() + P[sole, 0].max()) / 2
        cz = (P[sole, 2].min() + P[sole, 2].max()) / 2
        dx, dz = P[foot, 0] - cx, P[foot, 2] - cz
        r = np.hypot(dx, dz)
        th = np.arctan2(dz, dx)
        # The outline at each height and bearing: the furthest out there.
        bins, bands = 72, 18
        bi = ((th + np.pi) / (2 * np.pi) * bins).astype(int) % bins
        hb = np.minimum((h[foot] / FOOT_TOP * bands).astype(int), bands - 1)
        out_r = np.zeros((bands, bins))
        np.maximum.at(out_r, (hb, bi), r)
        for b in range(bands):
            row = out_r[b]
            if (row > 0).sum() == 0:
                continue
            # (Empty bearings filled from their neighbours, round.)
            idx = np.where(row > 0)[0]
            full = np.interp(np.arange(bins), np.concatenate([idx - bins, idx, idx + bins]), np.tile(row[idx], 3))
            # (Smoothed a little: one vertex further out does not dent it.)
            out_r[b] = np.maximum(full, np.convolve(np.tile(full, 3), np.ones(5) / 5, "same")[bins:2 * bins])
        star = star_radius(th, points, np.radians(turn_deg)) * (1 - FOOT_SPLAY * np.clip(h[foot] / FOOT_FULL, 0, 1))
        u = np.clip((FOOT_TOP - h[foot]) / (FOOT_TOP - FOOT_FULL), 0, 1)
        w = u * u * (3 - 2 * u)
        scale = 1 + w * (star / np.maximum(out_r[hb, bi], 1e-4) - 1)
        Q[foot, 0] = cx + dx * scale
        Q[foot, 2] = cz + dz * scale
        weight[foot] = np.maximum(weight[foot], w)
        print(f"foot {side:+d}: middle ({cx:.3f}, {cz:.3f}), {len(foot)} positions")
    # Faceted: the foot's faces (any corner in it) each its own corners, its
    # own normal; the rest as they were.
    faceted = (h[inv[F]] < FOOT_TOP).any(axis=1)
    V = Q[inv]
    keep = ~faceted
    newV = [V]; newN = [N0]; newUV = [uv0]
    F2 = F.copy()
    tri = V[F[faceted]]
    fn = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
    fn /= np.linalg.norm(fn, axis=1, keepdims=True) + 1e-12
    base = len(V)
    nf = faceted.sum()
    newV.append(tri.reshape(-1, 3))
    newN.append(np.repeat(fn, 3, axis=0))
    newUV.append(uv0[F[faceted]].reshape(-1, 2))
    F2[faceted] = base + np.arange(nf * 3).reshape(-1, 3)
    from trimesh.visual import TextureVisuals
    mat = g.visual.material
    # The normal map's relief on the foot faded flat (its wrinkles were the
    # paw's).
    from PIL import Image, ImageDraw, ImageFilter
    nmap = mat.normalTexture.convert("RGB")
    W, H = nmap.size
    mask = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(mask)
    fw = weight[inv[F]].max(axis=1)
    for f in np.where(faceted)[0]:
        pts = [(uv0[i, 0] * W, (1 - uv0[i, 1]) * H) for i in F[f]]
        draw.polygon(pts, fill=int(255 * min(1.0, 0.4 + fw[f])))
    mask = mask.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(3))
    mat.normalTexture = Image.composite(Image.new("RGB", (W, H), (128, 128, 255)), nmap, mask)
    body = trimesh.Trimesh(np.vstack(newV), F2, vertex_normals=np.vstack(newN), visual=TextureVisuals(uv=np.vstack(newUV), material=mat), process=False)
    # (Unused corners left by the faceting stay: harmless, and the seams'
    # other copies keep theirs.)
    s.geometry["body"] = body
    s.export(out)
    print("feet: faceted", int(nf), "faces")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    seed = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
    if "--seed" in sys.argv: args.remove(str(seed))
    inp = args[0] if args else "creatures/Meshy_AI_Refine_the_impaled_cr_1105151647_texture.glb"
    out = args[1] if len(args) > 1 else "creatures/impaled.glb"
    with tempfile.TemporaryDirectory() as tmp:
        flat = os.path.join(tmp, "flat.glb")
        flatten(inp, flat)
        spikes = measure_spikes(inp)
        sharded = os.path.join(tmp, "shards.glb")
        shards(flat, spikes, sharded, seed)
        feet(sharded, out)
    print("written", out)
