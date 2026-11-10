"""Concept turnaround (top row) against renders of a GLB from the same directions (bottom row), same scale."""
import sys, numpy as np, trimesh, pyrender
from pathlib import Path
from PIL import Image, ImageDraw
glb, views_dir, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]); T = 700
sc0 = trimesh.load(glb, force="scene"); mesh = trimesh.util.concatenate(list(sc0.geometry.values()))
parts = mesh.split(only_watertight=False); areas = sorted((p.area for p in parts), reverse=True)
tot = sum(areas); print(f"components: {len(parts)}; largest {areas[0]/tot:.1%} of area, top 5 {[round(a/tot,3) for a in areas[:5]]}, "
      f"{sum(a < 0.001*tot for a in areas)} pieces under 0.1% of area")
mesh.apply_translation(-mesh.bounding_box.centroid); mesh.apply_scale(1.0 / mesh.extents[1])   # unit height
mesh.visual = trimesh.visual.ColorVisuals(mesh=mesh)   # drop textures: shading below is by depth
print("extents w,h,d after unit height:", mesh.extents.round(3))
mat = pyrender.MetallicRoughnessMaterial(baseColorFactor=[0.78]*3 + [1.0], metallicFactor=0.0, roughnessFactor=0.9)
pm = pyrender.Mesh.from_trimesh(mesh, material=mat, smooth=False)
def look(eye, up=(0, 1, 0)):
    eye = np.array(eye, float); f = -eye / np.linalg.norm(eye); u = np.array(up, float)
    s = np.cross(f, u); s /= np.linalg.norm(s); u = np.cross(s, f)
    M = np.eye(4); M[:3, 0] = s; M[:3, 1] = u; M[:3, 2] = -f; M[:3, 3] = eye; return M
r = pyrender.OffscreenRenderer(T, T)
def render(eye):
    # the concept draws the far side dark: shade vertices by depth along the view so both rows follow one convention
    d = -np.array(eye, float) / np.linalg.norm(eye)
    depth = mesh.vertices @ d
    t = (depth - depth.min()) / max(depth.max() - depth.min(), 1e-9)      # 0 = nearest, 1 = farthest
    shade = (1.0 - 0.7 * t) * 255
    m2 = mesh.copy()
    m2.visual.vertex_colors = np.c_[shade, shade, shade, np.full(len(shade), 255)].astype(np.uint8)
    pm2 = pyrender.Mesh.from_trimesh(m2, smooth=False)
    sc = pyrender.Scene(bg_color=[131/255]*3 + [1.0], ambient_light=[0.7]*3); sc.add(pm2)
    sc.add(pyrender.OrthographicCamera(xmag=0.56, ymag=0.56, znear=0.01, zfar=10), pose=look(eye))
    sc.add(pyrender.DirectionalLight(intensity=1.2), pose=look((1, 2, 2))); sc.add(pyrender.DirectionalLight(intensity=0.5), pose=look((-2, 0.5, -1)))
    return Image.fromarray(r.render(sc)[0])
model = {"front": render((0, 0, 3)), "left side": render((-3, 0, 0)), "right side": render((3, 0, 0)), "back": render((0, 0, -3))}
r.delete()
def concept(name):
    p = views_dir / f"{name}.png"
    if not p.exists(): return Image.new("RGB", (T, T), (131,)*3)
    im = Image.open(p).convert("RGBA"); bg = Image.new("RGBA", im.size, (131, 131, 131, 255))
    return Image.alpha_composite(bg, im).convert("RGB").resize((T, T), Image.LANCZOS)
top = [("concept front", concept("front")), ("concept profile", concept("profile")), ("concept profile (mirrored)", concept("profile").transpose(Image.FLIP_LEFT_RIGHT)), ("concept back", concept("back"))]
bot = [("model " + k, v) for k, v in model.items()]
sheet = Image.new("RGB", (T * 4, T * 2 + 60), (131,)*3); d = ImageDraw.Draw(sheet)
for row, items in enumerate((top, bot)):
    for col, (label, im) in enumerate(items):
        sheet.paste(im, (col * T, row * (T + 30) + 30)); d.text((col * T + 12, row * (T + 30) + 8), label, fill=(240,)*3)
sheet.save(out); print("->", out)
