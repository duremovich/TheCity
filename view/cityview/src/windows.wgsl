// LOD2 massing facades (CITY_SPEC §PHASE P3, SPEC §TWR 7.4 "massing boxes plus emissive window cards"): the
// StandardMaterial lit as usual, plus procedural windows on the vertical faces. Floors from the building's base
// (a ground floor, then upper floors), bays on the tile grid; each (face, floor, room) hashed per building: lit
// (share <= the building's, SPEC §PAL 3.4, irregular: rooms span 1-2 bays, floors vary) as window_warm /
// fluoro_cold, else glass_dark. A window grammar per district (style), a landmark's crown band in the key hue.
// Far away (a bay under ~3 px) windows group into 2^L cells so the facade stays dotted without shimmer.
// Depth cue (review p3 r1): a height haze toward Grade A's fog below the view's reference height, and a canyon
// term darkening the lowest ~15 m of every facade.
//
// Per vertex: color = (lit share, warm share, pattern seed, upper floor height), uv_b = (base z, ground floor h),
// uv = (style, crown flag).
// Camera occlusion (P4): the see-through volume cut (glclient::occlusion; the prepass runs the same discard).

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif
#import glclient::occlusion::{occ_apply, occ_face_normal, occ_inside, occ_under_floor, OCC_FLOOR, OCC_FLOOR_GLOW, OCC_WALL, OCC_WALL_GLOW}

struct Windows {
    warm: vec4<f32>,     // rgb linear window_warm, a = strength scale
    cold: vec4<f32>,     // rgb linear fluoro_cold
    glass: vec4<f32>,    // rgb linear glass_dark, a = its roughness
    misc: vec4<f32>,     // x unused, y lit multiplier (debug), z wall albedo scale, w unused
    key: vec4<f32>,      // rgb linear crown key hue (Grade A magenta), a = its strength
    haze: vec4<f32>,     // rgb linear haze (Grade A fog), a = max amount (0 = off)
    haze_z: vec4<f32>,   // x reference z (no haze at / above), y range m (full haze this far below)
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> win: Windows;

fn pcg3(v0: vec3<u32>) -> vec3<u32> {
    var v = v0 * 1664525u + 1013904223u;
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    return v;
}

fn hash3(v: vec3<u32>) -> vec3<f32> {
    return vec3<f32>(pcg3(v)) * (1.0 / 4294967295.0);
}

// window grammar: x = bay width m, y = glass x share (centred), z = glass y low, w = glass y high (of a floor)
fn grammar(style: i32) -> vec4<f32> {
    switch style {
        case 1: { return vec4<f32>(2.0, 0.42, 0.36, 0.78); }   // walk-up (D1, D7): small punched windows
        case 2: { return vec4<f32>(1.5, 0.92, 0.10, 0.90); }   // curtain wall (D2): ribbon glass
        case 3: { return vec4<f32>(2.0, 0.60, 0.30, 0.82); }   // residential tower (D4)
        case 4: { return vec4<f32>(4.0, 0.90, 0.70, 0.88); }   // industrial (D6): a clerestory strip
        case 5: { return vec4<f32>(3.0, 0.70, 0.18, 0.86); }   // villa (D5): big windows
        default: { return vec4<f32>(3.0, 0.76, 0.36, 0.76); }  // D3 stacks: wide landscape windows
    }
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    let wp = in.world_position.xyz;
    let occ_n = occ_face_normal(wp);
    let occ_rim = occ_apply(wp, in.position, occ_n, in.instance_index, false);
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    var emissive = vec3<f32>(0.0);
#ifdef VERTEX_COLORS
#ifdef VERTEX_UVS_B
#ifdef VERTEX_UVS_A
    let lit = in.color.x * win.misc.y;
    let warm = in.color.y;
    let seed = u32(round(in.color.z));     // 16-bit seeds, rounded: exact through interpolation
    let fh = max(in.color.w, 1.0);
    let base = in.uv_b.x;
    let gh = max(in.uv_b.y, 1.0);
    let style = i32(in.uv.x + 0.5);
    let crown = in.uv.y > 0.5 && in.uv.y < 1.5;
    let blank = in.uv.y > 1.5;
    let gr = grammar(style);
    let tone = hash3(vec3<u32>(seed, 17u, 3u));
    // wall: a concrete tone per building (SPEC §PAL: concrete / stained / a little brick), never brighter than #6A6A72
    let wall = mix(vec3<f32>(0.0212, 0.0212, 0.0284), vec3<f32>(0.0423, 0.0423, 0.0545), tone.x);
    var albedo = mix(wall, vec3<f32>(0.0685, 0.0232, 0.0232), step(0.85, tone.y)) * win.misc.z;

    let n = normalize(in.world_normal);
    let h = wp.y - base;
    if (abs(n.y) < 0.5 && !blank) {
        // canyon: the street-level metres of a facade get less sky
        albedo *= mix(0.45, 1.0, smoothstep(0.0, 15.0, h));
        let side_x = abs(n.x) > abs(n.z);
        let along = select(wp.x, wp.z, side_x);
        let bw = gr.x;
        let px = max(fwidth(along), fwidth(wp.y)) / bw;
        let lvl = max(0.0, ceil(log2(px / 0.33)));
        let g = exp2(lvl);
        var fl: i32;
        var fy: f32;
        if (lvl == 0.0 && h < gh) {
            fl = 0;
            fy = h / gh;
        } else if (lvl == 0.0) {
            let k = (h - gh) / fh;
            fl = 1 + i32(floor(k));
            fy = fract(k);
        } else {
            let k = h / (fh * g);
            fl = 1 + i32(floor(k));
            fy = fract(k);
        }
        let b = along / (bw * g);
        let bay = i32(floor(b));
        let fu = fract(b);
        let face = select(select(0u, 1u, n.z > 0.0), select(2u, 3u, n.x > 0.0), side_x);
        // rooms: 1-2 bays; a curtain wall lights in long office runs (6 bays)
        var span = 1 + i32(hash3(vec3<u32>(seed, u32(fl + 4096), 7u)).x * 2.0);
        if (style == 2) { span = 6; }
        if (lvl > 0.0) { span = 1; }
        let room = vec3<u32>(seed ^ (face * 7919u) ^ (u32(lvl) << 20u), u32(fl + 4096), u32(bay / span + 1048576));
        let r = hash3(room);
        // floors vary (some floors dark, some busy): the mean share is kept
        let fvar = mix(0.3, 1.7, hash3(vec3<u32>(seed, u32(fl + 4096), 11u)).x);
        let shop = fl == 0 && lvl == 0.0;
        // (no `a < x, b > y` inside a call: WGSL reads it as a template list)
        let half_x = select(gr.y, 0.88, shop) * 0.5;
        let y_lo = select(gr.z, 0.06, shop);
        let y_hi = select(gr.w, 0.72, shop);
        let in_x = abs(fu - 0.5) < half_x;
        let in_y = (fy > y_lo) && (fy < y_hi);
        let p_lit = select(min(1.0, lit * fvar), min(1.0, lit * 1.6), shop);
        let hue = mix(win.cold.rgb, win.warm.rgb, select(0.0, 1.0, r.y < warm));
        let strength = mix(0.6, 1.4, r.z) * win.warm.a;
        var is_glass = false;
        if (in_x && in_y && h > 0.0) {
            is_glass = true;
            if (r.x < p_lit) {
                emissive = hue * strength;
            }
        }
        if (crown) {
            // a landmark's crown: a lit band in the key hue (SPEC §TWR 7.2 "a lit corporate logo")
            let band = (fy > 0.35) && (fy < 0.65);
            if (band) {
                emissive = win.key.rgb * win.key.a;
                is_glass = false;
            }
        }
        if (is_glass && lvl < 2.0) {
            albedo = win.glass.rgb;
            pbr_input.material.perceptual_roughness = win.glass.a;
        }
    }
    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
#endif
#endif
#endif
    emissive += occ_rim;
    // V4b: the inside of a cut box (double-sided: only seen through the cutaway): a floor plate under the subject's
    // floor (the far walls painted as the cap they stand for), the room's bare walls above it; no windows
    var inside = occ_inside(wp, is_front);
    if (inside < 0.0 && occ_under_floor(wp)) {
        inside = 1.0;
    }
    if (inside >= 0.0) {
        pbr_input.material.perceptual_roughness = 0.9;
        if (inside > 0.5) {
            pbr_input.material.base_color = vec4<f32>(OCC_FLOOR, 1.0);
            pbr_input.N = vec3<f32>(0.0, 1.0, 0.0);
            emissive = OCC_FLOOR_GLOW;
        } else {
            pbr_input.material.base_color = vec4<f32>(OCC_WALL, 1.0);
            pbr_input.N = -pbr_input.N;
            emissive = OCC_WALL_GLOW;
        }
    }
    pbr_input.material.emissive = vec4<f32>(emissive, 0.0);

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // height haze: lower = hazier (lit windows keep half their punch through it)
    let hz = clamp((win.haze_z.x - wp.y) / max(win.haze_z.y, 1.0), 0.0, 1.0) * win.haze.a;
    out.color = vec4<f32>(mix(out.color.rgb, win.haze.rgb, hz) + emissive * 0.5 * hz, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
