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
//@group(2) @binding(0) var<storage, read> densities: array<f32>;
@fragment
fn trace(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let camera_angle_elevation: vec2<f32> = vec2<f32>(0.0,0.0); 
    //let camera_init_direction: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0); //must be a normal vector
    let camera_direction: vec3<f32> = vec3<f32>(cos(camera_angle_elevation.y)*sin(camera_angle_elevation.x),sin(camera_angle_elevation.y),cos(camera_angle_elevation.y)*cos(camera_angle_elevation.x));
    
    let camera_plane_x: vec3<f32> = vec3<f32>(cos(camera_angle_elevation.x),0.0,sin(camera_angle_elevation.x)); //points in the direction in world space that the x-axis of the camera lens is in
    let camera_plane_y: vec3<f32> = vec3<f32>(-sin(camera_angle_elevation.x)*sin(camera_angle_elevation.y),cos(camera_angle_elevation.y),cos(camera_angle_elevation.x)*sin(camera_angle_elevation.y)); //points in the direction in world space that the y-axis of the camera lens is in
    let camera_position: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    let window_size: vec2<f32> = vec2<f32>(20.0, 15.0);
    let relative_camera_pixel_position: vec2<f32> = vec2<f32>(window_size.x*uv.x, window_size.y*uv.y);
    let camera_pixel_position: vec3<f32> = relative_camera_pixel_position.x*camera_plane_x + relative_camera_pixel_position.y*camera_plane_y + 
camera_position;
    // TODO: Loop over bodies, raycast, find closest intersection (if any)
    // TODO: Shade pixel, for now inverse-depth
    var index_of_nearest_intersection: i32 = -1;
    var distance_of_nearest_intersection: f32 = 100000000000.0;
    var intensity: f32 = 0.0;
    for (var i: u32 = 0u; i < arrayLength(&positions); i++) {
        let body_position: vec3<f32> = positions[i];
        let body_depth: f32 = length(body_position*camera_direction);
        let body_position_2d: vec3<f32> = body_position*camera_plane_x+body_position*camera_plane_y;
        let body_density: f32 = 1.0; //densities[i];
	//let body_radius: f32 = masses[i]/body_density; //yep, once implemented, this'll be good
	let body_radius: f32 = (0.1f*log2((2.0f+masses[i]))); //for now, do this to control density a bit
        let offset_2d: vec3<f32> = camera_pixel_position-body_position_2d;
        let intersection_normal : vec3<f32> = offset_2d/body_radius+camera_direction*sin(acos(length(offset_2d/body_radius)));
        let intersection : vec3<f32> = intersection_normal*body_radius;
        let intersection_depth : f32 = length(intersection*camera_direction);
        if (length(offset_2d)<body_radius && (body_depth-intersection_depth) < distance_of_nearest_intersection) {
          index_of_nearest_intersection = i32(i);
          distance_of_nearest_intersection = body_depth-intersection_depth;
          intensity = dot(intersection_normal,camera_direction);
        }
    }
    if index_of_nearest_intersection == -1 {
        return vec4(0.0, 0.0, 0.0, 1.0);
    } else {
        let new_intensity: f32 = pow(intensity,1.0);
        return vec4(new_intensity, new_intensity, new_intensity, 1.0);
    }
}
