pub mod error;

use crate::error::Result;
use winit::{
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

/// Common rendering API used by the engine and backends.
pub trait Renderer {
    /// Initialize the renderer with window and event loop.
    fn initialize(&mut self, window: &Window, event_loop: &ActiveEventLoop) -> Result<()>;

    /// Handle window events (resize, close, etc).
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent);

    /// Draw a frame (stub for now).
    fn render(&mut self) -> Result<()>;
}
