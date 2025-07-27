//! src/error.rs
use std::{error::Error as StdError, fmt};

use libloading::Error as LibloadingError;      // ← real loader error
use vulkanalia::vk;
use winit::error::EventLoopError;

#[derive(Debug)]
pub enum AppError {
    Lib(LibloadingError),       // dynamic‑lib load failures
    Vk(vk::ErrorCode),          // Vulkan API result codes
    Winit(EventLoopError),      // winit’s EventLoopError
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lib(e)   => write!(f, "libloading: {e}"),
            Self::Vk(e)    => write!(f, "Vulkan error: {e:?}"),
            Self::Winit(e) => write!(f, "winit: {e}"),
        }
    }
}

impl StdError for AppError {}

/// `?` conversions
impl From<LibloadingError>  for AppError { fn from(e: LibloadingError) -> Self { Self::Lib(e) } }
impl From<vk::ErrorCode>    for AppError { fn from(e: vk::ErrorCode)    -> Self { Self::Vk(e) } }
impl From<EventLoopError>   for AppError { fn from(e: EventLoopError)   -> Self { Self::Winit(e) } }
