/// Requires shaderc library to be installed => https://github.com/google/shaderc
extern crate shaderc;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    renderwindow()?;
    Ok(())
}

// TODO: create a struct to statically handle error's instead of boxing them.
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

    // Logical size represents the total number of pixels within a monitor.
    // Physical size is the dimensions the OS will allocate and then apply a scale factor to.
    // For example the OS might decide the physical size to be 1024 wide on a 2048px wide display,
    // then apply a 2x scale factor. On the other hand if the display was to be 1024px; it'd
    // apply a scale factor of 1x on the logical size.
    let (logical_size, physical_size) = {
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
        .with_inner_size(logical_size)
        .build(&event_loop)?;

    let instance = backend::Instance::create(WINDOW_TITLE, 1).expect("Unsupported backend]");
    let surface = unsafe { instance.create_surface(&window)? };
    let adapter = instance.enumerate_adapters().remove(0);

    let (device, mut queue_group) = {
        use gfx_hal::queue::QueueFamily;

        let queue_family = adapter
            .queue_families
            .iter()
            .find(|family| {
                surface.supports_queue_family(family) && family.queue_type().supports_graphics()
            })
            .expect("No compatible queue family found");
        let mut gpu = unsafe {
            use gfx_hal::adapter::PhysicalDevice;
            adapter
                .physical_device
                .open(&[(queue_family, &[1.0])], gfx_hal::Features::empty())
                .expect("Failed to open device")
        };

        (gpu.device, gpu.queue_groups.pop().unwrap())
    };

    // command buffer => a structure of commands to render anything to the GPU, this is done
    //                   via a command buffer. These command buffers are allocated from a
    //                   command pool.
    let (command_pool, mut command_buffer) = unsafe {
        use gfx_hal::command::Level;
        use gfx_hal::pool::{CommandPool, CommandPoolCreateFlags};

        let mut command_pool = device
            .create_command_pool(queue_group.family, CommandPoolCreateFlags::empty())
            .expect("Out of memory");
        let command_buffer = command_pool.allocate_one(Level::Primary);

        (command_pool, command_buffer)
    };

    // The swapchain is a chain of images to render onto.
    let mut configure_swapchain = true;

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
                    configure_swapchain = true;
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    surface_extent = Extent2D {
                        width: new_inner_size.width,
                        height: new_inner_size.height,
                    };
                    configure_swapchain = true;
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
