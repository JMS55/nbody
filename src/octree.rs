use crate::WORLD_SIZE;
use encase::ShaderType;
use glam::Vec3;

#[derive(ShaderType)]
pub struct OctreeNode {
    center_of_mass: Vec3,
    pos_min: Vec3,
    pos_max: Vec3,
    range: f32,
    total_mass: f32,
    child_indices: [u32; 8],
    is_leaf: u32,
    max_depth: u32,
}

impl OctreeNode {
    pub fn new_tree(positions: &[Vec3], masses: &[f32]) -> Vec<Self> {
        let root_node = Self::new_dummy();
        let root_extents = WORLD_SIZE / 2.0;
        let root_center = Vec3::splat(root_extents);
        let mut max_depth = 0;

        let mut nodes = vec![root_node];
        let nodes_ptr = (&mut nodes) as *mut Vec<Self>;

        for (position, mass) in positions.iter().zip(masses) {
            max_depth = max_depth.max(nodes[0].insert(
                *position,
                *mass,
                root_center,
                root_extents,
                nodes_ptr,
            ));
        }
        nodes[0].max_depth = max_depth;
        //dbg!(max_depth); //multiply by 8, that's the max traversal
        nodes
    }

    fn insert(
        &mut self,
        position: Vec3,
        mass: f32,
        self_center: Vec3,
        self_extents: f32,
        nodes_ptr: *mut Vec<Self>,
    ) -> u32 {
        if self.total_mass == 0.0 {
            self.total_mass = mass;
            self.center_of_mass = position;
            self.pos_min = position;
            self.pos_max = position;
            self.range = (self.pos_max - self.pos_min).max_element();
            self.is_leaf = 1;
            self.max_depth = 0;

            return (WORLD_SIZE / self_extents).ceil().log2().ceil() as u32;
        }

        let mut max_depth: u32 = 0;

        let node_a_position = self.center_of_mass;
        let node_a_mass = self.total_mass;
        let node_b_position = position;
        let node_b_mass = mass;

        self.total_mass = node_a_mass + node_b_mass;
        self.center_of_mass =
            ((node_a_position * node_a_mass) + (node_b_position * node_b_mass)) / self.total_mass;
        self.pos_min = self.pos_min.min(position);
        self.pos_max = self.pos_max.max(position);
        self.range = (self.pos_max - self.pos_min).max_element();

        let self_extents = self_extents / 2.0;
        let nodes = unsafe { &mut *nodes_ptr };

        let ci_b = node_index_for_child(self_center, node_b_position);
        self.ensure_has_child(ci_b, nodes);

        if self.is_leaf == 1 {
            self.is_leaf = 0;

            let ci_a = node_index_for_child(self_center, node_a_position);
            self.ensure_has_child(ci_a, nodes);

            max_depth = nodes[self.child_indices[ci_a] as usize].insert(
                node_a_position,
                node_a_mass,
                self_center + (self_extents * extent_weights(ci_a)),
                self_extents,
                nodes_ptr,
            );
        }

        max_depth = max_depth.max(nodes[self.child_indices[ci_b] as usize].insert(
            node_b_position,
            node_b_mass,
            self_center + (self_extents * extent_weights(ci_b)),
            self_extents,
            nodes_ptr,
        ));
        return max_depth;
    }

    fn ensure_has_child(&mut self, ci: usize, nodes: &mut Vec<Self>) {
        if self.child_indices[ci] == 0 {
            let i = nodes.len();
            nodes.push(Self::new_dummy());
            self.child_indices[ci] = i as u32;
        }
    }

    fn new_dummy() -> Self {
        Self {
            center_of_mass: Vec3::ZERO,
            pos_min: Vec3::ZERO,
            pos_max: Vec3::ZERO,
            range: 0.0,
            total_mass: 0.0,
            child_indices: [0; 8],
            is_leaf: 0,
            max_depth: 0,
        }
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

fn extent_weights(ci: usize) -> Vec3 {
    Vec3 {
        x: -1.0 + (2.0 * (((ci & 0b100) >> 2) as f32)),
        y: -1.0 + (2.0 * (((ci & 0b010) >> 1) as f32)),
        z: -1.0 + (2.0 * ((ci & 0b001) as f32)),
    }
}
