use std::borrow::Cow;
use std::mem;
use wgpu::util::*; //include some stuff outside of spec
use wgpu::*;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

const N_BODIES: usize = 4;
//const N_WORKGROUPS: u32 = 1;
const WG_SIZE: u32 = 64;

const MASS_GROUP: u32 = 0;
const POSVEL_IN_GROUP: u32 = 1;
const POSVEL_OUT_GROUP: u32 = 2;
const MASS_BINDING: u32 = 0;
const POS_BINDING: u32 = 0; //bindings, not the bind groups
const VEL_BINDING: u32 = 1; //bindings, not the bind groups

const BASE_MASSES: &'static [f32] = &[0.2, 0.4, 16.0, 800.0];
/*const BASE_POSITIONS: &'static [[f32; 3]] = &[
    [1.0, 1.0, 2.0],
    [2.0, 2.0, 3.0],
	[0.2, 0.2, 0.2],
	[5.0, 1.0, 2.0]
];*/
//WGSL vec3 type is aligned to 16 bytes, so need to pad out the values for actual correctness!!
const BASE_POSITIONS: &'static [f32; 16] = &[
    1.0, 1.0, 2.0, 0.0,
    3.0, 4.0, 5.0, 0.0,
	3.0, 8.0, 4.0, 0.0,
	8.0, 7.0, 8.5, 0.0
];

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
    let size = window.inner_size();
    let mut config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_supported_formats(&adapter)[0],
        width: size.width,
        height: size.height,
        present_mode: PresentMode::Fifo,
        alpha_mode: surface.get_supported_alpha_modes(&adapter)[0],
    };
    surface.configure(&device, &config);

    //TODO: read input data / gen random
    let masses = BASE_MASSES;
    let positions = BASE_POSITIONS;

    /*setup:
    	* buffers for I/O that the shader will r/w
    	* if using BufferInitDescriptor, requires: initial state already allocated in CPU mem
    	* bind ground: collection of resource (buffers, etc.)
    	* pipeline: wrapper around shader
    	*/
    //buffers: ronly: mass, rw: position, vel

    let mass_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("mass_buffer"),
        contents: bytemuck::cast_slice(masses),
        usage: BufferUsages::STORAGE, //inherently mapped at creation, as it creates and copies the data in one fn call
    });
    let pos_buffer_a = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("pos_buffer_a"),
        contents: bytemuck::cast_slice(positions),
        usage: BufferUsages::STORAGE, //inherently mapped at creation, as it creates and copies the data in one fn call
    });
    let pos_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("pos_buffer_b"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let vel_buffer_a = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("vel_buffer_a"),
        contents: bytemuck::cast_slice(&[0.0; N_BODIES * 3]), //init to 0, also *3 cast_slice flat
        usage: BufferUsages::STORAGE, //inherently mapped at creation, as it creates and copies the data in one fn call
    });
    let vel_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("vel_buffer_b"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
	
    //bind group layouts
    let mass_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("mass_bind_group_layout"),
        entries: &[BindGroupLayoutEntry {
            binding: MASS_BINDING,
            visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None, //TODO: optimize
            },
            count: None, //other values only for Texture, not Storage
        }],
    });
    let posvel_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("posvel_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                //position's entry
                binding: POS_BINDING,
                visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None, //TODO: optimize
                },
                count: None, //other values only for Texture, not Storage
            },
            BindGroupLayoutEntry {
                //velocity's entry
                binding: VEL_BINDING,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None, //TODO: optimize
                },
                count: None, //other values only for Texture, not Storage
            },
        ],
    });

    //bind groups: mass, pos_both, vel_both
    let mass_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("mass_bind_group"),
        layout: &mass_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: MASS_BINDING,
            /*resource: BindingResource::Buffer(BufferBinding {
                buffer: &mass_buffer,
                offset: 0,
                size: None,
            }),*/
			resource: mass_buffer.as_entire_binding()
        }],
    });
    let mut posvel_bind_group_a = device.create_bind_group(&BindGroupDescriptor {
        label: Some("posvel_bind_group_a"),
        layout: &posvel_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: POS_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &pos_buffer_a,
                    offset: 0,
                    size: None,
                }),*/
				resource: pos_buffer_a.as_entire_binding()
            },
            BindGroupEntry {
                binding: VEL_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &vel_buffer_a,
                    offset: 0,
                    size: None,
                }),*/
				resource: vel_buffer_a.as_entire_binding()
            },
        ],
    });
    let mut posvel_bind_group_b = device.create_bind_group(&BindGroupDescriptor {
        label: Some("posvel_bind_group_b"),
        layout: &posvel_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: POS_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &pos_buffer_b,
                    offset: 0,
                    size: None,
                }),*/
				resource: pos_buffer_b.as_entire_binding()
            },
            BindGroupEntry {
                binding: VEL_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &vel_buffer_b,
                    offset: 0,
                    size: None,
                }),*/
				resource: vel_buffer_b.as_entire_binding()
            },
        ],
    });
	
    //this shader must be compiled to a pipeline, and the handle thereof retrieved
    let nbody_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nbody_shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("nbody.wgsl"))),
    });
    //pipeline layout, then pipeline the compute shader
    let nbody_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nbody_pipeline_layout"),
        bind_group_layouts: &[
            &mass_bind_group_layout,
            &posvel_bind_group_layout,
            &posvel_bind_group_layout,
        ],
        push_constant_ranges: &[], //empty is fine
    });
    //pipeline itself
    let nbody_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("nbody_pipeline"),
        layout: Some(&nbody_pipeline_layout),
        module: &nbody_shader,
        entry_point: "nbody_step", //name of fn within shader that gets called
    });

    let trace_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("trace_shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("trace.wgsl"))),
    });
    let trace_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("trace_pipeline_layout"),
        bind_group_layouts: &[&mass_bind_group_layout, &posvel_bind_group_layout],
        push_constant_ranges: &[],
    });
    let trace_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("trace_pipeline"),
        layout: Some(&trace_pipeline_layout),
        vertex: VertexState {
            module: &trace_shader,
            entry_point: "fullscreen_vertex_shader",
            buffers: &[],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: &trace_shader,
            entry_point: "trace",
            targets: &[Some(surface.get_supported_formats(&adapter)[0].into())],
        }),
        multiview: None,
    });

    event_loop.run(move |event, _, control_flow| {
        //let _ = (&instance, &adapter, &nbody_shader);

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                config.width = size.width;
                config.height = size.height;
                surface.configure(&device, &config);
                window.request_redraw();
            }

            // Update simulation (nbody.wgsl)
            Event::MainEventsCleared => {
                //create a command encoder for each step call, which tells the pipeline to do Some ops on Some data
                
				let mut nbody_step_cmd_encoder =
                    device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("nbody_step_cmd_encoder"),
                    });
                {
                    //code block because we want lifetime of compute pass to end after
                    let mut nbody_step_pass =
                        nbody_step_cmd_encoder.begin_compute_pass(&ComputePassDescriptor {
                            label: Some("nbody_step_pass"),
                        });
                    //now that we've initialized: set_pipeline, set_bind_group, dispatch_workgroups
                    nbody_step_pass.set_pipeline(&nbody_pipeline);
                    nbody_step_pass.set_bind_group(MASS_GROUP, &mass_bind_group, &[]);
                    nbody_step_pass.set_bind_group(POSVEL_IN_GROUP, &posvel_bind_group_a, &[]);
                    nbody_step_pass.set_bind_group(POSVEL_OUT_GROUP, &posvel_bind_group_b, &[]);
                    //dispatch!
                    //can use dispatch_workgroups_indirect to use GPU to compute clustering
                    //then the GPU can determine n_workgroups from an in-GPU buffer without copy-paste
					let n_workgroups:u32 = ( (((N_BODIES as f32)/(WG_SIZE as f32)).ceil() ) ) as u32;
					//TODO: probably something else...
						//at minimum: spread across x,y,z to support more than like, 2^22 bodies
                    nbody_step_pass.dispatch_workgroups(n_workgroups, 1, 1);
                }
                //submit command encoder to queue
                let _ = queue.submit(Some(nbody_step_cmd_encoder.finish()));
				
                //swap groups a and b to alternate I/O
                mem::swap(&mut posvel_bind_group_a, &mut posvel_bind_group_b);
				
				window.request_redraw();
            }

            // Render (trace.wgsl)
            Event::RedrawRequested(_) => {
                let frame = surface.get_current_texture().unwrap();
                let view = frame.texture.create_view(&TextureViewDescriptor::default());
                let mut trace_cmd_encoder =
                    device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("trace_cmd_encoder"),
                    });

                {
                    let mut trace_pass =
                        trace_cmd_encoder.begin_render_pass(&RenderPassDescriptor {
                            label: Some("trace_pass"),
                            color_attachments: &[Some(RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: Operations::default(),
                            })],
                            depth_stencil_attachment: None,
                        });
                    trace_pass.set_pipeline(&trace_pipeline);
                    trace_pass.set_bind_group(0, &mass_bind_group, &[]);
                    trace_pass.set_bind_group(1, &posvel_bind_group_b, &[]);
                    trace_pass.draw(0..3, 0..1);
                }

                queue.submit(Some(trace_cmd_encoder.finish()));
                frame.present();
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
