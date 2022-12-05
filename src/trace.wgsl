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

struct Camera {
    position: vec3<f32>,
    angle_elevation: vec2<f32>,
};

@group(0) @binding(0) var<storage, read> masses: array<f32>;
@group(1) @binding(0) var<storage, read_write> positions: array<vec3<f32>>;
@group(2) @binding(0) var<uniform> camera: Camera;
@group(3) @binding(0) var<storage, read> densities: array<f32>;
//@group(4) @binding(0) var<storage, read> emitters: array<u32>;

// formula for ray-sphere intersect from: https://facultyweb.cs.wwu.edu/~wehrwes/courses/csci480_21w/lectures/L07/L07_notes.pdf
//     formula for sphere: (position_on_surface-center_of_sphere)**2 = radius ** 2
//     formula for ray: position_on_surface = ray_o + ray_d * t
//     revised formula for sphere: dot(position_on_surface-center_of_sphere,position_on_surface-center_of_sphere) - radius**2 = 0
//     combined formula: dot((ray_o + ray_d * t)-center_of_sphere,(ray_o + ray_d * t)-center_of_sphere) - radius**2 = 0
//     FOILed formula: dot(ray_o,ray_o)+dot(ray_o,ray_d*t)-dot(ray_o,center_of_sphere)+dot(ray_d*t,ray_o)+dot(ray_d*t,ray_d*t)-dot(ray_d*t,center_of_sphere)-dot(center_of_sphere,ray_o)-dot(center_of_sphere,ray_d*t)+dot(center_of_sphere,center_of_sphere)-radius**2
//     At**2+Bt+C = dot(ray_d,ray_d)*t**2+t*dot(ray_o,ray_d)-t*dot(center_of_sphere,ray_d)+t*dot(ray_d,ray_o)-t*dot(ray_d,center_of_sphere)-dot(ray_o,center_of_sphere)-dot(center_of_sphere,ray_o)+dot(ray_o,ray_o)+dot(center_of_sphere,center_of_sphere)-radius**2
//     A = dot(ray_d,ray_d)
//     B = 2*dot(ray_o,ray_d)-2*dot(center_of_sphere,ray_d)
//     C = dot(ray_o,ray_o)+dot(center_of_sphere,center_of_sphere)-2*dot(ray_o,center_of_sphere)-radius**2
//     t = (-B+- sqrt(B**2-4*A*C))/(2*A)
//     t = (-(2*dot(ray_o,ray_d)-2*dot(center_of_sphere,ray_d))+- sqrt((2*dot(ray_o,ray_d)-2*dot(center_of_sphere,ray_d))**2-4*dot(ray_d,ray_d)*(dot(ray_o,ray_o)+dot(center_of_sphere,center_of_sphere)-2*dot(ray_o,center_of_sphere))))/(2*dot(ray_d,ray_d))


struct Intersection {
    intersect_point: vec3<f32>,
    distance: f32,
    normal: vec3<f32>,
    mass: f32,
    radius: f32,
    wi: vec3<f32>,
    wo: vec3<f32>,
}

let PI: f32 = 3.14159265358;

fn color_from_intersection(intersect: Intersection) -> vec4<f32> {
    //return vec4<f32>(intersect.normal,1.0);
    if intersect.distance >= 0.0 {
        let intensity: f32 = dot(intersect.normal, intersect.wo) / intersect.distance;
        //return vec4<f32>(intensity,intensity,intensity,1.0);
        let red: f32 = 1.0;
        let green: f32 = intersect.radius - trunc(intersect.radius);
        let blue: f32 = intersect.mass - trunc(intersect.mass);
        return vec4<f32>(red * intensity, green * intensity, blue * intensity, 1.0);
    } else {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
}



//JASMINE: feel free to modify this if you import an acceleration structure as a uniform; for now it just loops over the spheres
fn ray_trace(ray_o: vec3<f32>, ray_d: vec3<f32>) -> Intersection { //vec3<f32> is position of intersection, distance is
    //let PI: f32 = 3.14159265358;
    let wo: vec3<f32> = -normalize(ray_d);
    var intersection: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    var distance: f32 = -1.0;
    var normal: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    var mass: f32 = 0.0;
    var radius1: f32 = 0.0;
    var wi: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    for (var i: u32 = 0u; i < arrayLength(&positions); i++) {
        let center: vec3<f32> = positions[i];
        //TODO: once densities are incorporated, uncomment the below formula to get an accurate radius
        let radius: f32 = pow(3.0/4.0*(masses[i] / densities[i])/PI,1.0/3.0); //inverse formula for sphere volume //pow(masses[i], 1.0 / 3.0) / 4.0;//
        let A: f32 = dot(ray_d, ray_d);
        let B: f32 = 2.0 * dot(ray_o, ray_d) - 2.0 * dot(center, ray_d);
        let C: f32 = dot(ray_o, ray_o) + dot(center, center) - 2.0 * dot(ray_o, center) - pow(radius, 2.0);
        let square_root_term: f32 = B * B - 4.0 * A * C;
        if square_root_term <= 0.0 {
            continue;
        }
        let tplus: f32 = (-B + sqrt(square_root_term)) / (2.0 * A);
        let tminus: f32 = (-B - sqrt(square_root_term)) / (2.0 * A);
        let t: f32 = tminus;
        //var t:f32;
        //if (tminus > 0.0) {
        //    t = tminus;
        //} else {
        //    t = tplus;
        //}
        if t < 0.0 {
            continue;
        }
        if distance < 0.0 || t < distance {
            intersection = ray_o + t * ray_d;
            distance = t;
            normal = normalize(intersection - center);
            mass = masses[i];
            radius1 = radius;
            //let quaternion_d: vec4<f32> = vec4<f32>(0.0,wo);
            //let quaternion_rot: vec4<f32> = vec4<f32>(cos(PI/2.0),normal.x*sin(PI/2.0),normal.y*sin(PI/2.0),normal.z*sin(PI/2.0));
            //let quaternion_conj: vec4<f32> = vec4<f32>(quaternion_rot.x,-quaternion_rot.yzw);
            let perpendicular_a: vec3<f32> = normalize(cross(normal, wo));
            let perpendicular_b: vec3<f32> = normalize(cross(normal, perpendicular_a));
            let perpendicular_wo: vec3<f32> = dot(perpendicular_b, wo) * perpendicular_b;
            let dot_wo: vec3<f32> = dot(wo, normal) * normal;
            wi = normalize(-perpendicular_wo + dot_wo);
            //wi = normalize(((quaternion_rot*quaternion_d)*quaternion_conj).yzw);
        }
    }
    return Intersection(intersection, distance, normal, mass, radius1, wi, wo);
}

fn sample_cosine_hemisphere(normal: vec3<f32>) -> vec3<f32> {
    return normal;
}

fn pdf_cosine_hemisphere(normal: vec3<f32>) -> f32 {
    return dot(normal,normal);
}

@fragment
fn trace(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let brute_force_diffuse: bool = true;
    let num_diffuse_samples: u32 = 0u; //when set to 0, all lighting of spheres is determined by specular reflection unless brute_force_diffuse is true
    //must be odd number greater than or equal to 1
    //let basic_diffuse: bool = false; //if true, will just implement diffuse by testing normal between a line to light-sphere and the point being colored
    //basic diffuse is much faster, but not physically accurate, as it doesn't account for occlusion
    
    let vertical_direction: f32 = sin(camera.angle_elevation.y);
    let horizontal_direction: vec2<f32> = vec2<f32>(sin(camera.angle_elevation.x),cos(camera.angle_elevation.x));
    let scaled_horz_direction: vec2<f32> = horizontal_direction*cos(camera.angle_elevation.y);
    let aspect_ratio: f32 = 0.75;
    
    //Y is vertical, Z is front-back
    let camera_direction: vec3<f32> = normalize(vec3<f32>(scaled_horz_direction.x, vertical_direction, scaled_horz_direction.y));
    let camera_plane_x: vec3<f32> = normalize(vec3<f32>(-horizontal_direction.y, 0.0, horizontal_direction.x));
    let camera_plane_y: vec3<f32> = normalize(vec3<f32>(-vertical_direction*horizontal_direction.x,cos(camera.angle_elevation.y),-vertical_direction*horizontal_direction.y));
    //return (dot(camera_direction,camera_plane_x)+dot(camera_direction,camera_plane_y)+dot(camera_plane_x,camera_plane_y))*vec4<f32>(1.0,1.0,1.0,1.0);
    //
    //normalize(cross(camera_plane_x,camera_direction));
    
    
    //Y is front-back, Z is vertical
    /*let camera_direction: vec3<f32> = normalize(vec3<f32>(scaled_horz_direction.x, scaled_horz_direction.y, vertical_direction));
    let camera_plane_x: vec3<f32> = normalize(vec3<f32>(horizontal_direction.y, horizontal_direction.x,0.0));
    let camera_plane_y: vec3<f32> = normalize(cross(camera_plane_x,camera_direction));*/
    
    //Y is left-right, Z is vertical
    /*let camera_direction: vec3<f32> = normalize(vec3<f32>(scaled_horz_direction.y, scaled_horz_direction.x, vertical_direction));
    let camera_plane_x: vec3<f32> = normalize(vec3<f32>(horizontal_direction.x, horizontal_direction.y,0.0));
    let camera_plane_y: vec3<f32> = normalize(cross(camera_plane_x,camera_direction));*/
    
    //Y is vertical, Z is left-right
    /*let camera_direction: vec3<f32> = normalize(vec3<f32>(scaled_horz_direction.y, vertical_direction, scaled_horz_direction.x));
    let camera_plane_x: vec3<f32> = normalize(vec3<f32>(horizontal_direction.x, horizontal_direction.y,0.0));
    let camera_plane_y: vec3<f32> = normalize(cross(camera_plane_x,camera_direction));*/
    
    
    let focal_point: vec3<f32> = camera.position - camera_direction*0.5;
    let relative_camera_pixel_position: vec2<f32> = vec2<f32>(uv.x*2.0 - 1.0,uv.y*aspect_ratio*2.0 - aspect_ratio);
    let camera_pixel_position: vec3<f32> = (relative_camera_pixel_position.x) * camera_plane_x + (relative_camera_pixel_position.y) * camera_plane_y + camera.position;
    let camera_ray_direction: vec3<f32> = normalize(camera_pixel_position-focal_point);

    let first_intersect: Intersection = ray_trace(camera_pixel_position, normalize(camera_ray_direction));
    let second_intersect: Intersection = ray_trace(first_intersect.intersect_point + normalize(first_intersect.wi) * 0.001, normalize(first_intersect.wi));
    var second_diffuse_color: vec4<f32> = vec4<f32>(0.0,0.0,0.0,1.0);
    if brute_force_diffuse {
        if first_intersect.distance >= 0.0 {
            for (var i: u32 = 0u; i < arrayLength(&positions); i++) {
            //for (var i2: u32 = 0u; i2 < arrayLength(&emitters); i2++) {
            //    let i: u32 = emitters[i2];
                let diffuse_direction: vec3<f32> = positions[i]-first_intersect.intersect_point;
                if dot(diffuse_direction,first_intersect.normal) < 0.0 {
                    continue;
                }
                let diffuse_normal_direction: vec3<f32> = normalize(diffuse_direction);
                let radius: f32 = pow(3.0/4.0*(masses[i] / densities[i])/PI,1.0/3.0); //inverse formula for sphere volume //pow(masses[i], 1.0 / 3.0) / 4.0;//
                let diffuse_intersect: Intersection = ray_trace(first_intersect.intersect_point+diffuse_normal_direction * 0.001,diffuse_normal_direction);
                if diffuse_intersect.distance < 0.0 {
                    continue;
                }
                if length(diffuse_intersect.intersect_point-positions[i]) > (radius+0.001) {
                    continue;
                }
                second_diffuse_color += color_from_intersection(diffuse_intersect);
            }
        }
    } else {
        if num_diffuse_samples != 0u {
            for (var i: u32 = 1u; i <= num_diffuse_samples; i++) {
                let u: f32 = f32(i)/f32(num_diffuse_samples+1u) - 0.5;
                for (var j: u32 = 1u; j <= num_diffuse_samples; j++) {
                    let v: f32 = f32(j)/f32(num_diffuse_samples+1u) - 0.5;
                    let diffuse_direction: vec3<f32> = vec3<f32>(sin(u)*cos(v),cos(u)*cos(v),cos(u)*sin(v));
                    let diffuse_normal_direction: vec3<f32> = normalize(diffuse_direction);
                    let diffuse_intersect: Intersection = ray_trace(first_intersect.intersect_point+diffuse_normal_direction * 0.001,diffuse_normal_direction);
                    if diffuse_intersect.distance < 0.0 {
                        continue;
                    }
                    second_diffuse_color += color_from_intersection(diffuse_intersect);
                }
            }
        }
    }
    //second_diffuse_color /= f32(num_diffuse_samples);
    second_diffuse_color.a = 1.0;
    let first_color: vec4<f32> = color_from_intersection(first_intersect);
    let second_color: vec4<f32> = color_from_intersection(second_intersect);
    if first_intersect.distance < 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    } else if second_intersect.distance < 0.0 {
        return first_color + first_color * second_diffuse_color;
    } else {
        return first_color + first_color * (second_color + second_diffuse_color);
    }
}
