"""Render a GLB as orthographic front / left / back / top views plus a 3/4 view, flat grey, into one contact sheet."""
import sys, numpy as np, trimesh, pyrender
from pathlib import Path
from PIL import Image
src, out = Path(sys.argv[1]), Path(sys.argv[2]); size = int(sys.argv[3]) if len(sys.argv) > 3 else 512
loaded = trimesh.load(src, force="scene")
mesh = trimesh.util.concatenate([g for g in loaded.dump()]) if isinstance(loaded, trimesh.Scene) else loaded
print(f"{src.name}: {len(mesh.vertices)} verts, {len(mesh.faces)} faces, extents {mesh.extents.round(3)}, watertight={mesh.is_watertight}")
mesh.apply_translation(-mesh.bounding_box.centroid); mesh.apply_scale(1.0 / mesh.extents.max())
mat = pyrender.MetallicRoughnessMaterial(baseColorFactor=[0.75, 0.75, 0.75, 1.0], metallicFactor=0.0, roughnessFactor=0.9)
pm = pyrender.Mesh.from_trimesh(mesh, material=mat, smooth=False)
def look(eye, up=(0, 1, 0)):
    eye = np.array(eye, float); f = -eye / np.linalg.norm(eye); u = np.array(up, float)
    s = np.cross(f, u); s /= np.linalg.norm(s); u = np.cross(s, f)
    M = np.eye(4); M[:3, 0] = s; M[:3, 1] = u; M[:3, 2] = -f; M[:3, 3] = eye; return M
views = {"front": (0, 0, 3), "left": (-3, 0, 0), "back": (0, 0, -3), "top": (0, 3, 0.001), "3/4": (2, 1.5, 2)}
tiles = []
r = pyrender.OffscreenRenderer(size, size)
for name, eye in views.items():
    sc = pyrender.Scene(bg_color=[0.5, 0.5, 0.5, 1.0], ambient_light=[0.35]*3); sc.add(pm)
    cam = pyrender.OrthographicCamera(xmag=0.6, ymag=0.6, znear=0.01, zfar=10); sc.add(cam, pose=look(eye))
    sc.add(pyrender.DirectionalLight(color=np.ones(3), intensity=3.0), pose=look((1, 2, 2)))
    sc.add(pyrender.DirectionalLight(color=np.ones(3), intensity=1.0), pose=look((-2, 0.5, -1)))
    col, _ = r.render(sc); tiles.append(Image.fromarray(col))
r.delete()
sheet = Image.new("RGB", (size * len(tiles), size))
for i, t in enumerate(tiles): sheet.paste(t, (size * i, 0))
sheet.save(out); print("sheet ->", out, "views:", ", ".join(views))
