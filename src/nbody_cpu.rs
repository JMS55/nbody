use encase::{ShaderType, StorageBuffer, UniformBuffer};
use glam::{Vec2, Vec3};
use rand::Rng;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use std::borrow::Cow;
use std::mem;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use winit::{
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

const N_BODIES: usize = 100;
pub const WORLD_SIZE: f32 = 1000.0;

const MASS_BINDING: u32 = 0;
const DENSITIES_BINDING: u32 = 1;
const EMITTERS_BINDING: u32 = 2;
const POS_BINDING: u32 = 0;

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
    let mut velocities_1 = vec![Vec3::ZERO; N_BODIES];
    let mut accelerations_1 = vec![Vec3::ZERO; N_BODIES];

    let mut positions_2 = vec![Vec3::ZERO; N_BODIES];
    let mut velocities_2 = vec![Vec3::ZERO; N_BODIES];
    let mut accelerations_2 = vec![Vec3::ZERO; N_BODIES];

    // Generate random bodies
    let mut rng = rand::thread_rng();
    for n in 0..N_BODIES {
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
        positions_1.push(position);
        densities.push(density);
        if position.x.round() as u32 % 20 == 0 {
            emitters.push(n);
        }
    }
    emitters.shrink_to_fit();

    // Setup GPU buffers
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
        contents: bytemuck::cast_slice(&emitters),
        usage: BufferUsages::STORAGE,
    });
    let mut pos_buffer = StorageBuffer::new(Vec::new());
    pos_buffer.write(&positions_1).unwrap();
    let pos_buffer = pos_buffer.into_inner();
    let pos_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("pos_buffer"),
        contents: &pos_buffer,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    // Create bind group layouts
    let static_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("static_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: MASS_BINDING,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: DENSITIES_BINDING,
                visibility: ShaderStages::FRAGMENT,
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

    let positions_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("positions_bind_group_layout"),
        entries: &[BindGroupLayoutEntry {
            binding: POS_BINDING,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
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

    let positions_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("positions_bind_group"),
        layout: &positions_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: POS_BINDING,
            resource: pos_buffer.as_entire_binding(),
        }],
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
            &positions_bind_group_layout,
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
    camera.position.z -= 10.0;
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

            // Update simulation (CPU)
            Event::MainEventsCleared => {
                // Update particle state (in parallel)
                let p_s = RawPtr::from_mut(&mut positions_2);
                let v_s = RawPtr::from_mut(&mut velocities_2);
                let a_s = RawPtr::from_mut(&mut accelerations_2);
                (0..N_BODIES).into_par_iter().for_each(|n| {
                    let positions_1 = &positions_1;
                    let masses = &masses;
                    let positions_2 = unsafe { p_s.into_mut() };
                    let velocities_2 = unsafe { v_s.into_mut() };
                    let accelerations_2 = unsafe { a_s.into_mut() };

                    // Determine particle acceleration
                    let p = positions_1[n];
                    let a = (0..N_BODIES)
                        .into_iter()
                        .filter(|n2| *n2 != n)
                        .map(|n2| {
                            let distance = positions_1[n2] - p;
                            let distance_norm = distance.dot(distance);
                            (6.674 * masses[n2] * distance)
                                / (distance_norm * distance_norm * distance_norm)
                        })
                        .sum();

                    // Update particle acceleration, velocity, position
                    accelerations_2[n] = a;
                    let v = velocities_2[n] + a;
                    velocities_2[n] = v;
                    positions_2[n] += v;
                });

                // Copy positions to GPU buffer
                let mut pos_data = StorageBuffer::new(Vec::new());
                pos_data.write(&positions_1).unwrap();
                let pos_data = pos_data.into_inner();
                queue.write_buffer(&pos_buffer, 0, &pos_data);

                // Swap buffers
                mem::swap(&mut positions_1, &mut positions_2);
                mem::swap(&mut velocities_1, &mut velocities_2);
                mem::swap(&mut accelerations_1, &mut accelerations_2);

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
                    trace_pass.set_bind_group(1, &positions_bind_group, &[]);
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

// Bypass rust safety - this is safe because the buffers will never be dropped
// before the references to them, and we won't have conflicting modifications.
#[derive(Clone, Copy)]
struct RawPtr(*mut Vec<Vec3>);
impl RawPtr {
    fn from_mut(v: &mut Vec<Vec3>) -> Self {
        Self(v as *mut Vec<Vec3>)
    }
    unsafe fn into_mut(self) -> &'static mut Vec<Vec3> {
        &mut *self.0
    }
}
unsafe impl Sync for RawPtr {}
unsafe impl Send for RawPtr {}
