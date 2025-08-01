// --- Standard logging macros: error!, info!, warn! ---
use log::{error, info, warn};

// --- Vulkanalia imports for Vulkan loader, entry, and window integration ---
use vulkanalia::loader::{LIBRARY, LibloadingLoader};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::EntryV1_1; // For enumerate_instance_version (Vulkan 1.1+)
use vulkanalia::vk::{self, ExtDebugUtilsExtension}; // For validation/debug utils
use vulkanalia::window as vk_window;

// --- winit: cross-platform window/event loop ---
use winit::{
    application::ApplicationHandler, // Trait for event callbacks (like a "window app class")
    event::WindowEvent,              // Event enum (close, resize, etc)
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, // Event loop and control flow
    window::{Window, WindowAttributes, WindowId}, // Window types & creation
};

// --- Custom error type for robust error handling ---
use wolf_engine::error::AppError; // Or whatever your custom error type is
type Result<T> = std::result::Result<T, AppError>; // Convenience alias

//---------------------------------------------------
// Core struct for your application ("App state")
#[derive(Default)]
struct App {
    window: Option<Window>,                    // Store the main window
    entry: Option<Entry>,                      // Vulkan entry point (dlopen/libloading)
    instance: Option<Instance>,                // Vulkan instance (handle to the Vulkan API)
    debug: Option<vk::DebugUtilsMessengerEXT>, // Debug messenger (for validation/debug output)
}

//---------------------------------------------------
// Debug callback: handles Vulkan validation messages (called by the driver)
unsafe extern "system" fn debug_callback(
    sev: vk::DebugUtilsMessageSeverityFlagsEXT, // Message severity (error, warning, info)
    ty: vk::DebugUtilsMessageTypeFlagsEXT,      // Message type (general, validation, performance)
    data: *const vk::DebugUtilsMessengerCallbackDataEXT, // The actual message data
    _ud: *mut std::ffi::c_void,                 // User data (unused here)
) -> vk::Bool32 {
    let message = unsafe { std::ffi::CStr::from_ptr((*data).message).to_string_lossy() };
    // Log with different levels depending on severity
    if sev.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        error!("[{ty:?}] {message}");
    } else if sev.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        warn!("[{ty:?}] {message}");
    } else {
        info!("[{ty:?}] {message}");
    }
    vk::FALSE // Don't abort
}

//---------------------------------------------------
// Main application logic: window & Vulkan initialization
impl ApplicationHandler for App {
    // Called when the app resumes (window becomes visible or app starts)
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // --- 1. Create the main window ---
        let window = event_loop
            .create_window(WindowAttributes::default())
            .expect("Failed to create window");
        // Now window is alive and we can use it for Vulkan surface/ext detection.

        // --- 2. Load Vulkan library and create entry point ---
        let loader =
            unsafe { LibloadingLoader::new(LIBRARY) }.expect("Failed to load Vulkan loader");
        let entry = unsafe { Entry::new(loader) }.expect("Failed to create Vulkan entry");
        // The "entry" is how we call instance-level Vulkan functions.

        // --- 3. Gather required extensions ---
        // Core extensions required by the window system (surface presentation)
        let mut exts: Vec<*const i8> = vk_window::get_required_instance_extensions(&window)
            .iter()
            .map(|e| e.as_ptr())
            .collect();
        // Debug utils extension (for validation layers and debug callbacks)
        exts.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        // On macOS (MoltenVK), we also need portability enumeration for full driver support:
        #[cfg(target_os = "macos")]
        exts.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());

        // --- 4. Enable validation layers if available ---
        // Check which layers are supported by this Vulkan runtime:
        let available_layers: Vec<String> = unsafe {
            entry
                .enumerate_instance_layer_properties()
                .unwrap()
                .iter()
                .map(|p| {
                    std::ffi::CStr::from_ptr(p.layer_name.as_ptr())
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        };
        let mut layer_pointers = Vec::<*const i8>::new();
        if available_layers
            .iter()
            .any(|l| l == "VK_LAYER_KHRONOS_validation")
        {
            // Add validation layer if present (critical for debugging!)
            layer_pointers.push(b"VK_LAYER_KHRONOS_validation\0".as_ptr() as *const i8);
            info!("✅ Validation layer enabled");
        }

        // --- 5. Instance creation flags ---
        // macOS requires the portability flag to enumerate all devices
        let mut flags = vk::InstanceCreateFlags::empty();
        #[cfg(target_os = "macos")]
        {
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }

        // --- 6. Query Vulkan API version supported by the driver ---
        let supported = unsafe {
            entry
                .enumerate_instance_version()
                .expect("Failed to query Vulkan instance version")
        };

        // --- 7. Application info struct ---
        let app_info = vk::ApplicationInfo::builder()
            .application_name(b"Wolf Engine\0") // App name
            .engine_name(b"Wolf Engine\0") // Engine name
            .api_version(supported); // Vulkan version supported by the driver

        // --- 8. Debug utils (validation) create info ---
        let mut debug_ci = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .user_callback(Some(debug_callback)); // Hook up the callback

        // --- 9. Compose instance creation info ---
        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&exts)
            .enabled_layer_names(&layer_pointers)
            .flags(flags)
            .push_next(&mut debug_ci); // Add debug create info to the chain

        // --- 10. Create Vulkan instance ---
        let instance =
            unsafe { entry.create_instance(&create_info, None) }.expect("vkCreateInstance failed");
        info!("🎉 Vulkan instance ready");

        // --- 11. Create debug messenger (runtime validation output) ---
        let debug = unsafe { instance.create_debug_utils_messenger_ext(&debug_ci, None) }
            .expect("debug utils messenger");

        // --- 12. Store Vulkan and window handles in the App struct ---
        self.window = Some(window);
        self.entry = Some(entry);
        self.instance = Some(instance);
        self.debug = Some(debug);
    }

    // Called on every window event (close, resize, etc)
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit(); // Close app when requested
        }
    }
}

//---------------------------------------------------
// Ensures debug messenger is destroyed on App drop
impl Drop for App {
    fn drop(&mut self) {
        if let (Some(instance), Some(debug)) = (&self.instance, &self.debug) {
            unsafe {
                instance.destroy_debug_utils_messenger_ext(*debug, None);
            }
        }
    }
}

//---------------------------------------------------
// Main entry point: sets up logger, runs window/event loop, error handling
fn main() -> Result<()> {
    env_logger::init(); // Set up logging (so info! and error! work)
    let mut app = App::default(); // Start with empty app state
    let event_loop = EventLoop::new()?; // Create a new event loop (can fail, so `?`)
    event_loop.set_control_flow(ControlFlow::Poll); // Poll for events continuously
    event_loop.run_app(&mut app)?; // Run the app; handle errors gracefully
    Ok(())
}
