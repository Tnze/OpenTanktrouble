use std::{error::Error, process::exit};

#[allow(unused_imports)]
use log::{debug, error, info};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowBuilder},
};

use crate::input::{
    Controller,
    gamepad_controller::Gamepad,
    keyboard_controller::{Key::LogicKey, Keyboard},
};
use crate::scene::playground::{GameScene, Scene};

mod input;
mod scene;

pub struct WindowState {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    size: winit::dpi::PhysicalSize<u32>,

    current_scene: Box<dyn Scene>,
}

impl WindowState {
    pub async fn new(window: &Window) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();

        #[cfg(not(target_arch = "wasm32"))]
            let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        #[cfg(target_arch = "wasm32")]
            let instance = wgpu::Instance::new(wgpu::BackendBit::all());

        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or("No compatible adapters are found")?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Main device"),
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None, // Trace path
            )
            .await?;
        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format: adapter.get_swap_chain_preferred_format(&surface),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        let current_scene = Box::new(GameScene::new(&device, &sc_desc));

        Ok(Self {
            surface,
            device,
            queue,
            sc_desc,
            swap_chain,
            size,
            current_scene,
        })
    }

    pub fn resize(&mut self, new_size: Option<winit::dpi::PhysicalSize<u32>>) {
        let new_size = new_size.unwrap_or(self.size);
        self.size = new_size;
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SwapChainError> {
        let frame = self.swap_chain.get_current_frame()?.output;
        let frame_size = (self.sc_desc.width, self.sc_desc.height);
        self.current_scene
            .render(&self.device, &self.queue, &frame, frame_size)?;
        Ok(())
    }

    pub fn add_controller(&self, ctrl: Controller) {
        self.current_scene.add_controller(ctrl);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn abort(err: &dyn Error) -> ! {
    error!("Error in main: {}", err);
    msgbox::create("Error", &*err.to_string(), msgbox::IconType::Error)
        .unwrap_or_else(|err2| error!("Display message-box error: {:?}", err2));
    exit(2);
}

#[cfg(target_arch = "wasm32")]
fn abort(err: &dyn Error) -> ! {
    use wasm_bindgen::prelude::*;
    extern crate wasm_bindgen;
    #[wasm_bindgen]
    extern "C" {
        fn alert(s: &str);
    }
    error!("Error in main: {}", err);
    alert(&*err.to_string());
    exit(2);
}

fn main_loop(
    event_loop: EventLoop<()>,
    window: Window,
    mut window_state: WindowState,
    keyboard_controller: Keyboard,
    mut gamepad_controller: Gamepad,
) {
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

pub fn run() {
    // Create window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .unwrap_or_else(|e| abort(&e));
    info!("Successfully create window");

    // Init controller
    let keyboard_controller = Keyboard::new();
    let mut gamepad_controller = Gamepad::new();

    #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;
            console_log::init().expect("could not initialize logger");
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            // On wasm, append the canvas to the document body
            web_sys::window()
                .and_then(|win| win.document())
                .and_then(|doc| doc.body())
                .and_then(|body| {
                    body.append_child(&web_sys::Element::from(window.canvas()))
                        .ok()
                })
                .expect("couldn't append canvas to document body");
        }

    #[cfg(not(target_arch = "wasm32"))]
        {
            use futures::executor::block_on;
            let mut window_state =
                block_on(WindowState::new(&window)).unwrap_or_else(|e| abort(e.as_ref()));
            main_loop(
                event_loop,
                window,
                window_state,
                keyboard_controller,
                gamepad_controller,
            );
        }
    #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let mut window_state = WindowState::new(&window)
                    .await
                    .unwrap_or_else(|e| abort(e.as_ref()));
                main_loop(
                    event_loop,
                    window,
                    window_state,
                    keyboard_controller,
                    gamepad_controller,
                );
            });
        }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run();
}
