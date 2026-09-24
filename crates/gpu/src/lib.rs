//! Render with the GPU into Wove pixel elements.
//!
//! A `Gpu` opens a device with no window, renders whatever a caller records
//! into a texture the size of a `Pixels` element, and copies the result back
//! into the element, which the tree then draws with block characters like
//! any other. wgpu is re-exported, so an application writes ordinary wgpu
//! pipelines and shaders against it.
//!
//! ```no_run
//! use wove::elements::Pixels;
//! use wove_gpu::{wgpu, Gpu};
//! let gpu = Gpu::new()?;
//! let mut pixels = Pixels::new(64, 32);
//! gpu.render(&mut pixels, |device, queue, target| {
//!     let mut encoder = device.create_command_encoder(&Default::default());
//!     encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//!         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
//!             view: target,
//!             depth_slice: None,
//!             resolve_target: None,
//!             ops: wgpu::Operations {
//!                 load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
//!                 store: wgpu::StoreOp::Store,
//!             },
//!         })],
//!         ..Default::default()
//!     });
//!     queue.submit([encoder.finish()]);
//! })?;
//! # Ok::<(), wove_gpu::Error>(())
//! ```
#![forbid(unsafe_code)]
pub use wgpu;
use wove::elements::Pixels;

/// A device could not be opened, or a frame could not be read back.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// The format frames are rendered in. It is sRGB, so shaders write linear
/// color and the bytes read back are ready for a terminal.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// A headless device and its queue.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: wgpu::AdapterInfo,
}

impl Gpu {
    /// Open an adapter that can render off screen: a real GPU when there is
    /// one, otherwise a software renderer such as Mesa's or Windows' WARP.
    /// A terminal's frame is small, so the low-power adapter is preferred
    /// and a laptop's discrete GPU stays asleep.
    pub fn new() -> Result<Self, Error> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(async {
            let options = wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            };
            let primary = match instance.request_adapter(&options).await {
                Ok(adapter) => return Ok(adapter),
                Err(error) => error,
            };
            let fallback = wgpu::RequestAdapterOptions {
                force_fallback_adapter: true,
                ..Default::default()
            };
            instance
                .request_adapter(&fallback)
                .await
                .map_err(|error| format!("{primary}; no software adapter either: {error}"))
        })
        .map_err(|error| format!("no GPU adapter is available: {error}"))?;
        let info = adapter.get_info();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("wove"),
                ..Default::default()
            }))?;
        Ok(Self {
            device,
            queue,
            info,
        })
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// The adapter in use: its name, backend, and whether it is software.
    pub fn info(&self) -> &wgpu::AdapterInfo {
        &self.info
    }

    /// Render a frame the size of `pixels` and copy it into them. `draw`
    /// records and submits whatever it likes against `target`, a `FORMAT`
    /// texture view; the copy is submitted after it and waited for.
    pub fn render(
        &self,
        pixels: &mut Pixels,
        draw: impl FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView),
    ) -> Result<(), Error> {
        let (columns, rows) = pixels.size();
        let (width, height) = (u32::try_from(columns)?, u32::try_from(rows)?);
        if width == 0 || height == 0 {
            return Ok(());
        }
        let texture = self.frame_texture(width, height)?;
        draw(
            &self.device,
            &self.queue,
            &texture.create_view(&Default::default()),
        );
        let (staging, row) = self.copy_out(&texture);
        let data = self.read(&staging)?;
        let mut packed = Vec::with_capacity(columns * rows * 4);
        for y in 0..rows {
            let start = y * row;
            packed.extend_from_slice(&data[start..start + columns * 4]);
        }
        pixels.write(&packed);
        Ok(())
    }

    /// A texture to render into and copy from, within the device's limits.
    fn frame_texture(&self, width: u32, height: u32) -> Result<wgpu::Texture, Error> {
        let limit = self.device.limits().max_texture_dimension_2d;
        if width > limit || height > limit {
            return Err(format!(
                "a {width} by {height} frame exceeds the device's {limit} pixel limit"
            )
            .into());
        }
        Ok(self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wove frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        }))
    }

    /// Submit a copy of the texture into a buffer the CPU can map. Returns
    /// the buffer and its row stride in bytes, which the copy pads to the
    /// device's alignment.
    fn copy_out(&self, texture: &wgpu::Texture) -> (wgpu::Buffer, usize) {
        let size = texture.size();
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let row = (size.width * 4).div_ceil(align) * align;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wove readback"),
            size: u64::from(row) * u64::from(size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );
        self.queue.submit([encoder.finish()]);
        (staging, row as usize)
    }

    /// Wait for the copy and take the buffer's bytes.
    fn read(&self, staging: &wgpu::Buffer) -> Result<Vec<u8>, Error> {
        let (sender, receiver) = std::sync::mpsc::channel();
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        receiver
            .recv()
            .map_err(|_| "the GPU did not answer the readback")??;
        let data = staging.slice(..).get_mapped_range()?.to_vec();
        staging.unmap();
        Ok(data)
    }
}
