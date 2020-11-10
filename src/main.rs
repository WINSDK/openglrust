/// Requires shaderc library to be installed => https://github.com/google/shaderc
extern crate shaderc;

use gfx_hal::device::Device;
use shaderc::ShaderKind;
use std::error::Error;
use std::mem::ManuallyDrop;

fn main() -> Result<(), Box<dyn Error>> {
    renderwindow()?;
    Ok(())
}

pub struct GpuResources<B: gfx_hal::Backend> {
    instance: B::Instance,
    surface: B::Surface,
    device: B::Device,
    render_passes: Vec<B::RenderPass>,
    pipeline_layouts: Vec<B::PipelineLayout>,
    pipelines: Vec<B::GraphicsPipeline>,
    command_pool: B::CommandPool,
    submission_fence: B::Fence,
    rendering_semaphore: B::Semaphore,
}

// Required because drop requires &mut self whilst destroy..() in gfx_hal takes exclusive ownership
// of the object through self.
struct ResourceHolder<B: gfx_hal::Backend>(ManuallyDrop<GpuResources<B>>);

impl<B: gfx_hal::Backend> Drop for ResourceHolder<B> {
    fn drop(&mut self) {
        unsafe {
            use gfx_hal::window::PresentationSurface;
            use gfx_hal::Instance;

            let GpuResources {
                instance,
                mut surface,
                device,
                render_passes,
                pipeline_layouts,
                pipelines,
                command_pool,
                submission_fence,
                rendering_semaphore,
            } = ManuallyDrop::take(&mut self.0);

            device.destroy_semaphore(rendering_semaphore);
            device.destroy_fence(submission_fence);
            for pipeline in pipelines {
                device.destroy_graphics_pipeline(pipeline);
            }
            for pipeline_layout in pipeline_layouts {
                device.destroy_pipeline_layout(pipeline_layout);
            }
            for render_pass in render_passes {
                device.destroy_render_pass(render_pass);
            }
            device.destroy_command_pool(command_pool);
            surface.unconfigure_swapchain(&device);
            instance.destroy_surface(surface);
        }
    }
}
fn read_shader(path: &str, default_options: bool) -> String {
    use std::fs;

    let mut objects: Vec<u8> = Vec::new();

    if default_options {
        objects.append(&mut fs::read("shaders/options.glsl").unwrap());
        objects.append(&mut vec![0xA]);
        objects.append(&mut fs::read(path).unwrap());

        println!("{}", std::str::from_utf8(&objects).expect("Failed parsing"));

        String::from_utf8(objects).expect("Failed to parse utf-8 sequence")
    } else {
        objects.append(&mut fs::read("shaders/vertex.glsl").unwrap());

        String::from_utf8(objects).expect("Failed to parse utf-8 sequence")
    }
}

/// Create a pipeline with the given layout and shaders.
pub unsafe fn generate_pipeline<T: gfx_hal::Backend>(
    device: &T::Device,
    render_pass: &T::RenderPass,
    pipeline_layout: &T::PipelineLayout,
    vertex_shader: &str,
    fragment_shader: &str,
) -> T::GraphicsPipeline {
    use gfx_hal::pass::Subpass;
    use gfx_hal::pso::{
        BlendState, ColorBlendDesc, ColorMask, EntryPoint, GraphicsPipelineDesc,
        InputAssemblerDesc, Primitive, PrimitiveAssemblerDesc, Rasterizer, Specialization,
    };

    let vertex_shader_module = device
        .create_shader_module(&compile_shader(vertex_shader, "Vertex", ShaderKind::Vertex))
        .expect("Failed to create vertex shader module");

    let fragment_shader_module = device
        .create_shader_module(&compile_shader(
            fragment_shader,
            "Vertex",
            ShaderKind::Fragment,
        ))
        .expect("Failed to create vertex shader module");

    let vs_entry = EntryPoint {
        entry: "pipeline()",
        module: &vertex_shader_module,
        specialization: Specialization::default(),
    };

    let fs_entry = EntryPoint {
        entry: "pipeline()",
        module: &fragment_shader_module,
        specialization: Specialization::default(),
    };

    let primitive_assembler = PrimitiveAssemblerDesc::Vertex {
        buffers: &[],
        attributes: &[],
        input_assembler: InputAssemblerDesc {
            primitive: Primitive::TriangleList,
            with_adjacency: false,
            restart_index: None,
        },
        vertex: vs_entry,
        tessellation: None,
        geometry: None,
    };

    let mut pipeline_desc = GraphicsPipelineDesc::new(
        primitive_assembler,
        Rasterizer::FILL,
        Some(fs_entry),
        pipeline_layout,
        Subpass {
            index: 0,
            main_pass: render_pass,
        },
    );

    pipeline_desc.blender.targets.push(ColorBlendDesc {
        mask: ColorMask::ALL,
        blend: Some(BlendState::ALPHA),
    });

    let pipeline = device
        .create_graphics_pipeline(&pipeline_desc, None)
        .expect("Failed to create graphics pipeline");
    device.destroy_shader_module(vertex_shader_module);
    device.destroy_shader_module(fragment_shader_module);

    pipeline
}

/// Compiles glsl shader to SPIR-V required for gfx_hal
pub fn compile_shader(shader: &str, shader_name: &str, shader_kind: ShaderKind) -> Vec<u32> {
    let mut compiler = shaderc::Compiler::new()
        .unwrap_or_else(|| panic!("Failed to compile shader: {:?}", shader_kind));

    let compiled_shader = compiler
        .compile_into_spirv(shader, shader_kind, shader_name, "compile_shader()", None)
        .unwrap_or_else(|error| panic!("Failed to compile shader: {}", error));

    compiled_shader.as_binary().to_vec()
}

// TODO: create a struct to statically handle error's instead of boxing them.
pub fn renderwindow() -> Result<(), Box<dyn Error>> {
    use gfx_hal::{
        window::{Extent2D, PresentationSurface, Surface},
        Instance,
    };

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

    println!("{:?}\n", adapter.info);

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

    let surface_color_format = {
        use gfx_hal::format::{ChannelType, Format};

        let supported_formats = surface
            .supported_formats(&adapter.physical_device)
            .unwrap_or(vec![]);

        let default_format = *supported_formats.get(0).unwrap_or(&Format::Rgba8Srgb);

        supported_formats
            .into_iter()
            .find(|format| format.base_format().1 == ChannelType::Srgb)
            .unwrap_or(default_format)
    };

    let render_pass = {
        use gfx_hal::image::Layout;
        use gfx_hal::pass::{
            Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, SubpassDesc,
        };

        let attachment = Attachment {
            format: Some(surface_color_format),
            samples: 1,
            ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::Undefined..Layout::Present,
        };

        // colors: is refering to the first index of the list of attachments
        // passed into create_render_pass.
        let subpass = SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[],
        };

        unsafe {
            device
                .create_render_pass(&[attachment], &[subpass], &[])
                .expect("Out of memory")
        }
    };

    // This defines textures and matrices required by the shaders, not required for
    // the simple shaders i'm using right now.
    let pipeline_layout = unsafe {
        device
            .create_pipeline_layout(&[], &[])
            .expect("Out of memory")
    };

    let pipeline = unsafe {
        generate_pipeline::<backend::Backend>(
            &device,
            &render_pass,
            &pipeline_layout,
            &read_shader("shaders/vertex.glsl", true)[..],
            &read_shader("shaders/fragment.glsl", true)[..],
        )
    };

    let submission_fence = device.create_fence(true).expect("Out of memory");
    let rendering_semaphore = device.create_semaphore().expect("Out of memory");

    let mut resource_container: ResourceHolder<backend::Backend> =
        ResourceHolder(ManuallyDrop::new(GpuResources {
            instance,
            surface,
            device,
            command_pool,
            render_passes: vec![render_pass],
            pipeline_layouts: vec![pipeline_layout],
            pipelines: vec![pipeline],
            submission_fence,
            rendering_semaphore,
        }));

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
            Event::RedrawRequested(_) => {
                // Timout to prevent 'hanging' of the image.
                const TIMOUT: u64 = 1_000_000_000;

                let resources: &mut GpuResources<_> = &mut resource_container.0;
                //let render_pass = &resources.render_passes[0];
                //let pipeline = &resources.pipelines[0];

                unsafe {
                    use gfx_hal::pool::CommandPool;

                    resources
                        .device
                        .wait_for_fence(&resources.submission_fence, TIMOUT)
                        .expect("Failed to wait for fence");

                    resources
                        .device
                        .reset_fence(&resources.submission_fence)
                        .expect("Out of memory");

                    resources.command_pool.reset(false);
                }

                if configure_swapchain {
                    use gfx_hal::pso as Dimensions;
                    use gfx_hal::window::SwapchainConfig;

                    let caps = resources.surface.capabilities(&adapter.physical_device);

                    let swapchain_config =
                        SwapchainConfig::from_caps(&caps, surface_color_format, surface_extent);

                    /*
                    MacOS fullscreen shutdown fix
                    if caps.image_count.contains(&3) {
                        swapchain_config.image_count = 3;
                    }
                    */

                    surface_extent = swapchain_config.extent;

                    unsafe {
                        resources
                            .surface
                            .configure_swapchain(&resources.device, swapchain_config)
                            .expect("Failed to configure swapchain");
                    };

                    configure_swapchain = false;

                    let surface_image = unsafe {
                        match resources.surface.acquire_image(TIMOUT) {
                            Ok((image, _)) => image,
                            Err(_) => {
                                configure_swapchain = true;
                                return;
                            }
                        }
                    };

                    let framebuffer = unsafe {
                        use gfx_hal::image::Extent;
                        use std::borrow::Borrow;

                        resources
                            .device
                            .create_framebuffer(
                                &resources.render_passes[0],
                                vec![surface_image.borrow()],
                                Extent {
                                    width: surface_extent.width,
                                    height: surface_extent.height,
                                    depth: 1,
                                },
                            )
                            .unwrap()
                    };

                    let viewport = Dimensions::Viewport {
                        rect: Dimensions::Rect {
                            x: 0,
                            y: 0,
                            w: surface_extent.width as i16,
                            h: surface_extent.height as i16,
                        },
                        depth: 0.0..1.0,
                    };

                    unsafe {
                        use gfx_hal::command::{
                            ClearColor, ClearValue, CommandBuffer, CommandBufferFlags,
                            SubpassContents,
                        };
                        use gfx_hal::queue::{CommandQueue, Submission};

                        command_buffer.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

                        command_buffer.set_viewports(0, &[viewport.clone()]);
                        command_buffer.set_scissors(0, &[viewport.rect]);

                        command_buffer.begin_render_pass(
                            &resources.render_passes[0],
                            &framebuffer,
                            viewport.rect,
                            &[ClearValue {
                                color: ClearColor {
                                    float32: [0.0, 0.0, 0.0, 1.0],
                                },
                            }],
                            SubpassContents::Inline,
                        );

                        command_buffer.bind_graphics_pipeline(&resources.pipelines[0]);

                        command_buffer.draw(0..0, 0..1);

                        command_buffer.end_render_pass();
                        command_buffer.finish();

                        let submission = Submission {
                            command_buffers: vec![&command_buffer],
                            wait_semaphores: None,
                            signal_semaphores: vec![&resources.rendering_semaphore],
                        };

                        queue_group.queues[0].submit(submission, Some(&resources.submission_fence));

                        let result = queue_group.queues[0].present(
                            &mut resources.surface,
                            surface_image,
                            Some(&resources.rendering_semaphore),
                        );

                        configure_swapchain |= result.is_err();

                        resources.device.destroy_framebuffer(framebuffer);
                    };
                };
            }
            _ => (),
        }
    });
}
