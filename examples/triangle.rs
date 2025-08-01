//! examples/triangle.rs – window + validation layer + debug‑utils (vulkanalia 0.29)

use log::{error, info, warn};
use vulkanalia::{
    loader::LibloadingLoader,
    prelude::v1_0::*,
    vk::{self, EntryV1_1, ExtDebugUtilsExtension},
    window as vk_window,
};

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use wolf_engine::error::AppError;
type Result<T> = std::result::Result<T, AppError>;

/// single static C‑string stays alive for entire run
const VALIDATION_LAYER: &[u8] = b"VK_LAYER_KHRONOS_validation\0";

#[cfg(target_os = "macos")]
const VK_LIB: &str = "libvulkan.1.dylib";
#[cfg(target_os = "windows")]
const VK_LIB: &str = "vulkan-1.dll";
#[cfg(all(unix, not(target_os = "macos")))]
const VK_LIB: &str = "libvulkan.so.1";

#[derive(Default)]
struct App {
    window: Option<Window>,
    entry: Option<vulkanalia::Entry>,
    instance: Option<vulkanalia::Instance>,
    debug: Option<vk::DebugUtilsMessengerEXT>,
}

/// -------- debug callback ------------------------------------------------
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

/// -------- winit handler -------------------------------------------------
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // 1) window -------------------------------------------------------
        let window = event_loop
            .create_window(Window::default_attributes())
            .expect("create window");

        // 2) Vulkan entry -------------------------------------------------
        let loader = unsafe { LibloadingLoader::new(VK_LIB) }.expect("load Vulkan loader");
        let entry = unsafe { vulkanalia::Entry::new(loader) }.expect("create Entry");

        // 3) extension & layer lists --------------------------------------
        let mut exts: Vec<*const i8> = vk_window::get_required_instance_extensions(&window)
            .iter()
            .map(|e| e.as_ptr())
            .collect();
        exts.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        #[cfg(target_os = "macos")]
        exts.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());

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
            layer_pointers.push(VALIDATION_LAYER.as_ptr() as *const i8);
            info!("✅ Validation layer enabled");
        }

        // 4) instance -----------------------------------------------------
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

        let app_info = vk::ApplicationInfo::builder()
            .application_name(b"wolf-engine\0")
            .api_version(supported);

        // we’ll also pass the debug‑messenger CI on the pNext chain so
        // instance‑creation itself gets messaged.
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

        let instance_create = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&exts)
            .enabled_layer_names(&layer_pointers)
            .flags(flags)
            .push_next(&mut debug_ci); // <‑‑ hook for create/destroy

        let instance = unsafe { entry.create_instance(&instance_create, None) }
            .expect("vkCreateInstance failed");
        info!("🎉 Vulkan instance ready");

        // 5) final debug messenger (for runtime messages) -----------------
        let debug = unsafe { instance.create_debug_utils_messenger_ext(&debug_ci, None) }
            .expect("debug utils messenger");

        // store for Drop
        self.window = Some(window);
        self.entry = Some(entry);
        self.instance = Some(instance);
        self.debug = Some(debug);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        if matches!(ev, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
}

/// -------- clean‑up ------------------------------------------------------
impl Drop for App {
    fn drop(&mut self) {
        if let (Some(instance), Some(dbg)) = (&self.instance, &self.debug) {
            unsafe {
                instance.destroy_debug_utils_messenger_ext(*dbg, None);
            }
        }
    }
}

/// -------- entry‑point ---------------------------------------------------
fn main() -> Result<()> {
    env_logger::init();
    let mut app = App::default();
    let event_loop = EventLoop::new()?; // EventLoopError → AppError via `From`
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app)?;
    Ok(())
}
