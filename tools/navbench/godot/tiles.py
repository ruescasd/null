"""The bench's terrain cut into each body's tiles for Godot (its bake takes one
tile's triangles at a time): each triangle in every tile it touches, with
the tile's border. `python -I tools/navbench/godot/tiles.py target/navbench [scale] [border]`
writes tiles/<Body>/<x>_<z>.bin there (tiles `scale` times the bench's
side, bordered by `border` tiles, as Godot's chunks demo does with one;
those to tiles/<Body>-x<scale>-b<border>/): vertex and index counts (int32),
vertices (float32 x, y, z), indices (int32)."""

import pathlib
import sys

import numpy as np

d = pathlib.Path(sys.argv[1])
scale = int(sys.argv[2]) if len(sys.argv) > 2 else 1
border = int(sys.argv[3]) if len(sys.argv) > 3 else 0
raw = (d / "terrain.bin").read_bytes()
nv, nt = np.frombuffer(raw[:8], np.uint32)
V = np.frombuffer(raw[8:8 + nv * 12], np.float32).reshape(-1, 3)
T = np.frombuffer(raw[8 + nv * 12:], np.uint32).reshape(-1, 3).astype(np.int64)
corners = V[T]
lo, hi = corners.min(axis=1), corners.max(axis=1)
(d / "tiles").mkdir(exist_ok=True)
# (The terrain's height range, a metre over: Recast keeps a span's height in
# 13 bits, so a bake box far taller than the terrain clamps it.)
(d / "tiles" / "heights.txt").write_text(f"{V[:, 1].min() - 1.0} {V[:, 1].max() + 1.0}\n")

for line in (d / "settings.txt").read_text().splitlines():
    w = line.split()
    if w[0] != "body":
        continue
    name, s = w[1], {w[i]: float(w[i + 1]) for i in range(2, len(w), 2)}
    side = s["tile_side"] * scale
    pad = border * side if border else s["border_cells"] * s["cell_size"]
    tlo = np.floor((lo[:, [0, 2]] - pad) / side).astype(np.int64)
    thi = np.floor((hi[:, [0, 2]] + pad) / side).astype(np.int64)
    span = (thi - tlo).max()
    keys, tris = [], []
    for dx in range(span + 1):
        for dz in range(span + 1):
            m = (tlo[:, 0] + dx <= thi[:, 0]) & (tlo[:, 1] + dz <= thi[:, 1])
            idx = np.nonzero(m)[0]
            keys.append((tlo[idx, 0] + dx) * 1_000_000 + (tlo[idx, 1] + dz))
            tris.append(idx)
    keys, tris = np.concatenate(keys), np.concatenate(tris)
    order = np.argsort(keys, kind="stable")
    keys, tris = keys[order], tris[order]
    starts = np.flatnonzero(np.r_[True, keys[1:] != keys[:-1]])
    out = d / "tiles" / (name if (scale, border) == (1, 0) else f"{name}-x{scale}-b{border}")
    out.mkdir(parents=True, exist_ok=True)
    for f in out.glob("*.bin"):
        f.unlink()
    for i, start in enumerate(starts):
        end = starts[i + 1] if i + 1 < len(starts) else len(keys)
        k = int(keys[start])
        tx, tz = (k + 500_000) // 1_000_000, k - ((k + 500_000) // 1_000_000) * 1_000_000
        used, local = np.unique(T[tris[start:end]], return_inverse=True)
        verts = V[used].astype(np.float32)
        with open(out / f"{tx}_{tz}.bin", "wb") as f:
            f.write(np.array([len(verts), local.size], np.int32).tobytes())
            f.write(verts.tobytes())
            f.write(local.astype(np.int32).tobytes())
    print(f"{name}: {len(starts)} tiles of {side:.2f} m, border {pad:.2f} m")
