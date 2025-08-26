use crate::core::renderer::api::Renderer;
use crate::error::Result;
use log::{error, info, warn};
use smallvec::SmallVec;
use std::ffi::CStr;
use vulkanalia::loader::{LIBRARY, LibloadingLoader};
use vulkanalia::prelude::v1_0::*;
use vulkanalia::vk::EntryV1_1;
use vulkanalia::vk::KhrSurfaceExtension;
use vulkanalia::vk::{self, ExtDebugUtilsExtension, KhrSwapchainExtension};
use vulkanalia::window as vk_window;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

const KHR_PORTABILITY_SUBSET_EXTENSION_NAME: &std::ffi::CStr =
    unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(b"VK_KHR_portability_subset\0") };

#[derive(Default)]
pub struct VulkanRenderer {
    entry: Option<Entry>,
    instance: Option<Instance>,
    debug: Option<vk::DebugUtilsMessengerEXT>,
    surface: Option<vk::SurfaceKHR>,
    physical_device: Option<vk::PhysicalDevice>,
    device: Option<Device>,
    graphics_queue: Option<vk::Queue>,
    present_queue: Option<vk::Queue>,
    queue_family_indices: Option<(u32, u32)>,

    swapchain: Option<vk::SwapchainKHR>,

    // Usually 2–3; inline capacity 4 avoids a heap alloc on typical setups.
    swapchain_images: SmallVec<[vk::Image; 4]>,
    swapchain_image_views: SmallVec<[vk::ImageView; 4]>,
    swapchain_format: Option<vk::Format>,
    swapchain_extent: Option<vk::Extent2D>,

    render_pass: Option<vk::RenderPass>,

    // Typically same count as images.
    framebuffers: SmallVec<[vk::Framebuffer; 4]>,
}

impl VulkanRenderer {
    /// Shared teardown; idempotent and safe to call multiple times.
    fn cleanup(&mut self) {
        unsafe {
            if let Some(device) = &self.device {
                // ensure GPU is idle before destroying resources
                device.device_wait_idle().ok();

                // destroy framebuffers
                for fb in self.framebuffers.drain(..) {
                    device.destroy_framebuffer(fb, None);
                }

                // destroy render pass
                if let Some(rp) = self.render_pass {
                    device.destroy_render_pass(rp, None);
                }
                self.render_pass = None;

                // destroy image views
                for iv in self.swapchain_image_views.drain(..) {
                    device.destroy_image_view(iv, None);
                }

                // destroy swapchain
                if let Some(swapchain) = self.swapchain {
                    device.destroy_swapchain_khr(swapchain, None);
                }
                self.swapchain = None;
            }

            // destroy debug messenger
            if let (Some(instance), Some(debug)) = (&self.instance, &self.debug) {
                instance.destroy_debug_utils_messenger_ext(*debug, None);
            }
            self.debug = None;

            // destroy surface
            if let (Some(instance), Some(surface)) = (&self.instance, self.surface) {
                instance.destroy_surface_khr(surface, None);
            }
            self.surface = None;

            // destroy logical device
            if let Some(device) = &self.device {
                device.destroy_device(None);
            }
            self.device = None;

            // destroy instance
            if let Some(instance) = &self.instance {
                instance.destroy_instance(None);
            }
            self.instance = None;
        }

        // clear other state
        self.entry = None;
        self.physical_device = None;
        self.graphics_queue = None;
        self.present_queue = None;
        self.queue_family_indices = None;
        self.swapchain_images.clear();
        self.swapchain_format = None;
        self.swapchain_extent = None;
    }

    fn create_swapchain(&mut self) {
        let instance = self.instance.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let surface = self.surface.unwrap();
        let physical_device = self.physical_device.unwrap();

        let surface_caps = unsafe {
            instance
                .get_physical_device_surface_capabilities_khr(physical_device, surface)
                .unwrap()
        };

        let surface_formats = unsafe {
            instance
                .get_physical_device_surface_formats_khr(physical_device, surface)
                .unwrap()
        };

        let format = surface_formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
            .unwrap_or(&surface_formats[0]);
        let extent = match surface_caps.current_extent.width {
            std::u32::MAX => vk::Extent2D {
                width: 800,
                height: 600,
            },
            _ => surface_caps.current_extent,
        };

        let present_modes = unsafe {
            instance
                .get_physical_device_surface_present_modes_khr(physical_device, surface)
                .unwrap()
        };

        let present_mode = if present_modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else {
            vk::PresentModeKHR::FIFO
        };

        let _queue_family_indices = self.queue_family_indices.unwrap();
        let mut image_count = surface_caps.min_image_count + 1;
        if surface_caps.max_image_count > 0 && image_count > surface_caps.max_image_count {
            image_count = surface_caps.max_image_count;
        }

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

        let swapchain = unsafe { device.create_swapchain_khr(&swapchain_info, None) }
            .expect("Failed to create swapchain");

        let images_raw = unsafe { device.get_swapchain_images_khr(swapchain).unwrap() };

        let mut images: SmallVec<[vk::Image; 4]> = SmallVec::with_capacity(images_raw.len());
        images.extend_from_slice(&images_raw);

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

        self.swapchain = Some(swapchain);
        self.swapchain_images = images;
        self.swapchain_image_views = image_views;
        self.swapchain_format = Some(format.format);
        self.swapchain_extent = Some(extent);

        info!("✅ Swapchain and image views created!");
    }

    fn create_render_pass(&mut self) {
        let device = self.device.as_ref().unwrap();
        let format = self.swapchain_format.unwrap();

        let color_attachment = vk::AttachmentDescription::builder()
            .format(format)
            .samples(vk::SampleCountFlags::_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let color_attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref));

        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(std::slice::from_ref(&color_attachment))
            .subpasses(std::slice::from_ref(&subpass));

        let render_pass = unsafe { device.create_render_pass(&render_pass_info, None) }
            .expect("Failed to create render pass");

        self.render_pass = Some(render_pass);
        info!("✅ Render pass created!");
    }

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

    // Debug callback (wrap all unsafe ops in explicit unsafe blocks)
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
}

impl Renderer for VulkanRenderer {
    fn initialize(&mut self, window: &Window, _event_loop: &ActiveEventLoop) -> Result<()> {
        let loader = unsafe { LibloadingLoader::new(LIBRARY) }?;
        let entry = unsafe { Entry::new(loader) }?;

        // Instance extensions (tiny set) → SmallVec
        let mut exts: SmallVec<[*const i8; 8]> =
            vk_window::get_required_instance_extensions(window)
                .iter()
                .map(|e| e.as_ptr())
                .collect();
        exts.push(vk::EXT_DEBUG_UTILS_EXTENSION.name.as_ptr());
        #[cfg(target_os = "macos")]
        exts.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name.as_ptr());

        // Probe instance layers without allocating Strings.
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

        // Build enabled layer name pointers.
        let mut layer_pointers: SmallVec<[*const i8; 4]> = SmallVec::new();
        if has_validation_layer {
            layer_pointers.push(b"VK_LAYER_KHRONOS_validation\0".as_ptr() as *const i8);
            info!("✅ Validation layer enabled");
        }

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
            .user_callback(Some(Self::debug_callback));

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

        // Probe device extensions without allocating Strings.
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

        // Build device extension pointer list (tiny set) → SmallVec
        let mut device_exts: SmallVec<[*const i8; 4]> = SmallVec::new();
        device_exts.push(vk::KHR_SWAPCHAIN_EXTENSION.name.as_ptr());
        if has_portability_subset {
            device_exts.push(KHR_PORTABILITY_SUBSET_EXTENSION_NAME.as_ptr());
            info!("✅ VK_KHR_portability_subset enabled");
        }

        // Unique queues (max 2) → SmallVec
        let mut unique_queues: SmallVec<[u32; 2]> = SmallVec::new();
        unique_queues.push(graphics_family);
        if graphics_family != present_family {
            unique_queues.push(present_family);
        }

        // Queue create infos (max 2) → SmallVec; pre-size exactly
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

        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_exts);

        let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .expect("Failed to create logical device");

        let graphics_queue = unsafe { device.get_device_queue(graphics_family, 0) };
        let present_queue = unsafe { device.get_device_queue(present_family, 0) };

        self.entry = Some(entry);
        self.instance = Some(instance);
        self.debug = Some(debug);
        self.surface = Some(surface);
        self.physical_device = Some(physical_device);
        self.queue_family_indices = Some((graphics_family, present_family));
        self.device = Some(device);
        self.graphics_queue = Some(graphics_queue);
        self.present_queue = Some(present_queue);
        self.create_swapchain();
        self.create_render_pass();
        self.create_framebuffers();
        Ok(())
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: &WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }

    fn render(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        // best-effort fallback; must not panic
        self.cleanup();
    }
}
