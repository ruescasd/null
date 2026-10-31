"""Merge the clips of several GLBs of one rigged creature into one GLB.

    python tools/merge_clips.py OUT.glb BASE.glb OTHER.glb [OTHER.glb ...]

Riggers like Meshy and Mixamo give one file per clip, each holding the same
skinned mesh. This keeps BASE whole and adds every other file's animations to
it, matching their joints by name; a joint missing from BASE is an error.
"""

import json
import struct
import sys
from pathlib import Path


def read(path):
    data = Path(path).read_bytes()
    if data[:4] != b"glTF":
        sys.exit(f"{path} is not a binary glTF (GLB)")
    n = struct.unpack("<I", data[12:16])[0]
    gltf = json.loads(data[20 : 20 + n])
    rest = data[20 + n :]
    blob = rest[8 : 8 + struct.unpack("<I", rest[:4])[0]] if rest else b""
    return gltf, bytearray(blob)


def write(path, gltf, blob):
    text = json.dumps(gltf, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    blob = bytes(blob) + b"\0" * (-len(blob) % 4)
    out = struct.pack("<III", 0x46546C67, 2, 28 + len(text) + len(blob))
    out += struct.pack("<II", len(text), 0x4E4F534A) + text
    out += struct.pack("<II", len(blob), 0x004E4942) + blob
    Path(path).write_bytes(out)


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    out, base, others = sys.argv[1], sys.argv[2], sys.argv[3:]
    gltf, blob = read(base)
    nodes = {n.get("name"): i for i, n in enumerate(gltf["nodes"])}
    names = {a.get("name") for a in gltf.get("animations", [])}
    for path in others:
        g, b = read(path)
        for anim in g.get("animations", []):
            if anim.get("name") in names:
                sys.exit(f"{path}: a clip named {anim.get('name')!r} is already in the merge")
            moved = {}

            def copy(index):
                # One accessor and its bytes, appended to the base's buffer.
                if index not in moved:
                    acc = dict(g["accessors"][index])
                    view = dict(g["bufferViews"][acc["bufferView"]])
                    start = view.get("byteOffset", 0)
                    blob.extend(b"\0" * (-len(blob) % 4))
                    view["byteOffset"] = len(blob)
                    view["buffer"] = 0
                    blob.extend(b[start : start + view["byteLength"]])
                    gltf["bufferViews"].append(view)
                    acc["bufferView"] = len(gltf["bufferViews"]) - 1
                    gltf["accessors"].append(acc)
                    moved[index] = len(gltf["accessors"]) - 1
                return moved[index]

            samplers = [
                dict(s, input=copy(s["input"]), output=copy(s["output"])) for s in anim["samplers"]
            ]
            channels = []
            for c in anim["channels"]:
                name = g["nodes"][c["target"]["node"]].get("name")
                if name not in nodes:
                    sys.exit(f"{path}: clip {anim.get('name')!r} moves {name!r}, which {base} has no node for")
                channels.append(dict(c, target=dict(c["target"], node=nodes[name])))
            gltf.setdefault("animations", []).append(
                {"name": anim.get("name"), "samplers": samplers, "channels": channels}
            )
            names.add(anim.get("name"))
    gltf["buffers"][0]["byteLength"] = len(blob) + (-len(blob) % 4)
    write(out, gltf, blob)
    print(f"{out}: clips {sorted(n for n in names if n)}")


if __name__ == "__main__":
    main()
