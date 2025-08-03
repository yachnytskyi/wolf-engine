mod app;
mod core;
mod error;

use crate::core::renderer::vulkan::VulkanRenderer;
use app::App;

fn main() -> error::Result<()> {
    env_logger::init();

    // Here you choose your backend; could be OpenGLRenderer etc in future
    App::<VulkanRenderer>::run()
}
