extern crate shaderc;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    renderwindow()?;
    Ok(())
}

pub fn renderwindow() -> Result<(), Box<dyn Error>> {
    use gfx_hal::{
        device::Device,
        window::{Extent2D, PresentationSurface, Surface},
        Instance,
    };
    use shaderc::ShaderKind;

    const WINDOW_TITLE: &str = "Sample text";
    const WINDOW_SIZE: [u32; 2] = [2160, 3840];

    let event_loop = winit::event_loop::EventLoop::new();

    let (real_size, physical_size) = {
        let dpi = event_loop.primary_monitor().scale_factor();
        let logical: winit::dpi::LogicalSize<u32> = WINDOW_SIZE.into();
        (logical, logical.to_physical(dpi))
    };
    let mut surface_extent = Extent2D {
        width: physical_size.width,
        height: physical_size.height,
    };

    let window = winit::window::WindowBuilder::new()
        .with_title(WINDOW_TITLE)
        .with_inner_size(real_size)
        .build(&event_loop)?;

    let instance = backend::Instance::create(WINDOW_TITLE, 1).expect("Unsupported backend]");
    let surface = unsafe { instance.create_surface(&window)? };
    let adapter = instance.enumerate_adapters().remove(0);

    let (device, mut queue_group) = {
        use gfx_hal::queue::QueueFamily;

        let queue_family = adapter.queue_families.iter().find(|family| {
            surface.supports_queue_family(family) && family.queue_type().supports_graphics()
        });
        let mut gpu = unsafe {
            use gfx_hal::adapter::PhysicalDevice;
            adapter
                .physical_device
                .open(&([(queue_family, &[1.0])]), gfx_hal::Features::empty())?;
        };
        (gpu.device, gpu.queue_groups.pop()?)
    }?;

    let mut should_configure_swapchain = true;

    event_loop.run(move |event, _, control_flow| {
        use winit::event::{Event, WindowEvent};
        use winit::event_loop::ControlFlow;

        match event {
            // Handles all the events related to window updates
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(updated) => {
                    surface_extent = Extent2D {
                        width: updated.width,
                        height: updated.height,
                    };
                    should_configure_swapchain = true;
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    surface_extent = Extent2D {
                        width: new_inner_size.width,
                        height: new_inner_size.height,
                    };
                    should_configure_swapchain = true;
                }
                _ => (),
            },
            // After input events, handle non-rendinering logic
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            // TODO: Rendering logic implementation
            Event::RedrawRequested(_) => {}
            _ => (),
        }
    });
}
