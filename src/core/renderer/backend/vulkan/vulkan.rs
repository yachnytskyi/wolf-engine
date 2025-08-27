// Import Vulkan debug utils extension only in debug builds
#[cfg(debug_assertions)]
use vulkanalia::vk::ExtDebugUtilsExtension;

// Only pull in error/warn when debug assertions are on
#[cfg(debug_assertions)]
use log::{error, warn};

use crate::core::renderer::api::Renderer;
use crate::error::Result;
use log::info;
use smallvec::SmallVec;
use std::ffi::CStr;

use vulkanalia::loader::{LIBRARY, LibloadingLoader};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::EntryV1_1;

use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::vk::{self, KhrSwapchainExtension};

use vulkanalia::window as vk_window;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

// Portability extension needed on some platforms (e.g., macOS + MoltenVK)
const KHR_PORTABILITY_SUBSET_EXTENSION_NAME: &std::ffi::CStr =
    unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(b"VK_KHR_portability_subset\0") };

/// Main Vulkan renderer struct.
/// Holds all Vulkan objects and resources needed to draw.
#[derive(Default)]
pub struct VulkanRenderer {
    entry: Option<Entry>,                      // Vulkan entry point (library handle)
    instance: Option<Instance>,                // Vulkan instance
    debug: Option<vk::DebugUtilsMessengerEXT>, // Debug messenger (only in debug builds)
    surface: Option<vk::SurfaceKHR>,           // Window surface
    physical_device: Option<vk::PhysicalDevice>, // Chosen physical GPU
    device: Option<Device>,                    // Logical device
    graphics_queue: Option<vk::Queue>,         // Graphics queue
    present_queue: Option<vk::Queue>,          // Presentation queue
    queue_family_indices: Option<(u32, u32)>,  // Queue family indices

    swapchain: Option<vk::SwapchainKHR>, // Swapchain for presenting images

    // Usually 2–3 images; SmallVec avoids heap allocation for small counts
    swapchain_images: SmallVec<[vk::Image; 4]>,
    swapchain_image_views: SmallVec<[vk::ImageView; 4]>,
    swapchain_format: Option<vk::Format>,   // Image format
    swapchain_extent: Option<vk::Extent2D>, // Image resolution

    render_pass: Option<vk::RenderPass>, // Render pass object

    // One framebuffer per swapchain image
    framebuffers: SmallVec<[vk::Framebuffer; 4]>,
}

impl VulkanRenderer {
    /// Cleans up all Vulkan resources.
    /// Safe to call multiple times, called automatically in Drop.
    fn cleanup(&mut self) {
        unsafe {
            if let Some(device) = &self.device {
                // Wait until GPU is idle before tearing down
                device.device_wait_idle().ok();

                // Destroy framebuffers
                for fb in self.framebuffers.drain(..) {
                    device.destroy_framebuffer(fb, None);
                }

                // Destroy render pass
                if let Some(rp) = self.render_pass {
                    device.destroy_render_pass(rp, None);
                }
                self.render_pass = None;

                // Destroy swapchain image views
                for iv in self.swapchain_image_views.drain(..) {
                    device.destroy_image_view(iv, None);
                }

                // Destroy swapchain
                if let Some(swapchain) = self.swapchain {
                    device.destroy_swapchain_khr(swapchain, None);
                }
                self.swapchain = None;
            }

            // Destroy debug messenger (only created in debug builds)
            #[cfg(debug_assertions)]
            if let (Some(instance), Some(debug)) = (&self.instance, &self.debug) {
                destroy_debug_messenger(instance, debug);
            }
            self.debug = None;

            // Destroy surface
            if let (Some(instance), Some(surface)) = (&self.instance, self.surface) {
                instance.destroy_surface_khr(surface, None);
            }
            self.surface = None;

            // Destroy logical device
            if let Some(device) = &self.device {
                device.destroy_device(None);
            }
            self.device = None;

            // Destroy Vulkan instance
            if let Some(instance) = &self.instance {
                instance.destroy_instance(None);
            }
            self.instance = None;
        }

        // Clear CPU-side state
        self.entry = None;
        self.physical_device = None;
        self.graphics_queue = None;
        self.present_queue = None;
        self.queue_family_indices = None;
        self.swapchain_images.clear();
        self.swapchain_format = None;
        self.swapchain_extent = None;
    }

    /// Creates the swapchain and image views.
    fn create_swapchain(&mut self) {
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let surface = self.surface.unwrap();
        let physical_device = self.physical_device.unwrap();

        // Query surface capabilities
        let surface_caps = unsafe {
            instance
                .get_physical_device_surface_capabilities_khr(physical_device, surface)
                .unwrap()
        };

        // Query supported formats
        let surface_formats = unsafe {
            instance
                .get_physical_device_surface_formats_khr(physical_device, surface)
                .unwrap()
        };

        // Prefer SRGB, fallback to first format
        let format = surface_formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
            .unwrap_or(&surface_formats[0]);

        // Pick swapchain resolution (use current_extent if fixed)
        let extent = match surface_caps.current_extent.width {
            std::u32::MAX => vk::Extent2D {
                width: 800,
                height: 600,
            },
            _ => surface_caps.current_extent,
        };

        // Query present modes
        let present_modes = unsafe {
            instance
                .get_physical_device_surface_present_modes_khr(physical_device, surface)
                .unwrap()
        };

        // Prefer MAILBOX (triple buffering), else fallback to FIFO (vsync)
        let present_mode = if present_modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else {
            vk::PresentModeKHR::FIFO
        };

        let _queue_family_indices = self.queue_family_indices.unwrap();

        // Request one more image than minimum if possible
        let mut image_count = surface_caps.min_image_count + 1;
        if surface_caps.max_image_count > 0 && image_count > surface_caps.max_image_count {
            image_count = surface_caps.max_image_count;
        }

        // Swapchain creation info
        let swapchain_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);

        // Create swapchain
        let swapchain = unsafe { device.create_swapchain_khr(&swapchain_info, None) }
            .expect("Failed to create swapchain");

        // Retrieve swapchain images
        let images_raw = unsafe { device.get_swapchain_images_khr(swapchain).unwrap() };

        // Store images
        let mut images: SmallVec<[vk::Image; 4]> = SmallVec::with_capacity(images_raw.len());
        images.extend_from_slice(&images_raw);

        // Create image views for each swapchain image
        let mut image_views: SmallVec<[vk::ImageView; 4]> = SmallVec::with_capacity(images.len());
        for &image in &images {
            let view_info = vk::ImageViewCreateInfo::builder()
                .image(image)
                .view_type(vk::ImageViewType::_2D)
                .format(format.format)
                .components(vk::ComponentMapping::default())
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build(),
                );

            let view = unsafe { device.create_image_view(&view_info, None).unwrap() };
            image_views.push(view);
        }

        // Save swapchain state
        self.swapchain = Some(swapchain);
        self.swapchain_images = images;
        self.swapchain_image_views = image_views;
        self.swapchain_format = Some(format.format);
        self.swapchain_extent = Some(extent);

        info!("✅ Swapchain and image views created!");
    }

    /// Creates a render pass for rendering into the swapchain images.
    fn create_render_pass(&mut self) {
        let device = self.device.as_ref().unwrap();
        let format = self.swapchain_format.unwrap();

        // Single color attachment (the swapchain image)
        let color_attachment = vk::AttachmentDescription::builder()
            .format(format)
            .samples(vk::SampleCountFlags::_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        // Reference to attachment in subpass
        let color_attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        // Subpass that writes to the color attachment
        let subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref));

        // Render pass creation info
        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(std::slice::from_ref(&color_attachment))
            .subpasses(std::slice::from_ref(&subpass));

        // Create render pass
        let render_pass = unsafe { device.create_render_pass(&render_pass_info, None) }
            .expect("Failed to create render pass");

        self.render_pass = Some(render_pass);
        info!("✅ Render pass created!");
    }

    /// Creates one framebuffer per swapchain image.
    fn create_framebuffers(&mut self) {
        let device = self.device.as_ref().unwrap();
        let render_pass = self.render_pass.unwrap();
        let extent = self.swapchain_extent.unwrap();

        let mut framebuffers: SmallVec<[vk::Framebuffer; 4]> =
            SmallVec::with_capacity(self.swapchain_image_views.len());

        for &view in &self.swapchain_image_views {
            let attachments = [view];
            let framebuffer_info = vk::FramebufferCreateInfo::builder()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1);

            let fb = unsafe { device.create_framebuffer(&framebuffer_info, None) }
                .expect("Failed to create framebuffer");
            framebuffers.push(fb);
        }

        self.framebuffers = framebuffers;

        info!("✅ Framebuffers created!");
    }
}

impl Renderer for VulkanRenderer {
    /// Initialize Vulkan: create instance, device, swapchain, render pass, etc.
    fn initialize(&mut self, window: &Window, _event_loop: &ActiveEventLoop) -> Result<()> {
        // Load Vulkan library
        let loader = unsafe { LibloadingLoader::new(LIBRARY) }?;
        let entry = unsafe { Entry::new(loader) }?;

        // Query required instance extensions from winit
        let mut exts: SmallVec<[*const i8; 8]> =
            vk_window::get_required_instance_extensions(window)
                .iter()
                .map(|e| e.as_ptr())
                .collect();

        // Add debug utils extension in debug builds
        #[cfg(debug_assertions)]
        {
            exts.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        }

        // On macOS, require portability extension
        #[cfg(target_os = "macos")]
        exts.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());

        // Check for validation layer availability (debug builds only)
        #[cfg(debug_assertions)]
        let has_validation_layer = unsafe {
            entry
                .enumerate_instance_layer_properties()
                .unwrap()
                .iter()
                .any(|p| {
                    CStr::from_ptr(p.layer_name.as_ptr()).to_bytes()
                        == b"VK_LAYER_KHRONOS_validation"
                })
        };

        // Enable validation layer (debug builds only)
        #[cfg(debug_assertions)]
        let mut layer_pointers: SmallVec<[*const i8; 4]> = SmallVec::new();

        #[cfg(not(debug_assertions))]
        let layer_pointers: SmallVec<[*const i8; 4]> = SmallVec::new();

        #[cfg(debug_assertions)]
        if has_validation_layer {
            layer_pointers.push(b"VK_LAYER_KHRONOS_validation\0".as_ptr() as *const i8);
            info!("✅ Validation layer enabled");
        }

        // macOS portability flag
        let mut flags = vk::InstanceCreateFlags::empty();
        #[cfg(target_os = "macos")]
        {
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }

        // Query supported Vulkan version
        let supported = unsafe {
            entry
                .enumerate_instance_version()
                .expect("Failed to query Vulkan instance version")
        };

        // Application info
        let app_info = vk::ApplicationInfo::builder()
            .application_name(b"Wolf Engine\0")
            .engine_name(b"Wolf Engine\0")
            .api_version(supported);

        // Instance creation info
        #[cfg(debug_assertions)]
        let mut create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&exts)
            .enabled_layer_names(&layer_pointers)
            .flags(flags);

        #[cfg(not(debug_assertions))]
        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&exts)
            .enabled_layer_names(&layer_pointers)
            .flags(flags);

        // --- Debug messenger setup now lives in helper fns ---
        #[cfg(debug_assertions)]
        let mut debug_ci = build_debug_messenger_ci();
        #[cfg(debug_assertions)]
        {
            create_info = create_info.push_next(&mut debug_ci);
        }

        // Create Vulkan instance
        let instance =
            unsafe { entry.create_instance(&create_info, None) }.expect("vkCreateInstance failed");
        info!("🎉 Vulkan instance ready");

        // Create debug messenger in debug builds (using helper)
        #[cfg(debug_assertions)]
        let debug = Some(create_debug_messenger(&instance, &debug_ci));

        #[cfg(not(debug_assertions))]
        let debug = None;

        // Create window surface
        let window_handle = window.window_handle().unwrap();
        let display_handle = window.display_handle().unwrap();
        let surface = unsafe {
            vk_window::create_surface(
                &instance,
                &display_handle as &dyn HasDisplayHandle,
                &window_handle as &dyn HasWindowHandle,
            )
        }
        .expect("Failed to create Vulkan surface");

        // Pick physical device + queue families
        let devices = unsafe { instance.enumerate_physical_devices() }
            .expect("Failed to enumerate physical devices");
        let (physical_device, graphics_family, present_family) = devices
            .iter()
            .find_map(|&dev| {
                let props = unsafe { instance.get_physical_device_queue_family_properties(dev) };
                let mut graphics_index = None;
                let mut present_index = None;
                for (i, info) in props.iter().enumerate() {
                    if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                        graphics_index = Some(i as u32);
                    }
                    let present_support = unsafe {
                        instance
                            .get_physical_device_surface_support_khr(dev, i as u32, surface)
                            .unwrap()
                    };
                    if present_support {
                        present_index = Some(i as u32);
                    }
                }
                if let (Some(g), Some(p)) = (graphics_index, present_index) {
                    Some((dev, g, p))
                } else {
                    None
                }
            })
            .expect("No suitable GPU found");

        // Enable device extensions (always need swapchain, maybe portability)
        let has_portability_subset = unsafe {
            instance
                .enumerate_device_extension_properties(physical_device, None)
                .expect("Failed to enumerate device extensions")
                .iter()
                .any(|e| {
                    CStr::from_ptr(e.extension_name.as_ptr())
                        == KHR_PORTABILITY_SUBSET_EXTENSION_NAME
                })
        };

        let mut device_exts: SmallVec<[*const i8; 4]> = SmallVec::new();
        device_exts.push(vk::KHR_SWAPCHAIN_EXTENSION.name.as_ptr());
        if has_portability_subset {
            device_exts.push(KHR_PORTABILITY_SUBSET_EXTENSION_NAME.as_ptr());
            info!("✅ VK_KHR_portability_subset enabled");
        }

        // Setup queue creation (graphics + present)
        let mut unique_queues: SmallVec<[u32; 2]> = SmallVec::new();
        unique_queues.push(graphics_family);
        if graphics_family != present_family {
            unique_queues.push(present_family);
        }

        let queue_priorities = [1.0_f32];
        let mut queue_create_infos: SmallVec<[vk::DeviceQueueCreateInfo; 2]> =
            SmallVec::with_capacity(unique_queues.len());

        for &family in &unique_queues {
            queue_create_infos.push(
                vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(family)
                    .queue_priorities(&queue_priorities)
                    .build(),
            );
        }

        // Create logical device
        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_exts);

        let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .expect("Failed to create logical device");

        // Retrieve queues
        let graphics_queue = unsafe { device.get_device_queue(graphics_family, 0) };
        let present_queue = unsafe { device.get_device_queue(present_family, 0) };

        // Save state
        self.entry = Some(entry);
        self.instance = Some(instance);
        self.debug = debug; // Debug messenger in debug builds
        self.surface = Some(surface);
        self.physical_device = Some(physical_device);
        self.queue_family_indices = Some((graphics_family, present_family));
        self.device = Some(device);
        self.graphics_queue = Some(graphics_queue);
        self.present_queue = Some(present_queue);

        // Continue with swapchain/rendering setup
        self.create_swapchain();
        self.create_render_pass();
        self.create_framebuffers();
        Ok(())
    }

    /// Handle window events (currently just close)
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: &WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }

    /// Render one frame (currently empty placeholder)
    fn render(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        // Ensure cleanup happens when renderer goes out of scope.
        // Must not panic.
        self.cleanup();
    }
}

//
// ===== Debug Utils helpers (only compiled in debug builds) =====
//

#[cfg(debug_assertions)]
unsafe extern "system" fn debug_callback(
    sev: vk::DebugUtilsMessageSeverityFlagsEXT,
    ty: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _ud: *mut std::ffi::c_void,
) -> vk::Bool32 {
    // Convert C string to Rust string
    let message = unsafe { std::ffi::CStr::from_ptr((*data).message).to_string_lossy() };

    // Log with appropriate severity
    if sev.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        error!("[{ty:?}] {message}");
    } else if sev.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        warn!("[{ty:?}] {message}");
    } else {
        info!("[{ty:?}] {message}");
    }
    vk::FALSE
}

#[cfg(debug_assertions)]
fn build_debug_messenger_ci() -> vk::DebugUtilsMessengerCreateInfoEXTBuilder<'static> {
    vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .user_callback(Some(debug_callback))
}

#[cfg(debug_assertions)]
fn create_debug_messenger(
    instance: &Instance,
    ci: &vk::DebugUtilsMessengerCreateInfoEXT,
) -> vk::DebugUtilsMessengerEXT {
    unsafe { instance.create_debug_utils_messenger_ext(ci, None) }
        .expect("debug utils messenger")
}

#[cfg(debug_assertions)]
fn destroy_debug_messenger(instance: &Instance, messenger: &vk::DebugUtilsMessengerEXT) {
    unsafe { instance.destroy_debug_utils_messenger_ext(*messenger, None) };
}
