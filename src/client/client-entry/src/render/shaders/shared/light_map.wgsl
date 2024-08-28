const SHADOW_BIAS: f32 = 0.0003;

fn is_lit(
    light_map: texture_2d<f32>,
    light_sampler: sampler,
    light_dir: vec3f,
    in_light_space: vec4f,
    in_normal: vec3f,
) -> bool {
    var light_space: vec3f = in_light_space.xyz;
    light_space /= in_light_space.w;

    var max_lit_depth: f32 = textureSample(
        light_map,
        light_sampler,
        light_space.xy * vec2f(0.5, -0.5) + 0.5,
    ).r;
    var my_lit_depth: f32 = light_space.z - SHADOW_BIAS;

    return my_lit_depth < max_lit_depth && dot(in_normal, light_dir) < 0.;
}
