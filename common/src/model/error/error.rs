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
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lib(error) => write!(formatter, "libloading: {error}"),
            Self::Vk(result, ctx) => {
                write!(formatter, "Vulkan error: {:?} (context: {})", result, ctx)
            }
            Self::Winit(error) => write!(formatter, "winit: {error}"),
            Self::Loader(error) => write!(formatter, "loader error: {}", error),
        }
    }
}

impl StdError for AppError {}

/// Alias used in other modules.
pub type Result<T> = std::result::Result<T, AppError>;

impl From<LibloadingError> for AppError {
    fn from(error: LibloadingError) -> Self {
        Self::Lib(error)
    }
}

impl From<vk::Result> for AppError {
    fn from(error: vk::Result) -> Self {
        Self::Vk(error, "unspecified")
    }
}

impl From<EventLoopError> for AppError {
    fn from(error: EventLoopError) -> Self {
        Self::Winit(error)
    }
}

impl From<Box<dyn LoaderError>> for AppError {
    fn from(error: Box<dyn LoaderError>) -> Self {
        Self::Loader(error)
    }
}
