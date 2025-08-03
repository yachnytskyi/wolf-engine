// src/app.rs

use crate::core::renderer::api::Renderer;
use crate::error::Result;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{WindowAttributes, WindowId},
};

pub struct App<R: Renderer + Default> {
    renderer: R,
    window: Option<winit::window::Window>,
}

impl<R: Renderer + Default> ApplicationHandler for App<R> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(WindowAttributes::default())
            .expect("Failed to create window");

        // keep window alive in App
        self.window = Some(window);

        // Safe to unwrap because we just set it
        let window_ref = self.window.as_ref().unwrap();

        self.renderer
            .initialize(window_ref, event_loop)
            .expect("Renderer initialization failed");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.renderer.window_event(event_loop, id, &event);
    }
}

impl<R: Renderer + Default> App<R> {
    pub fn run() -> Result<()> {
        let mut app = App {
            renderer: R::default(),
            window: None,
        };

        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut app)?;
        app.renderer.shutdown();
        Ok(())
    }
}
