use std::{
    sync::{Arc, Mutex},
    thread,
};

use gilrs::EventType;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, SubpassContents},
    device::{Device, DeviceExtensions},
    framebuffer::{Framebuffer, FramebufferAbstract},
    image::{ImageUsage, SwapchainImage},
    instance::{Instance, PhysicalDevice},
    swapchain::{
        self, AcquireError, ColorSpace, FullscreenExclusive, PresentMode, SurfaceTransform,
        Swapchain, SwapchainCreationError,
    },
    sync::{self, FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowBuilder},
};

use crate::input::{
    gamepad_controller::{Controller, Gamepad},
    keyboard_controller::{Key::LogicKey, Keyboard},
};
use crate::scene::{
    main_menu::MainMenuScene, playground::GameScene, user_interface::Scene as UIScene,
};

mod input;
mod scene;

fn main() {
    let required_extensions = vulkano_win::required_extensions();
    let instance = Instance::new(None, &required_extensions, None).unwrap();
    for i in PhysicalDevice::enumerate(&instance) {
        println!("Device: {}", i.name());
    }
    let physical = PhysicalDevice::enumerate(&instance).next().unwrap();
    println!(
        "Using device: {} (type: {:?})",
        physical.name(),
        physical.ty()
    );

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

    // Init keyboard controller
    let keyboard_controller = Arc::new(Mutex::new(Keyboard::new()));
    // Init gamepad controller
    let mut gamepad_controller = Gamepad::new();

    let scene = {
        let my_maze = Arc::new(GameScene::create(device.clone(), swapchain.format()));
        my_maze.add_tank(input::Controller::Keyboard(
            Keyboard::create_sub_controller(
                &keyboard_controller,
                [
                    LogicKey(VirtualKeyCode::E),
                    LogicKey(VirtualKeyCode::D),
                    LogicKey(VirtualKeyCode::S),
                    LogicKey(VirtualKeyCode::F),
                ],
            ),
        ));
        my_maze.add_tank(input::Controller::Keyboard(
            Keyboard::create_sub_controller(
                &keyboard_controller,
                [
                    LogicKey(VirtualKeyCode::Up),
                    LogicKey(VirtualKeyCode::Down),
                    LogicKey(VirtualKeyCode::Left),
                    LogicKey(VirtualKeyCode::Right),
                ],
            ),
        ));

        let phy_maze = Arc::clone(&my_maze);
        thread::spawn(move || phy_maze.run_physic());

        Box::new(my_maze) as Box<dyn UIScene>
    };
    let mut framebuffers = window_size_dependent_setup(&images, &scene);

    let mut recreate_swapchain = false;

    let mut previous_frame_end = Some(sync::now(device.clone()).boxed());

    event_loop.run(move |event, _, control_flow| {
        while let Some(e) = gamepad_controller.next() {
            if let gilrs::Event {
                id,
                event: EventType::Connected,
                ..
            } = e
            {
                // if let GameScene(tank_scene) = scene.borrow() {
                //     println!("change tank: {}", id);
                //     tank_scene.add_tank(input::Controller::Gamepad(
                //         Controller::create_gamepad_controller(&mut gamepad_controller, id),
                //     ));
                // }
            }
        }
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
                // Marks the current state as swapchain needing to be recreate
                recreate_swapchain = true;
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                keyboard_controller.lock().unwrap().input_event(&input);
                if let KeyboardInput {
                    virtual_keycode: Some(key),
                    state,
                    ..
                } = input
                {
                    match key {
                        VirtualKeyCode::Escape => {
                            // 按下Esc后退出
                            *control_flow = ControlFlow::Exit;
                        }
                        VirtualKeyCode::F11 if state == ElementState::Pressed => {
                            let window = surface.window();
                            window.set_fullscreen(match window.fullscreen() {
                                None => Some(Fullscreen::Borderless(None)),
                                Some(_) => None,
                            });
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawEventsCleared => {
                previous_frame_end.as_mut().unwrap().cleanup_finished();
                // Recreate the swapchain when the game window resized
                if recreate_swapchain {
                    let dimensions: [u32; 2] = surface.window().inner_size().into();
                    let (new_swapchain, new_images) =
                        match swapchain.recreate_with_dimensions(dimensions) {
                            Ok(r) => r,
                            Err(SwapchainCreationError::UnsupportedDimensions) => return,
                            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                        };
                    swapchain = new_swapchain;
                    framebuffers = window_size_dependent_setup(&new_images, &scene);
                    recreate_swapchain = false;
                }

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

                let clear_values = vec![[1.0, 1.0, 1.0, 1.0].into()];

                let mut builder = AutoCommandBufferBuilder::primary_one_time_submit(
                    device.clone(),
                    queue.family(),
                )
                    .unwrap();

                let frame_size = {
                    let dim = framebuffers[image_num].dimensions();
                    [dim[0] as f32, dim[1] as f32]
                };
                builder
                    .begin_render_pass(
                        framebuffers[image_num].clone(),
                        SubpassContents::Inline,
                        clear_values,
                    )
                    .unwrap();
                scene.draw(&mut builder, frame_size).unwrap();
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
    scene: &Box<dyn UIScene>,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();
    let dimensions = [dimensions[0] as f32, dimensions[1] as f32];
    scene.reset_viewport(dimensions);

    images
        .iter()
        .map(|image| {
            Arc::new(
                Framebuffer::start(scene.render_pass())
                    .add(image.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>()
}
