// Camera occlusion (CITY_SPEC §CAM, spec/city/P4_PLAN.md A): the shared part of every occluder material.
//
// L2 see-through volume: `occ_cut` discards a fragment that is inside a view-space capsule (feet -> head) around a
// player AND on the camera's side of the vertical plane through the player (by a margin, along the camera's
// ground-projected forward: s28, a view-depth test also cut the upper floors of buildings *behind* the player,
// which the tilted camera sees as nearer), with a noise-broken edge and a thin emissive rim
// just outside it (a scan edge, not a hole). L3 whole-object fade: `occ_fade` screen-doors a mesh whose MeshTag
// carries a fade level. Both only in the game camera's views (shadow views never cut: §CAM trap 2), and both run
// identically in the prepass and the main pass (trap 1: a main-pass-only discard leaves the occluder's depth).
// Walkable surfaces never cut where a material sets `keep_up`: horizontal faces at or below a player's feet
// (+0.5 m: its floor, the treads ahead); a slab over its head (a canopy, a landing) cuts like a wall.
//
// A material using this library declares `#[uniform(101)] occ: OccUniform` (glclient::occlusion) and returns
// AlphaMode::Mask from its extension (so the prepass runs a fragment shader at all), and calls
// `glclient::occlusion::occ_specialize` from its `specialize` (OCC_OFF for the pre-P4 baseline).

#define_import_path glclient::occlusion

#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_functions::get_tag

struct Occ {
    // the game camera's view_from_world (the capsule test always runs in its space)
    view_from_world: mat4x4<f32>,
    // xyz the game camera's world position (a view at another position is a shadow view: no cut), w = enabled
    cam: vec4<f32>,
    // per volume: feet (world xyz), w = current radius m (0 = off)
    feet: array<vec4<f32>, 4>,
    // per volume: head (world xyz), w = view z of the body centre (game camera; unused by the cut since s28)
    head: array<vec4<f32>, 4>,
    // x depth margin m, y flags (1 = cut in every view [control], 2 = no prepass cut [control], 4 = the view-depth
    // front test [diagnostic]), z L3 fade keep
    // share, w rim strength
    misc: vec4<f32>,
    // rgb linear rim hue, a rim width m
    rim: vec4<f32>,
    // per material: x keep_up (1 = horizontal faces never cut), y L3 fade by MeshTag (1), z L2 cut (1), w no L2 on
    // meshes whose MeshTag has KEEP_BIT (0x100: the scene buildings no body's ray hit; glclient::occlusion)
    mat: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> occ: Occ;

const DITHER: vec4<u32> = vec4<u32>(0x0a020800u, 0x060e040cu, 0x09010b03u, 0x050d070fu);

fn occ_flag(bit: u32) -> bool {
    return (u32(occ.misc.y + 0.5) & bit) != 0u;
}

fn occ_game_view() -> bool {
    return occ_flag(1u) || distance(view.world_position, occ.cam.xyz) < 0.05;
}

fn occ_hash(p: vec3<i32>) -> f32 {
    var h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ u32(p.z) * 83492791u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h & 0xffffu) / 65535.0;
}

// 3D value noise in [0, 1]
fn occ_noise(p: vec3<f32>) -> f32 {
    let i = vec3<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(occ_hash(i), occ_hash(i + vec3<i32>(1, 0, 0)), u.x);
    let b = mix(occ_hash(i + vec3<i32>(0, 1, 0)), occ_hash(i + vec3<i32>(1, 1, 0)), u.x);
    let c = mix(occ_hash(i + vec3<i32>(0, 0, 1)), occ_hash(i + vec3<i32>(1, 0, 1)), u.x);
    let d = mix(occ_hash(i + vec3<i32>(0, 1, 1)), occ_hash(i + vec3<i32>(1, 1, 1)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

// Signed distance (m, view-space xy) from the fragment to the nearest cut edge: < 0 = inside a hole; a large
// positive value when no volume applies (off, or the fragment is not in front of that player).
fn occ_edge(wp: vec3<f32>, walkable: bool) -> f32 {
    let vp = (occ.view_from_world * vec4<f32>(wp, 1.0)).xyz;
    // the edge wanders +-15 % of the radius (a dissolve, not a circle), the band capped at 0.72 m (2.4 m's): a
    // larger, context-sized portal keeps the same edge instead of shredding into shards
    let wob_n = occ_noise(wp * (1.0 / 0.7)) - 0.5;
    // the game camera's forward (world) projected on the ground: -(view z row); ortho, so one direction for all
    let m = occ.view_from_world;
    let f = normalize(-vec2<f32>(m[0].z, m[2].z));
    var best = 1e6;
    for (var k = 0; k < 4; k++) {
        let r = occ.feet[k].w;
        if (r <= 0.0) {
            continue;
        }
        if (walkable && wp.y <= occ.feet[k].y + 0.5) {
            continue;
        }
        // between the camera and the player: in front of the vertical plane through its feet by the margin
        // (flag 4, a diagnostic: the pre-s28 view-depth test against the body centre)
        if (occ_flag(4u)) {
            if (vp.z < occ.head[k].w + occ.misc.x) {
                continue;
            }
        } else if (dot(wp.xz - occ.feet[k].xz, f) > -occ.misc.x) {
            continue;
        }
        let a = (occ.view_from_world * vec4<f32>(occ.feet[k].xyz, 1.0)).xy;
        let b = (occ.view_from_world * vec4<f32>(occ.head[k].xyz, 1.0)).xy;
        let ab = b - a;
        let t = clamp(dot(vp.xy - a, ab) / max(dot(ab, ab), 1e-4), 0.0, 1.0);
        let d = length(vp.xy - (a + ab * t));
        best = min(best, d - (r + min(0.3 * r, 0.72) * wob_n));
    }
    return best;
}

// L2 + L3 for a fragment: discards, or returns the rim's emissive (linear rgb; zero away from an edge).
// `face_n` = the geometric normal (from derivatives: identical in the prepass and the main pass).
fn occ_apply(wp: vec3<f32>, frag_coord: vec4<f32>, face_n: vec3<f32>, instance_index: u32, prepass: bool) -> vec3<f32> {
#ifdef OCC_OFF
    // the pre-P4 baseline (glclient::occlusion::BUILT false): no discard anywhere, so early-Z stays on
    return vec3<f32>(0.0);
#else
    if (occ.cam.w < 0.5 || !occ_game_view()) {
        return vec3<f32>(0.0);
    }
    if (prepass && occ_flag(2u)) {
        return vec3<f32>(0.0);
    }
    // L3: a whole-object fade (MeshTag low byte = level 0..255), a stable 4 x 4 screen-door down to the keep share
    if (occ.mat.y > 0.5) {
        let lvl = f32(get_tag(instance_index) & 0xffu) / 255.0;
        if (lvl > 0.0) {
            let c = vec2<u32>(vec2<i32>(frag_coord.xy)) % vec2<u32>(4u);
            let th = (f32((DITHER[c.y] >> (c.x * 8u)) & 0xffu) + 0.5) / 16.0;
            if (th < lvl * (1.0 - occ.misc.z)) {
                discard;
            }
        }
    }
    if (occ.mat.z < 0.5) {
        return vec3<f32>(0.0);
    }
    if (occ.mat.w > 0.5 && (get_tag(instance_index) & 0x100u) != 0u) {
        return vec3<f32>(0.0);
    }
    let e = occ_edge(wp, occ.mat.x > 0.5 && abs(face_n.y) > 0.7);
    if (e < 0.0) {
        discard;
    }
    let rim = 1.0 - smoothstep(0.0, max(occ.rim.a, 1e-3), e);
    return occ.rim.rgb * occ.misc.w * rim;
#endif
}

fn occ_face_normal(wp: vec3<f32>) -> vec3<f32> {
    return normalize(cross(dpdx(wp), dpdy(wp)));
}
