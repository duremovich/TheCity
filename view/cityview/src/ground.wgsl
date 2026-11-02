// Ground and decks: the StandardMaterial (vertex colours) plus the city view's height haze (windows.wgsl), so
// streets and decks share the massing's depth cue (review p3 r1). V4b: the ground under the subject's own building,
// seen through its floor (a ground-floor room), is painted as the cutaway's floor plate (glclient::occlusion).

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
#import glclient::occlusion::{occ_under_floor, OCC_FLOOR, OCC_FLOOR_GLOW}

struct Haze {
    haze: vec4<f32>,     // rgb linear haze (Grade A fog), a = max amount (0 = off)
    haze_z: vec4<f32>,   // x reference z, y range m
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> hz_u: Haze;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    if (occ_under_floor(in.world_position.xyz)) {
        pbr_input.material.base_color = vec4<f32>(OCC_FLOOR, 1.0);
        pbr_input.material.emissive = vec4<f32>(OCC_FLOOR_GLOW, 0.0);
        pbr_input.material.perceptual_roughness = 0.9;
    }
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    let hz = clamp((hz_u.haze_z.x - in.world_position.y) / max(hz_u.haze_z.y, 1.0), 0.0, 1.0) * hz_u.haze.a;
    out.color = vec4<f32>(mix(out.color.rgb, hz_u.haze.rgb, hz), out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
