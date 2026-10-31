// A building glb's StandardMaterial with the camera-occlusion cut (glclient::occlusion): the standard PBR path,
// a discard inside a player's see-through volume, the rim as extra emissive.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#import glclient::occlusion::{occ_apply, occ_face_normal}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let wp = in.world_position.xyz;
    let n = occ_face_normal(wp);
    let rim = occ_apply(wp, in.position, n, in.instance_index, false);
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + rim, pbr_input.material.emissive.a);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
