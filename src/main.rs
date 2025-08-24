// src/main.rs
mod app;
mod core;
mod error;

use crate::core::renderer::backend::SelectedRenderer;
use app::App;

fn main() -> error::Result<()> {
    env_logger::init();

    App::<SelectedRenderer>::run()
}
