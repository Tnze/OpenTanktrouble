use std::{error::Error, thread};

use winit::event::VirtualKeyCode;
use winit::window::Window;

use crate::input::Controller;
use crate::input::input_center::InputCenter;
use crate::input::keyboard_controller::Key;
use crate::scene::game_scene::{GameScene, Scene};

pub struct WindowState {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    size: winit::dpi::PhysicalSize<u32>,

    current_scene: Box<dyn Scene>,
    gilrs: gilrs::Gilrs,
    pub input_center: InputCenter,
}

impl WindowState {
    pub async fn new(window: &Window) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
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
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let input_center = InputCenter::new();
        let (game_scene, update_thread) = GameScene::new(&device, &sc_desc);
        let current_scene = Box::new(game_scene);

        let input_handler = input_center.input_handler();
        thread::spawn(move || update_thread(input_handler));

        current_scene.add_controller(Box::new(
            input_center.keyboard_controller.create_sub_controller([
                Key::LogicKey(VirtualKeyCode::E),
                Key::LogicKey(VirtualKeyCode::D),
                Key::LogicKey(VirtualKeyCode::S),
                Key::LogicKey(VirtualKeyCode::F),
            ]),
        ));

        let gilrs = gilrs::Gilrs::new()?;

        Ok(Self {
            surface,
            device,
            queue,
            sc_desc,
            swap_chain,
            size,
            current_scene,
            gilrs,
            input_center,
        })
    }

    pub fn resize(&mut self, new_size: Option<winit::dpi::PhysicalSize<u32>>) {
        let new_size = new_size.unwrap_or(self.size);
        self.size = new_size;
        self.sc_desc.width = new_size.width.max(1);
        self.sc_desc.height = new_size.height.max(1);
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SwapChainError> {
        let frame = self.swap_chain.get_current_frame()?.output;
        let frame_size = [self.sc_desc.width, self.sc_desc.height];
        self.current_scene
            .render(&self.device, &self.queue, &frame, frame_size)?;
        Ok(())
    }

    pub fn update(&mut self) {
        while let Some(ref event) = self.gilrs.next_event() {
            self.input_center.gamepad_event(&mut self.gilrs, event);
        }
    }

    pub fn add_controller(&self, ctrl: Box<dyn Controller>) {
        self.current_scene.add_controller(ctrl);
    }
}
