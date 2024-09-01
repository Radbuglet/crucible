const SHADOW_BIAS: f32 = 0.0003;

fn shadow_level(
    light_map: texture_2d<f32>,
    light_sampler: sampler,
    light_dir: vec3f,
    in_light_space: vec4f,
    in_normal: vec3f,
) -> f32 {
    // Determine where we are in light-space.
    var light_space: vec3f = in_light_space.xyz;
    light_space /= in_light_space.w;
    let my_lit_depth: f32 = light_space.z - SHADOW_BIAS;

    // Handle back-surfaces
    if dot(in_normal, light_dir) > 0. {
        // The surface normal and the light direction are facing the same way. This surface is fully
        // in shadow.
        return 0f;
    }

    // Determine the spread factor
    let main_max_lit_depth = sample_light_space(light_map, light_sampler, light_space.xy);
    var spread_factor: f32;
    if my_lit_depth > main_max_lit_depth {
        spread_factor = min(1f, (my_lit_depth - main_max_lit_depth) * 50f);
    } else {
        spread_factor = 1f;
    }

    // Do some sampling!
    var percent = 0f;
    let scale = (0.1f + spread_factor * 5f) / vec2f(textureDimensions(light_map));

    for (var x = -5; x <= 5; x++) {
        for (var y = -5; y <= 5; y++) {
            let max_lit_depth = sample_light_space(
                light_map,
                light_sampler,
                light_space.xy + scale * vec2f(f32(x), f32(y)),
            );

            if my_lit_depth < max_lit_depth {
                percent += 1f;
            }
        }
    }

    return percent / 121f;
}

fn sample_light_space(
    light_map: texture_2d<f32>,
    light_sampler: sampler,
    pos: vec2f,
) -> f32 {
    return textureSample(
        light_map,
        light_sampler,
        pos * vec2f(0.5, -0.5) + 0.5,
    ).r;
    
}
