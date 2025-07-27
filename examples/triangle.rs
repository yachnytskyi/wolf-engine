use log::info;
use vulkanalia::{loader::LibloadingLoader, prelude::v1_0::*, window as vk_window};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{self, ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use wolf_engine::error::AppError;

// ── macOS‑only stuff ────────────────────────────────────────────────────────
#[cfg(target_os = "macos")]
const VK_LIB: &str = "libvulkan.1.dylib"; // bundled with LunarG SDK

#[cfg(not(target_os = "macos"))]
const VK_LIB: &str = "vulkan-1.dll\0"; // or libvulkan.so.1 on Linux (placeholder)
// ↑ This path is never *used* yet; it just satisfies the const for other targets.
//   You’ll replace it later when you add cross‑platform builds.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct App {
    window: Option<Window>,
    instance: Option<vulkanalia::Instance>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // 1) Create the window
        let window = event_loop
            .create_window(Window::default_attributes())
            .expect("create window");

        // 2) Dynamically load the Vulkan loader for *this* OS
        let loader = unsafe { LibloadingLoader::new(VK_LIB) }.expect("load Vulkan shared lib");
        let entry = unsafe { vulkanalia::Entry::new(loader) }.expect("create entry");

        // 3) Collect required surface extensions
        let mut ext_ptrs: Vec<*const i8> = vk_window::get_required_instance_extensions(&window)
            .iter()
            .map(|e| e.as_ptr())
            .collect();

        // ── macOS‑specific portability shim ────────────────────────────────
        #[cfg(target_os = "macos")]
        {
            ext_ptrs.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());
        }
        // ───────────────────────────────────────────────────────────────────

        // 4) InstanceCreateInfo
        let app_info = vk::ApplicationInfo::builder()
            .application_name(b"wolf-engine\0")
            .application_version(0)
            .engine_name(b"wolf-engine\0")
            .engine_version(0)
            .api_version(vk::make_version(1, 3, 0));

        let mut flags = vk::InstanceCreateFlags::empty();

        // portability flag only on macOS
        #[cfg(target_os = "macos")]
        {
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&ext_ptrs)
            .flags(flags);

        // 5) Create the instance
        let instance =
            unsafe { entry.create_instance(&create_info, None) }.expect("vkCreateInstance failed");

        info!("✅ Vulkan instance created!");
        self.window = Some(window);
        self.instance = Some(instance);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        if matches!(ev, WindowEvent::CloseRequested) {
            el.exit();
        }
    }
}

fn main() -> Result<(), AppError> {
    env_logger::init();

    let mut app = App::default();
    let event_loop = EventLoop::new()?; // converts to AppError on failure
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app)?; // bubbles EventLoopError via AppError
    Ok(())
}
