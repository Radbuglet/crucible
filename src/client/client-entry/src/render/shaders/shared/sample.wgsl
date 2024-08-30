fn sample_rand(tex: texture_2d<f32>, sam: sampler, pos: vec2f) -> f32 {
    var accum = 0f;

    let scale = vec2f(1f) / vec2f(textureDimensions(tex));

    for (var x = -2; x <= 2; x++) {
        for (var y = -2; y <= 2; y++) {
            accum += textureSample(tex, sam, pos + vec2f(f32(x), f32(y)) * scale).r;
        }
    }

    return accum / 16f;
}
