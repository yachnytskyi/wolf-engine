use std::{error::Error as StdError, fmt};

use libloading::Error as LibloadingError;
use vulkanalia::vk;
use winit::error::EventLoopError;

#[derive(Debug)]
pub enum AppError {
    Lib(LibloadingError),         // dynamic-lib load failures
    Vk(vk::Result, &'static str), // Vulkan API result + context
    Winit(EventLoopError),        // winit’s EventLoopError
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lib(e) => write!(f, "libloading: {e}"),
            Self::Vk(result, ctx) => write!(f, "Vulkan error: {:?} (context: {})", result, ctx),
            Self::Winit(e) => write!(f, "winit: {e}"),
        }
    }
}

impl StdError for AppError {}

/// `?` conversions
impl From<LibloadingError> for AppError {
    fn from(e: LibloadingError) -> Self {
        Self::Lib(e)
    }
}
impl From<vk::Result> for AppError {
    fn from(e: vk::Result) -> Self {
        Self::Vk(e, "unspecified") // fallback context
    }
}
impl From<EventLoopError> for AppError {
    fn from(e: EventLoopError) -> Self {
        Self::Winit(e)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

pub trait VkResultExt {
    fn into_app_error(self, context: &'static str) -> Result<()>;
}

impl VkResultExt for vk::Result {
    fn into_app_error(self, context: &'static str) -> Result<()> {
        if self == vk::Result::SUCCESS {
            Ok(())
        } else {
            Err(AppError::Vk(self, context))
        }
    }
}
