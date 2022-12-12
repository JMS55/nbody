use crate::WORLD_SIZE;
use encase::ShaderType;
use glam::Vec3;

//global constant, or base it upon our N_BODIES, not sure, probably the latter
//but also, to account for floating point error stuff, maybe we just actually cap it?
const MAX_DEPTH: u32 = 16;
const NODETYPE_DUMMY: u32 = 0;
const NODETYPE_LEAFBODY: u32 = 1;
const NODETYPE_LEAFLIST: u32 = 2;
const NODETYPE_INTERIOR: u32 = 3;

#[derive(ShaderType)]
pub struct OctreeNode {
    center_of_mass: Vec3,
    pos_min: Vec3,
    pos_max: Vec3,
    range: f32,
    total_mass: f32,
    child_indices: [u32; 8],
    node_type: u32,
    //max_depth: u32,
}

impl OctreeNode {
    pub fn new_tree(positions: &[Vec3], masses: &[f32]) -> Vec<Self> {
        let root_node = Self::new_dummy();
        let root_extents = WORLD_SIZE / 2.0;
        let root_center = Vec3::splat(root_extents);
        //let mut max_depth = 0;

        let mut nodes = vec![root_node];
        let nodes_ptr = (&mut nodes) as *mut Vec<Self>;
		let mut leaf_list_children:Vec<Vec<Self>> = vec![];
        let leaf_list_children_ptr = (&mut leaf_list_children) as *mut Vec<Vec<Self>>;
		let mut leaf_lists:Vec<*mut Self> = vec![];
		let leaf_lists_ptr = (&mut leaf_lists) as *mut Vec<*mut Self>;

        for (position, mass) in positions.iter().zip(masses) {
            /*max_depth = max_depth.max(nodes[0].insert(
                *position,
                *mass,
                root_center,
                root_extents,
                nodes_ptr,
            ));*/
            nodes[0].insert(*position, *mass, root_center, root_extents, nodes_ptr, leaf_list_children_ptr, leaf_lists_ptr, 0);
        }
		
		//cleanup: add all leaf-body nodes, each stored in vectors inside leaf_list_children, as contiguous blocks into nodes
		for (i,leaf_children) in leaf_list_children.iter_mut().enumerate() {
			let leaf_list = unsafe {&mut *leaf_lists[i]};
			leaf_list.child_indices[0] = nodes.len() as u32;
			leaf_list.child_indices[1] = leaf_children.len() as u32;
			nodes.append(leaf_children);
		}	
		
        //nodes[0].max_depth = max_depth;
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
		leaf_list_children_ptr: *mut Vec<Vec<Self>>,
		leaf_lists_ptr: *mut Vec<*mut Self>,
        curr_depth: u32,
    ) /*-> u32*/
    {
        if self.node_type == NODETYPE_DUMMY {
            self.total_mass = mass;
            self.center_of_mass = position;
            self.pos_min = position;
            self.pos_max = position;
            self.range = (self.pos_max - self.pos_min).max_element();
            self.node_type = NODETYPE_LEAFBODY;
            //self.max_depth = 0;

            //return (WORLD_SIZE / self_extents).ceil().log2().ceil() as u32;
			return;
        }

        //let mut max_depth: u32 = 0;

        //if non-dummy, always need to compute new CoM, total mass, etc.
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

        if curr_depth == MAX_DEPTH {
            //if we are at MAX_DEPTH, prevent degeneracy, we must become a Leaf-List node
			let leaf_list_children = unsafe { &mut *leaf_list_children_ptr };
			let leaf_lists = unsafe { &mut *leaf_lists_ptr };
			
			//as a Leaf-List:
			//	our leaf list index (into leaf_list_children_ptr) is: self.child_indices[0];
			//	our leaf list length (vector len within leaf_list_children_ptr) is: self.child_indices[1];
			//	a reference to ourselves will be stored in leaf_lists(_ptr) at the matching index
			//		this is essentially a sloppier approach than making a struct holding a reference to us and the vec
			
            if self.node_type != NODETYPE_LEAFLIST { //if not yet a Leaf-List, must have been a Leaf-Body -- initialize Leaf-List
				self.node_type = NODETYPE_LEAFLIST;
				self.child_indices[0] = leaf_list_children.len() as u32;
				self.child_indices[1] = 0;
				leaf_list_children.push(Vec::new()); //TODO: Jasmine check this!
				leaf_lists.push(self as *mut Self);
			}
			//already set-up, simple insertion time
			//create new Leaf-Body node for new body
			let mut child = Self::new_dummy();
			child.insert(
				node_a_position,
				node_a_mass,
				self_center,
				self_extents,
				nodes_ptr,
				leaf_list_children_ptr,
				leaf_lists_ptr,
				0
			);
			//delay pushing it to nodes, instead pushing to leaf_list_children, and record size for GPU processing in [1]
			leaf_list_children[self.child_indices[0] as usize].push(child);
			self.child_indices[1] += 1;
        } else {
            //we are not at MAX_DEPTH, and we aren't a Dummy, so we should sift our child down
            let self_extents = self_extents / 2.0;
            let nodes = unsafe { &mut *nodes_ptr };

            let ci_b = node_index_for_child(self_center, node_b_position);
            self.ensure_has_child(ci_b, nodes);

            if self.node_type == NODETYPE_LEAFBODY {
                //if we are a Leaf-Body, we need to sift down our current body as well as the new one
                //we aren't at MAX_DEPTH, so we become an Interior
                self.node_type = NODETYPE_INTERIOR;
                let ci_a = node_index_for_child(self_center, node_a_position);
                self.ensure_has_child(ci_a, nodes);

                /*max_depth = */
                nodes[self.child_indices[ci_a] as usize].insert(
                    node_a_position,
                    node_a_mass,
                    self_center + (self_extents * extent_weights(ci_a)),
                    self_extents,
                    nodes_ptr,
					leaf_list_children_ptr,
					leaf_lists_ptr,
                    curr_depth + 1,
                );
            }

            /*max_depth = max_depth.max(nodes[self.child_indices[ci_b] as usize].insert(
                node_b_position,
                node_b_mass,
                self_center + (self_extents * extent_weights(ci_b)),
                self_extents,
                nodes_ptr,
            ));
            return max_depth;*/

            //now, whether we were Interior or Leaf-Body before, we are now Interior, and can sift our new child easily
            nodes[self.child_indices[ci_b] as usize].insert(
                node_b_position,
                node_b_mass,
                self_center + (self_extents * extent_weights(ci_b)),
                self_extents,
                nodes_ptr,
				leaf_list_children_ptr,
				leaf_lists_ptr,
                curr_depth + 1,
            );
        }
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
            node_type: NODETYPE_DUMMY,
            //max_depth: 0,
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
