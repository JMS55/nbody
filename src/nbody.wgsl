@group(0) @binding(0) var<storage, read> masses: array<f32>;
//TODO: figure out if these access methods can be specified better
@group(1) @binding(0) var<storage, read_write> positions_in: array<vec3<f32>>;
@group(1) @binding(1) var<storage, read_write> velocities_in: array<vec3<f32>>;
@group(2) @binding(0) var<storage, read_write> positions_out: array<vec3<f32>>;
@group(2) @binding(1) var<storage, read_write> velocities_out: array<vec3<f32>>;

@compute
@workgroup_size(64)
fn nbody_step(@builtin(global_invocation_id) global_invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
	//let G:f32 = .000000000066743;
	//let G:f32 = .0000000000000066743;
    let G:f32 = .066743; //for testing
	let TIME_STEP:f32 = 1.0;
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
	if (i_id >= n_bodies) { //one quick and dirty branch that will only fork in a single workgroup; shouldn't be too bad
		return;
	}
	
	//now, every invocation processes exactly one node -- the one at the index equal to its invocation id
	/* process: 
		* compute acceleration based on forces
		* update position using acceleration and old velocity
		* update velocity using acceleration
		* assume no collisions ever occur, whatever
	*/
	let time_step = TIME_STEP;
	let mass:f32 = masses[i_id];
	var pos:vec3<f32> = positions_in[i_id];
	var vel:vec3<f32> = velocities_in[i_id];
	
	//get acceleration
		//F = m*g = m * (GM / r^2) = m * G*M / distance(us,them)^2
		//then ignore the m, know g = G*M / distance(us,them)^2
		//then multiply that scalar acceleration by normalize(distance_vector(us,them)) to get accel vector
	
	var acc:vec3<f32> = vec3(0.0,0.0,0.0);
	var i:u32 = 0u;
	loop {
		if (i != i_id) {
			let other_mass:f32 = masses[i];
			let other_pos:vec3<f32> = positions_in[i];
			let dist_vec = pos - other_pos;
			let dist_sqrd = dot(dist_vec, dist_vec);
			var g = G*other_mass / dist_sqrd;
			//g = min(g, 5.0);
			//g = max(g, 1.0);
			let normed_dist_vec = normalize(dist_vec);
			acc += g*normed_dist_vec;
		}
        i += 1u;
        if (i == n_bodies) {
            break;
        }
	}
	
	//update final position using: initial position, initial velocity, and acceleration
		//pos_f = pos_i + vel_i*t + .5*a*t^2
	pos += velocities_in[i_id]*time_step + (0.5)*acc*pow(time_step,2.0);
	
	//update velocity using: initial velocity and acceleration
	vel += acc*time_step;
	
	//pos += G*vel;
	//vel = normalize(pos);
	//update in the outputs
	positions_out[i_id] = pos;
	velocities_out[i_id] = vel;
}
