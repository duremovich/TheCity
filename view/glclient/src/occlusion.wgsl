// The cutaway (the view milestone V4b; glclient::occlusion): the shared part of every occluder material.
//
// `occ_apply` discards a fragment inside the cut box: on the camera's side of the subject (from `axis.w` m behind its
// feet onward, along the ground direction toward the camera), within `axis.z` m of the camera-to-subject line, and
// above the cut height (`subject.w`). A screen-door dither band `misc.x` m deep softens the box's faces (a section,
// not a sawn edge); a thin emissive rim just under the cut height marks the section line. Only in the game camera's
// views (shadow views never cut: §CAM trap 2), identically in the prepass and the main pass (trap 1: a main-pass-only
// discard leaves the occluder's depth).
//
// A material using this library declares `#[uniform(101)] occ: OccUniform` (glclient::occlusion), returns
// AlphaMode::Mask from its extension (so the prepass runs a fragment shader at all), and calls
// `glclient::occlusion::occ_specialize` from its `specialize` (OCC_OFF for the pre-P4 baseline).

#define_import_path glclient::occlusion

#import bevy_pbr::mesh_view_bindings::view

struct Occ {
    // xyz the game camera's world position (a view elsewhere is a shadow view: no cut), w 1 = the cut runs
    cam: vec4<f32>,
    // xyz the subject's feet (world), w the cut's height (world y)
    subject: vec4<f32>,
    // xy the ground direction from the subject toward the camera (world x, z), z the box's half width, w how far
    // behind the feet the box starts
    axis: vec4<f32>,
    // x the dither band m, y flags (1 = cut in every view [control], 2 = no prepass cut [control]), z the rim's
    // strength, w its height m
    misc: vec4<f32>,
    // rgb the rim's hue (linear), w the subject's ceiling (world y: its own building is cut above it)
    rim: vec4<f32>,
    // the subject's own building (indoors): min x, min z, max x, max z (world); empty = none
    own: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> occ: Occ;

const DITHER: vec4<u32> = vec4<u32>(0x0a020800u, 0x060e040cu, 0x09010b03u, 0x050d070fu);

fn occ_flag(bit: u32) -> bool {
    return (u32(occ.misc.y + 0.5) & bit) != 0u;
}

fn occ_game_view() -> bool {
    return occ_flag(1u) || distance(view.world_position, occ.cam.xyz) < 0.05;
}

// Discards inside the cut box, or returns the rim's emissive (linear rgb; zero away from the section line).
// `face_n` and `instance_index` are kept for the callers' signature (the P4 capsule used them).
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
    let d = wp.xz - occ.subject.xz;
    let a = occ.axis.xy;
    // metres inside each face of the box (> 0 inside): along the line past the back face, across, above the cut
    let along = dot(d, a) + occ.axis.w;
    let across = occ.axis.z - abs(d.x * a.y - d.y * a.x);
    let up = wp.y - occ.subject.w;
    let side = min(along, across);
    // the subject's own building: inside its rect, above its ceiling
    let o = min(min(wp.x - occ.own.x, occ.own.z - wp.x), min(wp.z - occ.own.y, occ.own.w - wp.z));
    let e = max(min(side, up), min(o, wp.y - occ.rim.w));
    let band = max(occ.misc.x, 1e-3);
    if (e > 0.0) {
        // a stable 4 x 4 screen-door through the band
        let c = vec2<u32>(vec2<i32>(frag_coord.xy)) % vec2<u32>(4u);
        let th = (f32((DITHER[c.y] >> (c.x * 8u)) & 0xffu) + 0.5) / 16.0;
        if (e >= band || th < e / band) {
            discard;
        }
    }
    // the section line: just under the cut height, inside the box's footprint
    let rim_m = max(occ.misc.w, 1e-3);
    if (side > 0.0 && up <= 0.0 && up > -rim_m) {
        return occ.rim.rgb * occ.misc.z * (1.0 + up / rim_m) * smoothstep(0.0, band, side);
    }
    return vec3<f32>(0.0);
#endif
}

// The cut plane's paint (the host's materials): the floor plate (linear albedo, a faint warm room glow: the windows
// say the rooms are lit, the night rig alone left a black pit) and the room's bare walls.
const OCC_FLOOR: vec3<f32> = vec3<f32>(0.32, 0.29, 0.26);
const OCC_FLOOR_GLOW: vec3<f32> = vec3<f32>(0.05, 0.022, 0.006);
const OCC_WALL: vec3<f32> = vec3<f32>(0.22, 0.21, 0.22);
const OCC_WALL_GLOW: vec3<f32> = vec3<f32>(0.025, 0.011, 0.003);

// The inside of a closed occluder (a back face: only ever seen through the cut): 1 under the subject's floor (painted
// as the floor plate the far walls stand for: the cap), 0 above it (the room's walls), -1 for a front face or no cut.
fn occ_inside(wp: vec3<f32>, is_front: bool) -> f32 {
    if (is_front || occ.cam.w < 0.5 || !occ_game_view()) {
        return -1.0;
    }
    return select(0.0, 1.0, wp.y < occ.subject.y + 0.02);
}

// Under the subject's floor in its own building, seen through the floor's opening: the camera's ray to the fragment
// crossed the floor's height inside the building's rect (a nested massing tier's roof, a far wall below the floor).
// The host paints it as the floor plate the subject stands on (the cap); a facade under the floor, seen from outside,
// crossed that height outside the rect and keeps its look.
fn occ_under_floor(wp: vec3<f32>) -> bool {
    let floor = occ.subject.y;
    if (occ.cam.w < 0.5 || !occ_game_view() || occ.own.z <= occ.own.x || wp.y > floor + 0.3 || occ.cam.y <= floor) {
        return false;
    }
    let t = (occ.cam.y - floor) / max(occ.cam.y - wp.y, 1e-3);
    let q = occ.cam.xyz + (wp - occ.cam.xyz) * t;
    return q.x > occ.own.x && q.x < occ.own.z && q.z > occ.own.y && q.z < occ.own.w;
}

fn occ_face_normal(wp: vec3<f32>) -> vec3<f32> {
    return normalize(cross(dpdx(wp), dpdy(wp)));
}
