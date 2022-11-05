struct FullscreenVertexOutput {
    @builtin(position)
    position: vec4<f32>,
    @location(0)
    uv: vec2<f32>,
};

@vertex
fn fullscreen_vertex_shader(@builtin(vertex_index) vertex_index: u32) -> FullscreenVertexOutput {
    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
    let clip_position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);

    return FullscreenVertexOutput(clip_position, uv);
}

// ------------------------------------------------------------------------------------------------

@group(0) @binding(0) var<storage, read> masses: array<f32>;
@group(1) @binding(0) var<storage, read_write> positions: array<vec3<f32>>;

@fragment
fn trace(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let camera_direction: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);
    let camera_position: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    let window_size: vec2<f32> = vec2<f32>(20.0,15.0);
    let camera_pixel_position: vec3<f32> = vec3<f32>(window_size.x*uv.x, window_size.y*uv.y, 0.0) + 
camera_position;
    // TODO: Loop over bodies, raycast, find closest intersection (if any)
    // TODO: Shade pixel, for now inverse-depth
    var index_of_nearest_intersection: i32 = -1;
    var distance_of_nearest_intersection: f32 = 100000000000.0;
    for (var i: u32 = 0u; i < arrayLength(&positions); i++) {
        let body_position: vec3<f32> = positions[i];
        let body_position_2d: vec2<f32> = body_position.xy;
        let body_radius: f32 = 1.0f*masses[i];
        if (length(body_position_2d-camera_pixel_position.xy)<body_radius) {
          index_of_nearest_intersection = i32(i);
        }
        /*let displacement: vec3<f32> = body_position - camera_pixel_position;
        let displacement_length: f32 = max(length(displacement), 0.001);
        let displacement_normal: vec3<f32> = displacement / displacement_length;
        let dot_product: f32 = dot(displacement_normal, camera_direction);
        if dot_product <= 0.0 {continue};
        let angle: f32 = acos(dot_product);
        let effective_radius: f32 = sin(angle) * displacement_length;
        if effective_radius <= body_radius && cos(angle) * displacement_length < distance_of_nearest_intersection {
            distance_of_nearest_intersection = cos(angle) * displacement_length;
            index_of_nearest_intersection = i32(i);
        }*/
    }
    if index_of_nearest_intersection == -1 {
        return vec4(1.0, 0.0, 0.0, 1.0);
    } else {
        return vec4(0.0, 0.0, 1.0, 1.0);
    }
}
