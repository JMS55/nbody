use std::borrow::Cow;
use wgpu::*;
use wgpu::util::*; //include some stuff outside of spec
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};
use std::mem;

const N_BODIES:usize = 4;
const N_WORKGROUPS:u32 = 1;

const MASS_GROUP:u32 = 0;
const POSVEL_IN_GROUP:u32 = 1;
const POSVEL_OUT_GROUP:u32 = 2;
const MASS_BINDING:u32 = 0;
const POS_BINDING:u32 = 0; //bindings, not the bind groups
const VEL_BINDING:u32 = 1; //bindings, not the bind groups

const BASE_MASSES:&'static [f32] = &[0.1,1.0,2.0,3.0];
const BASE_POSITIONS:&'static [[f32;3]] = &[[1.0,2.0,3.0],[4.0,5.0,6.0],[7.0,8.0,9.0],[10.0,11.0,12.0]];

async fn run(event_loop: EventLoop<()>, window: Window) {
    // Setup gpu
    let instance = Instance::new(Backends::all());
    let surface = unsafe { instance.create_surface(&window) };
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&DeviceDescriptor::default(), None)
        .await
        .unwrap();
	//TODO: read input data / gen random
	let masses = BASE_MASSES;
	let positions = BASE_POSITIONS;
	
    // TODO: Create constant gpu resources
	/*setup:
		* buffers for I/O that the shader will r/w
			* if using BufferInitDescriptor, requires: initial state already allocated in CPU mem
		* bind ground: collection of resource (buffers, etc.)
		* pipeline: wrapper around shader
	*/
	//buffers: ronly: mass, rw: position, vel
	
	let mass_buffer = device.create_buffer_init(&BufferInitDescriptor{
		label: Some("mass_buffer"),
		contents: bytemuck::cast_slice(masses),
		usage: BufferUsages::STORAGE
		//inherently mapped at creation, as it creates and copies the data in one fn call
	});
	let pos_buffer_a = device.create_buffer_init(&BufferInitDescriptor{
		label: Some("pos_buffer_a"),
		contents: bytemuck::cast_slice(positions),
		usage: BufferUsages::STORAGE
		//inherently mapped at creation, as it creates and copies the data in one fn call
	});
	let pos_buffer_b = device.create_buffer(&BufferDescriptor{
		label: Some("pos_buffer_b"),
		size: (mem::size_of::<[f32;3]>()*N_BODIES) as u64,
		usage: BufferUsages::STORAGE,
		mapped_at_creation: false
	});
	let vel_buffer_a = device.create_buffer_init(&BufferInitDescriptor{
		label: Some("vel_buffer_a"),
		contents: bytemuck::cast_slice(&[0.0;N_BODIES*3]), //init to 0, also *3 cast_slice flat
		usage: BufferUsages::STORAGE
		//inherently mapped at creation, as it creates and copies the data in one fn call
	});
	let vel_buffer_b = device.create_buffer(&BufferDescriptor{
		label: Some("vel_buffer_b"),
		size: (mem::size_of::<[f32;3]>()*N_BODIES) as u64,
		usage: BufferUsages::STORAGE,
		mapped_at_creation: false
	});
	//bind group layouts: mass:, pos&vel(2*(
	let mass_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor{
		label: Some("mass_bind_group_layout"),
		entries: &[BindGroupLayoutEntry{
			binding: MASS_BINDING,
			visibility: ShaderStages::COMPUTE,
			ty: BindingType::Buffer{
				ty: BufferBindingType::Storage{read_only: true},
				has_dynamic_offset: false,
				min_binding_size: None //TODO: optimize
			},
			count: None //other values only for Texture, not Storage
		}]
	});
	let posvel_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor{
		label: Some("posvel_bind_group_layout"),
		entries: &[BindGroupLayoutEntry{ //position's entry
			binding: POS_BINDING,
			visibility: ShaderStages::COMPUTE,
			ty: BindingType::Buffer{
				ty: BufferBindingType::Storage{read_only: false},
				has_dynamic_offset: false,
				min_binding_size: None //TODO: optimize
			},
			count: None //other values only for Texture, not Storage
		}, BindGroupLayoutEntry{ //velocity's entry
			binding: VEL_BINDING,
			visibility: ShaderStages::COMPUTE,
			ty: BindingType::Buffer{
				ty: BufferBindingType::Storage{read_only: false},
				has_dynamic_offset: false,
				min_binding_size: None //TODO: optimize
			},
			count: None //other values only for Texture, not Storage
		}]
	});
	
	//bind groups: mass, pos_both, vel_both
	let mass_bind_group = device.create_bind_group(&BindGroupDescriptor{
		label: Some("mass_bind_group"),
		layout: &mass_bind_group_layout,
		entries: &[BindGroupEntry{
			binding: MASS_BINDING,
			resource: BindingResource::Buffer(BufferBinding{
				buffer: &mass_buffer,
				offset: 0,
				size: None
			})
		}]
	});
	let mut posvel_bind_group_a = device.create_bind_group(&BindGroupDescriptor{
		label: Some("posvel_bind_group_a"),
		layout: &posvel_bind_group_layout,
		entries: &[BindGroupEntry{
			binding: POS_BINDING,
			resource: BindingResource::Buffer(BufferBinding{
				buffer: &pos_buffer_a,
				offset: 0,
				size: None
			})
		}, BindGroupEntry{
			binding: VEL_BINDING,
			resource: BindingResource::Buffer(BufferBinding{
				buffer: &vel_buffer_a,
				offset: 0,
				size: None
			})
		}]
	});
	let mut posvel_bind_group_b = device.create_bind_group(&BindGroupDescriptor{
		label: Some("posvel_bind_group_b"),
		layout: &posvel_bind_group_layout,
		entries: &[BindGroupEntry{
			binding: POS_BINDING,
			resource: BindingResource::Buffer(BufferBinding{
				buffer: &pos_buffer_b,
				offset: 0,
				size: None
			})
		}, BindGroupEntry{
			binding: VEL_BINDING,
			resource: BindingResource::Buffer(BufferBinding{
				buffer: &vel_buffer_b,
				offset: 0,
				size: None
			})
		}]
	});
	//this shader must be compiled to a pipeline, and the handle thereof retrieved
    let nbody_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("nbody.wgsl"))),
    });
	//pipeline layout, then pipeline the compute shader
	let nbody_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor{
		label: Some("nbody_pipeline_layout"),
		bind_group_layouts: &[&mass_bind_group_layout,&posvel_bind_group_layout,&posvel_bind_group_layout],
		push_constant_ranges: &[] //empty is fine
	});
	//pipeline itself
	let nbody_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor{
		label: Some("nbody_pipeline"),
		layout: Some(&nbody_pipeline_layout),
		module: &nbody_shader,
		entry_point: "nbody_step" //name of fn within shader that gets called
	});
	
    event_loop.run(move |event, _, control_flow| {
        //let _ = (&instance, &adapter, &nbody_shader);

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_size),
                ..
            } => {
                // TODO: Handle resize
            }
            Event::MainEventsCleared => {
                // TODO: Update simulation
				//create a command encoder for each step call, which tells the pipeline to do Some ops on Some data
				let mut nbody_step_cmd_encoder = device.create_command_encoder(&CommandEncoderDescriptor{label: Some("nbody_step_cmd_encoder")});
				{ //code block because we want lifetime of compute pass to end after
					let mut nbody_step_pass = nbody_step_cmd_encoder.begin_compute_pass(&ComputePassDescriptor{label: Some("nbody_step_pass")});
					//now that we've initialized: set_pipeline, set_bind_group, dispatch_workgroups
					nbody_step_pass.set_pipeline(&nbody_pipeline);
					nbody_step_pass.set_bind_group(MASS_GROUP,&mass_bind_group,&[]);
					nbody_step_pass.set_bind_group(POSVEL_IN_GROUP,&posvel_bind_group_a,&[]);
					nbody_step_pass.set_bind_group(POSVEL_OUT_GROUP,&posvel_bind_group_b,&[]);
					//dispatch!
						//can use dispatch_workgroups_indirect to use GPU to compute clustering
						//then the GPU can determine n_workgroups from an in-GPU buffer without copy-paste
					nbody_step_pass.dispatch_workgroups(N_WORKGROUPS,1,1);
				}
				//submit command encoder to queue
				let _ = queue.submit(Some(nbody_step_cmd_encoder.finish()));
				
				//swap groups a and b to alternate I/O
				mem::swap(&mut posvel_bind_group_a, &mut posvel_bind_group_b);
            }
            Event::RedrawRequested(_) => {
                // TODO: Render
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

fn main() {
    let event_loop = EventLoop::new();
    let window = Window::new(&event_loop).unwrap();
    pollster::block_on(run(event_loop, window));
}
