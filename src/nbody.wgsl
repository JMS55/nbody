@group(0) @binding(0) var<storage, read> masses: array<f32>;
@group(1) @binding(0) var<storage, read> positions_in: array<vec3<f32>>;
@group(1) @binding(1) var<storage, read> velocities_in: array<vec3<f32>>;
@group(2) @binding(0) var<storage, write> positions_out: array<vec3<f32>>;
@group(2) @binding(1) var<storage, write> velocities_out: array<vec3<f32>>;

@compute
@workgroup_size(64)
fn nbody_step(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let wg_id = global_invocation_id.x; //only using x coord for now
	//for now, write as though 1-d decomposition
	//non-positionally, but instead across the flat body array by-index
}
