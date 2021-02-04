use std::{error::Error, process::exit};

use futures::executor::block_on;
#[allow(unused_imports)]
use log::{debug, error, info};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, WindowBuilder},
};

use crate::input::{
    gamepad_controller::Gamepad,
    keyboard_controller::{Key::LogicKey, Keyboard},
};

mod input;
mod scene;
mod window;

fn abort(err: &dyn Error) -> ! {
    error!("Error in main: {}", err);
    msgbox::create("Error", &*err.to_string(), msgbox::IconType::Error)
        .unwrap_or_else(|err2| error!("Display message-box error: {:?}", err2));
    exit(2);
}

fn main() {
    // Init logger
    env_logger::init();
    // Create window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .unwrap_or_else(|e| abort(&e));
    info!("Successfully create window");
    let mut window_state =
        block_on(window::WindowState::new(&window)).unwrap_or_else(|e| abort(e.as_ref()));

    // Init controller
    let keyboard_controller = Keyboard::new();
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
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Q),
                        ..
                    } => window_state.add_controller(input::Controller::Keyboard(
                        keyboard_controller.create_sub_controller([
                            LogicKey(VirtualKeyCode::E),
                            LogicKey(VirtualKeyCode::D),
                            LogicKey(VirtualKeyCode::S),
                            LogicKey(VirtualKeyCode::F),
                        ]),
                    )),
                    // Other keyboard event
                    _ => keyboard_controller.input_event(&input),
                },
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(physical_size) => window_state.resize(Some(*physical_size)),
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    window_state.resize(Some(**new_inner_size))
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                use wgpu::SwapChainError::{Lost, OutOfMemory};

                match window_state.render() {
                    Ok(_) => {}
                    // Recreate the swap_chain if lost
                    Err(Lost) => window_state.resize(None),
                    // The system is out of memory, we should probably quit
                    Err(OutOfMemory) => abort(&OutOfMemory),
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
