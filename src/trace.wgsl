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

//PLEASE UNCOMMENT THESE TWO LINES AND COMMENT THE LINES BELOW
//@group(2) @binding(0) var<storage, read> camera_angle_elevation: vec2<f32>;
//@group(3) @binding(0) var<storage, read> camera_position: vec3<f32>;

/*formula for ray-sphere intersect from: https://facultyweb.cs.wwu.edu/~wehrwes/courses/csci480_21w/lectures/L07/L07_notes.pdf
*    formula for sphere: (position_on_surface-center_of_sphere)**2 = radius ** 2
*    formula for ray: position_on_surface = ray_o + ray_d * t
*    revised formula for sphere: dot(position_on_surface-center_of_sphere,position_on_surface-center_of_sphere) - radius**2 = 0
*    combined formula: dot((ray_o + ray_d * t)-center_of_sphere,(ray_o + ray_d * t)-center_of_sphere) - radius**2 = 0
*    FOILed formula: dot(ray_o,ray_o)+dot(ray_o,ray_d*t)-dot(ray_o,center_of_sphere)+dot(ray_d*t,ray_o)+dot(ray_d*t,ray_d*t)-dot(ray_d*t,center_of_sphere)-dot(center_of_sphere,ray_o)-dot(center_of_sphere,ray_d*t)+dot(center_of_sphere,center_of_sphere)-radius**2
*    At**2+Bt+C = dot(ray_d,ray_d)*t**2+t*dot(ray_o,ray_d)-t*dot(center_of_sphere,ray_d)+t*dot(ray_d,ray_o)-t*dot(ray_d,center_of_sphere)-dot(ray_o,center_of_sphere)-dot(center_of_sphere,ray_o)+dot(ray_o,ray_o)+dot(center_of_sphere,center_of_sphere)-radius**2
*    A = dot(ray_d,ray_d)
*    B = 2*dot(ray_o,ray_d)-2*dot(center_of_sphere,ray_d)
*    C = dot(ray_o,ray_o)+dot(center_of_sphere,center_of_sphere)-2*dot(ray_o,center_of_sphere)-radius**2
*    t = (-B+- sqrt(B**2-4*A*C))/(2*A)
*    t = (-(2*dot(ray_o,ray_d)-2*dot(center_of_sphere,ray_d))+- sqrt((2*dot(ray_o,ray_d)-2*dot(center_of_sphere,ray_d))**2-4*dot(ray_d,ray_d)*(dot(ray_o,ray_o)+dot(center_of_sphere,center_of_sphere)-2*dot(ray_o,center_of_sphere))))/(2*dot(ray_d,ray_d))
*/

struct Intersection {
    intersect_point: vec3<f32>,
    distance: f32,
    normal: vec3<f32>,
    mass: f32,
}



//JASMINE: feel free to modify this if you import an acceleration structure as a uniform; for now it just loops over the spheres
fn ray_trace(ray_o: vec3<f32>, ray_d: vec3<f32>) -> Intersection { //vec3<f32> is position of intersection, distance is 
    var intersection: vec3<f32> = vec3<f32>(0.0,0.0,0.0);
    var distance: f32 = -1.0;
    var normal: vec3<f32> = vec3<f32>(0.0,0.0,0.0);
    var mass: f32 = 0.0;
    for (var i: u32 = 0u; i < arrayLength(&positions); i++) {
        let center: vec3<f32> = positions[i];
        //TODO: once densities are incorporated, uncomment the below formula to get an accurate radius
        let radius: f32 = pow(masses[i],1.0/3.0)/4.0;//pow(3.0/4.0*(masses[i] / densities[i])/exp(1.0),1.0/3.0); //inverse formula for sphere volume
        let d: vec3<f32> = ray_d;
        let p: vec3<f32> = ray_o;
        let A: f32 = dot(ray_d,ray_d);
        let B: f32 = 2.0 * dot(ray_o,ray_d) - 2.0 * dot(center,ray_d);
        let C: f32 = dot(ray_o,ray_o) + dot(center,center) - 2.0 * dot(ray_o,center) - pow(radius,2.0);
        let square_root_term: f32 = B * B - 4.0 * A * C;
        if (square_root_term <= 0.0) {
            continue;
        }
        let tplus: f32 = (-B + square_root_term) / (2.0 * A);
        let tminus: f32 = (-B - square_root_term) / (2.0 * A);
        let t: f32 = tminus;
        /*var t:f32;
        if (tminus > 0.0) {
            t = tminus;
        } else {
            t = tplus;
        }*/
        if (t<0.0) {
            continue;
        }
        if (distance==-1.0 || t < distance) {
            intersection = ray_o + t * ray_d;
            distance = t;
            normal = intersection-center;
            mass = masses[i];
        }
    }
    return Intersection(intersection,distance,normal,mass);
}

@fragment
fn trace(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {

    //PLEASE COMMENT OUT THESE TWO LINES AND UNCOMMENT THE DECLARATIONS ABOVE
    let camera_angle_elevation: vec2<f32> = vec2<f32>(0.0,0.0);
    let camera_position: vec3<f32> = vec3<f32>(0.0,0.0,-10.0);
    
    
    let camera_direction: vec3<f32> = vec3<f32>(cos(camera_angle_elevation.y)*sin(camera_angle_elevation.x),sin(camera_angle_elevation.y),cos(camera_angle_elevation.y)*cos(camera_angle_elevation.x));
    let camera_plane_x: vec3<f32> = vec3<f32>(cos(camera_angle_elevation.x),0.0,sin(camera_angle_elevation.x)); //points in the direction in world space that the x-axis of the camera lens is in
    let camera_plane_y: vec3<f32> = vec3<f32>(-sin(camera_angle_elevation.x)*sin(camera_angle_elevation.y),cos(camera_angle_elevation.y),cos(camera_angle_elevation.x)*sin(camera_angle_elevation.y)); //points in the direction in world space that the y-axis of the camera lens is in
    let window_size: vec2<f32> = vec2<f32>(20.0, 15.0);
    let relative_camera_pixel_position: vec2<f32> = vec2<f32>(window_size.x*uv.x, window_size.y*uv.y);
    let camera_pixel_position: vec3<f32> = relative_camera_pixel_position.x*camera_plane_x + relative_camera_pixel_position.y*camera_plane_y + 
camera_position;
    // TODO: Loop over bodies, raycast, find closest intersection (if any)
    // TODO: Shade pixel, for now inverse-depth
    var index_of_nearest_intersection: i32 = -1;
    var distance_of_nearest_intersection: f32 = 100000000000.0;
    var intensity: f32 = 0.0;
    let intersect: Intersection = ray_trace(camera_pixel_position,camera_direction);
    if (intersect.distance<0.0) {
        return vec4(0.0,0.0,0.0,1.0);
    } else {
        let new_intensity: f32 = pow(-dot(intersect.normal,camera_direction),2.0);
        return vec4(new_intensity,new_intensity*intersect.mass/10.0,new_intensity,1.0);
    }
    /*for (var i: u32 = 0u; i < arrayLength(&positions); i++) {
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
    }*/
}
