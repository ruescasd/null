"""The navigation bench's verdicts (tools/navbench/CRITERIA.md): every
candidate's results/<name>.json read against the criteria, strictly, worst
cases not means. `python tools/navbench/judge.py target/navbench`."""

import json
import pathlib
import sys

WORLD_KM2 = 16.384 ** 2
PATCH_KM2 = 4.0
FRAME_BUDGET_MS = 2.0
LATENCY_FRAMES = 30


def pct(values, q):
    s = sorted(values)
    return s[min(len(s) - 1, int(len(s) * q))] if s else 0.0


def verdicts(r):
    q, runs, out = r["queries"], r["runs"], []

    def must_reach(name):
        rows = q[name]
        nofloor = sum(1 for x in rows if x[0] == -1)
        noway = sum(1 for x in rows if x[0] == 0)
        long = sum(1 for x in rows if x[0] == 1 and x[1] > 3 * x[2] + 1)
        return nofloor + noway + long == 0, f"{len(rows)} pairs: {nofloor} no floor, {noway} no way, {long} a long way round"

    out.append(("1 straight ground truth", *must_reach("straight")))
    out.append(("2 flat floor across tile edges", *must_reach("seam_flat")))
    steps = [x for x in q["seam_step"] if x[0] != -1]
    joined = sum(1 for x in steps if x[0] == 1 and x[1] < x[2] + 1)
    out.append(("3 real steps across tile edges", joined <= 0.01 * len(steps), f"{joined} of {len(steps)} joined straight across"))
    out.append(("4 no stalled way", r.get("stalled", 0) == 0, f"{r.get('stalled', 0)} stalled"))
    worst = {k: max(v["frames_ms"]) for k, v in runs.items()}
    out.append(("5 worst frame <= 2 ms", all(w <= FRAME_BUDGET_MS for w in worst.values()), ", ".join(f"{k} {w:.2f} ms (99th {pct(runs[k]['frames_ms'], 0.99):.2f})" for k, w in worst.items())))
    goal_runs = [k for k in ("reachable", "burst", "planned", "stream") if k in runs]
    late = {}
    for k in goal_runs:
        s = runs[k]["started_frame"]
        late[k] = sum(1 for f in s if f < 0 or f > LATENCY_FRAMES), len(s)
    out.append(("6 99% start within 0.5 s", all(n <= 0.01 * m for n, m in late.values()), ", ".join(f"{k} {n} of {m} late" for k, (n, m) in late.items())))
    stuck = {k: runs[k]["stuck"] for k in goal_runs}
    out.append(("7 none stuck alone", all(v == 0 for v in stuck.values()), ", ".join(f"{k} {v}" for k, v in stuck.items())))
    s = runs["stream"]
    out.append(("8 streaming holds", worst["stream"] <= FRAME_BUDGET_MS and s["stuck"] == 0, f"{s['added']} added, {s['removed']} removed, at most {s['resident_max']}; churn worst {max(s['churn_ms']):.2f} ms"))
    return out


def recorded(r):
    lines = []
    for body, b in r["build"].items():
        world_h = b["tiles"] * WORLD_KM2 / PATCH_KM2 * b["ms_per_tile_core"] / 1000 / 11 / 3600
        lines.append(f"build {body}: {b['tiles']} tiles, {b['polygons']} polygons, {b['ms_per_tile_core']:.0f} ms a tile a core, {b['bytes'] / 1e6:.0f} MB; world bake about {world_h:.2f} h on 11 workers")
    for name in ("near", "far"):
        rows = r["queries"][name]
        t = [x[3] for x in rows if x[0] != -1]
        lines.append(f"ways {name}: {sum(1 for x in rows if x[0] == 1)} of {len(rows)} found ({sum(1 for x in rows if x[0] == -1)} off any floor); mean {sum(t) / max(1, len(t)):.3f} ms, 99th {pct(t, 0.99):.3f}, worst {max(t, default=0):.3f}")
    for k, v in r["runs"].items():
        extra = f", kept up {v['kept_up']} of {v['agents']}" if "kept_up" in v else f", arrived {v['arrived']} of {v['agents']}"
        if "routed_frame" in v:
            extra += f"; {v['iters']} search steps a frame, routes in by {pct(v['routed_frame'], 0.99) / 60:.1f} s (99th), last {max(v['routed_frame']) / 60:.1f} s, {v['partial']} partial"
        lines.append(f"run {k}: {v['agents']} agents, mean {sum(v['frames_ms']) / len(v['frames_ms']):.3f} ms, jammed {v['jammed']}{extra}")
    if "out_of_nodes" in r:
        lines.append(f"searches out of nodes: {r['out_of_nodes']}")
    return lines


def main():
    d = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "target/navbench")
    results = {p.stem: json.loads(p.read_text()) for p in sorted((d / "results").glob("*.json"))}
    for name, r in results.items():
        v = verdicts(r)
        print(f"\n== {name}: {r['engine']}: {'PASS' if all(x[1] for x in v) else 'FAIL'}")
        for label, ok, note in v:
            print(f"  {'pass' if ok else 'FAIL'}  {label}: {note}")
        for line in recorded(r):
            print(f"        {line}")
    # Where candidates disagree on whether a way exists.
    names = list(results)
    for g in ("straight", "seam_flat", "near", "far"):
        for i in range(len(names)):
            for j in range(i + 1, len(names)):
                a, b = results[names[i]]["queries"][g], results[names[j]]["queries"][g]
                only_a = sum(1 for x, y in zip(a, b) if x[0] == 1 and y[0] != 1)
                only_b = sum(1 for x, y in zip(a, b) if y[0] == 1 and x[0] != 1)
                if only_a or only_b:
                    print(f"\n{g}: a way only on {names[i]} {only_a}, only on {names[j]} {only_b}")


if __name__ == "__main__":
    main()
