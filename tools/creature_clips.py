"""Measure a rigged creature's clips and write the file the game reads with it.

    python tools/creature_clips.py creatures/NAME.glb [--height M] [--walk CLIP] ...

The enemy pipeline (2026-10-30): a generated mesh, rigged (Mixamo, Meshy or by
hand), its clips from a library, exported as one GLB into `creatures/`. This
reads the GLB, finds its skeleton's joints by their usual names, and measures
what the game needs and cannot guess:

- how tall it is, standing (the game scales it to `height`);
- which way it faces (the way it walks: its feet go back under it);
- each walking clip's ground speed, from its feet: how fast a foot on the
  ground moves back under the body. Played at that speed, scaled, its feet
  stay where they are planted (played at any other, they slide);
- where in each walking clip its left foot comes down, so walk and run blend
  in step.

It writes `creatures/NAME.creature` beside the GLB (plain `key = value`
lines), which you may edit: `height`, the clips' names, the joints. Run it
again after changing the GLB; it keeps your height unless given another.
"""

import argparse
import json
import math
import re
import struct
import sys
from pathlib import Path

import numpy as np

# Joint names in the rigs we expect: Mixamo's (mixamorig:Hips ...), Quaternius'
# (the robot: Hips, Abdomen, Torso ...), and plain ones (Spine, LeftUpLeg ...).
SIDES = {"L": (("left_", "left", "l_"), (".l", "_l")), "R": (("right_", "right", "r_"), (".r", "_r"))}
ROLES = {
    "hips": ("hips", "pelvis"),
    "neck": ("neck",),
    "head": ("head",),
    "thigh": ("upleg", "upperleg", "thigh", "upper_leg"),
    "shin": ("leg", "lowerleg", "shin", "calf", "lower_leg", "knee"),
    "foot": ("foot", "ankle"),
    "upper_arm": ("arm", "upperarm", "upper_arm"),
    "forearm": ("forearm", "lowerarm", "lower_arm", "elbow"),
    "hand": ("hand", "palm2", "palm", "wrist"),
}
CLIPS = {
    "idle": ("idle",),
    "walk": ("walk", "walking"),
    "run": ("run", "running", "sprint"),
    "attack": ("attack", "punch", "slash", "swing", "strike"),
    "hit": ("hit", "hurt", "damage", "flinch"),
    "death": ("death", "die", "dying"),
}


def read_glb(path):
    data = Path(path).read_bytes()
    if data[:4] != b"glTF":
        sys.exit(f"{path} is not a binary glTF (GLB): export it as .glb")
    length = struct.unpack_from("<I", data, 12)[0]
    gltf = json.loads(data[20:20 + length])
    at = 20 + length
    blob = b""
    if at < len(data):
        blob_len = struct.unpack_from("<I", data, at)[0]
        blob = data[at + 8:at + 8 + blob_len]
    return gltf, blob


COMPONENTS = {5120: ("b", 127.0), 5121: ("B", 255.0), 5122: ("h", 32767.0), 5123: ("H", 65535.0), 5125: ("I", None), 5126: ("f", None)}
WIDTH = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def accessor(gltf, blob, index):
    acc = gltf["accessors"][index]
    view = gltf["bufferViews"][acc["bufferView"]]
    fmt, norm = COMPONENTS[acc["componentType"]]
    width = WIDTH[acc["type"]]
    size = struct.calcsize(fmt)
    stride = view.get("byteStride", size * width)
    start = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    rows = np.lib.stride_tricks.as_strided(
        np.frombuffer(blob, dtype=np.dtype("<" + fmt), offset=start, count=(acc["count"] - 1) * stride // size + width),
        shape=(acc["count"], width), strides=(stride, size))
    out = rows.astype(np.float64)
    if acc.get("normalized") and norm:
        out = np.maximum(out / norm, -1.0)
    return out


def quat_matrix(q):
    x, y, z, w = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def trs(t, r, s):
    m = np.eye(4)
    m[:3, :3] = quat_matrix(r) * np.asarray(s)
    m[:3, 3] = t
    return m


def slerp(a, b, f):
    d = float(np.dot(a, b))
    if d < 0:
        b, d = -b, -d
    if d > 0.9995:
        q = a + (b - a) * f
    else:
        th = math.acos(d)
        q = (math.sin((1 - f) * th) * a + math.sin(f * th) * b) / math.sin(th)
    return q / np.linalg.norm(q)


class Skeleton:
    def __init__(self, gltf, blob):
        self.gltf, self.blob = gltf, blob
        self.nodes = gltf["nodes"]
        self.parent = {}
        for i, n in enumerate(self.nodes):
            for c in n.get("children", []):
                self.parent[c] = i
        self.names = [n.get("name", f"node{i}") for i, n in enumerate(self.nodes)]
        joints = set()
        for skin in gltf.get("skins", []):
            joints.update(skin["joints"])
        if not joints:
            sys.exit("the GLB has no skin: rig it first (Mixamo, Meshy, or by hand)")
        self.joints = sorted(joints)
        self.clips = {a.get("name", f"clip{k}"): a for k, a in enumerate(gltf.get("animations", []))}

    def rest(self, i):
        n = self.nodes[i]
        if "matrix" in n:
            return np.array(n["matrix"]).reshape(4, 4).T
        return trs(n.get("translation", [0, 0, 0]), n.get("rotation", [0, 0, 0, 1]), n.get("scale", [1, 1, 1]))

    def tracks(self, clip):
        """Each animated node's paths: (times, values, interpolation)."""
        anim = self.clips[clip]
        out = {}
        for ch in anim["channels"]:
            node = ch["target"].get("node")
            if node is None:
                continue
            sampler = anim["samplers"][ch["sampler"]]
            times = accessor(self.gltf, self.blob, sampler["input"])[:, 0]
            values = accessor(self.gltf, self.blob, sampler["output"])
            interp = sampler.get("interpolation", "LINEAR")
            if interp == "CUBICSPLINE":
                values = values[1::3]
            out.setdefault(node, {})[ch["target"]["path"]] = (times, values, interp)
        return out

    def duration(self, clip):
        return max(t[0][-1] for paths in self.tracks(clip).values() for t in paths.values())

    def local(self, i, tracks, t):
        n = self.nodes[i]
        paths = tracks.get(i) if tracks else None
        if not paths:
            return self.rest(i)
        value = {"translation": n.get("translation", [0, 0, 0]), "rotation": n.get("rotation", [0, 0, 0, 1]), "scale": n.get("scale", [1, 1, 1])}
        for path, (times, values, interp) in paths.items():
            if path not in value:
                continue
            k = int(np.searchsorted(times, t, side="right")) - 1
            if k < 0:
                v = values[0]
            elif k >= len(times) - 1:
                v = values[-1]
            elif interp == "STEP":
                v = values[k]
            else:
                f = (t - times[k]) / max(times[k + 1] - times[k], 1e-9)
                v = slerp(values[k], values[k + 1], f) if path == "rotation" else values[k] + (values[k + 1] - values[k]) * f
            value[path] = v
        return trs(value["translation"], value["rotation"], value["scale"])

    def world(self, i, tracks=None, t=0.0):
        m = self.local(i, tracks, t)
        while i in self.parent:
            i = self.parent[i]
            m = self.local(i, tracks, t) @ m
        return m

    def find(self, role, side=None):
        """The joint for `role` (on `side`, L or R), by its name."""
        words = ROLES[role]
        best = None
        for j in self.joints:
            on, base = side_of(self.names[j].lower().replace("mixamorig:", "").replace("mixamorig_", ""))
            if on != side or base not in words:
                continue
            if best is None or words.index(base) < words.index(best[1]):
                best = (j, base)
        return None if best is None else best[0]


def side_of(name):
    """Which side a joint is on by its name (L, R or None), and its name
    without it: LeftUpLeg, UpperLeg.L, l_thigh, thigh_l."""
    for side, (before, after) in SIDES.items():
        for tag in before:
            if name.startswith(tag):
                return side, name[len(tag):]
        for tag in after:
            if name.endswith(tag):
                return side, name[: -len(tag)]
    return None, name


def clip_named(skel, role, asked):
    if asked is not None:
        if asked and asked not in skel.clips:
            sys.exit(f"no clip {asked!r} for {role}: the GLB has {sorted(skel.clips)}")
        return asked
    # By any word of its name: Walking, walk_forward, Armature|Walking|baselayer.
    for name in skel.clips:
        if any(word in CLIPS[role] for word in re.split(r"[|._\s-]+", name.lower())):
            return name
    return ""


def measure_gait(skel, clip, feet, height):
    """A walking clip's ground speed (model units/s), which way it faces, and
    where in it the left foot comes down (of its length)."""
    tracks = skel.tracks(clip)
    duration = skel.duration(clip)
    n = 240
    times = np.linspace(0.0, duration, n, endpoint=False)
    pos = np.array([[skel.world(f, tracks, t)[:3, 3] for f in feet] for t in times])  # (n, 2, 3)
    dt = duration / n
    speeds, ways, down_at = [], [], None
    for k in range(len(feet)):
        y = pos[:, k, 1]
        # On the ground: within 3% of its height of its lowest.
        down = y < y.min() + 0.03 * height
        vel = (np.roll(pos[:, k], -1, axis=0) - np.roll(pos[:, k], 1, axis=0)) / (2 * dt)
        flat = vel[down][:, [0, 2]]
        if len(flat) == 0:
            continue
        # (Its distance on the ground over its time there: a stylised clip's
        # foot goes back unevenly, and its median was a fifth too fast.)
        speeds.append(np.linalg.norm(flat, axis=1).mean())
        ways.extend(-flat)
        if k == 0:
            starts = [i for i in range(n) if down[i] and not down[i - 1]]
            down_at = (starts[0] / n) if starts else 0.0
    speed = float(np.mean(speeds))
    way = np.mean(ways, axis=0)
    way = way / max(np.linalg.norm(way), 1e-9)
    # How far its feet slide while down when played at that speed (model
    # units/s): the clip's own fault, which no playing of it can mend.
    slides = []
    for k in range(len(feet)):
        y = pos[:, k, 1]
        down = y < y.min() + 0.03 * height
        vel = (np.roll(pos[:, k], -1, axis=0) - np.roll(pos[:, k], 1, axis=0)) / (2 * dt)
        slides.extend(np.linalg.norm(vel[down][:, [0, 2]] + way * speed, axis=1))
    return speed, (float(way[0]), float(way[1])), down_at or 0.0, duration, float(np.median(slides))


def measure_impact(skel, clip, hands, hips):
    """When an attack lands (of its length): its hands fastest, about its
    hips."""
    tracks = skel.tracks(clip)
    duration = skel.duration(clip)
    n = 240
    times = np.linspace(0.0, duration, n, endpoint=False)
    rel = np.array([[skel.world(h, tracks, t)[:3, 3] - skel.world(hips, tracks, t)[:3, 3] for h in hands] for t in times])
    speed = np.linalg.norm(np.diff(rel, axis=0), axis=2).max(axis=1)
    return (int(np.argmax(speed)) + 0.5) / n, duration


def model_height(skel):
    """How tall it stands at rest (model units), and where its lowest point
    is: its skinned meshes' vertices, each moved by its joints at rest."""
    gltf = skel.gltf
    ys = []
    for i, node in enumerate(skel.nodes):
        if "mesh" not in node:
            continue
        if "skin" not in node:
            # (A rigid piece on a joint, as most of the robot's are.)
            m = skel.world(i)
            for prim in gltf["meshes"][node["mesh"]]["primitives"]:
                v = accessor(gltf, skel.blob, prim["attributes"]["POSITION"])
                ys.extend((v @ m[:3, :3].T + m[:3, 3])[:, 1])
            continue
        skin = gltf["skins"][node["skin"]]
        inverse = accessor(gltf, skel.blob, skin["inverseBindMatrices"]).reshape(-1, 4, 4).transpose(0, 2, 1)
        moves = np.array([skel.world(j) @ inverse[k] for k, j in enumerate(skin["joints"])])
        for prim in gltf["meshes"][node["mesh"]]["primitives"]:
            at = prim["attributes"]
            if "JOINTS_0" not in at:
                continue
            v = accessor(gltf, skel.blob, at["POSITION"])
            v = np.hstack([v, np.ones((len(v), 1))])
            joints = accessor(gltf, skel.blob, at["JOINTS_0"]).astype(int)
            weights = accessor(gltf, skel.blob, at["WEIGHTS_0"])
            blended = np.einsum("vk,vkij->vij", weights, moves[joints])
            ys.extend(np.einsum("vij,vj->vi", blended, v)[:, 1])
    if not ys:
        ys = [skel.world(j)[1, 3] for j in skel.joints]
    return max(ys) - min(ys), min(ys)


def read_kept(path):
    kept = {}
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.split("#", 1)[0].strip()
            if "=" in line:
                k, v = (s.strip() for s in line.split("=", 1))
                kept[k] = v
    return kept


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("glb")
    p.add_argument("--height", type=float, help="how tall it stands in the game (m); default: as before, else 2.4")
    for role in CLIPS:
        p.add_argument(f"--{role}", help=f"its {role} clip's name ('' for none); default: found by name")
    a = p.parse_args()

    glb = Path(a.glb)
    gltf, blob = read_glb(glb)
    skel = Skeleton(gltf, blob)
    out = glb.with_suffix(".creature")
    kept = read_kept(out)

    def need(role, side=None):
        j = skel.find(role, side)
        if j is None:
            sys.exit(f"no {role}{' ' + side if side else ''} joint found among {[skel.names[j] for j in skel.joints]}: rename it, or add its name to ROLES")
        return j

    hips, neck, head = need("hips"), need("neck"), need("head")
    spine = [neck]
    while spine[-1] != hips:
        if spine[-1] not in skel.parent:
            sys.exit(f"{skel.names[neck]} does not hang from {skel.names[hips]}")
        spine.append(skel.parent[spine[-1]])
    spine.reverse()
    legs = [[need(r, s) for r in ("thigh", "shin", "foot")] for s in ("L", "R")]
    arms = [[need(r, s) for r in ("upper_arm", "forearm", "hand")] for s in ("L", "R")]

    tall, floor = model_height(skel)
    head_y = skel.world(head)[1, 3] - floor
    clips = {role: clip_named(skel, role, getattr(a, role)) for role in CLIPS}
    for role in ("walk", "run"):
        if not clips[role]:
            sys.exit(f"no {role} clip found among {sorted(skel.clips)}: name it with --{role}")
    feet = [legs[0][2], legs[1][2]]
    gaits = {role: measure_gait(skel, clips[role], feet, tall) for role in ("walk", "run")}
    # (In place: the game moves the body; a clip that moves it too would
    # carry it twice as far, its feet sliding.)
    for role in ("walk", "run"):
        tracks = skel.tracks(clips[role])
        duration = gaits[role][3]
        went = skel.world(hips, tracks, duration * 0.999)[:3, 3] - skel.world(hips, tracks, 0.0)[:3, 3]
        if np.linalg.norm(went[[0, 2]]) / duration > 0.2 * gaits[role][0]:
            sys.exit(f"{role} ({clips[role]}) moves the body itself, {np.linalg.norm(went[[0, 2]]):.2f} units over the clip: export it in place (Mixamo: tick In Place)")
    forward = gaits["walk"][1]

    height = a.height or float(kept.get("height", 2.4))
    scale = height / tall
    names = lambda js: " ".join(skel.names[j] for j in js)
    lines = [
        f"# {out.name}: what the game needs to know of {glb.name}, measured by",
        "# tools/creature_clips.py. Edit height, the clips and the joints freely;",
        "# run the tool again after changing the GLB (measured lines are rewritten).",
        f"model = {glb.name}",
        f"# How tall it stands in the game (m): {scale:.4f} m to the model's unit.",
        f"height = {height:g}",
        f"# Measured: how tall the model stands and where its lowest point is (its units); how far its top is above its head joint; which way it faces (x z).",
        f"model_height = {tall:.4f}",
        f"model_floor = {floor:.4f}",
        f"head_length = {tall - head_y:.4f}",
        f"forward = {forward[0]:.3f} {forward[1]:.3f}",
        "# Clips. Walk and run: name, ground speed (model units/s, measured), where the",
        "# left foot comes down (of the clip), length (s).",
    ]
    lines.append("# Attack: name, when it lands (of the clip, measured: its hands fastest), length (s).")
    lines.append("# The rest: name, length (s). A clip left empty is not played.")
    for role in CLIPS:
        if role in gaits:
            speed, _, down_at, duration, slide = gaits[role]
            lines.append(f"{role} = {clips[role]} {speed:.4f} {down_at:.3f} {duration:.3f}   # {speed * scale:.2f} m/s, its feet sliding {slide * scale:.2f} m/s while down")
        elif role == "attack" and clips[role]:
            impact, duration = measure_impact(skel, clips[role], [arms[0][2], arms[1][2]], hips)
            lines.append(f"{role} = {clips[role]} {impact:.3f} {duration:.3f}")
        elif clips[role]:
            lines.append(f"{role} = {clips[role]} {skel.duration(clips[role]):.3f}")
        else:
            lines.append(f"{role} =")
    lines += [
        "# Joints: the spine from the hips up to the neck, the head, and each leg and",
        "# arm from its root out (left, right): its hits are taken on them.",
        f"spine = {names(spine)}",
        f"head = {skel.names[head]}",
        f"legs = {names(legs[0])}, {names(legs[1])}",
        f"arms = {names(arms[0])}, {names(arms[1])}",
        "# How thick each is (m): torso, neck, head, leg, arm.",
        f"radii = {kept.get('radii', '0.3 0.12 0.2 0.12 0.09')}",
    ]
    # Anything else given by hand (`shards = right`, ...), kept as it was.
    written = {l.split("=", 1)[0].strip() for l in lines if "=" in l and not l.startswith("#")}
    lines += [f"{k} = {v}" for k, v in kept.items() if k not in written]
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(l for l in lines if not l.startswith("#")))
    print(f"wrote {out}")
    for role in ("walk", "run"):
        speed, slide = gaits[role][0] * scale, gaits[role][4] * scale
        print(f"{role}: {speed:.2f} m/s at {height:g} m tall; its feet slide {slide:.2f} m/s while down (the clip's own; lower is better)")


if __name__ == "__main__":
    main()
