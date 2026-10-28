//! WGPU Surface & Rendering Lifecycle for Android.
//!
//! Properly manages Vulkan & OpenGL ES backends on Android, gracefully handling
//! surface creation, resizing, and teardown across app backgrounding and orientation changes.
//! Renders laid-out pages from WordCraft Session onto the native SurfaceView.

use log::{info, warn};
use wordcraft_engine::Session;
use wordcraft_render::{render_page, RenderOptions};

/// WGPU graphics context targeting Android SurfaceView / ANativeWindow.
pub struct AndroidRenderer {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
    pub width: u32,
    pub height: u32,
    pub is_vulkan: bool,
}

impl AndroidRenderer {
    /// Initialize WGPU instance with Vulkan primary backend and OpenGL ES fallback.
    pub async fn new() -> Result<Self, String> {
        info!("Initializing WGPU for Android");

        let mut backends = wgpu::Backends::VULKAN;
        if let Ok(val) = std::env::var("WGPU_BACKEND") {
            if val.eq_ignore_ascii_case("gl") || val.eq_ignore_ascii_case("gles") {
                backends = wgpu::Backends::GL;
            }
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let adapter_res = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }).await;

        let (adapter, is_vulkan) = match adapter_res {
            Ok(a) => (a, backends.contains(wgpu::Backends::VULKAN)),
            Err(_) => {
                warn!("Vulkan adapter unavailable; falling back to OpenGL ES");
                let gles_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends,
                    ..wgpu::InstanceDescriptor::new_without_display_handle()
                });
                let a = gles_instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                }).await.map_err(|e| format!("Failed to find suitable Vulkan or GLES adapter: {e:?}"))?;
                (a, false)
            }
        };

        info!("Selected GPU Adapter: {:?} (backend: {:?})", adapter.get_info().name, adapter.get_info().backend);

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Wordcraft Android Device"),
                ..Default::default()
            },
        ).await.map_err(|e| format!("Failed to create WGPU device: {e}"))?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
            width: 0,
            height: 0,
            is_vulkan,
        })
    }

    /// Attach an unsafe native surface (e.g. from ANativeWindow) and configure the swapchain.
    pub fn attach_surface_unsafe(&mut self, target: wgpu::SurfaceTargetUnsafe, width: u32, height: u32) -> Result<(), String> {
        info!("Attaching unsafe native surface: {width}x{height}");
        let surface = unsafe { self.instance.create_surface_unsafe(target) }.map_err(|e| format!("Failed to create surface: {e}"))?;
        let caps = surface.get_capabilities(&self.adapter);

        let format = caps.formats.iter().copied()
            .find(|f| f.is_srgb())
            .unwrap_or_else(|| caps.formats.first().copied().unwrap_or(wgpu::TextureFormat::Rgba8Unorm));

        let width = width.max(1);
        let height = height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::AutoVsync
            },
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Opaque),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);
        self.surface = Some(surface);
        self.surface_config = Some(config);
        self.width = width;
        self.height = height;

        Ok(())
    }

    /// Attach a native surface (e.g. from ANativeWindow) and configure the swapchain.
    pub fn attach_surface<T>(&mut self, target: T, width: u32, height: u32) -> Result<(), String>
    where
        T: Into<wgpu::SurfaceTarget<'static>>,
    {
        info!("Attaching surface: {width}x{height}");
        let surface = self.instance.create_surface(target).map_err(|e| format!("Failed to create surface: {e}"))?;
        let caps = surface.get_capabilities(&self.adapter);

        let format = caps.formats.iter().copied()
            .find(|f| f.is_srgb())
            .unwrap_or_else(|| caps.formats.first().copied().unwrap_or(wgpu::TextureFormat::Rgba8Unorm));

        let width = width.max(1);
        let height = height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::AutoVsync
            },
            alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Opaque),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);
        self.surface = Some(surface);
        self.surface_config = Some(config);
        self.width = width;
        self.height = height;

        Ok(())
    }

    /// Resize surface on rotation or window resize.
    pub fn resize_surface(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        if let (Some(surface), Some(config)) = (self.surface.as_mut(), self.surface_config.as_mut()) {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
            info!("Surface resized to {width}x{height}");
        }
    }

    /// Detach and drop the surface cleanly when the Android view is destroyed or suspended.
    pub fn detach_surface(&mut self) {
        info!("Detaching surface to prevent Android surface-loss crash");
        self.surface = None;
        self.surface_config = None;
    }

    /// Acquire the next surface texture for rendering.
    pub fn get_current_frame(&self) -> Option<(wgpu::SurfaceTexture, wgpu::TextureView)> {
        let surface = self.surface.as_ref()?;
        match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output) | wgpu::CurrentSurfaceTexture::Suboptimal(output) => {
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                Some((output, view))
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                warn!("WGPU Surface lost; will recreate on next frame");
                None
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                warn!("WGPU Surface outdated");
                None
            }
            _ => None,
        }
    }

    /// Renders the document pages and presents the swapchain texture.
    pub fn render_frame(&mut self, session: &mut Session, zoom: f32, _scroll_x: f32, scroll_y: f32) -> Result<(), String> {
        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return Ok(()),
        };

        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Lost => {
                warn!("Surface lost; dropping to re-create");
                self.detach_surface();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            wgpu::CurrentSurfaceTexture::Validation => return Err("Surface validation error".to_string()),
        };

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Wordcraft Canvas Encoder"),
        });

        // 1. Clear background to neutral workspace grey (matches desktop canvas #E6E6E6)
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.90, g: 0.90, b: 0.90, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        // 2. Rasterize visible pages using wordcraft_render
        let layout = session.layout();
        let opts = RenderOptions::default();

        for (page_idx, page) in layout.pages.iter().enumerate() {
            let page_gap = 18.0 * zoom;
            let page_height = page.h * zoom;
            let page_top = page_idx as f32 * (page_height + page_gap) - scroll_y;

            if page_top + page_height < 0.0 || page_top > self.height as f32 {
                continue;
            }

            let rendered = render_page(&session.doc, page, zoom * (96.0 / 72.0), &opts);
            if rendered.width == 0 || rendered.height == 0 {
                continue;
            }

            let page_size = wgpu::Extent3d {
                width: rendered.width,
                height: rendered.height,
                depth_or_array_layers: 1,
            };

            let page_tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Page Texture"),
                size: page_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &page_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rendered.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * rendered.width),
                    rows_per_image: Some(rendered.height),
                },
                page_size,
            );
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);

        Ok(())
    }
}
