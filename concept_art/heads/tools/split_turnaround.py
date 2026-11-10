"""Split an orthographic turnaround sheet into per-view RGBA PNGs with the flat background keyed out."""
import sys, numpy as np
from pathlib import Path
from PIL import Image
from scipy import ndimage as ndi

src = Path(sys.argv[1]); out = Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
names = sys.argv[3].split(",") if len(sys.argv) > 3 else None
img = np.asarray(Image.open(src).convert("RGB")).astype(np.int16)
H, W, _ = img.shape

# background colour = median of a border band
band = np.concatenate([img[:8].reshape(-1,3), img[-8:].reshape(-1,3), img[:, :8].reshape(-1,3), img[:, -8:].reshape(-1,3)])
bg = np.median(band, axis=0)
dist = np.abs(img - bg).max(axis=2)
print("background colour", bg, " dist percentiles", np.percentile(dist, [50, 80, 90, 95, 99]))

TOL = 14                         # pixels within this of the background are background
fg = dist > TOL
fg = ndi.binary_opening(fg, iterations=2)      # cuts 1-2 px floor grid lines, keeps struts

# column occupancy -> split into view columns
occ = fg.mean(axis=0)
cols = occ > 0.02
# label runs of occupied columns, merge gaps < 3% of width
lab, n = ndi.label(cols)
runs = [(np.where(lab == i)[0].min(), np.where(lab == i)[0].max()) for i in range(1, n+1)]
merged = []
for a, b in runs:
    if merged and a - merged[-1][1] < 0.03 * W: merged[-1] = (merged[-1][0], b)
    else: merged.append((a, b))
# drop tiny runs (text labels fall inside object columns anyway)
merged = [r for r in merged if r[1] - r[0] > 0.05 * W]
print("view columns:", merged)
if names is None: names = [f"view{i}" for i in range(len(merged))]
assert len(names) == len(merged), f"{len(merged)} views found, {len(names)} names given"

rgba_all = np.dstack([img.astype(np.uint8), (fg * 255).astype(np.uint8)])
tiles = []
for (a, b), name in zip(merged, names):
    sub = fg[:, a:b+1]
    lab, n = ndi.label(sub)
    sizes = ndi.sum(sub, lab, range(1, n+1))
    keep = np.isin(lab, 1 + np.where(sizes > 0.02 * sizes.max())[0])   # object + any big separate pieces; drops text
    ys, xs = np.where(keep)
    y0, y1, x0, x1 = ys.min(), ys.max(), xs.min() + a, xs.max() + a
    tile = rgba_all[y0:y1+1, x0:x1+1].copy()
    tile[..., 3] = (keep[y0:y1+1, x0-a:x1-a+1] * 255).astype(np.uint8)
    h, w = tile.shape[:2]; s = int(max(h, w) * 1.12)
    canvas = np.zeros((s, s, 4), np.uint8)
    oy, ox = (s - h)//2, (s - w)//2
    canvas[oy:oy+h, ox:ox+w] = tile
    Image.fromarray(canvas).save(out / f"{name}.png")
    print(f"{name}: bbox x{x0}-{x1} y{y0}-{y1}, {w}x{h}px, saved {s}x{s}")
    tiles.append(Image.fromarray(canvas).resize((512, 512), Image.LANCZOS))

# contact sheet over a checker so holes are visible
sheet = Image.new("RGBA", (512*len(tiles), 512), (0,0,0,0))
for i, t in enumerate(tiles): sheet.paste(t, (512*i, 0))
chk = np.indices((512, 512*len(tiles))).sum(axis=0) // 32 % 2
checker = Image.fromarray((np.stack([chk]*3, -1) * 60 + 170).astype(np.uint8)).convert("RGBA")
Image.alpha_composite(checker, sheet).convert("RGB").save(out / "_preview.png")
print("preview:", out / "_preview.png")
