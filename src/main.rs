use std::error::Error;

use futures::executor::block_on;
use log::{debug, error, info, log_enabled};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, WindowBuilder},
};

use crate::input::{
    gamepad_controller::{Controller, Gamepad},
    keyboard_controller::{Key::LogicKey, Keyboard},
};

mod input;
mod scene;
mod window;

fn abort(err: Box<dyn Error>) -> ! {
    error!("Error in main: {}", err);
    msgbox::create("Error", &*err.to_string(), msgbox::IconType::Error);
    panic!("{}", err);
}

fn main() {
    // Init logger
    env_logger::init();
    // Create window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .unwrap_or_else(|e| abort(Box::new(e)));
    info!("Successfully create window");
    let mut state = block_on(window::WindowState::new(&window)).unwrap_or_else(|e| abort(e));

    // Init controller
    let mut keyboard_controller = Keyboard::new();
    let mut gamepad_controller = Gamepad::new();

    event_loop.run(move |event, _, control_flow| {
        while let Some(_e) = gamepad_controller.next() {}
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::KeyboardInput { input, .. } => match input {
                    // Press Esc to quit
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    // Press F11 to enter fullscreen mode
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::F11),
                        ..
                    } => {
                        let fullscreen_mode = match window.fullscreen() {
                            None => Some(Fullscreen::Borderless(None)),
                            Some(_) => None,
                        };
                        info!("Fullscreen mode is changing to {:?}", fullscreen_mode);
                        window.set_fullscreen(fullscreen_mode);
                    }
                    // Other keyboard event
                    _ => keyboard_controller.input_event(&input),
                },
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(physical_size) => state.resize(Some(*physical_size)),
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    state.resize(Some(**new_inner_size))
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                match state.render() {
                    Ok(_) => {}
                    // Recreate the swap_chain if lost
                    Err(wgpu::SwapChainError::Lost) => state.resize(None),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SwapChainError::OutOfMemory) => {
                        error!("SwapChain out of memory");
                        *control_flow = ControlFlow::Exit
                    }
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => error!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => (),
        }
    });
}
