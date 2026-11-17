// Terrain surface: the standard PBR pipeline, plus
// - baked sky visibility (vertex colour alpha) applied to indirect light only;
//   direct sunlight is left to the shadow maps, so a crevice in full sun is
//   still lit;
// - light from pieces of geometry the mesh marks as lit (glowing etchings
//   and seams);
// - paving (off by default): in some regions of the ground, a pattern of
//   tiles with cut joints, a ruler underfoot and something to stream past.
// - the plates' subdivision (`--opt nosubdiv` for none): each plate split
//   on below what the geometry draws, by the same rule, into flush pieces
//   with grooved joints (see `subdivide`). What the ground has instead of a
//   texture: a sense of speed from the ground's own make-up.
// - the plates' walls split too (`--set walls N`, off, to judge; see
//   `wall`).
// The ground is otherwise plain. (Its grain, value-noise mottling, read as
// camouflage, a texture gone wrong, and the flecks and cracks tried after it
// as cosmetic: the user, 2026-10-17; see docs/removed.md. A sense of speed
// has to come from built things.)

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef VISIBILITY_RANGE_DITHER
#import bevy_pbr::pbr_functions::visibility_range_dither
#endif

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    shadows::set_far,
}
#endif

// x: strength of the baked occlusion (0 disables it), y: curvature 1 / 2R,
// zw: camera x and z.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> params: vec4<f32>;
// x: world wrap period in metres; the plates' subdivision: y on (1) or off
// (0), z the chance a piece splits again, w the largest pieces drawn (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> wrap: vec4<f32>;
// The plates' subdivision: x the smallest pieces drawn (m), y the slope of
// the joints' sides (degrees; 0, flat dark lines).
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> pieces: vec4<f32>;
// The plates' walls (see `wall`): x how (0 not), y a course's height (m), z
// a joint's width (m), w each course's or block's tone.
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> walls: vec4<f32>;
// The walls' panels (see `panels`): x the longest the smallest pieces may be
// (in courses), y how much of the wall the big pieces cover (1 about as much
// as each other size), z the chance one of the biggest cuts is stepped.
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var<uniform> layers: vec4<f32>;
// The panels' cuts (see `panels`): x the chance one is left open, y the
// chance one is a curve; z and w how far the middle and far levels of detail
// are lowered (m), so a wall's pattern is laid out where it stands.
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var<uniform> marks: vec4<f32>;
// Walls round a middle (the arena's): xy the middle (x, z), w on. A wall
// facing it or away is panelled along its curve (see the walls' branch).
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var<uniform> round_walls: vec4<f32>;
// x: brightness of lit geometry; the plates' subdivision: y each piece's
// tone (a share of the albedo either way), z its tilt (degrees), w how dark
// the joints are.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> glow: vec4<f32>;
// x: pattern (0 none, 1 checkerboard, 2 fractal), y: tile size (m), z:
// contrast, w: share of the ground paved.
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> paving: vec4<f32>;
// The far terrain's cached shadow maps (see far_shadow.rs, `MAPS`): the
// main sun's inner and outer, the second sun's; and what each was drawn
// with (`MapFrame::data`, eight texels a map).
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var far_inner: texture_depth_2d;
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var far_data: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var far_outer: texture_depth_2d;
@group(#{MATERIAL_BIND_GROUP}) @binding(112) var far_second: texture_depth_2d;

fn far_texel(map: i32, i: i32) -> vec4<f32> {
    return textureLoad(far_data, vec2<i32>(map * 8 + i, 0), 0);
}

fn far_depth(map: i32, at: vec2<i32>) -> f32 {
    if map == 0 {
        return textureLoad(far_inner, at, 0);
    } else if map == 1 {
        return textureLoad(far_outer, at, 0);
    }
    return textureLoad(far_second, at, 0);
}

fn far_size(map: i32) -> vec2<i32> {
    if map == 0 {
        return vec2<i32>(textureDimensions(far_inner));
    } else if map == 1 {
        return vec2<i32>(textureDimensions(far_outer));
    }
    return vec2<i32>(textureDimensions(far_second));
}

// Lit (1) or in shadow (0) by one far map at a point seen bent round the
// camera, with its normal; x -1 where the map is off or does not reach, y
// how near its edge (1 at it). Bent as the map was drawn, round where the
// camera was then. Four texels, weighed by where it falls among them, each
// against the receiver's own plane carried to the texel (its depth's change
// along the map's axes), so a flat receiver needs little bias and the
// steps' shadows are kept.
fn far_map(map: i32, world: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let bend = far_texel(map, 4);
    if bend.w < 0.5 {
        return vec2<f32>(-1.0, 0.0);
    }
    let sun = far_texel(map, 5).xyz;
    let facing = dot(normal, sun);
    if facing <= 0.0 {
        return vec2<f32>(1.0, 0.0);
    }
    let now = world.xz - params.zw;
    let then = world.xz - bend.xy;
    var p = world;
    p.y += dot(now, now) * params.y - dot(then, then) * bend.z;
    let right = far_texel(map, 6).xyz;
    let up = cross(right, -sun);
    // (Grazing ones held to 20 m a metre.)
    let slope = clamp(vec2<f32>(dot(normal, right), dot(normal, up)) / facing, vec2<f32>(-20.0), vec2<f32>(20.0));
    let clip_from_world = mat4x4<f32>(far_texel(map, 0), far_texel(map, 1), far_texel(map, 2), far_texel(map, 3));
    let c = clip_from_world * vec4<f32>(p, 1.0);
    let uv = c.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if any(uv <= vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0)) || c.z < 0.0 || c.z > 1.0 {
        return vec2<f32>(-1.0, 0.0);
    }
    let texel = far_texel(map, 7);
    let size = far_size(map);
    let at = uv * vec2<f32>(size);
    let t = at - 0.5;
    let i = vec2<i32>(floor(t));
    let f = t - floor(t);
    var lit = vec4<f32>(0.0);
    let offsets = array<vec2<i32>, 4>(vec2<i32>(0, 0), vec2<i32>(1, 0), vec2<i32>(0, 1), vec2<i32>(1, 1));
    for (var k = 0; k < 4; k++) {
        let tap = clamp(i + offsets[k], vec2<i32>(0), size - 1);
        // (The map's texel rows run down its y.)
        let d = (vec2<f32>(tap) + 0.5 - at) * vec2<f32>(1.0, -1.0) * texel.xy;
        let z = c.z + (dot(slope, d) - texel.w) / texel.z;
        lit[k] = select(0.0, 1.0, z <= far_depth(map, tap));
    }
    let edge = max(abs(uv.x - 0.5), abs(uv.y - 0.5)) * 2.0;
    return vec2<f32>(mix(mix(lit.x, lit.y, f.x), mix(lit.z, lit.w, f.x), f.y), edge);
}

// Lit (1) or in the far terrain's shadow (0) from each sun (x the main, y
// the second) at a point seen bent round the camera, with its normal. The
// main sun's from its inner map where it reaches, blending into the outer
// at its edge; the second's fading out at its map's edge (its live shadows
// end nearer still).
fn far_shadow(world: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    var main = 1.0;
    let inner = far_map(0, world, normal);
    if inner.x >= 0.0 && inner.y < 0.9 {
        main = inner.x;
    } else {
        let outer = far_map(1, world, normal);
        if outer.x >= 0.0 && inner.x >= 0.0 {
            main = mix(inner.x, outer.x, (inner.y - 0.9) / 0.1);
        } else if outer.x >= 0.0 {
            main = outer.x;
        } else if inner.x >= 0.0 {
            main = inner.x;
        }
    }
    var second = 1.0;
    let s = far_map(2, world, normal);
    if s.x >= 0.0 {
        second = mix(s.x, 1.0, clamp((s.y - 0.9) / 0.1, 0.0, 1.0));
    }
    return vec2<f32>(main, second);
}

fn hash(cell: vec2<i32>, period: i32) -> f32 {
    // Wrap the lattice so the pattern tiles with the world.
    let c = bitcast<vec2<u32>>(((cell % period) + period) % period);
    var h = (c.x * 0x8da6b343u) ^ (c.y * 0xd8163841u);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777216.0;
}

// A Voronoi diagram of sites `size` m apart (jittered as the plates' are),
// its own for each `seed`: the cell `p` is in, how far `p` is from that
// cell's border (m), and which way that border lies.
struct Cell {
    id: vec2<i32>,
    edge: f32,
    dir: vec2<f32>,
}

// A wall's panel (see `panels`): its piece, its nearest joint, how far and
// which way, and the sizes they fade by (m): the cut that made the joint,
// the shorter of its length and how far it parts; the piece's least side.
struct Panel {
    id: vec2<i32>,
    edge: f32,
    dir: vec2<f32>,
    cut: f32,
    piece: f32,
}

fn voronoi(p: vec2<f32>, size: f32, seed: i32) -> Cell {
    let q = p / size;
    let i = vec2<i32>(floor(q));
    let period = i32(round(wrap.x / size));
    let s = vec2<i32>(seed, seed * 7 + 3);
    var best = 1e9;
    var near = vec2<f32>(0.0);
    var id = i;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = i + vec2<i32>(x, y);
            let at = vec2<f32>(c) + 0.5 + (vec2<f32>(hash(c + s, period), hash(c + s + vec2(17, 31), period)) - 0.5) * 0.6;
            let d = dot(at - q, at - q);
            if d < best {
                best = d;
                near = at;
                id = c;
            }
        }
    }
    // (The distance to the border with each neighbour, along the line
    // between the two sites.)
    var edge = 1e9;
    var dir = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = id + vec2<i32>(x, y);
            if x == 0 && y == 0 {
                continue;
            }
            let at = vec2<f32>(c) + 0.5 + (vec2<f32>(hash(c + s, period), hash(c + s + vec2(17, 31), period)) - 0.5) * 0.6;
            let across = normalize(at - near);
            let e = dot((near + at) * 0.5 - q, across);
            if e < edge {
                edge = e;
                dir = across;
            }
        }
    }
    return Cell(id, edge * size, dir);
}

// A plate split on below what the geometry draws, by the plates' own rule:
// a piece splits into the pieces of the next, finer Voronoi diagram that fall
// inside it, and those may split again; every piece flush, but its own tone
// and tilt (lit as if tilted, as the canal's stones), with a fine joint round
// it. Pieces of 32, 8, 2 and 0.5 m (the plates are 128, 32 and 8); each
// scale fades out before its pieces near a pixel. `level` is the plate's own
// (0..=2), `id` its number, `smallest` the smallest pieces drawn (m).
struct Sub {
    tone: f32,
    tilt: vec2<f32>,
    joint: f32,
    // The joint's side sloping down to the border, as a groove's would: the
    // way it leans, faded where the groove is narrower than a pixel.
    groove: vec2<f32>,
}

fn subdivide(p: vec2<f32>, pixel: f32, level: i32, id: i32, smallest: f32) -> Sub {
    var out = Sub(0.0, vec2<f32>(0.0), 0.0, vec2<f32>(0.0));
    let sizes = array<f32, 4>(32.0, 8.0, 2.0, 0.5);
    let plate_size = 128.0 / f32(1 << u32(2 * level));
    var seed = id * 131 + level * 977;
    for (var k = 0; k < 4; k++) {
        let size = sizes[k];
        if size >= plate_size || size > wrap.w {
            continue;
        }
        if size < smallest {
            break;
        }
        let fade = saturate(size / (pixel * 6.0) - 1.0);
        if fade <= 0.0 {
            break;
        }
        let c = voronoi(p, size, seed);
        let period = i32(round(wrap.x / size));
        // Joints thinner the smaller the pieces: 10 cm round 32 m pieces,
        // 3 cm round 8 m, 1 cm round 2 m; thinner than a pixel, fainter.
        let w = 0.003 * size + 0.004;
        let drawn = max(w, pixel);
        let joint = saturate(1.0 - (c.edge - drawn * 0.5) / pixel) * (w / drawn) * fade;
        if joint > out.joint {
            out.joint = joint;
            out.groove = c.dir * joint;
        }
        let a = hash(c.id + vec2(seed, 5), period) * 6.2831;
        out.tone = mix(out.tone, hash(c.id + vec2(seed, 3), period) * 2.0 - 1.0, fade);
        out.tilt = mix(out.tilt, vec2<f32>(cos(a), sin(a)) * hash(c.id + vec2(seed, 7), period), fade);
        if hash(c.id + vec2(seed, 11), period) >= wrap.z {
            break;
        }
        seed = i32(hash(c.id + vec2(seed, 13), period) * 65536.0);
    }
    return out;
}

// A plate's wall split (the user, 2026-10-27: the ground's plates have their
// subdivision, the walls nothing): panels (see `panels`), on a course
// (`walls.y`) at heights fixed the world over, so walls side by side line up
// and a tall one can be counted. (Courses alone, blocks carried down from
// the top and a bond came first: see samples/shaders/wall_modes.wgsl.) Its
// seed is the wall's `line` (see `wall_code`), not its plate's, so a wall
// keeps it at every level of detail. `q` is where on the face (along it,
// up; m), `t` the way along it; the groove's x is along it, y up; the tone
// already scaled.
fn wall(q: vec2<f32>, dp: mat2x3<f32>, t: vec2<f32>, pixel: f32, line: i32) -> Sub {
    var out = Sub(0.0, vec2<f32>(0.0), 0.0, vec2<f32>(0.0));
    let course = walls.y;
    let seed = vec2<i32>(line * 131, 19);
    let w = walls.z;
    // (Each joint fades by the cut that made it, an 8 m slab's edge farther
    // than a course's, and by the pixel across it: from the foot of a tall
    // wall its top is seen edge on, a pixel long up it and short along it;
    // by the course and the pixel's longest way, all of it faded there, the
    // user. The tone by the piece's size.)
    let c = panels(q, course, seed);
    let along = vec2<f32>(dot(dp[0].xz, t), dot(dp[1].xz, t));
    let up = vec2<f32>(dp[0].y, dp[1].y);
    let across = max(length(along * c.dir.x + up * c.dir.y), 1e-4);
    let drawn = max(w, across);
    let shown = saturate(c.cut / (across * 6.0) - 1.0);
    out.joint = saturate(1.0 - (c.edge - drawn * 0.5) / across) * (w / drawn) * shown;
    out.groove = c.dir * out.joint;
    out.tone = (hash(c.id + seed, 1 << 20) * 2.0 - 1.0) * saturate(c.piece / (pixel * 2.0) - 1.0) * walls.w;
    return out;
}

// The floor's rule with rectangles (the user, 2026-10-27: the floor's Voronoi
// tried on walls, its sites in rows on the courses, squeezed along the wall
// and strayed up or down a little, measured, as layered slate at its best,
// but its polygons did not work): a wall cut into bands 8 courses high and
// 16 long, and each piece cut again near its middle, across when it is no
// longer than twice its height, else along; across only on the courses'
// heights the world over, so every joint along the wall lies on the
// measure, along on half a course. A piece stops by chance, the more likely
// the smaller, so that each size covers about as much of the wall (times
// `layers.y`, more of the big for more), and at a course high and no longer
// than `layers.x` courses. (Cut anywhere and stopped by an even chance, the
// pieces were big or bricks, few between, the user: half the bands stood
// whole, and a cut near a band's edge left a course-high strip that had to
// be cut down to bricks all along.) Each cut is drawn one of four ways,
// the same both sides of it, as it is one line:
// - straight;
// - stepped, by chance (`layers.z`), only among the biggest (the user: a
//   step is a feature of the wall's long lines; within small pieces it
//   read as a puzzle): one across jogs up or down a course partway along,
//   one along jogs aside half a course or a whole at a course's height;
// - open, by chance (`marks.x`), not drawn: the pieces either side merge,
//   and where they were cut on differently, into shapes no cutting gives
//   (the user's); a piece cut open keeps one tone whatever is cut in it
//   after, so a tone never changes where no joint is drawn;
// - a curve, by chance (`marks.y`): a half ellipse from the cut's ends,
//   bowing either way (up or down, left or right: an arch was only ever
//   upright, the user wanted it any way), at most half its span and
//   leaving a course beyond it; the piece it bows into is cut on clear of
//   it, the other is cut on into it.
// The cuts after a stepped one or a curve are made as if it were straight,
// so every point of a piece agrees on them; only its joints follow it. `q`
// is along the face and up (m): the piece, how far its border is (m), and
// which way (along, up).
fn panels(q: vec2<f32>, course: f32, seed: vec2<i32>) -> Panel {
    let band = 8.0;
    let width = course * band * 2.0;
    let v = q.y / course;
    let row = floor(v / band);
    let shift = hash(vec2<i32>(i32(row), 3) + seed, 1 << 20) * width;
    let col = floor((q.x + shift) / width);
    var u0 = col * width - shift;
    var u1 = u0 + width;
    var v0 = row * band;
    var v1 = v0 + band;
    var k = i32(hash(vec2<i32>(i32(col), i32(row)) + seed, 1 << 20) * 65536.0);
    var tone = k;
    var merged = false;
    let half = course * 0.5;
    // (Each of the piece's sides (left, right, bottom, top) as it stands
    // at the point (m), how it is drawn (0 straight, 1 stepped, 2 a curve,
    // 3 open), and its step (where it runs, m: up for a side's along, along
    // for a side's across; from where to where) or curve (its middle, half
    // its span, how far it bows, signed; m, the middle and span up a side's
    // along, along a side's across).)
    var at_point = array<f32, 4>(u0, u1, v0 * course, v1 * course);
    var how = array<i32, 4>(0, 0, 0, 0);
    var size = array<f32, 4>(band * course, band * course, band * course, band * course);
    var shape = array<vec3<f32>, 4>(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
    // (How many halvings from a band to the smallest piece.)
    let sizes = log2(band * band * 2.0 / layers.x) + 1.0;
    for (var depth = 0; depth < 12; depth++) {
        let high = v1 - v0;
        let long = (u1 - u0) / course;
        if high < 1.5 && long <= layers.x {
            break;
        }
        let left = max(sizes - f32(depth), 1.0);
        if long <= layers.x * high && hash(vec2<i32>(k, depth + 100), 1 << 20) < layers.y / left {
            break;
        }
        let r = hash(vec2<i32>(k, depth + 300), 1 << 20) - 0.5;
        var side = 0;
        let keyed = depth < 2 && hash(vec2<i32>(k, depth + 500), 1 << 20) < layers.z;
        let open = hash(vec2<i32>(k, depth + 900), 1 << 20) < marks.x;
        let r2 = hash(vec2<i32>(k, depth + 600), 1 << 20);
        let r3 = hash(vec2<i32>(k, depth + 700), 1 << 20);
        if high >= 2.0 && long <= high * 2.0 {
            let at = clamp(v0 + round(high * 0.5 + r * high * 0.3), v0 + 1.0, v1 - 1.0);
            // (Stepped: up or down a course from somewhere along it.)
            let jog = select(-1.0, 1.0, r2 < 0.5);
            let onset = clamp(round((u0 + (u1 - u0) * (0.25 + 0.5 * r3)) / half) * half, u0 + half, u1 - half);
            let keys = keyed && at + jog > v0 + 0.5 && at + jog < v1 - 0.5;
            // (A curve: from the cut's ends, up or down, as far as it may.)
            let up = hash(vec2<i32>(k, depth + 1200), 1 << 20) < 0.5;
            let mid = (u0 + u1) * 0.5;
            let span = (u1 - u0) * 0.5;
            let rise = min(span, select(at - v0, v1 - at, up) * course - course);
            let curved = !keys && !open && rise >= course && hash(vec2<i32>(k, depth + 1000), 1 << 20) < marks.y;
            let bow = select(-rise, rise, up);
            let t = clamp((q.x - mid) / span, -1.0, 1.0);
            let lift = select(0.0, bow * sqrt(max(1.0 - t * t, 0.0)), curved);
            let cut = select(at, at + jog, keys && q.x >= onset) * course + lift;
            let drawn = select(select(select(0, 2, curved), 1, keys), 3, open);
            let form = select(vec3<f32>(onset, min(at, at + jog) * course, max(at, at + jog) * course), vec3<f32>(mid, span, bow), curved);
            let clear = ceil(rise / course);
            let made = min(u1 - u0, high * course * 0.5);
            if q.y >= cut {
                v0 = select(at, at + clear, curved && up);
                at_point[2] = select(cut, at * course, curved);
                how[2] = drawn;
                shape[2] = form;
                size[2] = made;
                side = 1;
            } else {
                v1 = select(at, at - clear, curved && !up);
                at_point[3] = select(cut, at * course, curved);
                how[3] = drawn;
                shape[3] = form;
                size[3] = made;
            }
        } else {
            if u1 - u0 < half * 3.0 {
                break;
            }
            let at = clamp(round((u0 + (u1 - u0) * (0.5 + r * 0.3)) / half) * half, u0 + half, u1 - half);
            // (Stepped: aside half a course or a whole, from a course's
            // height up.)
            let jog = select(-1.0, 1.0, r2 < 0.5) * half * select(1.0, 2.0, r3 < 0.5);
            let onset = v0 + 1.0 + floor(hash(vec2<i32>(k, depth + 800), 1 << 20) * max(high - 1.0, 0.0));
            let keys = keyed && high >= 2.0 && at + jog > u0 + half * 0.5 && at + jog < u1 - half * 0.5;
            // (A curve: from the cut's ends, left or right, as far as it
            // may.)
            let right = hash(vec2<i32>(k, depth + 1200), 1 << 20) < 0.5;
            let mid = (v0 + v1) * 0.5 * course;
            let span = (v1 - v0) * 0.5 * course;
            let rise = min(span, select(at - u0, u1 - at, right) - course);
            let curved = !keys && !open && rise >= course && hash(vec2<i32>(k, depth + 1000), 1 << 20) < marks.y;
            let bow = select(-rise, rise, right);
            let t = clamp((q.y - mid) / span, -1.0, 1.0);
            let lift = select(0.0, bow * sqrt(max(1.0 - t * t, 0.0)), curved);
            let cut = select(at, at + jog, keys && v >= onset) + lift;
            let drawn = select(select(select(0, 2, curved), 1, keys), 3, open);
            let form = select(vec3<f32>(onset * course, min(at, at + jog), max(at, at + jog)), vec3<f32>(mid, span, bow), curved);
            let clear = ceil(rise / half) * half;
            let made = min(high * course, (u1 - u0) * 0.5);
            if q.x >= cut {
                u0 = select(at, at + clear, curved && right);
                at_point[0] = select(cut, at, curved);
                how[0] = drawn;
                shape[0] = form;
                size[0] = made;
                side = 1;
            } else {
                u1 = select(at, at - clear, curved && !right);
                at_point[1] = select(cut, at, curved);
                how[1] = drawn;
                shape[1] = form;
                size[1] = made;
            }
        }
        k = i32(hash(vec2<i32>(k, depth * 2 + side + 400), 1 << 20) * 65536.0);
        merged = merged || open;
        if !merged {
            tone = i32(hash(vec2<i32>(tone, depth * 2 + side + 1100), 1 << 20) * 65536.0);
        }
    }
    // (The nearest of its sides drawn as they stand at the point: straight,
    // the steps (a left or right side's runs across at a height, a bottom
    // or top's up at a place along), the curves (a left or right side's
    // bowing aside, a bottom or top's up or down).)
    let sides = array<f32, 4>(q.x - at_point[0], at_point[1] - q.x, q.y - at_point[2], at_point[3] - q.y);
    let dirs = array<vec2<f32>, 4>(vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, -1.0), vec2<f32>(0.0, 1.0));
    var edge = 1e9;
    var dir = vec2<f32>(0.0, 1.0);
    var made = band * course;
    for (var i = 0; i < 4; i++) {
        if how[i] == 3 {
            continue;
        }
        let f = shape[i];
        if how[i] == 2 {
            // (The curve beside the point, or past its ends the cut straight
            // on, as it divides there (a piece cut into a curve can reach
            // past the span of a curve cut in it after); worked as a bottom
            // or top's, a left or right side's turned to it.)
            let flip = i < 2;
            let p = select(q, q.yx, flip);
            let t = (p.x - f.x) / f.y;
            var d = 1e9;
            var to = vec2<f32>(0.0, 1.0);
            if abs(t) < 1.0 {
                let s = sqrt(max(1.0 - t * t, 1e-4));
                let slope = -f.z * t / (f.y * s);
                let g = sqrt(1.0 + slope * slope);
                let y = at_point[i] + f.z * s;
                d = abs(p.y - y) / g;
                to = vec2<f32>(-slope, 1.0) / g * sign(y - p.y);
            } else {
                d = abs(p.y - at_point[i]);
                to = vec2<f32>(0.0, sign(at_point[i] - p.y));
            }
            if d < edge {
                edge = d;
                dir = select(to, to.yx, flip);
                made = size[i];
            }
            continue;
        }
        if sides[i] < edge {
            edge = sides[i];
            dir = dirs[i];
            made = size[i];
        }
        if how[i] == 1 {
            if i < 2 && q.x >= f.y && q.x <= f.z && abs(q.y - f.x) < edge {
                edge = abs(q.y - f.x);
                dir = vec2<f32>(0.0, sign(f.x - q.y));
                made = size[i];
            } else if i >= 2 && q.y >= f.y && q.y <= f.z && abs(q.x - f.x) < edge {
                edge = abs(q.x - f.x);
                dir = vec2<f32>(sign(f.x - q.x), 0.0);
                made = size[i];
            }
        }
    }
    return Panel(vec2<i32>(tone, 7), edge, dir, made, min(u1 - u0, (v1 - v0) * course));
}

// The paving region a point falls in (cells of a Voronoi pattern 512 m
// apart, which fits the wrap period): whether it is paved, its centre, and a
// number of its own.
struct Region {
    paved: bool,
    centre: vec2<f32>,
    id: f32,
}

fn region(p: vec2<f32>) -> Region {
    let size = 512.0;
    let period = i32(round(wrap.x / size));
    let base = vec2<i32>(floor(p / size));
    var best = 1e9;
    var r = Region(false, vec2<f32>(0.0), 0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = base + vec2<i32>(x, y);
            let f = (vec2<f32>(c) + vec2<f32>(hash(c, period), hash(c + vec2(17, 31), period)) * 0.8 + 0.1) * size;
            let d = distance(p, f);
            if d < best {
                best = d;
                let id = hash(c + vec2(53, 7), period);
                r = Region(id < paving.w, f, id);
            }
        }
    }
    return r;
}

// Distance (in tile units) to the nearest joint of a unit square tiling at
// `q`, and the tile it is in.
fn tile_edge(q: vec2<f32>) -> f32 {
    let f = fract(q);
    let e = min(f, 1.0 - f);
    return min(e.x, e.y);
}

// The paving at `p` on a top surface: a tone (added to albedo, about -1..1)
// and how much of a joint is there (0..1), antialiased with `pixel`.
struct Paving {
    tone: f32,
    joint: f32,
}

fn pave(p: vec2<f32>, height: f32, pixel: f32) -> Paving {
    var out = Paving(0.0, 0.0);
    let r = region(p);
    if !r.paved {
        return out;
    }
    let tile = paving.y;
    let mode = i32(round(paving.x));
    // Each flat piece of ground at its own height its own turn and offset of
    // the pattern, so it breaks at every step: laid plate by plate.
    let level = vec2<i32>(i32(round(height * 4.0)), 977);
    let a = (r.id + hash(level, 1 << 20)) * 6.2831;
    let shift = vec2<f32>(hash(level + vec2(0, 13), 1 << 20), hash(level + vec2(0, 29), 1 << 20)) * tile * 8.0;
    let rot = mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a));
    let z = rot * (p - r.centre) + shift;
    let joint_w = 0.025; // in tile units: 5 cm on a 2 m tile
    if mode == 1 {
        let q = z / tile;
        let i = vec2<i32>(floor(q));
        let checker = f32((i.x + i.y) & 1) * 2.0 - 1.0;
        let aa = max(pixel / tile, 1e-4);
        out.tone = checker;
        out.joint = 1.0 - smoothstep(joint_w - aa, joint_w + aa, tile_edge(q));
        // Joints thinner than a pixel fade rather than shimmer.
        out.joint *= saturate(joint_w / aa);
    } else if mode == 2 {
        // Fractal: tiles four times the tile size split again and again (in
        // two each way), down to a quarter of it; each finished tile its own
        // tone; the joints thinner the smaller the tiles.
        var size = tile * 4.0;
        var edge_m = 1e9;
        var tone = 0.0;
        let period = 1 << 20;
        for (var level = 0; level < 6; level++) {
            let q = z / size;
            let i = vec2<i32>(floor(q));
            let w = 0.06 * pow(size / (tile * 4.0), 0.35);
            edge_m = min(edge_m, tile_edge(q) * size - w);
            let h = hash(i + vec2(level * 1013, level * 7919), period);
            tone = hash(i + vec2(level * 331 + 5, 91), period) * 2.0 - 1.0;
            if h > 0.7 || size < tile * 0.5 {
                break;
            }
            size *= 0.5;
        }
        out.tone = tone;
        out.joint = 1.0 - smoothstep(-pixel, pixel, edge_m);
    }
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // A column fading in or out (see terrain.rs, `Fading`).
#ifdef VISIBILITY_RANGE_DITHER
    visibility_range_dither(in.position, in.visibility_range_dither);
#endif
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    var lit = 0.0;
    // A plate's top: its level and number (see `plate_code` in worldgen);
    // its walls', their line's too (`wall_code`); the level of detail
    // drawing it (see `to_bevy_mesh`).
    var plate = -1;
    var lod = 0;
#ifdef VERTEX_COLORS
    if in.color.g > 0.5 {
        plate = i32(round(in.color.g)) - 1;
        lod = plate / 131072;
        plate = plate % 131072;
    }
    let visibility = mix(1.0, in.color.a, params.x);
    pbr_input.material.base_color = vec4<f32>(vec3<f32>(in.color.r), 1.0);
    lit = in.color.b;
    pbr_input.diffuse_occlusion *= visibility;
    pbr_input.specular_occlusion *= visibility;
#endif

    // Undo the curvature so the paving stays fixed to the ground as the
    // camera moves.
    var p = in.world_position.xyz;
    let d = p.xz - params.zw;
    p.y += dot(d, d) * params.y;

    let n = normalize(in.world_normal);
    // (The size of a pixel on the ground, in metres: outside the branch,
    // since derivatives need every pixel nearby to take the same path.)
    let pixel = max(length(fwidth(p.xz)), 1e-4);
    // (And on a wall: its height counts.)
    let pixel_wall = max(length(fwidth(p)), 1e-4);
    let dp = mat2x3<f32>(dpdx(p), dpdy(p));

    // The plates' subdivision, on their tops.
    if wrap.y > 0.5 && plate >= 0 && n.y > 0.85 {
        // (Its district may keep to the big pieces: see `Rules::pieces`.)
        let code = plate / 4;
        let smallest = select(pieces.x, max(pieces.x, 32.0), code >= 1024);
        let sub = subdivide(p.xz, pixel, plate % 4, code % 1024, smallest);
        // (Grooved, the joint is shaded by its slope, and darkened less.)
        let dark = select(glow.w, glow.w * 0.4, pieces.y > 0.0);
        let c = pbr_input.material.base_color.rgb * (1.0 + sub.tone * glow.y) * (1.0 - dark * sub.joint);
        pbr_input.material.base_color = vec4<f32>(c, 1.0);
        let t = tan(radians(glow.z));
        let g = tan(radians(pieces.y));
        pbr_input.N = normalize(pbr_input.N + vec3<f32>(sub.tilt.x, 0.0, sub.tilt.y) * t + vec3<f32>(sub.groove.x, 0.0, sub.groove.y) * g);
    }

    // Their walls'.
    if wrap.y > 0.5 && walls.x > 0.5 && plate >= 0 && abs(n.y) < 0.2 {
        var t = normalize(vec2<f32>(-n.z, n.x));
        // (Where it stands: a coarser level of detail is lowered; and the
        // world wraps, a wall drawn a world over from where it was made
        // when the camera has crossed the edge (its panels changed as it
        // was approached, the user, at the edge). Only a wall across the
        // edge itself changes there.)
        let home = p.xz - floor(p.xz / wrap.x) * wrap.x;
        let at = vec3<f32>(home.x, p.y + select(0.0, select(marks.z, marks.w, lod == 2), lod > 0), home.y);
        // (Its line's number: see `wall_code`.)
        var along = dot(at.xz, t);
        // (A wall round a middle, facing it or away, a ring of facets: along
        // its curve, so the facets are one wall, not each its own. Measured
        // round from +x the other way, its seam where the arena is cut.)
        if round_walls.w > 0.5 {
            let r = at.xz - round_walls.xy;
            let out = normalize(r);
            if abs(dot(normalize(n.xz), out)) > 0.98 {
                along = atan2(-r.y, -r.x) * length(r);
                t = vec2<f32>(-out.y, out.x);
            }
        }
        let sub = wall(vec2<f32>(along, at.y), dp, t, pixel_wall, plate / 8192);
        let dark = select(glow.w, glow.w * 0.4, pieces.y > 0.0);
        let c = pbr_input.material.base_color.rgb * (1.0 + sub.tone) * (1.0 - dark * sub.joint);
        pbr_input.material.base_color = vec4<f32>(c, 1.0);
        let g = tan(radians(pieces.y));
        pbr_input.N = normalize(pbr_input.N + (vec3<f32>(t.x, 0.0, t.y) * sub.groove.x + vec3<f32>(0.0, sub.groove.y, 0.0)) * g);
    }

    // Paving, on top surfaces in paved regions.
    if paving.x > 0.5 && n.y > 0.85 {
        let pv = pave(p.xz, p.y, pixel);
        let c = pbr_input.material.base_color.rgb * (1.0 + pv.tone * paving.z) * (1.0 - 0.55 * pv.joint);
        pbr_input.material.base_color = vec4<f32>(c, 1.0);
    }

    // Lit pieces of geometry (glowing etchings and seams).
    if lit > 0.0 {
        pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + vec3<f32>(lit * glow.x), 1.0);
    }

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    set_far(far_shadow(in.world_position.xyz, normalize(in.world_normal)), far_texel(0, 5).xyz, far_texel(2, 5).xyz);
    out.color = apply_pbr_lighting(pbr_input);
    // A void's walls sink into the dark below the world (see worldgen's
    // `VOID_DEPTH`): the hole reads as one, not as more ground (its far
    // wall, lit, did). By the height unbent, 60 m under the world's level
    // on (the ground's lowest is about -20 m).)
    let flat = in.world_position.xyz + vec3<f32>(0.0, dot(in.world_position.xz - params.zw, in.world_position.xz - params.zw) * params.y, 0.0);
    out.color = vec4<f32>(out.color.rgb * exp(min(flat.y + 60.0, 0.0) / 120.0), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
