//! Bounded headless wgpu 26 exposure backend. Only exposure is accelerated.
//! Device/pipeline are reused; each request has at most one 4 MiB tile in flight.
//! GPU input/output/readback plus upload staging and host result are charged to
//! RenderContext. Driver-internal allocation overhead cannot be measured here.
//! CPU exposure uses f64 intermediates; GPU uses FP32, so ordinary results have
//! FP32 rounding differences. Subnormal/zero output and hidden RGB are repaired
//! from original pixels to preserve values on adapters that flush denormals.
//! Inputs at a rounding-induced overflow boundary return an explicit fallback
//! error; genuine CPU-domain overflow also errors. No silent CPU execution.
//! A readback timeout or cancellation retires the device, preventing repeated
//! abandoned submissions from growing its queue. Adapter/device startup calls
//! depend on the platform driver and cannot be preempted by the readback deadline.
//! API checked against downloaded wgpu 26.0.1 (MSRV 1.84), not trunk examples:
//! https://docs.rs/wgpu/26.0.1/wgpu/ . Shader validation does not require an adapter.
use crate::{adjustments::Adjustments, Frame, RenderContext};
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, MutexGuard, TryLockError,
    },
    thread,
    time::{Duration, Instant},
};
use utoipa::ToSchema;

pub const EXPOSURE_SHADER: &str = include_str!("../shaders/exposure.wgsl");
const MAX_TILE_BYTES: u64 = 4 * 1024 * 1024;
const WORKGROUP_SIZE: u32 = 64;
const DEADLINE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GpuBackend {
    Vulkan,
    Metal,
    Dx12,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GpuDeviceKind {
    Integrated,
    Discrete,
    Virtual,
    Software,
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GpuOperation {
    Exposure,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GpuAdapterMetadata {
    pub name: String,
    pub vendor: u32,
    pub device: u32,
    pub backend: GpuBackend,
    pub kind: GpuDeviceKind,
    pub driver: String,
    pub driver_info: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GpuStatus {
    pub adapter: GpuAdapterMetadata,
    pub available: bool,
    pub operations: Vec<GpuOperation>,
    pub fp32: bool,
    pub maximum_tile_pixels: u32,
    pub readback_timeout_seconds: u32,
}

#[derive(Debug)]
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_layout: wgpu::BindGroupLayout,
    adapter: GpuAdapterMetadata,
    tile_pixels: u32,
    serial: Mutex<()>,
    retired: AtomicBool,
    error: Arc<Mutex<Option<String>>>,
}
impl GpuRenderer {
    pub fn new() -> Result<Self> {
        Self::create(false)
    }
    /// Explicit software adapter selection for reproducible Vulkan CI. Failure
    /// means unavailable, never a successful or silently skipped numeric check.
    pub fn new_software() -> Result<Self> {
        Self::create(true)
    }

    fn create(software_only: bool) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12,
            ..Default::default()
        });
        let options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: software_only,
            compatible_surface: None,
        };
        let adapter = match pollster::block_on(instance.request_adapter(&options)) {
            Ok(adapter) => adapter,
            Err(first) if !software_only => {
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    force_fallback_adapter: true,
                    ..options
                }))
                .with_context(|| format!("no compute adapter (hardware attempt: {first})"))?
            }
            Err(error) => return Err(error).context("software GPU adapter unavailable"),
        };
        ensure!(
            adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS),
            "adapter does not support compute shaders"
        );
        let metadata = adapter_metadata(&adapter)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Iris bounded exposure"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .context("creating GPU compute device")?;
        let error = Arc::new(Mutex::new(None));
        let errors = error.clone();
        device.on_uncaptured_error(Box::new(move |error: wgpu::Error| {
            if let Ok(mut slot) = errors.lock() {
                *slot = Some(error.to_string());
            }
        }));
        let errors = error.clone();
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = errors.lock() {
                *slot = Some(format!("device lost ({reason:?}): {message}"));
            }
        });
        let limits = device.limits();
        ensure!(
            limits.max_compute_workgroup_size_x >= WORKGROUP_SIZE
                && limits.max_compute_invocations_per_workgroup >= WORKGROUP_SIZE,
            "adapter workgroup limit too small"
        );
        let max_bytes = MAX_TILE_BYTES
            .min(limits.max_buffer_size)
            .min(limits.max_storage_buffer_binding_size as u64);
        let tile_pixels = (max_bytes / 16)
            .min(limits.max_compute_workgroups_per_dimension as u64 * WORKGROUP_SIZE as u64)
            .min(u32::MAX as u64) as u32;
        ensure!(tile_pixels > 0, "adapter storage/dispatch limit too small");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Iris exposure WGSL"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(EXPOSURE_SHADER)),
        });
        check_error(&error)?;
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Iris exposure FP32"),
            layout: None,
            module: &module,
            entry_point: Some("exposure"),
            compilation_options: Default::default(),
            cache: None,
        });
        check_error(&error)?;
        let bind_layout = pipeline.get_bind_group_layout(0);
        check_error(&error)?;
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_layout,
            adapter: metadata,
            tile_pixels,
            serial: Mutex::new(()),
            retired: AtomicBool::new(false),
            error,
        })
    }

    pub fn status(&self) -> GpuStatus {
        GpuStatus {
            adapter: self.adapter.clone(),
            available: !self.retired.load(Ordering::Acquire)
                && self.error.lock().is_ok_and(|e| e.is_none()),
            operations: vec![GpuOperation::Exposure],
            fp32: true,
            maximum_tile_pixels: self.tile_pixels,
            readback_timeout_seconds: DEADLINE.as_secs() as u32,
        }
    }
    /// Exact parameter capability test; all non-exposure fields must equal their
    /// defaults, even fields irrelevant to a currently disabled effect.
    pub fn supports(params: &Adjustments) -> bool {
        if params.validate().is_err() {
            return false;
        }
        let candidate = Adjustments {
            exposure: params.exposure,
            ..Default::default()
        };
        *params == candidate
    }
    fn lock(&self, ctx: &RenderContext) -> Result<MutexGuard<'_, ()>> {
        let start = Instant::now();
        loop {
            ctx.check_cancelled()?;
            match self.serial.try_lock() {
                Ok(lock) => return Ok(lock),
                Err(TryLockError::Poisoned(_)) => bail!("GPU submission lock poisoned"),
                Err(TryLockError::WouldBlock) => {}
            }
            ensure!(
                start.elapsed() < DEADLINE,
                "GPU queue wait timed out; CPU fallback required"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }
    fn retire(&self) {
        self.retired.store(true, Ordering::Release);
        self.device.destroy();
    }

    pub fn apply(&self, input: &Frame, params: &Adjustments, ctx: &RenderContext) -> Result<Frame> {
        input.validate()?;
        params.validate()?;
        ctx.check_cancelled()?;
        ensure!(
            Self::supports(params),
            "GPU supports exposure only; CPU fallback required for these adjustments"
        );
        ensure!(
            !self.retired.load(Ordering::Acquire),
            "GPU device retired; CPU fallback required"
        );
        let _lock = self.lock(ctx)?;
        ensure!(
            !self.retired.load(Ordering::Acquire),
            "GPU device retired; CPU fallback required"
        );
        check_error(&self.error)?;
        if params.exposure == 0.0 {
            let _memory = ctx.reserve(input.byte_len())?;
            return Ok(input.clone());
        }
        let gain = (params.exposure as f64).exp2();
        let gpu_gain = gain as f32;
        validate_products(input, gain, gpu_gain, ctx)?;
        let tile_pixels = (self.tile_pixels as usize).min(input.pixels.len());
        let tile_bytes = (tile_pixels as u64)
            .checked_mul(16)
            .ok_or_else(|| anyhow!("GPU tile size overflow"))?;
        let required = required_bytes(input, tile_bytes)?;
        let _memory = ctx.reserve(required)?;
        let pixels = self.process_tiles(input, tile_pixels, tile_bytes, gain, ctx)?;
        ctx.check_cancelled()?;
        Frame::new(input.width, input.height, pixels)
    }
    fn process_tiles(
        &self,
        input: &Frame,
        tile_pixels: usize,
        tile_bytes: u64,
        gain: f64,
        ctx: &RenderContext,
    ) -> Result<Vec<[f32; 4]>> {
        let gpu_gain = gain as f32;
        let buffers = TileBuffers::new(&self.device, tile_bytes);
        check_error(&self.error)?;
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Iris exposure tile"),
            layout: &self.bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffers.uniform.as_entire_binding(),
                },
            ],
        });
        check_error(&self.error)?;
        let mut pixels = Vec::with_capacity(input.pixels.len());
        for tile in input.pixels.chunks(tile_pixels) {
            ctx.check_cancelled()?;
            let count = tile.len() as u32;
            let bytes = tile.len() as u64 * 16;
            let uniform = [gpu_gain.to_bits(), count, 0, 0];
            self.queue
                .write_buffer(&buffers.input, 0, bytemuck::cast_slice(tile));
            self.queue
                .write_buffer(&buffers.uniform, 0, bytemuck::cast_slice(&uniform));
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Iris exposure tile commands"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Iris exposure tile compute"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(count.div_ceil(WORKGROUP_SIZE), 1, 1);
            }
            encoder.copy_buffer_to_buffer(&buffers.output, 0, &buffers.readback, 0, bytes);
            self.queue.submit([encoder.finish()]);
            // Every submitted tile is drained before reusing its buffers. Failure
            // retires the device so abandoned GPU work cannot build up over retries.
            if let Err(error) = self.read_tile(&buffers, tile, bytes, gain, &mut pixels, ctx) {
                self.retire();
                return Err(error);
            }
        }
        Ok(pixels)
    }
    fn read_tile(
        &self,
        buffers: &TileBuffers,
        original: &[[f32; 4]],
        bytes: u64,
        gain: f64,
        pixels: &mut Vec<[f32; 4]>,
        ctx: &RenderContext,
    ) -> Result<()> {
        check_error(&self.error)?;
        let slice = buffers.readback.slice(0..bytes);
        let (send, receive) = mpsc::sync_channel(1);
        buffers.mapping.store(true, Ordering::Release);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
        let start = Instant::now();
        loop {
            ctx.check_cancelled()?;
            self.device
                .poll(wgpu::PollType::Poll)
                .context("GPU poll failed")?;
            check_error(&self.error)?;
            match receive.try_recv() {
                Ok(result) => {
                    if let Err(error) = result {
                        buffers.mapping.store(false, Ordering::Release);
                        return Err(error).context("GPU readback mapping failed");
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    bail!("GPU readback callback disconnected")
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            ensure!(
                start.elapsed() < DEADLINE,
                "GPU readback timed out; device retired"
            );
            thread::sleep(Duration::from_millis(1));
        }
        {
            let mapped = slice.get_mapped_range();
            for (i, (data, source)) in mapped.chunks_exact(16).zip(original).enumerate() {
                if i % 4096 == 0 {
                    ctx.check_cancelled()?;
                }
                let mut pixel = *source;
                if source[3] != 0.0 {
                    for c in 0..3 {
                        let v = f32::from_ne_bytes(data[c * 4..c * 4 + 4].try_into().unwrap());
                        ensure!(v.is_finite(), "GPU returned non-finite RGB");
                        // Some devices flush subnormal inputs/results; signed zero
                        // is also specified by the CPU's final +0 grain operation.
                        pixel[c] = if source[c].abs() < f32::MIN_POSITIVE
                            || v.abs() <= 2.0 * f32::MIN_POSITIVE
                        {
                            (source[c] as f64 * gain) as f32 + 0.0
                        } else {
                            v
                        };
                    }
                }
                pixels.push(pixel);
            }
        }
        buffers.readback.unmap();
        buffers.mapping.store(false, Ordering::Release);
        Ok(())
    }
}
fn check_error(error: &Mutex<Option<String>>) -> Result<()> {
    let slot = error
        .lock()
        .map_err(|_| anyhow!("GPU error lock poisoned"))?;
    if let Some(message) = slot.as_ref() {
        bail!("GPU failure: {message}");
    }
    Ok(())
}

struct TileBuffers {
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    uniform: wgpu::Buffer,
    mapping: AtomicBool,
}
impl TileBuffers {
    fn new(device: &wgpu::Device, bytes: u64) -> Self {
        let make = |label, usage, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Self {
            mapping: AtomicBool::new(false),
            input: make(
                "Iris GPU input tile",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                bytes,
            ),
            output: make(
                "Iris GPU output tile",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                bytes,
            ),
            readback: make(
                "Iris GPU readback tile",
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                bytes,
            ),
            uniform: make(
                "Iris GPU exposure parameters",
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                16,
            ),
        }
    }
}
impl Drop for TileBuffers {
    fn drop(&mut self) {
        if self.mapping.swap(false, Ordering::AcqRel) {
            self.readback.unmap();
        }
        self.input.destroy();
        self.output.destroy();
        self.readback.destroy();
        self.uniform.destroy();
    }
}

fn adapter_metadata(adapter: &wgpu::Adapter) -> Result<GpuAdapterMetadata> {
    let info = adapter.get_info();
    let backend = match info.backend {
        wgpu::Backend::Vulkan => GpuBackend::Vulkan,
        wgpu::Backend::Metal => GpuBackend::Metal,
        wgpu::Backend::Dx12 => GpuBackend::Dx12,
        _ => bail!("unsupported/noncomputing GPU backend"),
    };
    let kind = match info.device_type {
        wgpu::DeviceType::IntegratedGpu => GpuDeviceKind::Integrated,
        wgpu::DeviceType::DiscreteGpu => GpuDeviceKind::Discrete,
        wgpu::DeviceType::VirtualGpu => GpuDeviceKind::Virtual,
        wgpu::DeviceType::Cpu => GpuDeviceKind::Software,
        wgpu::DeviceType::Other => GpuDeviceKind::Other,
    };
    Ok(GpuAdapterMetadata {
        name: info.name,
        vendor: info.vendor,
        device: info.device,
        backend,
        kind,
        driver: info.driver,
        driver_info: info.driver_info,
    })
}

fn validate_products(input: &Frame, gain: f64, gpu_gain: f32, ctx: &RenderContext) -> Result<()> {
    // Ordinary photographic values are far from FP32 overflow. Only values
    // near the conservative boundary need the two authoritative products.
    let overflow_check_above = f32::MAX as f64 / gain.max(gpu_gain as f64) * 0.999999;
    // WGSL assumes finite arithmetic. Reject overflow before sending any work;
    // include rounded FP32 gain as well as the authoritative CPU-domain gain.
    for (i, p) in input.pixels.iter().enumerate() {
        if i % 4096 == 0 {
            ctx.check_cancelled()?;
        }
        if p[3] == 0.0 {
            continue;
        }
        for v in &p[..3] {
            if (*v as f64).abs() < overflow_check_above {
                continue;
            }
            ensure!(
                (*v as f64 * gain).abs() <= f32::MAX as f64,
                "adjustment exceeds finite f32 range"
            );
            ensure!(
                (*v as f64 * gpu_gain as f64).abs() <= f32::MAX as f64,
                "GPU FP32 gain overflow boundary; CPU fallback required"
            );
        }
    }
    Ok(())
}

fn required_bytes(input: &Frame, tile_bytes: u64) -> Result<u64> {
    let required = input
        .byte_len()
        .checked_add(
            tile_bytes
                .checked_mul(4)
                .ok_or_else(|| anyhow!("GPU memory accounting overflow"))?,
        )
        .and_then(|n| n.checked_add(32))
        .ok_or_else(|| anyhow!("GPU memory accounting overflow"))?;
    Ok(required)
}
