pub mod renderer {
    pub mod backend {
        #[cfg(feature = "vulkan")]
        pub use backend_vulkan::VulkanRenderer as SelectedRenderer;

        #[cfg(not(feature = "vulkan"))]
        compile_error!("No renderer backend selected (enable the `vulkan` feature).");
    }
}

pub use renderer::backend::SelectedRenderer;
