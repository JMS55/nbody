use crate::WORLD_SIZE;
use encase::ShaderType;
use glam::Vec3;

#[derive(ShaderType)]
pub struct OctreeNode {
    center_of_mass: Vec3,
    pos_min: Vec3,
    pos_max: Vec3,
    total_mass: f32,
    child_indices: [u32; 8],
    is_leaf: u32,
}

impl OctreeNode {
    pub fn new_tree(positions: &[Vec3], masses: &[f32]) -> Vec<Self> {
        let root_node = Self {
            center_of_mass: Vec3::ZERO,
            pos_min: Vec3::ZERO,
            pos_max: Vec3::ZERO,
            total_mass: 0.0,
            child_indices: [0; 8],
            is_leaf: 0,
        };
        let root_extents = WORLD_SIZE / 2.0;
        let root_center = Vec3::splat(root_extents);

        let mut nodes = vec![root_node];
        let nodes_ptr = (&mut nodes) as *mut Vec<Self>;

        for (position, mass) in positions.iter().zip(masses) {
            nodes[0].insert(*position, *mass, root_center, root_extents, nodes_ptr);
        }

        nodes
    }
	
	fn range(&mut self) -> f32 {
	    return (self.pos_max - self.pos_min).reduce_partial_max(); // why doesn't it work? https://docs.rs/vek/0.14.1/vek/vec/repr_c/vec3/struct.Vec3.html#method.partial_max
	}

    fn insert(
        &mut self,
        position: Vec3,
        mass: f32,
        self_center: Vec3,
        self_extents: f32,
        nodes_ptr: *mut Vec<Self>,
    ) {
        if self.total_mass == 0.0 {
            self.total_mass = mass;
            self.center_of_mass = position;
            self.pos_min = position;
            self.pos_max = position;
            self.is_leaf = 1;

            return;
        }

        let node_a_position = self.center_of_mass;
        let node_a_mass = self.total_mass;
        let node_b_position = position;
        let node_b_mass = mass;

        self.total_mass = node_a_mass + node_b_mass;
        self.center_of_mass =
            ((node_a_position * node_a_mass) + (node_b_position * node_b_mass)) / self.total_mass;
		self.pos_min = Vec3::min(self.pos_min, position);
		self.pos_max = Vec3::max(self.pos_max, position);

        let self_extents = self_extents / 2.0;
        let nodes = unsafe { &mut *nodes_ptr };

        let ci_b = node_index_for_child(self_center, node_b_position);
        if self.child_indices[ci_b] == 0 {
            let i = nodes.len();
            nodes.push(Self {
                center_of_mass: Vec3::ZERO,
                pos_min: Vec3::ZERO,
                pos_max: Vec3::ZERO,
                total_mass: 0.0,
                child_indices: [0; 8],
                is_leaf: 0,
            });
            self.child_indices[ci_b] = i as u32;
        }

        if self.is_leaf == 1 {
            self.is_leaf = 0;

            let ci_a = node_index_for_child(self_center, node_a_position);
            if self.child_indices[ci_a] == 0 {
                let i = nodes.len();
                nodes.push(Self {
                    center_of_mass: Vec3::ZERO,
                    pos_min: Vec3::ZERO,
                    pos_max: Vec3::ZERO,
                    total_mass: 0.0,
                    child_indices: [0; 8],
                    is_leaf: 0,
                });
                self.child_indices[ci_a] = i as u32;
            }

            let xw = -1.0 + (2.0 * (((ci_a & 0b100) >> 2) as f32));
            let yw = -1.0 + (2.0 * (((ci_a & 0b010) >> 1) as f32));
            let zw = -1.0 + (2.0 * ((ci_a & 0b001) as f32));
            nodes[self.child_indices[ci_a] as usize].insert(
                node_a_position,
                node_a_mass,
                Vec3::new(
                    self_center.x + (self_extents * xw),
                    self_center.y + (self_extents * yw),
                    self_center.z + (self_extents * zw),
                ),
                self_extents,
                nodes_ptr,
            );
        }

        let xw = -1.0 + (2.0 * (((ci_b & 0b100) >> 2) as f32));
        let yw = -1.0 + (2.0 * (((ci_b & 0b010) >> 1) as f32));
        let zw = -1.0 + (2.0 * ((ci_b & 0b001) as f32));
        nodes[self.child_indices[ci_b] as usize].insert(
            node_b_position,
            node_b_mass,
            Vec3::new(
                self_center.x + (self_extents * xw),
                self_center.y + (self_extents * yw),
                self_center.z + (self_extents * zw),
            ),
            self_extents,
            nodes_ptr,
        );
    }
}

fn node_index_for_child(node_center: Vec3, child_position: Vec3) -> usize {
    let mut i = 0b000;
    if child_position.x >= node_center.x {
        i |= 0b100;
    }
    if child_position.y >= node_center.y {
        i |= 0b010;
    }
    if child_position.z >= node_center.z {
        i |= 0b001;
    }
    i
}
