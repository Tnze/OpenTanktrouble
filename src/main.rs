mod keyboard_controller;
mod main_menu;
mod maze;

use crate::keyboard_controller::{KeyboardController, Key::{LogicKey, PhysicKey}};
use cgmath::Vector2;
use std::sync::Mutex;
use std::{sync::Arc, thread, time::Instant};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool},
    command_buffer::{AutoCommandBufferBuilder, DynamicState},
    descriptor::descriptor_set::PersistentDescriptorSet,
    device::{Device, DeviceExtensions},
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{ImageUsage, SwapchainImage},
    instance::{Instance, PhysicalDevice},
    pipeline::{viewport::Viewport, GraphicsPipeline},
    swapchain,
    swapchain::{
        AcquireError, ColorSpace, FullscreenExclusive, PresentMode, SurfaceTransform, Swapchain,
        SwapchainCreationError,
    },
    sync,
    sync::{FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowBuilder},
};

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
				#version 450 core
				layout(set = 0, binding = 0) uniform Data {
                    vec2 trans;
                } uniforms;
				layout(location = 0) in vec2 position;

				void main() {
					gl_Position = vec4(position + uniforms.trans, 0.0, 1.0);
				}
			"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
				#version 450 core
				layout(location = 0) out vec4 f_color;
				void main() {
					f_color = vec4(1.0, 0.0, 0.0, 1.0);
				}
			"
    }
}

fn main() {
    let required_extensions = vulkano_win::required_extensions();
    let instance = Instance::new(None, &required_extensions, None).unwrap();
    for i in PhysicalDevice::enumerate(&instance) {
        println!("Device: {}", i.name());
    }
    let physical = PhysicalDevice::enumerate(&instance).next().unwrap();
    println!("Using device: {} (type: {:?})", physical.name(), physical.ty());

    let event_loop = EventLoop::new();
    let surface = WindowBuilder::new()
        .build_vk_surface(&event_loop, instance.clone())
        .unwrap();

    let (device, mut queues) = {
        let queue_family = physical
            .queue_families()
            .find(|&q| q.supports_graphics() && surface.is_supported(q).unwrap_or(false))
            .unwrap();
        let device_ext = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };
        Device::new(
            physical,
            physical.supported_features(),
            &device_ext,
            [(queue_family, 0.5)].iter().cloned(),
        )
            .unwrap()
    };

    let queue = queues.next().unwrap();

    let (mut swapchain, images) = {
        let caps = surface.capabilities(physical).unwrap();
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();

        let format = caps.supported_formats[0].0;

        let dimensions: [u32; 2] = surface.window().inner_size().into();

        Swapchain::new(
            Arc::clone(&device),
            surface.clone(),
            caps.min_image_count,
            format,
            dimensions,
            1,
            ImageUsage::color_attachment(),
            &queue,
            SurfaceTransform::Identity,
            alpha,
            PresentMode::Fifo,
            FullscreenExclusive::Default,
            true,
            ColorSpace::SrgbNonLinear,
        )
            .unwrap()
    };
    // We now create a buffer that will store the shape of our triangle.

    let (top, left, width, height) = (0.3, -0.7, 0.4, 0.2);

    let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(device.clone(), BufferUsage::all());
    let vs = vs::Shader::load(Arc::clone(&device)).unwrap();
    let fs = fs::Shader::load(Arc::clone(&device)).unwrap();

    let render_pass = Arc::new(
        vulkano::single_pass_renderpass!(
            Arc::clone(&device),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.format(),
                    // TODO:
                    samples: 1,
                }
            },
            pass: {color: [color],  depth_stencil: {}}
        )
            .unwrap(),
    );
    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer()
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ())
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(Arc::clone(&device))
            .unwrap(),
    );
    let mut dynamic_state = DynamicState {
        line_width: None,
        viewports: None,
        scissors: None,
        compare_mask: None,
        write_mask: None,
        reference: None,
    };
    let mut framebuffers =
        window_size_dependent_setup(&images, render_pass.clone(), &mut dynamic_state);

    let mut recreate_swapchain = false;

    #[derive(Default, Copy, Clone)]
    struct Vertex {
        position: (f32, f32),
    }
    vulkano::impl_vertex!(Vertex, position);

    let vb = {
        CpuAccessibleBuffer::from_iter(
            Arc::clone(&device),
            BufferUsage::all(),
            false,
            [
                Vertex { position: (left, top) },
                Vertex { position: (left + width, top) },
                Vertex { position: (left + width, top + height) },
                Vertex { position: (left, top) },
                Vertex { position: (left + width, top + height) },
                Vertex { position: (left, top + height) },
            ]
                .iter()
                .cloned(),
        )
            .unwrap()
    };

    let time_start = Instant::now();
    let mut previous_frame_end = Some(sync::now(Arc::clone(&device)).boxed());

    // 初始化键盘控制器
    let keyboard_controller = Arc::new(Mutex::new(keyboard_controller::KeyboardController::new()));

    let sub_controller = Box::new(KeyboardController::create_sub_controller(
        &keyboard_controller,
        [
            LogicKey(VirtualKeyCode::E),
            LogicKey(VirtualKeyCode::D),
            LogicKey(VirtualKeyCode::S),
            LogicKey(VirtualKeyCode::F),
        ],
    ));

    let mut my_maze = maze::GameScene::create();
    my_maze.add_tank(sub_controller);
    let my_maze = Arc::new(my_maze);
    {
        let my_maze = Arc::clone(&my_maze);
        thread::spawn(move || my_maze.run_physic());
    }

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                recreate_swapchain = true;
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                keyboard_controller.lock().unwrap().input_event(&input);
                // 按下Esc后退出
                if let KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Escape),
                    ..
                } = input
                {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::RedrawEventsCleared => {
                previous_frame_end.as_mut().unwrap().cleanup_finished();

                if recreate_swapchain {
                    let dimensions: [u32; 2] = surface.window().inner_size().into();
                    let (new_swapchain, new_images) =
                        match swapchain.recreate_with_dimensions(dimensions) {
                            Ok(r) => r,
                            Err(SwapchainCreationError::UnsupportedDimensions) => return,
                            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                        };

                    swapchain = new_swapchain;
                    framebuffers = window_size_dependent_setup(
                        &new_images,
                        render_pass.clone(),
                        &mut dynamic_state,
                    );
                    recreate_swapchain = false;
                }

                let uniform_buffer_subbuffer = {
                    let elapsed = time_start.elapsed();
                    let elapsed =
                        elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 / 1_000_000_000.0;
                    let trans = Vector2::new(0.5 + elapsed.sin() * 0.5, -0.5 + elapsed.cos() * 0.5);
                    let uniform_data = vs::ty::Data {
                        trans: trans.into(),
                    };

                    uniform_buffer.next(uniform_data).unwrap()
                };

                let layout = pipeline.layout().descriptor_set_layout(0).unwrap();
                let set = Arc::new(
                    PersistentDescriptorSet::start(layout.clone())
                        .add_buffer(uniform_buffer_subbuffer)
                        .unwrap()
                        .build()
                        .unwrap(),
                );

                let (image_num, suboptimal, acquire_future) =
                    match swapchain::acquire_next_image(swapchain.clone(), None) {
                        Ok(r) => r,
                        Err(AcquireError::OutOfDate) => {
                            recreate_swapchain = true;
                            return;
                        }
                        Err(e) => panic!("Failed to acquire next image: {:?}", e),
                    };

                if suboptimal {
                    recreate_swapchain = true;
                }

                let clear_values = vec![[0.0, 0.0, 1.0, 1.0].into()];

                let mut builder = AutoCommandBufferBuilder::primary_one_time_submit(
                    device.clone(),
                    queue.family(),
                )
                    .unwrap();

                builder.begin_render_pass(framebuffers[image_num].clone(), false, clear_values).unwrap();
                my_maze.draw(&mut builder, pipeline.clone(), &dynamic_state, vb.clone(), set.clone()).unwrap();
                builder.end_render_pass().unwrap();


                // Finish building the command buffer by calling `build`.
                let command_buffer = builder.build().unwrap();

                let future = previous_frame_end
                    .take()
                    .unwrap()
                    .join(acquire_future)
                    .then_execute(queue.clone(), command_buffer)
                    .unwrap()
                    .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
                    .then_signal_fence_and_flush();

                match future {
                    Ok(future) => {
                        previous_frame_end = Some(future.boxed());
                    }
                    Err(FlushError::OutOfDate) => {
                        recreate_swapchain = true;
                        previous_frame_end = Some(sync::now(device.clone()).boxed());
                    }
                    Err(e) => {
                        println!("Failed to flush future: {:?}", e);
                        previous_frame_end = Some(sync::now(device.clone()).boxed());
                    }
                }
            }
            _ => (),
        }
    });
}

/// This method is called once during initialization, then again whenever the window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0,
    };
    dynamic_state.viewports = Some(vec![viewport]);

    images
        .iter()
        .map(|image| {
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(image.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>()
}
