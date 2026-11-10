# The navigation bench on Godot 4.7.2's navigation server as shipped
# (tools/navbench/CRITERIA.md): each tile baked by the server's own Recast
# with the bench's settings, the tiles regions of one map, the cases asked by
# query_path, and the runs walked by NavigationAgent3D with avoidance.
# Godot_console --headless --path tools/navbench/godot --script bench.gd
#   --fixed-fps 60 -- <bench dir>
# writes results/godot.json there.
extends SceneTree

var dir: String
var bodies := []          # [{name, s: {key: value}}]
var groups := {}          # name -> {reach, points: [[Vector3...]]}
var json := {}
var state := "bake"
var baking := 0
var bake_started := 0
var body_index := 0
var meshes := {}          # body name -> {Vector2i: NavigationMesh}
var map: RID
var run: Dictionary
var runs := []
var run_index := 0
var nav_process_ms := 0.0
var joined_at := 0
var synced := 0


func _initialize() -> void:
	dir = OS.get_cmdline_user_args()[0]
	for line in FileAccess.get_file_as_string(dir + "/settings.txt").split("\n"):
		var w := line.split(" ", false)
		if w.size() == 0 or w[0] != "body":
			continue
		var s := {}
		for i in range(2, w.size(), 2):
			s[w[i]] = float(w[i + 1])
		bodies.append({"name": w[1], "s": s})
	var current := ""
	for line in FileAccess.get_file_as_string(dir + "/cases.txt").split("\n"):
		var w := line.split(" ", false)
		if w.size() == 0:
			continue
		if w[0] == "group":
			current = w[1]
			groups[current] = {"reach": float(w[2]), "points": []}
			continue
		var pts := []
		for i in range(0, w.size(), 3):
			pts.append(Vector3(float(w[i]), float(w[i + 1]), float(w[i + 2])))
		groups[current]["points"].append(pts)
	json["engine"] = "Godot %s navigation server, unpatched%s" % [Engine.get_version_info()["string"], (", edge connections off" if OS.get_environment("NB_NOEDGE") != "" else "") + (", tiles %s (chunks demo: border, snapped)" % tiles_spec if tiles_spec != "" else "")]
	json["build"] = {}
	json["runs"] = {}
	start_bake(bodies[0])


# (NB_TILES=x<scale>-b<border>: tiles `scale` times the bench's side, each
# baked with `border` tiles of its neighbours round it, its vertices
# snapped to a tenth of the map's cell: Godot's chunks demo, 3d/
# navigation_mesh_chunks, does one and snaps so. By default the bench's
# tiles and Recast's least border, unsnapped.)
var tiles_spec := OS.get_environment("NB_TILES")
var tile_scale := int(tiles_spec.get_slice("-", 0).trim_prefix("x")) if tiles_spec != "" else 1
var tile_border := int(tiles_spec.get_slice("-", 1).trim_prefix("b")) if tiles_spec != "" else 0
var tiles_suffix := "-" + tiles_spec if tiles_spec != "" else ""


func baked(nm: NavigationMesh) -> void:
	baking -= 1
	if tiles_spec == "":
		return
	var cell: float = bodies[0]["s"]["cell_size"] * 0.1
	var v := nm.vertices
	for i in v.size():
		v[i] = v[i].snappedf(cell)
	nm.vertices = v


# Every tile of `body` baked in the background, its border round it.
func start_bake(body: Dictionary) -> void:
	var s: Dictionary = body["s"]
	var side: float = s["tile_side"] * tile_scale
	var pad: float = tile_border * side if tile_border > 0 else s["border_cells"] * s["cell_size"]
	meshes[body["name"]] = {}
	var heights := Array(FileAccess.get_file_as_string(dir + "/tiles/heights.txt").split(" ", false)).map(func(v): return float(v))
	bake_started = Time.get_ticks_usec()
	bake_body = body
	bake_heights = heights
	bake_queue = Array(DirAccess.get_files_at(dir + "/tiles/" + body["name"] + tiles_suffix))
	feed_bakes()


var bake_body: Dictionary
var bake_heights: Array
var bake_queue := []


# (At most 24 tiles baking, each read only as it starts: all of them at once,
# bordered by their neighbours, ran out of memory.)
func feed_bakes() -> void:
	var body := bake_body
	var s: Dictionary = body["s"]
	var side: float = s["tile_side"] * tile_scale
	var pad: float = tile_border * side if tile_border > 0 else s["border_cells"] * s["cell_size"]
	var heights := bake_heights
	while baking < 24 and not bake_queue.is_empty():
		var f: String = bake_queue.pop_back()
		var key := f.get_basename().split("_")
		var k := Vector2i(int(key[0]), int(key[1]))
		var bytes := FileAccess.get_file_as_bytes(dir + "/tiles/" + body["name"] + tiles_suffix + "/" + f)
		var nv := bytes.decode_s32(0)
		var ni := bytes.decode_s32(4)
		var geo := NavigationMeshSourceGeometryData3D.new()
		geo.set_vertices(bytes.slice(8, 8 + nv * 12).to_float32_array())
		geo.set_indices(bytes.slice(8 + nv * 12, 8 + nv * 12 + ni * 4).to_int32_array())
		var nm := NavigationMesh.new()
		nm.cell_size = s["cell_size"]
		nm.cell_height = s["cell_height"]
		nm.agent_radius = s["radius"]
		nm.agent_height = s["height"]
		nm.agent_max_climb = s["climb"]
		nm.agent_max_slope = s["slope"]
		nm.region_min_size = sqrt(s["min_region_area"])
		nm.region_merge_size = sqrt(s["merge_region_area"])
		nm.edge_max_length = s["max_edge_len"] * s["cell_size"]
		nm.edge_max_error = s["max_error"]
		nm.vertices_per_polygon = s["max_vertices"]
		nm.detail_sample_distance = s["detail_sample_dist"] / s["cell_size"]
		nm.detail_sample_max_error = s["detail_sample_max_error"] / s["cell_height"]
		nm.filter_low_hanging_obstacles = true
		nm.filter_ledge_spans = true
		nm.filter_walkable_low_height_spans = true
		# (A hair under the cells it is: Godot rounds it up, and in f32 0.7 m
		# is just over 6 cells, so it trimmed a cell more than the box was
		# grown by, and no tile's edge met its neighbour's.)
		nm.border_size = pad - 0.01 * s["cell_size"]
		# (Godot bakes within this box, its border inside it: the box is the
		# tile and its border.)
		nm.filter_baking_aabb = AABB(Vector3(k.x * side - pad, heights[0], k.y * side - pad), Vector3(side + 2.0 * pad, heights[1] - heights[0], side + 2.0 * pad))
		meshes[body["name"]][k] = nm
		baking += 1
		NavigationServer3D.bake_from_source_geometry_data_async(nm, geo, func(): baked(nm))


func on(p: Vector3, reach: float) -> Variant:
	var c := NavigationServer3D.map_get_closest_point(map, p)
	if Vector2(c.x - p.x, c.z - p.z).length() <= reach and c.y - p.y <= 1.0 and p.y - c.y <= 6.0:
		return c
	return null


# The way's length from `a` to `b`, or -1 for none (a way to somewhere
# else, the nearest it could get).
func way(a: Vector3, b: Vector3, limit := 0) -> float:
	var q := NavigationPathQueryParameters3D.new()
	q.map = map
	q.start_position = a
	q.target_position = b
	q.path_search_max_polygons = limit
	var r := NavigationPathQueryResult3D.new()
	NavigationServer3D.query_path(q, r)
	var path := r.path
	if path.size() == 0 or path[path.size() - 1].distance_to(b) > 0.1:
		return -1.0
	var l := 0.0
	for i in range(1, path.size()):
		l += path[i - 1].distance_to(path[i])
	return l + a.distance_to(path[0])


func _physics_process(_delta: float) -> bool:
	nav_process_ms = Performance.get_monitor(Performance.TIME_NAVIGATION_PROCESS) * 1000.0
	match state:
		"bake":
			feed_bakes()
			if baking > 0 or not bake_queue.is_empty():
				return false
			var body: Dictionary = bodies[body_index]
			var seconds := (Time.get_ticks_usec() - bake_started) / 1e6
			var tiles := 0
			var polygons := 0
			var bytes := 0
			for nm in meshes[body["name"]].values():
				if nm.get_polygon_count() == 0:
					continue
				tiles += 1
				polygons += nm.get_polygon_count()
				bytes += nm.get_vertices().size() * 12
				for i in nm.get_polygon_count():
					bytes += nm.get_polygon(i).size() * 4
			var workers := 12
			json["build"][body["name"]] = {"tiles": tiles, "polygons": polygons, "seconds": seconds, "workers": workers, "ms_per_tile_core": seconds * 1000.0 * workers / max(1, tiles), "bytes": bytes}
			print("%s: %d tiles, %d polygons, baked in %.1f s on %d threads, %.0f MB" % [body["name"], tiles, polygons, seconds, workers, bytes / 1e6])
			body_index += 1
			if body_index < bodies.size():
				start_bake(bodies[body_index])
			else:
				state = "join"
		"join":
			var s: Dictionary = bodies[0]["s"]
			map = NavigationServer3D.map_create()
			NavigationServer3D.map_set_cell_size(map, s["cell_size"])
			NavigationServer3D.map_set_cell_height(map, s["cell_height"])
			NavigationServer3D.map_set_use_async_iterations(map, async_regions)
			# (NB_NOEDGE=1: tiles joined only where their edges' corners meet,
			# not by distance too; Godot's search for those is quadratic in
			# the map's free edges: 400 tiles 150 s, all 1311 about 30 min.)
			NavigationServer3D.map_set_use_edge_connections(map, OS.get_environment("NB_NOEDGE") == "")
			NavigationServer3D.map_set_active(map, true)
			var t := Time.get_ticks_usec()
			# (NB_SCALE=n: the map's first synchronisation timed with n, 2n,
			# 4n... tiles nearest the middle, on the main thread; then stop.)
			if OS.get_environment("NB_SCALE") != "":
				var keys: Array = meshes[bodies[0]["name"]].keys()
				var side: float = bodies[0]["s"]["tile_side"]
				keys.sort_custom(func(a, b): return Vector2(a).distance_to(Vector2(8192, 8192) / side) < Vector2(b).distance_to(Vector2(8192, 8192) / side))
				var n := int(OS.get_environment("NB_SCALE"))
				while n <= keys.size():
					var m := NavigationServer3D.map_create()
					NavigationServer3D.map_set_cell_size(m, s["cell_size"])
					NavigationServer3D.map_set_cell_height(m, s["cell_height"])
					NavigationServer3D.map_set_use_async_iterations(m, false)
					NavigationServer3D.map_set_use_edge_connections(m, OS.get_environment("NB_NOEDGE") == "")
					NavigationServer3D.map_set_active(m, true)
					var polys := 0
					var made := []
					for k in keys.slice(0, n):
						var r := NavigationServer3D.region_create()
						NavigationServer3D.region_set_use_async_iterations(r, false)
						NavigationServer3D.region_set_map(r, m)
						NavigationServer3D.region_set_navigation_mesh(r, meshes[bodies[0]["name"]][k])
						polys += meshes[bodies[0]["name"]][k].get_polygon_count()
						made.append(r)
					var t0 := Time.get_ticks_usec()
					NavigationServer3D.map_force_update(m)
					var took := (Time.get_ticks_usec() - t0) / 1e6
					print("scale: %d tiles, %d polygons: map synchronised in %.2f s; closest %s" % [n, polys, took, NavigationServer3D.map_get_closest_point(m, groups["route"]["points"][0][0])])
					for r in made:
						NavigationServer3D.free_rid(r)
					NavigationServer3D.free_rid(m)
					n *= 2
				quit()
				return false
			for k in meshes[bodies[0]["name"]]:
				add_region(k)
			joined_at = t
			synced = NavigationServer3D.map_get_iteration_id(map)
			state = "sync"
		"sync":
			# (Godot builds regions and synchronises a map in the background:
			# asked before, it answers nothing. Ready once every region is
			# built and the map synchronised since.)
			if not ready():
				return false
			print("joined in %.2f s" % ((Time.get_ticks_usec() - joined_at) / 1e6))
			var merges := {"edges": Performance.NAVIGATION_EDGE_COUNT, "merged": Performance.NAVIGATION_EDGE_MERGE_COUNT, "connected": Performance.NAVIGATION_EDGE_CONNECTION_COUNT, "free": Performance.NAVIGATION_EDGE_FREE_COUNT}
			for m in merges:
				merges[m] = Performance.get_monitor(merges[m])
			print("edges: ", merges)
			json["edges"] = merges
			queries()
			runs = [["reachable", "agents", true, 40], ["unreachable", "agents", false, 40], ["burst", "burst", true, 30], ["stream", "route", true, 0]]
			start_run()
			state = "run"
		"stream_sync":
			if not ready():
				return false
			state = "run"
		"run":
			if step_run():
				run_index += 1
				if run_index < runs.size():
					start_run()
					if run["name"] == "stream":
						state = "stream_sync"
				else:
					var f := FileAccess.open(dir + "/results/" + ("godot" + ("-noedge" if OS.get_environment("NB_NOEDGE") != "" else "") + tiles_suffix) + ".json", FileAccess.WRITE)
					f.store_string(JSON.stringify(json))
					f.close()
					return true
	return false


var regions := {}
# (NB_SYNC=1: regions built and the map synchronised on the main thread;
# by default, as Godot ships, off it.)
var async_regions := OS.get_environment("NB_SYNC") == ""
var regions_built := false
var steady := 0


var waited := 0
var last_said := 0
var steady_since := 0


func ready() -> bool:
	waited += 1
	if Time.get_ticks_msec() - last_said > 2000:
		last_said = Time.get_ticks_msec()
		var unbuilt := 0
		for r in regions.values():
			if NavigationServer3D.region_get_iteration_id(r) == 0:
				unbuilt += 1
		print("waiting at %.0f s: %d of %d regions unbuilt, map iteration %d, built %s, steady %d, closest %s" % [Time.get_ticks_msec() / 1000.0, unbuilt, regions.size(), NavigationServer3D.map_get_iteration_id(map), regions_built, steady, NavigationServer3D.map_get_closest_point(map, groups["route"]["points"][0][0])])
	if not regions_built:
		for r in regions.values():
			if NavigationServer3D.region_get_iteration_id(r) == 0:
				return false
		regions_built = true
		NavigationServer3D.map_force_update(map)
		synced = NavigationServer3D.map_get_iteration_id(map)
		steady_since = Time.get_ticks_msec()
		return false
	var now := NavigationServer3D.map_get_iteration_id(map)
	if now != synced:
		synced = now
		steady_since = Time.get_ticks_msec()
		return false
	# (Steady for a second of real time, and answering.)
	steady = Time.get_ticks_msec() - steady_since
	if steady < 1000 or NavigationServer3D.map_get_closest_point(map, groups["route"]["points"][0][0]) == Vector3.ZERO:
		return false
	regions_built = false
	return true


func add_region(k: Vector2i) -> void:
	var nm: NavigationMesh = meshes[bodies[0]["name"]][k]
	if nm.get_polygon_count() == 0:
		return
	var r := NavigationServer3D.region_create()
	NavigationServer3D.region_set_use_async_iterations(r, async_regions)
	NavigationServer3D.region_set_map(r, map)
	NavigationServer3D.region_set_navigation_mesh(r, nm)
	regions[k] = r


func queries() -> void:
	json["queries"] = {}
	# (NB_QPROBE=1: what a query's time goes on, and stop.)
	if OS.get_environment("NB_QPROBE") != "":
		var p: Array = groups["straight"]["points"][0]
		var a: Vector3 = on(p[0], 0.5)
		var b: Vector3 = on(p[1], 0.5)
		var t := Time.get_ticks_usec()
		for i in 10:
			NavigationServer3D.map_get_closest_point(map, a)
		print("qprobe: closest point %.2f ms" % ((Time.get_ticks_usec() - t) / 10000.0))
		var near: Variant = on(a + Vector3(0.5, 0, 0), 1.0)
		for limit in [0, 4096]:
			t = Time.get_ticks_usec()
			for i in 10:
				way(a, near if near != null else a, limit)
			print("qprobe: way of 0.5 m (limit %d) %.2f ms" % [limit, (Time.get_ticks_usec() - t) / 10000.0])
			t = Time.get_ticks_usec()
			for i in 10:
				way(a, b, limit)
			print("qprobe: way of %.0f m (limit %d) %.2f ms" % [a.distance_to(b), limit, (Time.get_ticks_usec() - t) / 10000.0])
		quit()
		return
	if OS.get_environment("NB_DEBUG") != "":
		var p: Array = groups["straight"]["points"][0]
		var c0 := NavigationServer3D.map_get_closest_point(map, p[0])
		var c1 := NavigationServer3D.map_get_closest_point(map, p[1])
		print("debug: points ", p, " closest ", c0, " ", c1, "; regions ", NavigationServer3D.map_get_regions(map).size(), ", iteration ", NavigationServer3D.map_get_iteration_id(map))
		var q := NavigationPathQueryParameters3D.new()
		q.map = map
		q.start_position = c0
		q.target_position = c1
		q.path_search_max_polygons = 0
		var r := NavigationPathQueryResult3D.new()
		NavigationServer3D.query_path(q, r)
		print("debug: path ", r.path)
		quit()
	for name in ["straight", "seam_flat", "seam_step", "near", "far"]:
		var g: Dictionary = groups[name]
		var rows := []
		var found := 0
		for p in g["points"]:
			var a = on(p[0], g["reach"])
			var b = on(p[1], g["reach"])
			if a == null or b == null:
				rows.append([-1, 0, 0, 0])
				continue
			var t := Time.get_ticks_usec()
			var l := way(a, b)
			var took := (Time.get_ticks_usec() - t) / 1000.0
			found += int(l >= 0.0)
			rows.append([int(l >= 0.0), max(l, 0.0), a.distance_to(b), took])
		json["queries"][name] = rows
		print("%s: %d of %d found" % [name, found, rows.size()])


# A run: its agents (Node3D bodies, each with a NavigationAgent3D), their
# goals, what it saw.
func start_run() -> void:
	var spec: Array = runs[run_index]
	run = {"name": spec[0], "reachable": spec[2], "frame": 0, "frames": spec[3] * 60, "times": [], "bodies": [], "agents": [], "goals": [], "history": [], "safe": [], "stuck": {}, "jammed": {}}
	var starts := []
	if spec[0] == "stream":
		var route := []
		for p in groups["route"]["points"]:
			route.append(p[0])
		run["route"] = route
		var length := 0.0
		for i in range(1, route.size()):
			length += route[i - 1].distance_to(route[i])
		run["length"] = length
		run["frames"] = int(length / 6.0) * 60
		var s0 = on(route[0], 1.0)
		for p in groups["followers"]["points"]:
			if starts.size() >= 30:
				break
			var a = on(p[0], 1.0)
			if a != null and way(a, s0) >= 0.0:
				starts.append(a)
				run["goals"].append(route[route.size() - 1])
		# (Only the tiles round its start, from here on.)
		for k in regions.keys():
			NavigationServer3D.free_rid(regions[k])
		regions.clear()
		for k in wanted(route[0]):
			add_region(k)
		synced = NavigationServer3D.map_get_iteration_id(map)
		run.merge({"churn": [], "added": 0, "removed": 0, "most": 0, "sent": Vector3(1e9, 0, 0)})
	else:
		for p in groups[spec[1]]["points"]:
			if starts.size() >= 50:
				break
			var a = on(p[0], 3.0)
			var b = on(p[1], 3.0)
			if a == null or b == null or (way(a, b) >= 0.0) != spec[2]:
				continue
			starts.append(a)
			run["goals"].append(b)
	var speed := 7.5 if spec[0] == "stream" else 5.0
	for i in starts.size():
		var body := Node3D.new()
		var agent := NavigationAgent3D.new()
		agent.radius = 0.35
		agent.height = 2.6
		agent.max_speed = speed
		agent.avoidance_enabled = true
		agent.path_desired_distance = 0.5
		agent.target_desired_distance = 1.0
		body.add_child(agent)
		root.add_child(body)
		agent.set_navigation_map(map)
		body.global_position = starts[i]
		run["bodies"].append(body)
		run["agents"].append(agent)
		run["history"].append([starts[i]])
		run["safe"].append(Vector3.ZERO)
		var idx := i
		agent.velocity_computed.connect(func(v: Vector3): run["safe"][idx] = v)


func wanted(p: Vector3) -> Array:
	var side: float = bodies[0]["s"]["tile_side"] * tile_scale
	var out := []
	for k in meshes[bodies[0]["name"]]:
		if Vector2((k.x + 0.5) * side - p.x, (k.y + 0.5) * side - p.z).length() < 450.0:
			out.append(k)
	return out


# One frame of the run; true when it's over.
func step_run() -> bool:
	var f: int = run["frame"]
	var dt := 1.0 / 60.0
	var nav_us := 0
	var t0 := Time.get_ticks_usec()
	if run["name"] == "stream":
		var route: Array = run["route"]
		var d := 6.0 * f / 60.0
		var p: Vector3 = route[route.size() - 1]
		for i in range(1, route.size()):
			var l: float = route[i - 1].distance_to(route[i])
			if d <= l:
				p = route[i - 1].lerp(route[i], d / max(l, 1e-6))
				break
			d -= l
		var want := wanted(p)
		for k in regions.keys():
			if not want.has(k):
				NavigationServer3D.free_rid(regions[k])
				regions.erase(k)
				run["removed"] += 1
		for k in want:
			if not regions.has(k):
				add_region(k)
				run["added"] += 1
		run["most"] = max(run["most"], regions.size())
		run["churn"].append((Time.get_ticks_usec() - t0) / 1000.0)
		if Vector2(p.x - run["sent"].x, p.z - run["sent"].z).length() > 1.0:
			for a in run["agents"]:
				a.target_position = p
			run["sent"] = p
	elif f == 0:
		for i in run["agents"].size():
			run["agents"][i].target_position = run["goals"][i]
	nav_us += Time.get_ticks_usec() - t0
	var agents: Array = run["agents"]
	for i in agents.size():
		var agent: NavigationAgent3D = agents[i]
		var body: Node3D = run["bodies"][i]
		var t := Time.get_ticks_usec()
		var next := agent.get_next_path_position()
		var finished := agent.is_navigation_finished()
		var flat := Vector3(next.x - body.global_position.x, 0.0, next.z - body.global_position.z)
		agent.velocity = Vector3.ZERO if finished or flat.length() < 1e-4 else flat.normalized() * agent.max_speed
		nav_us += Time.get_ticks_usec() - t
		# (Moved as avoidance allows, its height eased toward the way's next
		# corner, on the navmesh: the game's part, not timed. Godot's
		# closest-point query, every agent every frame, made a run take an
		# hour.)
		var moved: Vector3 = body.global_position + run["safe"][i] * dt
		var ahead := Vector2(next.x - moved.x, next.z - moved.z).length()
		var step := Vector2(run["safe"][i].x, run["safe"][i].z).length() * dt
		moved.y += (next.y - moved.y) * clampf(step / maxf(ahead, 1e-3), 0.0, 1.0)
		body.global_position = moved
		var h: Array = run["history"][i]
		h.append(moved)
		if run["reachable"] and h.size() > 120 and h[h.size() - 1].distance_to(h[h.size() - 121]) < 0.5:
			var goal: Vector3 = agent.target_position
			var far := moved.distance_to(goal) > 5.0
			var crowded := false
			for j in agents.size():
				if j != i and run["history"][j][run["history"][j].size() - 1].distance_to(moved) < 1.5:
					crowded = true
			if crowded or not far:
				run["jammed"][i] = true
			else:
				run["stuck"][i] = true
	# (The server's own step, last frame's, on this thread.)
	run["times"].append(nav_us / 1000.0 + nav_process_ms)
	run["frame"] = f + 1
	if run["frame"] < run["frames"]:
		return false
	var started := []
	var arrived := 0
	var kept := 0
	for i in agents.size():
		var h: Array = run["history"][i]
		var s := -1
		for k in h.size():
			if h[k].distance_to(h[0]) > 0.1:
				s = k
				break
		started.append(s)
		if h[h.size() - 1].distance_to(run["goals"][i]) < 2.0:
			arrived += 1
		if run["name"] == "stream" and h[h.size() - 1].distance_to(run["route"][run["route"].size() - 1]) < 30.0:
			kept += 1
	var out := {"agents": agents.size(), "frames_ms": run["times"], "stuck": run["stuck"].size(), "jammed": run["jammed"].size(), "arrived": arrived, "started_frame": started}
	if run["name"] == "stream":
		out.merge({"churn_ms": run["churn"], "added": run["added"], "removed": run["removed"], "resident_max": run["most"], "kept_up": kept})
	json["runs"][run["name"]] = out
	var times: Array = run["times"].duplicate()
	times.sort()
	print("%s: %d agents; frame 99th %.3f, worst %.3f ms; %d stuck, %d jammed, %d arrived" % [run["name"], agents.size(), times[int(times.size() * 0.99)], times[times.size() - 1], run["stuck"].size(), run["jammed"].size(), arrived])
	for b in run["bodies"]:
		b.queue_free()
	return true
