# Concept-art tools (prototype, 2026-10-18)

Python 3.12; `pip install numpy pillow scipy trimesh pyrender PyOpenGL manifold3d`.

- `split_turnaround.py <sheet.jpg> <outdir> front,profile,back` — cuts a flat-background turnaround sheet into per-view RGBA PNGs.
- `lattice_head.py <out.glb> [--set param=value ...]` — procedural lattice head (egg shell + weighted surface Voronoi + chamfered ribbons, unioned). Parameters and their meaning at the top of the file.
- `compare_views.py <model.glb> <views_dir> <out.png>` — concept views on top, the model rendered from the same directions below, same scale.
- `render_glb.py <model.glb> <out.png> [size]` — front/left/back/top/three-quarter contact sheet of any GLB.
