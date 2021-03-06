use std::{error::Error, sync::Arc, thread};

use crossbeam_channel::{bounded, Receiver, Sender, unbounded};
#[allow(unused_imports)]
use log::{debug, error, info, log_enabled};
use winit::window::Window;

use crate::input::{
    Controller,
    input_center::{InputCenter, InputEventSender},
    keyboard_controller::Key,
};
use crate::scene::{prepare_scene, SceneRender, SceneUpdater};

pub struct WindowState {
    surface: wgpu::Surface,
    device: Arc<wgpu::Device>,
    queue: wgpu::Queue,
    sc_desc: wgpu::SwapChainDescriptor,
    swap_chain: wgpu::SwapChain,
    size: winit::dpi::PhysicalSize<u32>,

    current_scene: Box<dyn SceneRender + Sync + Send>,
    update_scene_chan: Receiver<Box<dyn SceneRender + Sync + Send>>,
    gilrs: gilrs::Gilrs,
    pub input_event_sender: InputEventSender,
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
        let device = Arc::new(device);
        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let (update_scene_sender, update_scene_chan) = unbounded();
        let (input_event_sender_sender, input_event_sender_receiver) = bounded(1);
        {
            let device = device.clone();
            let format = sc_desc.format;
            thread::spawn(move || {
                debug!("Update thread start");
                let (input_center, input_event_sender) = InputCenter::new();
                input_event_sender_sender.send(input_event_sender).unwrap();

                let (render, updater) = prepare_scene::new(device.clone(), format);
                let render: Box<dyn SceneRender + Sync + std::marker::Send> = Box::new(render);
                update_scene_sender.send(render).unwrap();
                let mut updater: Box<dyn SceneUpdater> = Box::new(updater);

                while let Some((render_n, updater_n)) =
                updater.update(device.as_ref(), format, &input_center)
                {
                    update_scene_sender.send(render_n).unwrap();
                    updater = updater_n;
                }
                debug!("Update thread stop");
            });
        }
        let input_event_sender = input_event_sender_receiver.recv()?;
        let current_scene = update_scene_chan.recv()?;

        let gilrs = gilrs::Gilrs::new()?;

        Ok(Self {
            surface,
            device,
            queue,
            sc_desc,
            swap_chain,
            size,
            current_scene,
            update_scene_chan,
            gilrs,
            input_event_sender,
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
            self.input_event_sender.gamepad_event(&mut self.gilrs, event);
        }
        if let Ok(scene) = self.update_scene_chan.try_recv() {
            self.current_scene = scene;
        }
    }
}
