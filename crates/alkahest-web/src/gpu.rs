use alkahest_core::error::AlkahestError;
use wgpu::{
    Adapter, Device, DeviceDescriptor, Instance, InstanceDescriptor, InstanceFlags,
    PowerPreference, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat,
    TextureUsages,
};

/// Holds all WebGPU resources initialized at startup.
pub struct GpuContext {
    #[allow(dead_code)] // Used in later milestones for limit queries
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub surface_format: TextureFormat,
    pub adapter_name: String,
    pub backend: String,
}

/// Initialize WebGPU asynchronously (C-GPU-1).
///
/// Requests `BROWSER_WEBGPU` backend only — no WebGL fallback.
/// Uses `HighPerformance` power preference and the adapter's own limits.
pub async fn init_gpu(
    canvas: web_sys::HtmlCanvasElement,
    width: u32,
    height: u32,
) -> Result<GpuContext, AlkahestError> {
    let instance = Instance::new(&InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        flags: InstanceFlags::default(),
        ..Default::default()
    });

    let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
    // Canvas is owned by the DOM and lives for 'static in the web backend.
    let surface: Surface<'static> = instance
        .create_surface(surface_target)
        .map_err(|e| AlkahestError::SurfaceConfigFailed(format!("{e}")))?;

    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| {
            AlkahestError::AdapterNotFound(
                "No WebGPU adapter found. Your browser may not support WebGPU.".into(),
            )
        })?;

    let adapter_info = adapter.get_info();
    let adapter_name = adapter_info.name.clone();
    let backend = format!("{:?}", adapter_info.backend);

    // Log adapter info and limits for diagnostics
    log::info!("Adapter: {} ({})", adapter_name, backend);
    log::info!("Adapter limits: {:?}", adapter.limits());

    // Check storage buffer limit for future milestones
    let max_storage = adapter.limits().max_storage_buffers_per_shader_stage;
    if max_storage < 8 {
        log::warn!(
            "Adapter supports only {} storage buffers per shader stage (need 8 for M3+)",
            max_storage
        );
    }

    let (device, queue) = adapter
        .request_device(
            &DeviceDescriptor {
                label: Some("alkahest-device"),
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                ..Default::default()
            },
            None,
        )
        .await
        .map_err(|e| AlkahestError::DeviceRequestFailed(format!("{e}")))?;

    // Select sRGB surface format with fallback
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

    let surface_config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &surface_config);

    log::info!(
        "Surface format: {:?}, size: {}x{}",
        surface_format,
        width,
        height
    );

    Ok(GpuContext {
        adapter,
        device,
        queue,
        surface,
        surface_config,
        surface_format,
        adapter_name,
        backend,
    })
}
