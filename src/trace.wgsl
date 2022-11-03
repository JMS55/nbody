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

//bind groups (which can contain multiple bindings) are collections of resources
	//alloc'd in GPU mem, to which CPU has handles (like ptrs)
@group(0) @binding(0) var<storage, read> positions: array<vec3<f32>>;
@group(0) @binding(1) var<storage, read> masses: array<f32>;
//pipeline: set of shaders etc. to run
	//get a handle to a compiled pipeline
	//run the pipeline (handle), giving it access to bindgroups(s) (as handles)
@fragment
fn trace(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    // TODO: Loop over bodies, raycast, find closest intersection (if any)
    // TODO: Shade pixel, for now inverse-depth
    return vec4(0.0, 0.0, 0.0, 1.0);
}
