use std::{error::Error as StdError, fmt};

use libloading::Error as LibloadingError;
use vulkanalia::loader::LoaderError;
use vulkanalia::vk;
use winit::error::EventLoopError;

/// Application-wide error type.
#[derive(Debug)]
pub enum AppError {
    Lib(LibloadingError),         // dynamic library loading errors
    Vk(vk::Result, &'static str), // Vulkan error + context string
    Winit(EventLoopError),        // winit event loop errors
    Loader(Box<dyn LoaderError>), // Vulkanalia loader errors (trait object)
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lib(e) => write!(f, "libloading: {e}"),
            Self::Vk(result, ctx) => {
                write!(f, "Vulkan error: {:?} (context: {})", result, ctx)
            }
            Self::Winit(e) => write!(f, "winit: {e}"),
            Self::Loader(e) => write!(f, "loader error: {}", e),
        }
    }
}

impl StdError for AppError {}

/// Alias used in other modules.
pub type Result<T> = std::result::Result<T, AppError>;

impl From<LibloadingError> for AppError {
    fn from(e: LibloadingError) -> Self {
        Self::Lib(e)
    }
}

impl From<vk::Result> for AppError {
    fn from(e: vk::Result) -> Self {
        Self::Vk(e, "unspecified")
    }
}

impl From<EventLoopError> for AppError {
    fn from(e: EventLoopError) -> Self {
        Self::Winit(e)
    }
}

impl From<Box<dyn LoaderError>> for AppError {
    fn from(e: Box<dyn LoaderError>) -> Self {
        Self::Loader(e)
    }
}
