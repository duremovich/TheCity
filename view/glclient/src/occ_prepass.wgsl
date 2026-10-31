// The prepass (depth / normal) and shadow-pass fragment of every occluder material (§CAM trap 1: the same discard
// as the main pass, so a hole leaves no depth behind; shadow views run it too and `occ_apply` keeps them whole).
// The occluder materials have no normal maps, so the normal prepass writes the interpolated world normal. Every
// occluder material extends StandardMaterial, so the base's own alpha test (a MASK cutout) runs first, as in Bevy's
// prepass: its holes leave no depth and cast no shadow.

#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_prepass_functions,
}
#import glclient::occlusion::{occ_apply, occ_face_normal}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let wp = in.world_position.xyz;
    let n = occ_face_normal(wp);
    pbr_prepass_functions::prepass_alpha_discard(in);
    let rim = occ_apply(wp, in.position, n, in.instance_index, true);
    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
    out.normal = vec4<f32>(normalize(in.world_normal) * 0.5 + vec3<f32>(0.5), 1.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
#endif
    return out;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    let wp = in.world_position.xyz;
    let n = occ_face_normal(wp);
    pbr_prepass_functions::prepass_alpha_discard(in);
    let rim = occ_apply(wp, in.position, n, in.instance_index, true);
}
#endif
