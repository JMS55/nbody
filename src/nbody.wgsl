@group(0) @binding(0) var<storage, read> masses: array<f32>;
//TODO: figure out if these access methods can be specified better
@group(1) @binding(0) var<storage, read_write> positions_in: array<vec3<f32>>;
@group(1) @binding(1) var<storage, read_write> velocities_in: array<vec3<f32>>;
@group(1) @binding(2) var<storage, read_write> accelerations_in: array<vec3<f32>>;
@group(2) @binding(0) var<storage, read_write> positions_out: array<vec3<f32>>;
@group(2) @binding(1) var<storage, read_write> velocities_out: array<vec3<f32>>;
@group(2) @binding(2) var<storage, read_write> accelerations_out: array<vec3<f32>>;

@compute
@workgroup_size(64)
fn nbody_step(@builtin(global_invocation_id) global_invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let G: f32 = .000000000066743; //can shift decimal as you see fit
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
    let mass: f32 = masses[i_id];
    var pos: vec3<f32> = positions_in[i_id];
    var vel: vec3<f32> = velocities_in[i_id];

	//get acceleration
		//F = m*g = m * (GM / r^2) = m * G*M / distance(us,them)^2
		//then ignore the m, kn-ow g = G*M / distance(us,them)^2
		//then multiply that scalar acceleration by normalize(distance_vector(us,them)) to get accel vector

    var acc: vec3<f32> = vec3(0.0, 0.0, 0.0);
    var i: u32 = 0u;
    loop {
		//if (i != i_id) { //inclusion of a non-zero softener prevents divby0
        let other_mass: f32 = masses[i];
        let other_pos: vec3<f32> = positions_in[i];
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

			//previous approach, including legacy hacks
				//let dist_sqrd = dot(dist_vec, dist_vec);
			//let dist_sqrd = pow(distance(other_pos,pos), 2.0);
				//let dist_sqrd = pow(distance(other_pos,pos), 1.1);
			//var g = G*other_mass / dist_sqrd;
			//var g = G*other_mass / (dist_sqrd + SOFTENING_SQRD);
			//a hack to make things weightier
			//g /= mass;

			//bias to account more for slowdowns than progressive speedups
				//this is important while we aren't doing dynamic timestep
					//because an imbalance of steps due to higher velocity on inbound than outbound of proximity
						//results in a slingshot effect not seen in real physics
				//acos(a dot b)/(magA * magB)
				//mag(a) = 2-norm(a) = distance(0vec, a)
			//start with the angle between the accelerator and the current velocity
			//var bias = acos(dot(vel,acc))/(distance(vec3(0.0),dist_vec)*distance(vec3(0.0),vel));
			//sqrt to rein in the extremes
			//bias = pow(bias, 0.6);
			//g *= bias;

			//a hack to make things less absurd
				//g = clamp(g, 0.01, 0.1);
			//g = min(g, 0.1);

			//let normed_dist_vec = normalize(dist_vec);
				//unclear behavior for normalize(0,0,0), so still an issue for zero vector here
			//acc += g*normed_dist_vec;
		//}
        i += 1u;
        if i == n_bodies {
			break;
        }
    }

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
