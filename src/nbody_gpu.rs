use encase::{ShaderType, StorageBuffer, UniformBuffer};
use glam::{Vec2, Vec3};
use rand::Rng;
use std::borrow::Cow;
use std::mem;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use winit::{
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

const N_BODIES: usize = 250;
pub const WORLD_SIZE: f32 = 250.0;

const WG_SIZE: usize = 64;
const STATIC_GROUP: u32 = 0;
const KINEMATICS_IN_GROUP: u32 = 1;
const KINEMATICS_OUT_GROUP: u32 = 2;
const MASS_BINDING: u32 = 0;
const DENSITIES_BINDING: u32 = 1;
const EMITTERS_BINDING: u32 = 2;
const POS_BINDING: u32 = 0; //bindings, not the bind groups
const VEL_BINDING: u32 = 1; //bindings, not the bind groups
const ACC_BINDING: u32 = 2; //bindings, not the bind groups

async fn run(event_loop: EventLoop<()>, window: Window) {
    // Setup GPU adapter/surface
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

    // Setup CPU buffers
    let mut masses = Vec::with_capacity(N_BODIES);
    let mut densities = Vec::with_capacity(N_BODIES);
    let mut emitters = Vec::with_capacity(N_BODIES);

    let mut positions_1 = Vec::with_capacity(N_BODIES);

    // Generate random bodies
    let mut rng = rand::thread_rng();
    for n in 0..N_BODIES {
        let mass = rng.gen_range(0.5..=8.0) * rng.gen_range(0.5..=8.0);
        let lower_bound = WORLD_SIZE / 5.0;
        let upper_bound = 4.0 * lower_bound;
        let position = Vec3::new(
            rng.gen_range(lower_bound..=upper_bound),
            rng.gen_range(lower_bound..=upper_bound),
            rng.gen_range(lower_bound..=upper_bound),
        );
        //let density = rng.gen_range(1.0..=2.5);
        let density = 1.0;
        masses.push(mass);
        positions_1.push(position);
        densities.push(density);
        if position.x.round() as u32 % 20 == 0 || n == 0 {
            emitters.push(n as u32);
        }
    }
    emitters.shrink_to_fit();

    // Setup GPU buffers
    let mut mass_buffer = StorageBuffer::new(Vec::new());
    mass_buffer.write(&masses).unwrap();
    let mass_buffer = mass_buffer.into_inner();
    let mass_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("mass_buffer"),
        contents: &mass_buffer,
        usage: BufferUsages::STORAGE, //inherently mapped at creation, as it creates and copies the data in one fn call
    });
    let mut densities_buffer = StorageBuffer::new(Vec::new());
    densities_buffer.write(&densities).unwrap();
    let densities_buffer = densities_buffer.into_inner();
    let densities_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("densities_buffer"),
        contents: &densities_buffer,
        usage: BufferUsages::STORAGE,
    });
    let mut emitters_buffer = StorageBuffer::new(Vec::new());
    emitters_buffer.write(&emitters).unwrap();
    let emitters_buffer = emitters_buffer.into_inner();
    let emitters_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("emitters_buffer"),
        contents: &emitters_buffer,
        usage: BufferUsages::STORAGE,
    });
    let mut pos_buffer_a = StorageBuffer::new(Vec::new());
    pos_buffer_a.write(&positions_1).unwrap();
    let pos_buffer_a = pos_buffer_a.into_inner();
    let mut pos_buffer_a = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("pos_buffer_a"),
        contents: &pos_buffer_a,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });
    let mut pos_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("pos_buffer_b"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let vel_buffer_a = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("vel_buffer_a"),
        contents: bytemuck::cast_slice(&[0.0; N_BODIES * 3]),
        usage: BufferUsages::STORAGE,
    });
    let vel_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("vel_buffer_b"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let acc_buffer_a = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("acc_buffer_a"),
        contents: bytemuck::cast_slice(&[0.0; N_BODIES * 3]),
        usage: BufferUsages::STORAGE,
    });
    let acc_buffer_b = device.create_buffer(&BufferDescriptor {
        label: Some("acc_buffer_b"),
        size: (mem::size_of::<[f32; 4]>() * N_BODIES) as u64,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    // Create bind group layouts
    let static_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("static_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: MASS_BINDING,
                visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: DENSITIES_BINDING,
                visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
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
                    binding: POS_BINDING,
                    visibility: ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: VEL_BINDING,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: ACC_BINDING,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
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
                min_binding_size: None,
            },
            count: None,
        }],
    });

    // Create bind groups
    let static_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("static_bind_group"),
        layout: &static_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: MASS_BINDING,
                resource: mass_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: DENSITIES_BINDING,
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
                resource: pos_buffer_a.as_entire_binding(),
            },
            BindGroupEntry {
                binding: VEL_BINDING,
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
                resource: pos_buffer_b.as_entire_binding(),
            },
            BindGroupEntry {
                binding: VEL_BINDING,
                resource: vel_buffer_b.as_entire_binding(),
            },
            BindGroupEntry {
                binding: ACC_BINDING,
                resource: acc_buffer_b.as_entire_binding(),
            },
        ],
    });

    // Compile nbody pipeline/shader
    let nbody_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("nbody_shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("nbody.wgsl"))),
    });
    let nbody_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("nbody_pipeline_layout"),
        bind_group_layouts: &[
            &static_bind_group_layout,
            &kinematics_bind_group_layout,
            &kinematics_bind_group_layout,
        ],
        push_constant_ranges: &[],
    });
    let nbody_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("nbody_pipeline"),
        layout: Some(&nbody_pipeline_layout),
        module: &nbody_shader,
        entry_point: "nbody_step",
    });

    // Compile render pipeline/shader
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
    let mut render_bool: bool = true;
    camera.position = positions_1[0];
    event_loop.run(move |event, _, control_flow| {
        match event {
            // Handle window resize
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                config.width = size.width;
                config.height = size.height;
                surface.configure(&device, &config);
                window.request_redraw();
            }

            // Handle camera control inputs
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
                let mut nbody_step_cmd_encoder =
                    device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("nbody_step_cmd_encoder"),
                    });

                // Queue nbody sim job
                {
                    let mut nbody_step_pass =
                        nbody_step_cmd_encoder.begin_compute_pass(&ComputePassDescriptor {
                            label: Some("nbody_step_pass"),
                        });
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

                    let n_workgroups: u32 = (((N_BODIES as f32) / (WG_SIZE as f32)).ceil()) as u32;
                    nbody_step_pass.dispatch_workgroups(n_workgroups, 1, 1);
                }

                queue.submit(Some(nbody_step_cmd_encoder.finish()));

                // Swap buffers
                mem::swap(&mut kinematics_bind_group_a, &mut kinematics_bind_group_b);
                mem::swap(&mut pos_buffer_a, &mut pos_buffer_b);

                // Alternate rendering every other frame
                if render_bool {
                    window.request_redraw();
                }
                render_bool = !render_bool;
            }

            // Render (trace.wgsl)
            Event::RedrawRequested(_) => {
                // Setup frame
                let frame = surface.get_current_texture().unwrap();
                let view = frame.texture.create_view(&TextureViewDescriptor::default());
                let mut trace_cmd_encoder =
                    device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("trace_cmd_encoder"),
                    });

                // Create camera uniform
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

                // Queue render job
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
            }

            // Handle window exit
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
