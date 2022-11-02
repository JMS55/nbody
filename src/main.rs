use std::borrow::Cow;
use wgpu::*;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

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

    // Create constant gpu resources
    // TODO
    let nbody_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("nbody.wgsl"))),
    });

    event_loop.run(move |event, _, control_flow| {
        let _ = (&instance, &adapter, &nbody_shader);

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_size),
                ..
            } => {
                // TODO: Handle resize
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
