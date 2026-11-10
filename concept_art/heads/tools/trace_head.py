"""Trace a lattice head out of its orthographic turnaround.

Per view: the lit (near-side) struts are separated from the dark far-side ones by brightness,
skeletonised, and turned into a graph of straight strut segments with their drawn widths. Each
graph is lifted onto an egg shell (plus a neck below it) by ray-casting along the view direction,
each view keeps the quarter of the surface that faces it, loose ends at the seams are stitched to
the neighbouring view, and the result is built as chamfered ribbons unioned into one mesh.
No cross-view correspondence is needed, so the views need not agree with each other, only with the shell.
"""
import sys, json, argparse, numpy as np, trimesh
from pathlib import Path
from collections import defaultdict
from PIL import Image
from scipy import ndimage as ndi
from skimage.morphology import skeletonize

sys.path.insert(0, str(Path(__file__).parent))
from lattice_head import shell, ribbon, douglas_peucker, unit  # noqa: E402

DEFAULTS = dict(
    # shell the drawings are lifted onto (height 1 = crown to foot)
    width=0.386, depth=0.547, head_top=1.0, head_bottom=0.26, taper=0.4, shear=0.06, bump_n=0,
    neck_top=0.45, neck_rx=0.12, neck_rz=0.08,    # cylinder below the egg that the legs are lifted onto
    # tracing
    lum_thr=160, close_iter=3, open_iter=2, min_comp=0.002,
    spur_px=14, simplify_px=16, straight_frac=0.7,   # edges deviating less than this × width from their chord are straight
    # geometry
    strut_t=0.022, chamfer=0.45, w_min=0.028, w_max=0.07, w_scale=1.5,
    leg_zone=0.3, leg_view="front",               # below this height only leg_view's struts are used
    weld_in=0.02, weld_x=0.05,                    # merge radius for nodes within a view / across seams
    stitch_r=0.1, same_view_penalty=1.5,          # loose ends reach this far; other views preferred
    min_component=0.2,                            # drop lattice fragments shorter than this in total
    union=True, subdiv=5,
    views={"front": "front.png", "left": "profile.png", "right": "profile_mirrored.png", "back": "back.png"},
)
VIEW_DIR = {"front": (0, 0, 1), "back": (0, 0, -1), "left": (-1, 0, 0), "right": (1, 0, 0)}     # camera side
VIEW_RIGHT = {"front": (1, 0, 0), "back": (-1, 0, 0), "left": (0, 0, 1), "right": (0, 0, -1)}  # image right → world
NBRS = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]


def trace_view(path, P):
    """One view → nodes (h, y) and edges [(a, b, points (h, y), width)] in height-normalised units."""
    im = np.asarray(Image.open(path).convert("RGBA")).astype(float)
    alpha = im[..., 3] > 128
    rows = np.where(alpha.any(1))[0]
    cols = np.where(alpha.any(0))[0]
    top, bottom = rows.min(), rows.max()
    cx = (cols.min() + cols.max()) / 2
    Hpx = float(bottom - top)
    lum = im[..., :3].mean(-1)
    near = alpha & (lum > P["lum_thr"])
    near = ndi.binary_closing(near, iterations=P["close_iter"])
    near = ndi.binary_opening(near, iterations=P["open_iter"])
    lab, n = ndi.label(near)
    sizes = ndi.sum(near, lab, range(1, n + 1))
    near = np.isin(lab, 1 + np.where(sizes > P["min_comp"] * sizes.max())[0])
    skel = skeletonize(near)
    edt = ndi.distance_transform_edt(near)
    k = np.ones((3, 3)); k[1, 1] = 0
    deg = ndi.convolve(skel.astype(int), k, mode="constant") * skel
    nodepx = skel & (deg != 2)
    nlab, nn = ndi.label(nodepx, structure=np.ones((3, 3)))
    cents = ndi.center_of_mass(nodepx, nlab, range(1, nn + 1))
    H, W = skel.shape
    visited = np.zeros_like(skel, bool)
    raw = []
    for nid in range(1, nn + 1):
        for r, c in zip(*np.where(nlab == nid)):
            for dr, dc in NBRS:
                r2, c2 = r + dr, c + dc
                if not (0 <= r2 < H and 0 <= c2 < W) or not skel[r2, c2] or nlab[r2, c2] == nid:
                    continue
                if nlab[r2, c2] > 0:
                    if nid < nlab[r2, c2]:
                        raw.append((nid, int(nlab[r2, c2]), [(r, c), (r2, c2)]))
                    continue
                if visited[r2, c2]:
                    continue
                pth = [(r, c), (r2, c2)]
                visited[r2, c2] = True
                pr, pc, cr, cc, end = r, c, r2, c2, None
                while True:
                    nxt = None
                    for dr2, dc2 in NBRS:
                        r3, c3 = cr + dr2, cc + dc2
                        if not (0 <= r3 < H and 0 <= c3 < W) or not skel[r3, c3] or (r3, c3) == (pr, pc):
                            continue
                        if nlab[r3, c3] > 0:
                            if nlab[r3, c3] == nid and len(pth) < 4:
                                continue
                            end = int(nlab[r3, c3]); pth.append((r3, c3)); break
                        if not visited[r3, c3]:
                            nxt = (r3, c3)
                    if end is not None or nxt is None:
                        break
                    visited[nxt] = True
                    pr, pc, cr, cc = cr, cc, nxt[0], nxt[1]
                    pth.append(nxt)
                if end is not None:
                    raw.append((nid, end, pth))
    # spur removal: short edges to a free end are skeleton artefacts
    for _ in range(2):
        degree = defaultdict(int)
        for a, b, _p in raw:
            degree[a] += 1; degree[b] += 1
        raw = [(a, b, p) for a, b, p in raw
               if not (len(p) < P["spur_px"] and (degree[a] == 1 or degree[b] == 1))]

    def to_unit(rc):
        return np.array([(rc[1] - cx) / Hpx, 1 - (rc[0] - top) / Hpx])
    nodes = {i + 1: to_unit(cents[i]) for i in range(nn)}
    edges = []
    for a, b, p in raw:
        pts = np.array([to_unit(q) for q in p])
        pts[0], pts[-1] = nodes[a], nodes[b]
        wpx = np.median([2 * edt[q] for q in p])
        chord = pts[-1] - pts[0]
        L = np.linalg.norm(chord)
        rel = pts - pts[0]
        dev = (np.abs(rel[:, 0] * chord[1] - rel[:, 1] * chord[0]) / L).max() if L > 1e-9 else 0.0
        if dev * Hpx < P["straight_frac"] * wpx:       # a drawn straight strut: snap to its chord
            pts = pts[[0, -1]]
        else:
            pts = douglas_peucker(np.c_[pts, np.zeros(len(pts))], P["simplify_px"] / Hpx)[:, :2]
        edges.append(dict(a=a, b=b, pts=pts, w=float(wpx / Hpx)))
    return nodes, edges, dict(Hpx=Hpx, near=near, skel=skel)


def build_cast_mesh(P):
    sph = trimesh.creation.icosphere(subdivisions=P["subdiv"])
    egg = trimesh.Trimesh(shell(sph.vertices.copy(), P), sph.faces, process=False)
    # neck: an elliptical cylinder from inside the egg down to the floor, so drawn legs stay vertical in depth
    ys = np.linspace(P["neck_top"], 0.0, 14)
    ang = np.linspace(0, 2 * np.pi, 40, endpoint=False)
    V, F = [], []
    for y in ys:
        V.append(np.stack([P["neck_rx"] * np.sin(ang), np.full_like(ang, y), P["neck_rz"] * np.cos(ang)], 1))
    V = np.vstack(V)
    m = len(ang)
    for i in range(len(ys) - 1):
        for j in range(m):
            a, b = i * m + j, i * m + (j + 1) % m
            c, d = a + m, b + m
            F += [[a, c, b], [b, c, d]]
    neck = trimesh.Trimesh(V, np.array(F), process=False)
    return trimesh.util.concatenate([egg, neck])


def lift(points2d, view, cast, P, far=False):
    """Lift (h, y) points of a view onto the cast mesh along the view direction. Returns xyz, normals, hit flags."""
    d = np.array(VIEW_DIR[view], float)
    rgt = np.array(VIEW_RIGHT[view], float)
    base = points2d[:, :1] * rgt + np.stack([np.zeros(len(points2d)), points2d[:, 1], np.zeros(len(points2d))], 1)
    origins = base + d * 2.0
    dirs = np.tile(-d, (len(points2d), 1))
    loc, iray, itri = cast.ray.intersects_location(origins, dirs, multiple_hits=True)
    xyz, nrm, hit = base.copy(), np.tile(d, (len(points2d), 1)), np.zeros(len(points2d), bool)
    best = {}
    for L, r, t in zip(loc, iray, itri):
        depth = (L - base[r]) @ d
        if r not in best or (depth < best[r][0] if far else depth > best[r][0]):
            best[r] = (depth, L, t)
    for r, (depth, L, t) in best.items():
        xyz[r], nrm[r], hit[r] = L, cast.face_normals[t], True
    miss = ~hit
    if miss.any():
        cp, _dist, tri = cast.nearest.on_surface(base[miss])
        xyz[miss] = base[miss] + np.outer((cp - base[miss]) @ d, d)
        nrm[miss] = cast.face_normals[tri]
    return xyz, nrm, hit


def weld(pieces, node_xyz, node_deg0, r, cross):
    """Merge nodes closer than r (across views only if cross, else within a view); dedupe the struts."""
    from scipy.spatial import cKDTree
    keys = list(node_xyz)
    idx = {k: i for i, k in enumerate(keys)}
    P3 = np.array([node_xyz[k] for k in keys])
    parent = list(range(len(keys)))

    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i
    for i, j in cKDTree(P3).query_pairs(r):
        same = keys[i][0] == keys[j][0]
        if (cross and same) or (not cross and not same):
            continue
        parent[find(i)] = find(j)
    members = defaultdict(list)
    for i in range(len(keys)):
        members[find(i)].append(i)
    rep, new_xyz, new_deg0 = {}, {}, {}
    for root, ms in members.items():
        k0 = keys[root]
        mean = P3[ms].mean(0)
        for m in ms:
            rep[keys[m]] = k0
        new_xyz[k0] = mean
        new_deg0[k0] = max(node_deg0.get(keys[m], 0) for m in ms)
    out, seen = [], set()
    for pc in pieces:
        pc["a"], pc["b"] = rep[pc["a"]], rep[pc["b"]]
        if pc["a"] == pc["b"]:
            continue
        pc["pts"] = pc["pts"].copy()
        pc["pts"][0], pc["pts"][-1] = new_xyz[pc["a"]], new_xyz[pc["b"]]
        key = frozenset((pc["a"], pc["b"]))
        if key in seen:
            continue
        seen.add(key)
        out.append(pc)
    return out, new_xyz, new_deg0


def owner(xyz):
    """Which view owns a surface point: the compass quarter it lies in around the vertical axis."""
    x, z = xyz[:, 0], xyz[:, 2]
    out = np.where(np.abs(z) >= np.abs(x), np.where(z >= 0, "front", "back"), np.where(x >= 0, "right", "left"))
    return list(out)


def build(views_dir, P):
    views_dir = Path(views_dir)
    cast = build_cast_mesh(P)
    pieces = []            # dict(pts xyz, nrm, w, view, a, b)
    node_xyz, node_deg0, node_deg = {}, {}, defaultdict(int)
    report = {}
    for view, fname in P["views"].items():
        nodes, edges, _info = trace_view(views_dir / fname, P)
        deg0 = defaultdict(int)
        for e in edges:
            deg0[e["a"]] += 1; deg0[e["b"]] += 1
        # lift every polyline point
        allpts = np.vstack([e["pts"] for e in edges])
        xyz, nrm, hit = lift(allpts, view, cast, P)
        own = owner(xyz)
        kept = 0
        i = 0
        for e in edges:
            n = len(e["pts"])
            exyz, enrm, eown = xyz[i:i + n], nrm[i:i + n], own[i:i + n]
            i += n
            ymin = e["pts"][:, 1].min()
            if ymin < P["leg_zone"]:
                keep = view == P["leg_view"]
            else:
                keep = eown[n // 2] == view or (n % 2 == 0 and eown[n // 2 - 1] == view)
            if not keep:
                continue
            kept += 1
            pieces.append(dict(pts=exyz, nrm=enrm, w=e["w"], view=view, a=(view, e["a"]), b=(view, e["b"])))
            for key, p in (((view, e["a"]), exyz[0]), ((view, e["b"]), exyz[-1])):
                node_xyz[key] = p
                node_deg[key] += 1
                node_deg0[key] = deg0[key[1]]
        report[view] = dict(nodes=len(nodes), edges=len(edges), kept=kept, lifted=int(hit.sum()), of=len(allpts))
    # weld: junction clusters within a view, then coincident nodes across the seams; drop duplicate struts
    pieces, node_xyz, node_deg0 = weld(pieces, node_xyz, node_deg0, P["weld_in"], cross=False)
    pieces, node_xyz, node_deg0 = weld(pieces, node_xyz, node_deg0, P["weld_x"], cross=True)
    node_deg = defaultdict(int)
    for pc in pieces:
        node_deg[pc["a"]] += 1
        node_deg[pc["b"]] += 1
    report["after_weld"] = len(pieces)
    # stitch: ends created by the ownership filter reach for the nearest node of another view
    keys = list(node_xyz)
    P3 = np.array([node_xyz[k] for k in keys])
    stitched = 0
    for k in keys:
        if node_deg[k] == 1 and node_deg0[k] >= 2:
            d = np.linalg.norm(P3 - node_xyz[k], axis=1)
            cands = []
            for j in range(len(keys)):
                if keys[j] == k or d[j] > P["stitch_r"] or d[j] < 1e-6:
                    continue
                cost = d[j] * (P["same_view_penalty"] if keys[j][0] == k[0] else 1.0)
                cands.append((cost, keys[j]))
            if not cands:
                continue
            _, kb = min(cands)
            w = np.mean([pc["w"] for pc in pieces if k in (pc["a"], pc["b"])])
            pa, pb = node_xyz[k], node_xyz[kb]
            nn = unit(np.array([pa[0] + pb[0], 0, pa[2] + pb[2]]) + 1e-9)
            pieces.append(dict(pts=np.vstack([pa, pb]), nrm=np.vstack([nn, nn]), w=w, view="stitch", a=k, b=kb))
            node_deg[k] += 1; node_deg[kb] += 1
            stitched += 1
    # drop small disconnected fragments (far-side struts that passed the brightness test, etc.)
    parent = {}

    def find(x):
        while parent.setdefault(x, x) != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x
    for pc in pieces:
        parent[find(pc["a"])] = find(pc["b"])
    length = defaultdict(float)
    for pc in pieces:
        length[find(pc["a"])] += np.linalg.norm(np.diff(pc["pts"], axis=0), axis=1).sum()
    before = len(pieces)
    pieces = [pc for pc in pieces if length[find(pc["a"])] >= P["min_component"]]
    report["dropped_fragments"] = before - len(pieces)
    # geometry
    meshes, joints = [], defaultdict(list)

    def key(p):
        return tuple(np.round(p / 1e-5).astype(int))
    for pc in pieces:
        pts, nrm = pc["pts"], pc["nrm"]
        w = float(np.clip(pc["w"] * P["w_scale"], P["w_min"], P["w_max"]))
        for i in range(len(pts) - 1):
            p, q = pts[i], pts[i + 1]
            if np.linalg.norm(q - p) < 1e-6:
                continue
            n = unit(nrm[i] + nrm[i + 1])
            if abs(n @ unit(q - p)) > 0.98:            # ribbon along the normal (legs seen edge-on): pick any frame
                n = unit(np.cross(unit(q - p), [0, 1, 0.001]))
            m, rp, rq = ribbon(p, q, n, w, P["strut_t"], P["chamfer"])
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
    report["stitched"] = stitched
    report["pieces"] = len(pieces)
    report["joint_failures"] = failed
    report["faces"] = int(len(out.faces))
    report["watertight"] = bool(out.is_watertight)
    report["extents"] = [round(float(v), 3) for v in out.extents]
    return out, report


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("views_dir")
    ap.add_argument("out")
    ap.add_argument("--set", nargs="*", default=[], help="param=value (JSON values)")
    a = ap.parse_args()
    P = dict(DEFAULTS)
    for kv in a.set:
        k, v = kv.split("=", 1)
        if k not in P:
            raise SystemExit(f"unknown parameter {k}; known: {', '.join(P)}")
        P[k] = json.loads(v)
    mesh, rep = build(a.views_dir, P)
    mesh.export(a.out)
    print(json.dumps(rep), "->", a.out)
