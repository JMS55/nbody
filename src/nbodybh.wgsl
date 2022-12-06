struct OctreeNode {
	center_of_mass: vec3<f32>,
    pos_min: vec3<f32>,
    pos_max: vec3<f32>,
    range: f32,
	total_mass: f32,
	child_indices: array<u32, 8>,
	is_leaf: u32,
    max_depth: u32,
};

@group(0) @binding(0) var<storage, read> masses: array<f32>;
//TODO: figure out if these access methods can be specified better
@group(1) @binding(0) var<storage, read_write> positions_in: array<vec3<f32>>;
@group(1) @binding(1) var<storage, read_write> velocities_in: array<vec3<f32>>;
@group(1) @binding(2) var<storage, read_write> accelerations_in: array<vec3<f32>>;
@group(2) @binding(0) var<storage, read_write> positions_out: array<vec3<f32>>;
@group(2) @binding(1) var<storage, read_write> velocities_out: array<vec3<f32>>;
@group(2) @binding(2) var<storage, read_write> accelerations_out: array<vec3<f32>>;
@group(3) @binding(0) var<storage, read> octree: array<OctreeNode>;

var theta:f32 = 0.5;
const max_max_depth = 20;
override max_depth = max_max_depth; //i think overrides are somehow a way to pass things in?

var<private> acc: vec3<f32>;
var<private> stack: arr<u32, max_depth>; //the docs say you aren't allowed to do this (use size from an override) for private variables (says only for wg-shared), but it seems to compile...

// array<i32,8>, array<i32,8u>, and array<i32,width> are the same type.
// Their element counts evaluate to 8.
var<private> d: array<i32,width>;

fn acc_div(pos1: vec3<f32>, pos2: vec3<f32>) -> f32 {
    let SOFTENING_SQRD: f32 = 1.0;

    var divisor: f32 = pow(distance(pos2, pos1), 2.0);
    divisor += SOFTENING_SQRD;
    divisor = pow(divisor, 1.5);
    return divisor;
}

fn direct_acc_comp(i: u32, n: OctreeNode) -> vec3<f32> {
    let G: f32 = 0.00066743; //can shift decimal as you see fit

    let pos1 = positions_in[i];
    let pos2 = n.center_of_mass;
    let dist_vec = pos2 - pos1;
    var divisor = acc_div(pos1, pos2);
    var g = G * other_mass / divisor; // TODO: Where is other_mass coming from?
    var bias = acos(
        dot(vel, dist_vec) / (distance(vec3(0.0), dist_vec) * distance(vec3(0.0), vel) + 1.0)
    );
    bias = pow(bias, .1);
    g *= bias;
    return g * dist_vec;
}

fn tree_force(i: u32, n: OctreeNode) -> void {
    if n.is_leaf == 1 {
        acc += direct_acc_comp(i, n);
        return;
    } else {
        let d = get_range(n);
        let r = distance(positions_in[i], n.center_of_mass);
        if (d / r) < theta {
            acc += direct_acc_comp(i, n);
            return;
        } else {
            var i: u32 = 1u;
            loop{
                if n.child_indices[i] != 0 {
                    tree_force(i, octree[n.child_indices]);
                }
                i += 1u;
                if i > 8 {
                    break;
                }
            }
        }
    }
}

@compute
@workgroup_size(64)
fn nbody_step(@builtin(global_invocation_id) global_invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let G: f32 = 0.00066743; //can shift decimal as you see fit
    let TIME_STEP: f32 = 0.1;
    let SOFTENING_SQRD: f32 = 1.0;
    let i_id = global_invocation_id.x; //only using x coord for now
	//let i_id = local_invocation_id.x; //only using x coord for now
	//let i_id = 1u; //only using x coord for now
    let nwg = num_workgroups.x; //only using x coord for now
    let n_bodies = arrayLength(&masses); //hopefully that works alright

	//for basic first iteration, since we can ask for an obscene number of workgroups, just do that
		//so every invocation will process {0,1} bodies
		//and we have (n_bodies + ((64-(n_bodies%64))%64)) invocations
		//so n_bodies invocations are useful and [0,64), all in the same single workgroup, are wasted

	//kill off excess invocations
	//positions_out[i_id%arrayLength(&positions_out)] = vec3(f32(i_id)); //for debugging
    if i_id >= n_bodies { //one quick and dirty branch that will only fork in a single workgroup; shouldn't be too bad
        return;
	}

	//now, every invocation processes exactly one node -- the one at the index equal to its invocation id
	// process:
	// 	* compute acceleration based on forces
	// 	* update position using acceleration and old velocity
	// 	* update velocity using acceleration
	// 	* assume no collisions ever occur, whatever

    let time_step = TIME_STEP;

	//get acceleration
		//F = m*g = m * (GM / r^2) = m * G*M / distance(us,them)^2
		//then ignore the m, kn-ow g = G*M / distance(us,them)^2
		//then multiply that scalar acceleration by normalize(distance_vector(us,them)) to get accel vector

    acc = vec3(0.0, 0.0, 0.0);

    tree_force(i_id, octree[0]);

	//update final position using: initial position, initial velocity, and acceleration

	//euclidean integration: pos_f = pos_i + vel_i*t + .5*a*t^2
	//pos += velocities_in[i_id]*time_step + (0.5)*acc*pow(time_step,2.0);
	//a little hack to make things less floaty: decay (kinda like a friction)
	//vel *= 0.9996;
	//vel += acc*time_step;
	//another problem: this was using a_i+1 instead of a_i to determine p_i+1

	//Leapfrog-Verlet Integration: second-order, so hopefully stabilizes oscillation
		//acc_i+1 = A(pos_i), which we've just done
		//pos_i+1 = pos_i + vel_i*t + .5*a_i*t^2
		//vel_i+1 = v_i + 1/2(a_i + a_i+1)*t
			//essentially a trapezoidal approximation rather than RRAM
    pos += vel * time_step + 0.5 * accelerations_in[i_id] * pow(time_step, 2.0);
    vel += 0.5 * (accelerations_in[i_id] + acc) * time_step;

	//update in the outputs
    positions_out[i_id] = pos;
    velocities_out[i_id] = vel;
    accelerations_out[i_id] = acc;
}
