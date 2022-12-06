mod octree;

use crate::octree::OctreeNode;
use encase::{ShaderType, StorageBuffer, UniformBuffer};
use glam::{Vec2, Vec3};
 use rand::Rng;
use std::borrow::Cow;
use std::mem;
// use std::time::{Duration, Instant};
use wgpu::util::*; //include some stuff outside of spec
use wgpu::*;
use winit::{
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

const N_BODIES: usize = 100;
pub const WORLD_SIZE: f32 = 100.0;
//const N_WORKGROUPS: u32 = 1;
const WG_SIZE: u32 = 64;

const STATIC_GROUP: u32 = 0;
const KINEMATICS_IN_GROUP: u32 = 1;
const KINEMATICS_OUT_GROUP: u32 = 2;
const OCTREE_GROUP: u32 = 3;
const MASS_BINDING: u32 = 0;
const DENSITIES_BINDING: u32 = 1;
const EMITTERS_BINDING: u32 = 2;
const POS_BINDING: u32 = 0; //bindings, not the bind groups
const VEL_BINDING: u32 = 1; //bindings, not the bind groups
const ACC_BINDING: u32 = 2; //bindings, not the bind groups

const BASE_MASSES: &'static [f32] = &[];//.0.2, 0.4, 16.0, 800.0];
const BASE_DENSITIES: &'static [f32] = &[];//1.0, 1.0, 2.0, 5.0];
const BASE_EMITTERS: &'static [u32] = &[0, 2, 3];
const BASE_POSITIONS: &'static [Vec3; 0] = &[
    //Vec3::new(1.0, 1.0, 2.0),
    //Vec3::new(3.0, 4.0, 5.0),
    //Vec3::new(3.0, 8.0, 4.0),
    //Vec3::new(8.0, 7.0, 8.5),
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

    let mut masses = BASE_MASSES.to_vec();
    let mut positions = BASE_POSITIONS.to_vec();
    let mut densities = BASE_DENSITIES.to_vec();
    let emitters = BASE_EMITTERS;
     let mut rng = rand::thread_rng();
     for _ in 0..N_BODIES {
         let mass = rng.gen_range(0.5..=500.0);
         let lower_bound = WORLD_SIZE / 3.0;
         let upper_bound = 2.0 * lower_bound;
         let position = Vec3::new(
             rng.gen_range(lower_bound..=upper_bound),
             rng.gen_range(lower_bound..=upper_bound),
             rng.gen_range(lower_bound..=upper_bound),
         );
         let density = rng.gen_range(1.0..=6.0);
         masses.push(mass);
         positions.push(position);
         densities.push(density);
     }

    /*setup:
    	* buffers for I/O that the shader will r/w
    	* if using BufferInitDescriptor, requires: initial state already allocated in CPU mem
    	* bind ground: collection of resource (buffers, etc.)
    	* pipeline: wrapper around shader
    	*/
    //buffers: ronly: mass, rw: position, vel

    let mass_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("mass_buffer"),
        contents: bytemuck::cast_slice(&masses),
        usage: BufferUsages::STORAGE, //inherently mapped at creation, as it creates and copies the data in one fn call
    });
    let densities_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("densities_buffer"),
        contents: bytemuck::cast_slice(&densities),
        usage: BufferUsages::STORAGE,
    });
    let emitters_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("emitters_buffer"),
        contents: bytemuck::cast_slice(emitters),
        usage: BufferUsages::STORAGE,
    });
    let mut pos_buffer_a = StorageBuffer::new(Vec::new());
    pos_buffer_a.write(&positions).unwrap();
    let pos_buffer_a = pos_buffer_a.into_inner();
    let mut pos_buffer_a = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("pos_buffer_a"),
        contents: &pos_buffer_a,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, //inherently mapped at creation, as it creates and copies the data in one fn call
    });
    let mut pos_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("pos_buffer_b"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
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
    let acc_buffer_a = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("acc_buffer_a"),
        contents: bytemuck::cast_slice(&[0.0; N_BODIES * 3]), //init to 0, also *3 cast_slice flat
        usage: BufferUsages::STORAGE, //inherently mapped at creation, as it creates and copies the data in one fn call
    });
    let acc_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("acc_buffer_b"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let pos_readback_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("pos_readback_buffer"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    //bind group layouts
    let static_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("static_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: MASS_BINDING,
                visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None, //TODO: optimize
                },
                count: None, //other values only for Texture, not Storage
            },
            BindGroupLayoutEntry {
                binding: DENSITIES_BINDING,
                visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None, //TODO: optimize
                },
                count: None, //other values only for Texture, not Storage
            },
            BindGroupLayoutEntry {
                binding: EMITTERS_BINDING,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let kinematics_bind_group_layout =
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("kinematics_bind_group_layout"),
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
                BindGroupLayoutEntry {
                    //acceleration's entry
                    binding: ACC_BINDING,
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

    let camera_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("camera_bind_group_layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None, //TODO: optimize
            },
            count: None, //other values only for Texture, not Storage
        }],
    });

    let octree_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("octree_bind_group_layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None, //TODO: optimize
            },
            count: None, //other values only for Texture, not Storage
        }],
    });

    //bind groups: mass, pos_both, vel_both
    let static_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("static_bind_group"),
        layout: &static_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: MASS_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &mass_buffer,
                    offset: 0,
                    size: None,
                }),*/
                resource: mass_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: DENSITIES_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &mass_buffer,
                    offset: 0,
                    size: None,
                }),*/
                resource: densities_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: EMITTERS_BINDING,
                resource: emitters_buffer.as_entire_binding(),
            },
        ],
    });

    let mut kinematics_bind_group_a = device.create_bind_group(&BindGroupDescriptor {
        label: Some("kinematics_bind_group_a"),
        layout: &kinematics_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: POS_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &pos_buffer_a,
                    offset: 0,
                    size: None,
                }),*/
                resource: pos_buffer_a.as_entire_binding(),
            },
            BindGroupEntry {
                binding: VEL_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &vel_buffer_a,
                    offset: 0,
                    size: None,
                }),*/
                resource: vel_buffer_a.as_entire_binding(),
            },
            BindGroupEntry {
                binding: ACC_BINDING,
                resource: acc_buffer_a.as_entire_binding(),
            },
        ],
    });
    let mut kinematics_bind_group_b = device.create_bind_group(&BindGroupDescriptor {
        label: Some("kinematics_bind_group_b"),
        layout: &kinematics_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: POS_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &pos_buffer_b,
                    offset: 0,
                    size: None,
                }),*/
                resource: pos_buffer_b.as_entire_binding(),
            },
            BindGroupEntry {
                binding: VEL_BINDING,
                /*resource: BindingResource::Buffer(BufferBinding {
                    buffer: &vel_buffer_b,
                    offset: 0,
                    size: None,
                }),*/
                resource: vel_buffer_b.as_entire_binding(),
            },
            BindGroupEntry {
                binding: ACC_BINDING,
                resource: acc_buffer_b.as_entire_binding(),
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
            &static_bind_group_layout,
            &kinematics_bind_group_layout,
            &kinematics_bind_group_layout,
            &octree_bind_group_layout,
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
        bind_group_layouts: &[
            &static_bind_group_layout,
            &kinematics_bind_group_layout,
            &camera_bind_group_layout,
        ],
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

    let mut camera = Camera::default();
    camera.position.z -= 10.0;
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

            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: event_input,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                let key_val = event_input;
                let camera_direction: Vec3 = Vec3 {
                    x: f32::sin(camera.angle_elevation.x) * f32::cos(camera.angle_elevation.y),
                    y: f32::sin(camera.angle_elevation.y),
                    z: f32::cos(camera.angle_elevation.x) * f32::cos(camera.angle_elevation.y),
                };
                let camera_plane_x: Vec3 = Vec3 {
                    x: -f32::cos(camera.angle_elevation.x),
                    y: 0.0,
                    z: f32::sin(camera.angle_elevation.x),
                };
                let camera_plane_y: Vec3 = Vec3 {
                    x: -f32::sin(camera.angle_elevation.y) * f32::sin(camera.angle_elevation.x),
                    y: f32::cos(camera.angle_elevation.y),
                    z: -f32::sin(camera.angle_elevation.y) * f32::cos(camera.angle_elevation.x),
                };
                match key_val {
                    Option::None => {}
                    Option::Some(vkc) => {
                        if vkc == VirtualKeyCode::Left {
                            camera.angle_elevation.x += 0.1;
                        } else if vkc == VirtualKeyCode::Right {
                            camera.angle_elevation.x -= 0.1;
                        } else if vkc == VirtualKeyCode::Up {
                            camera.angle_elevation.y -= 0.1;
                        } else if vkc == VirtualKeyCode::Down {
                            camera.angle_elevation.y += 0.1;
                        } else if vkc == VirtualKeyCode::A {
                            camera.position = camera.position - (camera_plane_x);
                        } else if vkc == VirtualKeyCode::D {
                            camera.position = camera.position + (camera_plane_x);
                        } else if vkc == VirtualKeyCode::Q {
                            camera.position = camera.position - (camera_plane_y);
                        } else if vkc == VirtualKeyCode::E {
                            camera.position = camera.position + (camera_plane_y);
                        } else if vkc == VirtualKeyCode::S {
                            camera.position = camera.position - (camera_direction);
                        } else if vkc == VirtualKeyCode::W {
                            camera.position = camera.position + (camera_direction);
                        }
                    }
                }
            }

            // Update simulation (nbody.wgsl)
            Event::MainEventsCleared => {
                //create a command encoder for each step call, which tells the pipeline to do Some ops on Some data

                let octree = OctreeNode::new_tree(&positions, &masses);
                let mut octree_buffer = StorageBuffer::new(Vec::new());
                octree_buffer.write(&octree).unwrap();
                let octree_buffer = octree_buffer.into_inner();
                let octree_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("octree_buffer"),
                    contents: &octree_buffer,
                    usage: BufferUsages::STORAGE,
                });
                let octree_bind_group = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("octree_bind_group"),
                    layout: &octree_bind_group_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: octree_buffer.as_entire_binding(),
                    }],
                });

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
                    nbody_step_pass.set_bind_group(STATIC_GROUP, &static_bind_group, &[]);
                    nbody_step_pass.set_bind_group(
                        KINEMATICS_IN_GROUP,
                        &kinematics_bind_group_a,
                        &[],
                    );
                    nbody_step_pass.set_bind_group(
                        KINEMATICS_OUT_GROUP,
                        &kinematics_bind_group_b,
                        &[],
                    );
                    nbody_step_pass.set_bind_group(OCTREE_GROUP, &octree_bind_group, &[]);

                    //dispatch!
                    //can use dispatch_workgroups_indirect to use GPU to compute clustering
                    //then the GPU can determine n_workgroups from an in-GPU buffer without copy-paste
                    let n_workgroups: u32 = (((N_BODIES as f32) / (WG_SIZE as f32)).ceil()) as u32;
                    //TODO: probably something else...
                    //at minimum: spread across x,y,z to support more than like, 2^22 bodies
                    nbody_step_pass.dispatch_workgroups(n_workgroups, 1, 1);
                }

                nbody_step_cmd_encoder.copy_buffer_to_buffer(
                    &pos_buffer_b,
                    0,
                    &pos_readback_buffer,
                    0,
                    (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
                );

                //submit command encoder to queue
                let _ = queue.submit(Some(nbody_step_cmd_encoder.finish()));

                let pos_slice = pos_readback_buffer.slice(..);
                let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
                pos_slice.map_async(MapMode::Read, move |v| sender.send(v).unwrap());
                device.poll(Maintain::Wait);
                pollster::block_on(receiver.receive());
                let data = pos_slice.get_mapped_range();
                let buf = StorageBuffer::new(&*data);
                buf.read(&mut positions).unwrap();
                drop(data);
                pos_readback_buffer.unmap();

                //swap groups a and b to alternate I/O
                mem::swap(&mut kinematics_bind_group_a, &mut kinematics_bind_group_b);
                mem::swap(&mut pos_buffer_a, &mut pos_buffer_b);

                window.request_redraw();
            }

            // Render (trace.wgsl)
            Event::RedrawRequested(_) => {
                // static mut running_average_time_render: Duration = Duration::new(0, 0);
                // static mut number_frames_rendered: u32 = 0;
                // let start_time: Instant = Instant::now();
                let frame = surface.get_current_texture().unwrap();
                let view = frame.texture.create_view(&TextureViewDescriptor::default());
                let mut trace_cmd_encoder =
                    device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("trace_cmd_encoder"),
                    });

                let mut camera_buffer = UniformBuffer::new(Vec::new());
                camera_buffer.write(&camera).unwrap();
                let camera_buffer = camera_buffer.into_inner();
                let camera_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("camera_buffer"),
                    contents: &camera_buffer,
                    usage: BufferUsages::UNIFORM,
                });
                let camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("camera_bind_group"),
                    layout: &camera_bind_group_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    }],
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
                    trace_pass.set_bind_group(0, &static_bind_group, &[]);
                    trace_pass.set_bind_group(1, &kinematics_bind_group_b, &[]);
                    trace_pass.set_bind_group(2, &camera_bind_group, &[]);
                    trace_pass.draw(0..3, 0..1);
                }

                queue.submit(Some(trace_cmd_encoder.finish()));
                frame.present();
                // let new_time: Instant = Instant::now();
                // println!(
                //     "Current Frame Time: {:?}",
                //     new_time.duration_since(start_time)
                // );
                // unsafe {
                //     running_average_time_render = (running_average_time_render
                //         * number_frames_rendered
                //         + new_time.duration_since(start_time))
                //         / (number_frames_rendered + 1);
                //     number_frames_rendered += 1;
                //     println!("Average Frame Time: {:?}", running_average_time_render);
                // }
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

#[derive(ShaderType, Default)]
struct Camera {
    position: Vec3,
    angle_elevation: Vec2,
}
