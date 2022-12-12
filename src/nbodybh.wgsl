struct OctreeNode {
	center_of_mass: vec3<f32>,
    pos_min: vec3<f32>,
    pos_max: vec3<f32>,
    range: f32,
	total_mass: f32,
	child_indices: array<u32, 8>,
	node_type: u32,
};
let MAX_DEPTH: u32 = 16u;
let NODETYPE_DUMMY: u32 = 0u;
let NODETYPE_LEAFBODY: u32 = 1u;
let NODETYPE_LEAFLIST: u32 = 2u;
let NODETYPE_INTERIOR: u32 = 3u;

let WORLD_SIZE:f32 = 250.0;

@group(0) @binding(0) var<storage, read> masses: array<f32>;
//TODO: figure out if these access methods can be specified better
@group(1) @binding(0) var<storage, read_write> positions_in: array<vec3<f32>>;
@group(1) @binding(1) var<storage, read_write> velocities_in: array<vec3<f32>>;
@group(1) @binding(2) var<storage, read_write> accelerations_in: array<vec3<f32>>;
@group(2) @binding(0) var<storage, read_write> positions_out: array<vec3<f32>>;
@group(2) @binding(1) var<storage, read_write> velocities_out: array<vec3<f32>>;
@group(2) @binding(2) var<storage, read_write> accelerations_out: array<vec3<f32>>;
@group(3) @binding(0) var<storage, read> octree: array<OctreeNode>;

@compute
@workgroup_size(64)
fn nbody_step(@builtin(global_invocation_id) global_invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let G: f32 = 0.0066743; //can shift decimal as you see fit
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
    var pos: vec3<f32> = positions_in[i_id];
    var vel: vec3<f32> = velocities_in[i_id];
	var acc: vec3<f32> = vec3(0.0, 0.0, 0.0);
	//now, every invocation processes exactly one node -- the one at the index equal to its invocation id
	// process:
	// 	* compute acceleration based on forces
	// 	* update position using acceleration and old velocity
	// 	* update velocity using acceleration
	// 	* assume no collisions ever occur, whatever

    let time_step = TIME_STEP;
	let theta = 0.5;

	var stack:array<u32, 800>; //NEEDS to be variably sized
	//size needed for stack: MAX(n, MAX_DEPTH*BRANCHING_FACTOR); this MAX can be computed on the CPU, just *need* to pass it in here
	var top:i32 = 0;
	stack[top] = 0u;
    top = top + 1;

	loop {
		if (top<=0) {
			break;
		}

		top = top - 1;
		var node:OctreeNode = octree[stack[top]];
		if (node.node_type == NODETYPE_LEAFBODY) {
			let other_mass: f32 = node.total_mass;
			let other_pos: vec3<f32> = node.center_of_mass;
			let dist_vec = other_pos - pos;

				//divisor = distance^2 + softening^2
			var divisor: f32 = pow(distance(other_pos, pos), 2.0);
			divisor += SOFTENING_SQRD;
				//take to 3/2 power for a third power of norm of distance, to normalize dist_vec
			divisor = pow(divisor, 1.5);

				//acc += G*other_mass*dist_vec/divisor;
			var g = G * other_mass / divisor;

				//bias to account more for slowdowns than progressive speedups
					//this is important while we aren't doing dynamic timestep
						//because an imbalance of steps due to higher velocity on inbound than outbound of proximity
							//results in a slingshot effect not seen in real physics
					//acos(a dot b)/(magA * magB)
					//mag(a) = 2-norm(a) = distance(0vec, a)
				//start with the angle between the accelerator and the current velocity
			var bias = acos( dot(vel, dist_vec) / (distance(vec3(0.0), dist_vec) * distance(vec3(0.0), vel) + 1.0) );
				//pow to rein in the extremes
			bias = pow(bias, .15);
			g *= bias;

			acc += g * dist_vec;			
		} else {
			let d = node.range;
			let r = distance(pos, node.center_of_mass);
			if ((d/r) < theta) {
				let other_mass: f32 = node.total_mass;
				let other_pos: vec3<f32> = node.center_of_mass;
				let dist_vec = other_pos - pos;

					//divisor = distance^2 + softening^2
				var divisor: f32 = pow(distance(other_pos, pos), 2.0);
				divisor += SOFTENING_SQRD;
					//take to 3/2 power for a third power of norm of distance, to normalize dist_vec
				divisor = pow(divisor, 1.5);

					//acc += G*other_mass*dist_vec/divisor;
				var g = G * other_mass / divisor;

					//bias to account more for slowdowns than progressive speedups
						//this is important while we aren't doing dynamic timestep
							//because an imbalance of steps due to higher velocity on inbound than outbound of proximity
								//results in a slingshot effect not seen in real physics
						//acos(a dot b)/(magA * magB)
						//mag(a) = 2-norm(a) = distance(0vec, a)
					//start with the angle between the accelerator and the current velocity
				var bias = acos(
					dot(vel, dist_vec) / (distance(vec3(0.0), dist_vec) * distance(vec3(0.0), vel) + 1.0)
				);
					//pow to rein in the extremes
				bias = pow(bias, .15);
				g *= bias;

				acc += g * dist_vec;				
			} else { //case: not approximable
				var i:u32 = 0u;
					if (node.node_type == NODETYPE_LEAFLIST) { //case: leaf-list, all-pairs with its list
						var j:u32 = node.child_indices[0];
						loop{
							stack[top] = j+i;
							top = top + 1;
							i += 1u;
							if (i >= node.child_indices[1]) {
								break;
						}
					}
				} else { //case: interior node, recur onto children
					loop{
						if (node.child_indices[i] != 0u) {
							stack[top] = node.child_indices[i];
							top = top + 1;
						}
						i += 1u;
						if (i >= 8u) {
							break;
						}
					}
				}
			}
		}
	}

    pos += vel * time_step + 0.5 * accelerations_in[i_id] * pow(time_step, 2.0);
    vel += 0.5 * (accelerations_in[i_id] + acc) * time_step;

	//update in the outputs
    positions_out[i_id] = clamp(pos, vec3<f32>(0.0), vec3<f32>(WORLD_SIZE));
    velocities_out[i_id] = vel;
    accelerations_out[i_id] = acc;
}