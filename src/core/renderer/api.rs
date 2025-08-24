use crate::error::Result;
use winit::{event::WindowEvent, event_loop::ActiveEventLoop, window::Window, window::WindowId};

pub trait Renderer {
    /// Initialize the renderer with window and event loop.
    fn initialize(&mut self, window: &Window, event_loop: &ActiveEventLoop) -> Result<()>;

    /// Handle window events (resize, close, etc).
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent);

    /// Draw a frame (stub for now, you can expand later).
    fn render(&mut self) -> Result<()>;
}
