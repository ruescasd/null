// The navigation bench on C++ Recast/Detour as shipped (tools/navbench/
// CRITERIA.md): the shared terrain built in tiles by Recast's own steps
// with the bench's settings, the cases asked of Detour's queries, and the
// runs walked by its crowd. `navbench <bench dir>` writes
// results/detour.json there.

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <map>
#include <set>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

#include "DetourAlloc.h"
#include "DetourCommon.h"
#include "DetourCrowd.h"
#include "DetourPathQueue.h"
#include "DetourNavMesh.h"
#include "DetourNavMeshBuilder.h"
#include "DetourNavMeshQuery.h"
#include "Recast.h"

using Clock = std::chrono::steady_clock;
static double ms_since(Clock::time_point t) { return std::chrono::duration<double, std::milli>(Clock::now() - t).count(); }

struct V3 { float x, y, z; };
static float dist(V3 a, V3 b) { return std::sqrt((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y) + (a.z - b.z) * (a.z - b.z)); }
static float flat(V3 a, V3 b) { return std::sqrt((a.x - b.x) * (a.x - b.x) + (a.z - b.z) * (a.z - b.z)); }

struct Group { std::string name; float reach; std::vector<std::vector<V3>> points; };

struct Body {
    std::string name;
    std::map<std::string, float> s;
    float f(const char* k) const { auto i = s.find(k); if (i == s.end()) { fprintf(stderr, "settings: no %s for %s\n", k, name.c_str()); exit(1); } return i->second; }
    int n(const char* k) const { return (int)std::lround(f(k)); }
};

struct Tile { int x, z; unsigned char* data = nullptr; int size = 0; };

// One body's tiles, built in parallel on `workers` threads.
static std::vector<Tile> build(const Body& b, const std::vector<float>& verts, const std::vector<int>& tris, float ymin, float ymax, int workers, double& seconds, int& failed) {
    const float cs = b.f("cell_size"), ch = b.f("cell_height"), side = b.f("tile_side");
    const int border = b.n("border_cells"), tileCells = b.n("tile_cells");
    const float pad = border * cs;
    // Each triangle in every tile it touches, its border with it.
    std::map<std::pair<int, int>, std::vector<int>> bins;
    const int ntris = (int)tris.size() / 3;
    for (int t = 0; t < ntris; t++) {
        float lo[2] = {1e30f, 1e30f}, hi[2] = {-1e30f, -1e30f};
        for (int k = 0; k < 3; k++) {
            const float* v = &verts[tris[t * 3 + k] * 3];
            lo[0] = std::min(lo[0], v[0]); hi[0] = std::max(hi[0], v[0]);
            lo[1] = std::min(lo[1], v[2]); hi[1] = std::max(hi[1], v[2]);
        }
        for (int z = (int)std::floor((lo[1] - pad) / side); z <= (int)std::floor((hi[1] + pad) / side); z++)
            for (int x = (int)std::floor((lo[0] - pad) / side); x <= (int)std::floor((hi[0] + pad) / side); x++)
                bins[{x, z}].push_back(t);
    }
    std::vector<std::pair<std::pair<int, int>, std::vector<int>>> jobs(bins.begin(), bins.end());
    std::vector<Tile> tiles(jobs.size());
    std::atomic<int> next{0}, bad{0};
    auto t0 = Clock::now();
    auto work = [&]() {
        rcContext ctx(false);
        std::vector<int> local;
        std::vector<unsigned char> areas;
        for (;;) {
            int j = next++;
            if (j >= (int)jobs.size()) return;
            auto [key, list] = jobs[j];
            tiles[j].x = key.first;
            tiles[j].z = key.second;
            local.clear();
            for (int t : list) for (int k = 0; k < 3; k++) local.push_back(tris[t * 3 + k]);
            areas.assign(list.size(), 0);
            float bmin[3] = {key.first * side - pad, ymin - 1.0f, key.second * side - pad};
            float bmax[3] = {(key.first + 1) * side + pad, ymax + 1.0f, (key.second + 1) * side + pad};
            const int width = tileCells + border * 2;
            rcHeightfield* hf = rcAllocHeightfield();
            rcCompactHeightfield* chf = rcAllocCompactHeightfield();
            rcContourSet* cset = rcAllocContourSet();
            rcPolyMesh* pm = rcAllocPolyMesh();
            rcPolyMeshDetail* dm = rcAllocPolyMeshDetail();
            bool ok = rcCreateHeightfield(&ctx, *hf, width, width, bmin, bmax, cs, ch);
            const int wh = b.n("walkable_height"), wc = b.n("walkable_climb"), wr = b.n("walkable_radius");
            if (ok) {
                rcMarkWalkableTriangles(&ctx, b.f("slope"), verts.data(), (int)verts.size() / 3, local.data(), (int)list.size(), areas.data());
                ok = rcRasterizeTriangles(&ctx, verts.data(), (int)verts.size() / 3, local.data(), areas.data(), (int)list.size(), *hf, wc);
            }
            if (ok) {
                rcFilterLowHangingWalkableObstacles(&ctx, wc, *hf);
                rcFilterLedgeSpans(&ctx, wh, wc, *hf);
                rcFilterWalkableLowHeightSpans(&ctx, wh, *hf);
                ok = rcBuildCompactHeightfield(&ctx, wh, wc, *hf, *chf) && rcErodeWalkableArea(&ctx, wr, *chf) && rcBuildDistanceField(&ctx, *chf) &&
                     rcBuildRegions(&ctx, *chf, border, b.n("min_region_area"), b.n("merge_region_area")) &&
                     rcBuildContours(&ctx, *chf, b.f("max_error"), b.n("max_edge_len"), *cset, RC_CONTOUR_TESS_WALL_EDGES) &&
                     rcBuildPolyMesh(&ctx, *cset, b.n("max_vertices"), *pm) &&
                     rcBuildPolyMeshDetail(&ctx, *pm, *chf, b.f("detail_sample_dist"), b.f("detail_sample_max_error"), *dm);
            }
            if (!ok) bad++;
            if (ok && pm->npolys > 0) {
                for (int i = 0; i < pm->npolys; i++) { pm->flags[i] = 1; pm->areas[i] = 0; }
                dtNavMeshCreateParams p;
                memset(&p, 0, sizeof(p));
                p.verts = pm->verts; p.vertCount = pm->nverts; p.polys = pm->polys; p.polyAreas = pm->areas; p.polyFlags = pm->flags;
                p.polyCount = pm->npolys; p.nvp = pm->nvp;
                p.detailMeshes = dm->meshes; p.detailVerts = dm->verts; p.detailVertsCount = dm->nverts; p.detailTris = dm->tris; p.detailTriCount = dm->ntris;
                p.walkableHeight = b.f("height"); p.walkableRadius = b.f("radius"); p.walkableClimb = b.f("climb");
                p.tileX = key.first; p.tileY = key.second; p.tileLayer = 0;
                rcVcopy(p.bmin, pm->bmin); rcVcopy(p.bmax, pm->bmax);
                p.cs = cs; p.ch = ch; p.buildBvTree = true;
                if (!dtCreateNavMeshData(&p, &tiles[j].data, &tiles[j].size)) bad++;
            }
            rcFreeHeightField(hf); rcFreeCompactHeightfield(chf); rcFreeContourSet(cset); rcFreePolyMesh(pm); rcFreePolyMeshDetail(dm);
        }
    };
    std::vector<std::thread> pool;
    for (int i = 0; i < workers; i++) pool.emplace_back(work);
    for (auto& t : pool) t.join();
    seconds = ms_since(t0) / 1000.0;
    failed = bad;
    std::vector<Tile> out;
    for (auto& t : tiles) if (t.data) out.push_back(t);
    return out;
}

static unsigned char* copy(const Tile& t) { auto* d = (unsigned char*)dtAlloc(t.size, DT_ALLOC_PERM); memcpy(d, t.data, t.size); return d; }

static dtNavMesh* navmesh(const Body& b, const std::vector<Tile>& tiles, int maxPolys) {
    dtNavMeshParams np;
    memset(&np, 0, sizeof(np));
    np.tileWidth = np.tileHeight = b.f("tile_side");
    np.maxTiles = 8192;
    np.maxPolys = maxPolys;
    dtNavMesh* nav = dtAllocNavMesh();
    if (dtStatusFailed(nav->init(&np))) { fprintf(stderr, "navmesh init failed\n"); exit(1); }
    for (auto& t : tiles)
        if (dtStatusFailed(nav->addTile(copy(t), t.size, DT_TILE_FREE_DATA, 0, nullptr))) { fprintf(stderr, "addTile failed\n"); exit(1); }
    return nav;
}

static dtQueryFilter filter;

// The floor nearest `p`: within `reach` across, a metre over it or 6 m
// under it (as landmass's `on`).
static bool on(const dtNavMeshQuery* q, V3 p, float reach, dtPolyRef& ref, V3& at) {
    float c[3] = {p.x, p.y, p.z}, h[3] = {reach, 6.0f, reach};
    if (dtStatusFailed(q->findNearestPoly(c, h, &filter, &ref, &at.x)) || !ref) return false;
    return flat(p, at) <= reach && at.y - p.y <= 1.0f && p.y - at.y <= 6.0f;
}

static int out_of_nodes = 0;

// The way's length from `a` to `b`, or -1 for none (or only part of one).
static float way(const dtNavMeshQuery* q, dtPolyRef ra, V3 a, dtPolyRef rb, V3 b) {
    static dtPolyRef polys[16384];
    static float corners[4096 * 3];
    int n = 0;
    dtStatus s = q->findPath(ra, rb, &a.x, &b.x, &filter, polys, &n, 16384);
    if (dtStatusDetail(s, DT_OUT_OF_NODES)) out_of_nodes++;
    if (dtStatusFailed(s) || n == 0 || polys[n - 1] != rb) return -1.0f;
    int m = 0;
    q->findStraightPath(&a.x, &b.x, polys, n, corners, nullptr, nullptr, &m, 4096);
    float l = 0;
    V3 last = a;
    for (int i = 0; i < m; i++) { V3 c{corners[i * 3], corners[i * 3 + 1], corners[i * 3 + 2]}; l += dist(last, c); last = c; }
    return l;
}

struct Walked { std::vector<double> times; int stuck = 0, jammed = 0, arrived = 0; std::vector<int> started; std::vector<V3> ends; };

// The crowd's agents walked `frames` frames; `each(frame)` first, timed with
// the update when `time_each`; `goals` each agent's, for arriving.
template <class F>
static Walked walk(dtCrowd* crowd, const std::vector<int>& ids, const std::vector<V3>& goals, int frames, bool reachable, F each) {
    Walked w;
    std::vector<std::vector<V3>> history(ids.size());
    for (size_t i = 0; i < ids.size(); i++) history[i].push_back(*(V3*)crowd->getAgent(ids[i])->npos);
    std::set<int> stuck, jammed;
    for (int f = 0; f < frames; f++) {
        auto t = Clock::now();
        each(f);
        crowd->update(1.0f / 60.0f, nullptr);
        w.times.push_back(ms_since(t));
        for (size_t i = 0; i < ids.size(); i++) {
            const dtCrowdAgent* ag = crowd->getAgent(ids[i]);
            V3 p = *(V3*)ag->npos;
            history[i].push_back(p);
            auto& h = history[i];
            if (!reachable || h.size() <= 120 || dist(h.back(), h[h.size() - 121]) >= 0.5f) continue;
            const float* tp = ag->targetPos;
            bool far = dist(p, V3{tp[0], tp[1], tp[2]}) > 5.0f;
            bool crowded = false;
            for (size_t j = 0; j < ids.size(); j++) if (j != i && dist(history[j].back(), p) < 1.5f) crowded = true;
            if (crowded || !far) jammed.insert((int)i); else stuck.insert((int)i);
        }
    }
    for (size_t i = 0; i < ids.size(); i++) {
        auto& h = history[i];
        int s = -1;
        for (size_t k = 0; k < h.size(); k++) if (dist(h[k], h[0]) > 0.1f) { s = (int)k; break; }
        w.started.push_back(s);
        w.ends.push_back(h.back());
        if (i < goals.size() && dist(h.back(), goals[i]) < 2.0f) w.arrived++;
    }
    w.stuck = (int)stuck.size();
    w.jammed = (int)jammed.size();
    return w;
}

static dtCrowdAgentParams agent(float speed) {
    dtCrowdAgentParams ap;
    memset(&ap, 0, sizeof(ap));
    ap.radius = 0.35f; ap.height = 2.6f; ap.maxAcceleration = 50.0f; ap.maxSpeed = speed;
    ap.collisionQueryRange = ap.radius * 12.0f; ap.pathOptimizationRange = ap.radius * 30.0f;
    ap.updateFlags = DT_CROWD_ANTICIPATE_TURNS | DT_CROWD_OPTIMIZE_VIS | DT_CROWD_OPTIMIZE_TOPO | DT_CROWD_OBSTACLE_AVOIDANCE | DT_CROWD_SEPARATION;
    ap.obstacleAvoidanceType = 3; ap.separationWeight = 2.0f;
    return ap;
}

static void list(std::string& o, const std::vector<double>& v) { o += "["; for (size_t i = 0; i < v.size(); i++) { char b[32]; snprintf(b, 32, "%s%.4f", i ? ", " : "", v[i]); o += b; } o += "]"; }
static void list(std::string& o, const std::vector<int>& v) { o += "["; for (size_t i = 0; i < v.size(); i++) o += (i ? ", " : "") + std::to_string(v[i]); o += "]"; }

int main(int argc, char** argv) {
    if (argc < 2) { fprintf(stderr, "navbench <bench dir> [result name] [engine label]\n"); return 1; }
    std::string dir = argv[1];
    std::string result = argc > 2 ? argv[2] : "detour";
    std::string label = argc > 3 ? argv[3] : "recastnavigation 9f4ce64 (C++ Recast/Detour/DetourCrowd), unpatched";
    // Settings.
    std::vector<Body> bodies;
    {
        std::ifstream in(dir + "/settings.txt");
        std::string line;
        while (std::getline(in, line)) {
            std::istringstream ss(line);
            std::string w, name;
            ss >> w;
            if (w != "body") continue;
            Body b;
            ss >> b.name;
            std::string k; float v;
            while (ss >> k >> v) b.s[k] = v;
            bodies.push_back(b);
        }
    }
    // Terrain.
    std::vector<float> verts;
    std::vector<int> tris;
    float ymin = 1e30f, ymax = -1e30f;
    {
        FILE* f = fopen((dir + "/terrain.bin").c_str(), "rb");
        if (!f) { fprintf(stderr, "no terrain.bin in %s\n", dir.c_str()); return 1; }
        unsigned nv, nt;
        fread(&nv, 4, 1, f); fread(&nt, 4, 1, f);
        verts.resize(nv * 3); tris.resize(nt * 3);
        fread(verts.data(), 4, verts.size(), f); fread(tris.data(), 4, tris.size(), f);
        fclose(f);
        for (size_t i = 1; i < verts.size(); i += 3) { ymin = std::min(ymin, verts[i]); ymax = std::max(ymax, verts[i]); }
        printf("terrain: %u vertices, %u triangles\n", nv, nt);
    }
    // Cases.
    std::vector<Group> groups;
    {
        std::ifstream in(dir + "/cases.txt");
        std::string line;
        while (std::getline(in, line)) {
            std::istringstream ss(line);
            std::string w;
            ss >> w;
            if (w == "group") { Group g; int n; ss >> g.name >> g.reach >> n; groups.push_back(g); continue; }
            std::istringstream row(line);
            std::vector<V3> pts;
            V3 v;
            while (row >> v.x >> v.y >> v.z) pts.push_back(v);
            groups.back().points.push_back(pts);
        }
    }
    auto group = [&](const char* name) -> Group& { for (auto& g : groups) if (g.name == name) return g; fprintf(stderr, "no group %s\n", name); exit(1); };

    const int workers = std::max(1, (int)std::thread::hardware_concurrency() / 2 - 1);
    // (`navbench <dir> nodes`: step 2d of docs/outdoor-navigation.md. The
    // biped's navmesh by Recast on dir's terrain, each pair of
    // dir/pairs.txt asked of Detour: written to dir/detour.txt, a line a
    // pair: whether both points were on a floor, whether a way was found,
    // the plates' answer (copied), then the way's corners.)
    if (argc > 2 && std::string(argv[2]) == "nodes") {
        double seconds; int failed;
        auto tiles = build(bodies[0], verts, tris, ymin, ymax, workers, seconds, failed);
        int most = 0;
        for (auto& t : tiles) most = std::max(most, ((dtMeshHeader*)t.data)->polyCount);
        int maxPolys = 1;
        while (maxPolys < most) maxPolys <<= 1;
        dtNavMesh* nav = navmesh(bodies[0], tiles, maxPolys);
        dtNavMeshQuery* q = dtAllocNavMeshQuery();
        q->init(nav, 65535);
        printf("built %zu tiles in %.1f s (%d failed)\n", tiles.size(), seconds, failed);
        std::ifstream in(dir + "/pairs.txt");
        FILE* out = fopen((dir + "/detour.txt").c_str(), "wb");
        std::string line;
        int agree = 0, nodes_only = 0, detour_only = 0, off = 0, total = 0;
        static dtPolyRef polys[16384];
        static float corners[4096 * 3];
        while (std::getline(in, line)) {
            std::istringstream ss(line);
            V3 a0, b0; int same;
            if (!(ss >> a0.x >> a0.y >> a0.z >> b0.x >> b0.y >> b0.z >> same)) continue;
            total++;
            dtPolyRef ra, rb; V3 a, b;
            bool onfloor = on(q, a0, 0.5f, ra, a) && on(q, b0, 0.5f, rb, b);
            int found = 0, m = 0;
            if (onfloor) {
                int n = 0;
                dtStatus s = q->findPath(ra, rb, &a.x, &b.x, &filter, polys, &n, 16384);
                found = dtStatusSucceed(s) && n > 0 && polys[n - 1] == rb;
                if (found) q->findStraightPath(&a.x, &b.x, polys, n, corners, nullptr, nullptr, &m, 4096);
                // (No way: where its partial way stops, nearest the goal, as
                // its one corner.)
                if (!found && n > 0) {
                    q->closestPointOnPoly(polys[n - 1], &b.x, corners, nullptr);
                    m = 1;
                }
            } else off++;
            if (onfloor) { if (found == same) agree++; else if (same) nodes_only++; else detour_only++; }
            fprintf(out, "%d %d %d", (int)onfloor, found, same);
            for (int i = 0; i < m; i++) fprintf(out, " %.4f %.4f %.4f", corners[i * 3], corners[i * 3 + 1], corners[i * 3 + 2]);
            fprintf(out, "\n");
        }
        fclose(out);
        printf("pairs %d: off a floor %d; of the rest, agree %d, a way only by the plates' nodes %d, only by Detour %d\n", total, off, agree, nodes_only, detour_only);
        return 0;
    }
    std::string json = "{\"engine\": \"" + label + "\", \"build\": {";
    std::vector<Tile> biped;
    int bipedPolys = 0;
    for (size_t bi = 0; bi < bodies.size(); bi++) {
        double seconds; int failed;
        auto tiles = build(bodies[bi], verts, tris, ymin, ymax, workers, seconds, failed);
        long bytes = 0; int polys = 0, most = 0;
        for (auto& t : tiles) { bytes += t.size; auto* h = (dtMeshHeader*)t.data; polys += h->polyCount; most = std::max(most, h->polyCount); }
        printf("%s: %zu tiles, %d polygons, built in %.1f s on %d workers (%.0f ms a tile a core), %d failed, %.0f MB\n", bodies[bi].name.c_str(), tiles.size(), polys, seconds, workers, seconds * 1000 * workers / tiles.size(), failed, bytes / 1e6);
        char b[512];
        snprintf(b, 512, "%s\"%s\": {\"tiles\": %zu, \"polygons\": %d, \"seconds\": %.3f, \"workers\": %d, \"ms_per_tile_core\": %.2f, \"failed\": %d, \"bytes\": %ld}", bi ? ", " : "", bodies[bi].name.c_str(), tiles.size(), polys, seconds, workers, seconds * 1000 * workers / tiles.size(), failed, bytes);
        json += b;
        if (bi == 0) { biped = tiles; bipedPolys = most; }
    }
    json += "}, \"queries\": {";
    const Body& body = bodies[0];
    int maxPolys = 1;
    while (maxPolys < bipedPolys) maxPolys <<= 1;
    auto t = Clock::now();
    dtNavMesh* nav = navmesh(body, biped, maxPolys);
    printf("joined in %.2f s\n", ms_since(t) / 1000);
    dtNavMeshQuery* q = dtAllocNavMeshQuery();
    q->init(nav, 65535);

    // (NB_POINT=x,y,z: what Detour has round a point, and stop.)
    if (const char* e = getenv("NB_POINT")) {
        V3 p; sscanf(e, "%f,%f,%f", &p.x, &p.y, &p.z);
        float c[3] = {p.x, p.y, p.z}, h[3] = {3.0f, 6.0f, 3.0f};
        dtPolyRef found[64]; int n = 0;
        q->queryPolygons(c, h, &filter, found, &n, 64);
        int tx, tz; nav->calcTileLoc(c, &tx, &tz);
        printf("point %.3f %.3f %.3f in tile %d %d; %d polygons within 3 m\n", p.x, p.y, p.z, tx, tz, n);
        for (int i = 0; i < n; i++) {
            const dtMeshTile* tile; const dtPoly* poly; nav->getTileAndPolyByRefUnsafe(found[i], &tile, &poly);
            float lo[3] = {1e9f, 1e9f, 1e9f}, hi[3] = {-1e9f, -1e9f, -1e9f};
            for (int k = 0; k < poly->vertCount; k++) { const float* v = &tile->verts[poly->verts[k] * 3]; dtVmin(lo, v); dtVmax(hi, v); }
            printf("  tile %d %d poly: x %.3f..%.3f y %.2f..%.2f z %.3f..%.3f\n", tile->header->x, tile->header->y, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
        }
        dtPolyRef r; V3 at;
        bool ok = on(q, p, 0.5f, r, at);
        printf("on: %d at %.3f %.3f %.3f\n", ok, at.x, at.y, at.z);
        return 0;
    }
    // (NB_PAIR=ax,ay,az,bx,by,bz: Detour's way between two points, its
    // corridor and corners, and stop.)
    if (const char* e = getenv("NB_PAIR")) {
        V3 a0, b0; sscanf(e, "%f,%f,%f,%f,%f,%f", &a0.x, &a0.y, &a0.z, &b0.x, &b0.y, &b0.z);
        dtPolyRef ra, rb; V3 a, b;
        if (!on(q, a0, 0.5f, ra, a) || !on(q, b0, 0.5f, rb, b)) { printf("off the floor\n"); return 0; }
        static dtPolyRef polys[16384]; int n = 0;
        q->findPath(ra, rb, &a.x, &b.x, &filter, polys, &n, 16384);
        for (int i = 0; i < n; i++) {
            const dtMeshTile* tile; const dtPoly* poly; nav->getTileAndPolyByRefUnsafe(polys[i], &tile, &poly);
            float lo[3] = {1e9f, 1e9f, 1e9f}, hi[3] = {-1e9f, -1e9f, -1e9f};
            for (int k = 0; k < poly->vertCount; k++) { const float* v = &tile->verts[poly->verts[k] * 3]; dtVmin(lo, v); dtVmax(hi, v); }
            printf("  poly %d tile %d %d: x %.2f..%.2f y %.2f z %.2f..%.2f\n", i, tile->header->x, tile->header->y, lo[0], hi[0], lo[1], lo[2], hi[2]);
            for (int k = 0; k < poly->vertCount; k++) {
                const float* v = &tile->verts[poly->verts[k] * 3];
                const float* w = &tile->verts[poly->verts[(k + 1) % poly->vertCount] * 3];
                if (!(poly->neis[k] & DT_EXT_LINK)) continue;
                int links = 0;
                for (unsigned l = poly->firstLink; l != DT_NULL_LINK; l = tile->links[l].next) if (tile->links[l].edge == k) links++;
                printf("    edge %d (%.3f %.3f %.3f)-(%.3f %.3f %.3f) to side %d: %d links\n", k, v[0], v[1], v[2], w[0], w[1], w[2], poly->neis[k] & 0xff, links);
                for (unsigned l = poly->firstLink; l != DT_NULL_LINK; l = tile->links[l].next) {
                    if (tile->links[l].edge != k) continue;
                    const dtMeshTile* t2; const dtPoly* p2; nav->getTileAndPolyByRefUnsafe(tile->links[l].ref, &t2, &p2);
                    float l2[3] = {1e9f, 1e9f, 1e9f}, h2[3] = {-1e9f, -1e9f, -1e9f};
                    for (int u = 0; u < p2->vertCount; u++) { const float* x = &t2->verts[p2->verts[u] * 3]; dtVmin(l2, x); dtVmax(h2, x); }
                    printf("      to tile %d %d poly x %.2f..%.2f y %.2f..%.2f z %.2f..%.2f, portal %d..%d of 255\n", t2->header->x, t2->header->y, l2[0], h2[0], l2[1], h2[1], l2[2], h2[2], tile->links[l].bmin, tile->links[l].bmax);
                }
            }
        }
        static float corners[4096 * 3]; int m = 0;
        q->findStraightPath(&a.x, &b.x, polys, n, corners, nullptr, nullptr, &m, 4096);
        for (int i = 0; i < m; i++) printf("  corner %.2f %.2f %.2f\n", corners[i * 3], corners[i * 3 + 1], corners[i * 3 + 2]);
        // With any-angle search (raycasts along the way), as a setting.
        dtPolyRef sliced[16384]; int ns = 0;
        q->initSlicedFindPath(ra, rb, &a.x, &b.x, &filter, DT_FINDPATH_ANY_ANGLE);
        q->updateSlicedFindPath(1 << 20, nullptr);
        q->finalizeSlicedFindPath(sliced, &ns, 16384);
        q->findStraightPath(&a.x, &b.x, sliced, ns, corners, nullptr, nullptr, &m, 4096);
        float l = 0; V3 last = a;
        for (int i = 0; i < m; i++) { V3 c{corners[i * 3], corners[i * 3 + 1], corners[i * 3 + 2]}; l += dist(last, c); last = c; }
        printf("any-angle: %d polys, length %.2f\n", ns, l);
        return 0;
    }
    const char* names[] = {"straight", "seam_flat", "seam_step", "near", "far"};
    for (int gi = 0; gi < 5; gi++) {
        Group& g = group(names[gi]);
        json += std::string(gi ? ", " : "") + "\"" + names[gi] + "\": [";
        int found = 0;
        for (size_t i = 0; i < g.points.size(); i++) {
            dtPolyRef ra, rb; V3 a, b;
            char row[128];
            if (!on(q, g.points[i][0], g.reach, ra, a) || !on(q, g.points[i][1], g.reach, rb, b)) {
                snprintf(row, 128, "%s[-1, 0, 0, 0]", i ? ", " : "");
            } else {
                auto t = Clock::now();
                float l = way(q, ra, a, rb, b);
                double took = ms_since(t);
                found += l >= 0;
                snprintf(row, 128, "%s[%d, %.3f, %.3f, %.4f]", i ? ", " : "", l >= 0, std::max(l, 0.0f), dist(a, b), took);
            }
            json += row;
        }
        json += "]";
        printf("%s: %d of %zu found\n", names[gi], found, g.points.size());
    }
    json += "}, \"runs\": {";

    // Agents: the first 50 with a way, then 50 with none; then 50 given far
    // goals all in one frame.
    struct Run { const char* name; const char* group; bool reachable; int seconds; };
    Run runs[] = {{"reachable", "agents", true, 40}, {"unreachable", "agents", false, 40}, {"burst", "burst", true, 30}};
    for (int ri = 0; ri < 3; ri++) {
        Run& r = runs[ri];
        std::vector<std::pair<V3, V3>> pairs;
        std::vector<std::pair<dtPolyRef, dtPolyRef>> refs;
        for (auto& p : group(r.group).points) {
            if (pairs.size() >= 50) break;
            dtPolyRef ra, rb; V3 a, b;
            if (!on(q, p[0], 3.0f, ra, a) || !on(q, p[1], 3.0f, rb, b)) continue;
            if ((way(q, ra, a, rb, b) >= 0) != r.reachable) continue;
            pairs.push_back({a, b});
            refs.push_back({ra, rb});
        }
        dtCrowd* crowd = dtAllocCrowd();
        crowd->init(128, 0.45f, nav);
        std::vector<int> ids;
        std::vector<V3> goals;
        dtCrowdAgentParams ap = agent(5.0f);
        for (size_t i = 0; i < pairs.size(); i++) ids.push_back(crowd->addAgent(&pairs[i].first.x, &ap));
        for (auto& p : pairs) goals.push_back(p.second);
        Walked w = walk(crowd, ids, goals, 60 * r.seconds, r.reachable, [&](int f) {
            if (f == 0) for (size_t i = 0; i < ids.size(); i++) crowd->requestMoveTarget(ids[i], refs[i].second, &pairs[i].second.x);
        });
        if (getenv("NB_STUCK"))
            for (size_t i = 0; i < ids.size(); i++) {
                const dtCrowdAgent* ag = crowd->getAgent(ids[i]);
                printf("  agent %zu: %.0f m from its goal, target state %d, corridor %d polys, ends on the goal's polygon %d, %d corners\n", i, dist(*(V3*)ag->npos, pairs[i].second), ag->targetState, ag->corridor.getPathCount(), ag->corridor.getLastPoly() == ag->targetRef, ag->ncorners);
            }
        std::vector<double> sorted = w.times;
        std::sort(sorted.begin(), sorted.end());
        double mean = 0; for (double x : w.times) mean += x; mean /= w.times.size();
        printf("%s: %zu agents; update mean %.3f, 99th %.3f, worst %.3f ms; %d stuck, %d jammed, %d arrived\n", r.name, ids.size(), mean, sorted[sorted.size() * 99 / 100], sorted.back(), w.stuck, w.jammed, w.arrived);
        json += std::string(ri ? ", " : "") + "\"" + r.name + "\": {\"agents\": " + std::to_string(ids.size()) + ", \"frames_ms\": ";
        list(json, w.times);
        json += ", \"stuck\": " + std::to_string(w.stuck) + ", \"jammed\": " + std::to_string(w.jammed) + ", \"arrived\": " + std::to_string(w.arrived) + ", \"started_frame\": ";
        list(json, w.started);
        json += "}";
        dtFreeCrowd(crowd);
    }

    // Far goals planned as a game would: each route searched by Detour's own
    // path queue (sliced: NB_ITERS steps a frame shared by all, 65535 nodes;
    // it holds 8 requests, the rest asked again as it frees), the crowd sent
    // along the route a corner at a time; until its route comes, an agent
    // goes by the crowd's own way to the goal.
    {
        const int iters = getenv("NB_ITERS") ? atoi(getenv("NB_ITERS")) : 1000;
        std::vector<std::pair<V3, V3>> pairs;
        std::vector<std::pair<dtPolyRef, dtPolyRef>> refs;
        for (auto& p : group("burst").points) {
            if (pairs.size() >= 50) break;
            dtPolyRef ra, rb; V3 a, b;
            if (!on(q, p[0], 3.0f, ra, a) || !on(q, p[1], 3.0f, rb, b) || way(q, ra, a, rb, b) < 0) continue;
            pairs.push_back({a, b});
            refs.push_back({ra, rb});
        }
        const size_t n = pairs.size();
        dtPathQueue pathq;
        pathq.init(16384, 65535, nav);
        dtCrowd* crowd = dtAllocCrowd();
        crowd->init(128, 0.45f, nav);
        dtCrowdAgentParams ap = agent(5.0f);
        std::vector<int> ids;
        std::vector<V3> goals;
        for (auto& p : pairs) { ids.push_back(crowd->addAgent(&p.first.x, &ap)); goals.push_back(p.second); }
        std::vector<dtPathQueueRef> req(n, DT_PATHQ_INVALID);
        std::vector<int> routed(n, -1), sent(n, -1), next(n, 0);
        std::vector<char> asked(n, 0);
        std::vector<std::vector<V3>> corners(n);
        std::vector<std::vector<dtPolyRef>> crefs(n);
        int partial = 0;
        static dtPolyRef polys[16384];
        static float pts[4096 * 3];
        static dtPolyRef prefs[4096];
        Walked w = walk(crowd, ids, goals, 60 * 30, true, [&](int f) {
            for (size_t i = 0; i < n; i++) {
                if (f == 0) crowd->requestMoveTarget(ids[i], refs[i].second, &pairs[i].second.x);
                if (!asked[i]) {
                    req[i] = pathq.request(refs[i].first, refs[i].second, &pairs[i].first.x, &pairs[i].second.x, &filter);
                    asked[i] = req[i] != DT_PATHQ_INVALID;
                }
            }
            pathq.update(iters);
            for (size_t i = 0; i < n; i++) {
                if (req[i] == DT_PATHQ_INVALID) continue;
                dtStatus st = pathq.getRequestStatus(req[i]);
                // (0: still waiting its turn in the queue.)
                if (st == 0 || dtStatusInProgress(st)) continue;
                int np = 0;
                if (dtStatusSucceed(st)) pathq.getPathResult(req[i], polys, &np, 16384);
                if (getenv("NB_STUCK")) printf("  route %zu at frame %d: status %08x, %d polys\n", i, f, (unsigned)st, np);
                req[i] = DT_PATHQ_INVALID;
                routed[i] = f;
                if (np == 0) continue;
                if (polys[np - 1] != refs[i].second) partial++;
                int m = 0;
                q->findStraightPath(&pairs[i].first.x, &pairs[i].second.x, polys, np, pts, nullptr, prefs, &m, 4096);
                for (int k = 0; k < m; k++) { corners[i].push_back(V3{pts[k * 3], pts[k * 3 + 1], pts[k * 3 + 2]}); crefs[i].push_back(prefs[k]); }
                // (From the corner nearest where it has got to.)
                V3 at = *(V3*)crowd->getAgent(ids[i])->npos;
                int best = 0;
                for (int k = 0; k < m; k++) if (dist(at, corners[i][k]) < dist(at, corners[i][best])) best = k;
                next[i] = best;
            }
            for (size_t i = 0; i < n; i++) {
                if (corners[i].empty()) continue;
                V3 at = *(V3*)crowd->getAgent(ids[i])->npos;
                while (next[i] + 1 < (int)corners[i].size() && flat(at, corners[i][next[i]]) < 8.0f) next[i]++;
                if (next[i] != sent[i]) { crowd->requestMoveTarget(ids[i], crefs[i][next[i]], &corners[i][next[i]].x); sent[i] = next[i]; }
            }
        });
        std::vector<double> sorted = w.times;
        std::sort(sorted.begin(), sorted.end());
        if (getenv("NB_STUCK"))
            for (size_t i = 0; i < n; i++) {
                const dtCrowdAgent* ag = crowd->getAgent(ids[i]);
                V3 at = *(V3*)ag->npos;
                printf("  planned %zu: corner %d of %zu, %.1f m from it, %.0f m from its goal, routed frame %d; target state %d, corridor %d polys ends on target %d, velocity %.2f\n", i, next[i], corners[i].size(), corners[i].empty() ? -1.0f : dist(at, corners[i][next[i]]), dist(at, pairs[i].second), routed[i], ag->targetState, ag->corridor.getPathCount(), ag->corridor.getLastPoly() == ag->targetRef, dtVlen(ag->vel));
            }
        std::vector<int> late = routed;
        std::sort(late.begin(), late.end());
        printf("planned (%d steps a frame): %zu agents; update 99th %.3f, worst %.3f ms; routes in by frame %d (99th), %d partial; %d stuck, %d jammed\n", iters, n, sorted[sorted.size() * 99 / 100], sorted.back(), late[late.size() * 99 / 100], partial, w.stuck, w.jammed);
        json += ", \"planned\": {\"agents\": " + std::to_string(n) + ", \"iters\": " + std::to_string(iters) + ", \"frames_ms\": ";
        list(json, w.times);
        json += ", \"stuck\": " + std::to_string(w.stuck) + ", \"jammed\": " + std::to_string(w.jammed) + ", \"arrived\": " + std::to_string(w.arrived) + ", \"partial\": " + std::to_string(partial) + ", \"routed_frame\": ";
        list(json, routed);
        json += ", \"started_frame\": ";
        list(json, w.started);
        json += "}";
        dtFreeCrowd(crowd);
    }

    // Streaming: tiles within 450 m of a point walking the route at 6 m/s,
    // added and removed; 30 followers after it, asked again whenever it is a
    // metre on from where they were last sent.
    {
        auto& route = group("route").points;
        std::vector<V3> pts;
        for (auto& p : route) pts.push_back(p[0]);
        float length = 0;
        for (size_t i = 1; i < pts.size(); i++) length += dist(pts[i - 1], pts[i]);
        auto along = [&](float d) {
            for (size_t i = 1; i < pts.size(); i++) {
                float l = dist(pts[i - 1], pts[i]);
                if (d <= l) { float u = d / std::max(l, 1e-6f); return V3{pts[i - 1].x + (pts[i].x - pts[i - 1].x) * u, pts[i - 1].y + (pts[i].y - pts[i - 1].y) * u, pts[i - 1].z + (pts[i].z - pts[i - 1].z) * u}; }
                d -= l;
            }
            return pts.back();
        };
        std::vector<V3> starts;
        dtPolyRef r0; V3 s0;
        on(q, pts[0], 1.0f, r0, s0);
        for (auto& p : group("followers").points) {
            if (starts.size() >= 30) break;
            dtPolyRef r; V3 a;
            if (on(q, p[0], 1.0f, r, a) && way(q, r, a, r0, s0) >= 0) starts.push_back(a);
        }
        const float side = body.f("tile_side");
        std::map<std::pair<int, int>, const Tile*> byKey;
        for (auto& t : biped) byKey[{t.x, t.z}] = &t;
        auto wanted = [&](V3 p) {
            std::set<std::pair<int, int>> w;
            for (auto& [k, t] : byKey) { float cx = (k.first + 0.5f) * side, cz = (k.second + 0.5f) * side; if (std::hypot(cx - p.x, cz - p.z) < 450.0f) w.insert(k); }
            return w;
        };
        dtNavMeshParams np;
        memset(&np, 0, sizeof(np));
        np.tileWidth = np.tileHeight = side; np.maxTiles = 8192; np.maxPolys = maxPolys;
        dtNavMesh* snav = dtAllocNavMesh();
        snav->init(&np);
        std::map<std::pair<int, int>, dtTileRef> resident;
        for (auto& k : wanted(pts[0])) { dtTileRef ref; snav->addTile(copy(*byKey[k]), byKey[k]->size, DT_TILE_FREE_DATA, 0, &ref); resident[k] = ref; }
        dtCrowd* crowd = dtAllocCrowd();
        crowd->init(128, 0.45f, snav);
        dtCrowdAgentParams ap = agent(7.5f);
        std::vector<int> ids;
        for (auto& s : starts) ids.push_back(crowd->addAgent(&s.x, &ap));
        int added = 0, removed = 0, most = 0;
        std::vector<double> churn;
        V3 sent{1e9f, 0, 0};
        int seconds = (int)(length / 6.0f);
        Walked w = walk(crowd, ids, {}, 60 * seconds, true, [&](int f) {
            V3 p = along(6.0f * f / 60.0f);
            auto t = Clock::now();
            auto want = wanted(p);
            for (auto it = resident.begin(); it != resident.end();) {
                if (!want.count(it->first)) { snav->removeTile(it->second, nullptr, nullptr); it = resident.erase(it); removed++; } else ++it;
            }
            for (auto& k : want) if (!resident.count(k)) { dtTileRef ref; snav->addTile(copy(*byKey[k]), byKey[k]->size, DT_TILE_FREE_DATA, 0, &ref); resident[k] = ref; added++; }
            most = std::max(most, (int)resident.size());
            churn.push_back(ms_since(t));
            if (flat(p, sent) > 1.0f) {
                dtPolyRef r; float at[3]; float c[3] = {p.x, p.y, p.z}, h[3] = {1.0f, 3.0f, 1.0f};
                crowd->getNavMeshQuery()->findNearestPoly(c, h, &filter, &r, at);
                if (r) { for (int id : ids) crowd->requestMoveTarget(id, r, at); sent = p; }
            }
        });
        int kept = 0;
        for (auto& e : w.ends) kept += dist(e, pts.back()) < 30.0f;
        std::vector<double> sorted = w.times;
        std::sort(sorted.begin(), sorted.end());
        printf("stream: %.0f m, %d added, %d removed, at most %d; frame 99th %.3f, worst %.3f ms; %d stuck, %d jammed; %d of %zu kept up\n", length, added, removed, most, sorted[sorted.size() * 99 / 100], sorted.back(), w.stuck, w.jammed, kept, starts.size());
        json += ", \"stream\": {\"agents\": " + std::to_string(starts.size()) + ", \"frames_ms\": ";
        list(json, w.times);
        json += ", \"churn_ms\": ";
        list(json, churn);
        json += ", \"added\": " + std::to_string(added) + ", \"removed\": " + std::to_string(removed) + ", \"resident_max\": " + std::to_string(most) + ", \"stuck\": " + std::to_string(w.stuck) + ", \"jammed\": " + std::to_string(w.jammed) + ", \"kept_up\": " + std::to_string(kept) + ", \"started_frame\": ";
        list(json, w.started);
        json += "}";
    }
    json += "}, \"stalled\": 0, \"out_of_nodes\": " + std::to_string(out_of_nodes) + "}";
    FILE* f = fopen((dir + "/results/" + result + ".json").c_str(), "wb");
    fwrite(json.data(), 1, json.size(), f);
    fclose(f);
    printf("out of nodes: %d\n", out_of_nodes);
    return 0;
}
