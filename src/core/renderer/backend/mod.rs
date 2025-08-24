// src/core/renderer/backend/mod.rs
#[cfg(feature = "vulkan")]
pub mod vulkan;

// Re-export the selected backend under a common name:
#[cfg(feature = "vulkan")]
pub use vulkan::VulkanRenderer as SelectedRenderer;
