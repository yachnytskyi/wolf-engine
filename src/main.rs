use log::{error, info, warn};
use vulkanalia::loader::{LIBRARY, LibloadingLoader};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::EntryV1_1;
use vulkanalia::vk::{self, ExtDebugUtilsExtension};
use vulkanalia::window as vk_window;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};
use wolf_engine::error::AppError; // or your custom error

type Result<T> = std::result::Result<T, AppError>;

#[derive(Default)]
struct App {
    window: Option<Window>,
    entry: Option<Entry>,
    instance: Option<Instance>,
    debug: Option<vk::DebugUtilsMessengerEXT>,
}

// ... debug_callback and Drop impl stay the same

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(WindowAttributes::default())
            .expect("Failed to create window");

        let loader =
            unsafe { LibloadingLoader::new(LIBRARY) }.expect("Failed to load Vulkan loader");
        let entry = unsafe { Entry::new(loader) }.expect("Failed to create Vulkan entry");

        // === Extensions ===
        let mut exts: Vec<*const i8> = vk_window::get_required_instance_extensions(&window)
            .iter()
            .map(|e| e.as_ptr())
            .collect();
        exts.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        #[cfg(target_os = "macos")]
        exts.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());

        // === Layers ===
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
            layer_pointers.push(b"VK_LAYER_KHRONOS_validation\0".as_ptr() as *const i8);
            info!("✅ Validation layer enabled");
        }

        // === Instance create flags ===
        let mut flags = vk::InstanceCreateFlags::empty();
        #[cfg(target_os = "macos")]
        {
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }

        let supported = unsafe {
            entry
                .enumerate_instance_version()
                .expect("Failed to query Vulkan instance version")
        };

        // === App info ===
        let app_info = vk::ApplicationInfo::builder()
            .application_name(b"Wolf Engine\0")
            .engine_name(b"Wolf Engine\0")
            .api_version(supported);

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
            .user_callback(Some(debug_callback));

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&exts)
            .enabled_layer_names(&layer_pointers)
            .flags(flags)
            .push_next(&mut debug_ci);

        let instance =
            unsafe { entry.create_instance(&create_info, None) }.expect("vkCreateInstance failed");
        info!("🎉 Vulkan instance ready");

        let debug = unsafe { instance.create_debug_utils_messenger_ext(&debug_ci, None) }
            .expect("debug utils messenger");

        self.window = Some(window);
        self.entry = Some(entry);
        self.instance = Some(instance);
        self.debug = Some(debug);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
}

unsafe extern "system" fn debug_callback(
    sev: vk::DebugUtilsMessageSeverityFlagsEXT,
    ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _ud: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = unsafe { std::ffi::CStr::from_ptr((*data).message).to_string_lossy() };
    if sev.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        error!("[{ty:?}] {message}");
    } else if sev.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        warn!("[{ty:?}] {message}");
    } else {
        info!("[{ty:?}] {message}");
    }
    vk::FALSE
}

impl Drop for App {
    fn drop(&mut self) {
        if let (Some(instance), Some(debug)) = (&self.instance, &self.debug) {
            unsafe {
                instance.destroy_debug_utils_messenger_ext(*debug, None);
            }
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let mut app = App::default();
    let event_loop = EventLoop::new()?; // Handles error
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app)?; // Handles error
    Ok(())
}
