// L4 silhouettes (CITY_SPEC §CAM): a twin of each player mesh drawn after the opaque pass (premultiplied blend, no depth write,
// depth compare Always) that shows a flat colour only where the depth prepass holds something more than `margin`
// in front of it: the parts an occluder still hides after the L2 cut (pillars, signs, cut-floor stubs). The
// character's own limbs are within the margin, so they never count. Needs DepthPrepass on the camera.
// Where the twin is the front-most surface it draws a faint fresnel rim instead (s30: the player lost in the night
// streets keeps an outline without lifting the scene).

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    prepass_utils::prepass_depth,
    view_transformations::depth_ndc_to_view_z,
    mesh_view_bindings::view,
}

struct Sil {
    // rgb linear colour, a = opacity
    colour: vec4<f32>,
    // x margin m (view z), y rim boost
    misc: vec4<f32>,
    // the visible body's rim: rgb linear colour, a = opacity at a grazing edge (0 = off)
    rim: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> sil: Sil;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var out: FragmentOutput;
#ifdef DEPTH_PREPASS
    let scene = depth_ndc_to_view_z(prepass_depth(in.position, 0u));
    let me = depth_ndc_to_view_z(in.position.z);
    let fwd = normalize(view.world_from_view[2].xyz);
    let edge = 1.0 - abs(dot(normalize(in.world_normal), fwd));
    // -z is forward: the scene is nearer when its view z is larger
    if (scene < me + sil.misc.x) {
        // the front-most surface (the body itself, not a limb behind the torso): the rim, strongest at the edge
        if (sil.rim.a <= 0.0 || scene > me + 0.03) {
            discard;
        }
        // premultiplied with alpha 0: added light, never a veil over the body's own colours / emissives
        let k = edge * edge * edge;
        out.color = vec4<f32>(sil.rim.rgb * sil.rim.a * k, 0.0);
        return out;
    }
    // a little brighter toward the silhouette's edge (normals across the view): reads as a shape, not a decal
    let rim = 1.0 + sil.misc.y * edge;
    // premultiplied: the flat colour blended at its opacity
    out.color = vec4<f32>(sil.colour.rgb * rim * sil.colour.a, sil.colour.a);
#else
    discard;
#endif
    return out;
}
