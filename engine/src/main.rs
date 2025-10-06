mod app;
mod core;

use app::App;
use common::model::error::Result;
use core::SelectedRenderer;

fn main() -> Result<()> {
    env_logger::init();
    App::<SelectedRenderer>::run()
}
