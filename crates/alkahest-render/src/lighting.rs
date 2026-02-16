use alkahest_core::constants::MAX_DYNAMIC_LIGHTS;
use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

/// GPU point light data (32 bytes, matches WGSL GpuPointLight).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPointLight {
    pub position: [f32; 3],
    pub radius: f32,
    pub color: [f32; 3],
    pub intensity: f32,
}

/// Light configuration uniform (48 bytes, matches WGSL LightConfig).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightConfig {
    pub ambient_color: [f32; 3],
    pub light_count: u32,
    pub sky_zenith_color: [f32; 3],
    pub max_shadow_lights: u32,
    pub sky_horizon_color: [f32; 3],
    pub _padding: u32,
}

impl Default for LightConfig {
    fn default() -> Self {
        Self {
            ambient_color: [0.15, 0.15, 0.18],
            light_count: 1,
            sky_zenith_color: [0.1, 0.15, 0.4],
            max_shadow_lights: alkahest_core::constants::MAX_SHADOW_RAYS_PER_PIXEL,
            sky_horizon_color: [0.5, 0.45, 0.35],
            _padding: 0,
        }
    }
}

/// Manages dynamic point lights extracted from emissive voxels.
/// Created at init, updated per-frame (C-PERF-2: no per-frame GPU allocation).
pub struct LightManager {
    /// GPU buffer holding the light array (MAX_DYNAMIC_LIGHTS * 32 bytes).
    pub light_buffer: wgpu::Buffer,
    /// CPU-side light data for upload.
    lights: Vec<GpuPointLight>,
    /// Current light configuration.
    pub config: LightConfig,
}

impl LightManager {
    /// Create the LightManager with pre-allocated GPU buffer.
    pub fn new(device: &wgpu::Device) -> Self {
        // Pre-allocate buffer for max lights (C-PERF-2: no per-frame allocation)
        let buffer_size = (MAX_DYNAMIC_LIGHTS as usize) * std::mem::size_of::<GpuPointLight>();

        // Initialize with a single default sun light
        let default_light = GpuPointLight {
            position: [128.0, 160.0, 128.0],
            radius: 512.0,
            color: [1.0, 0.95, 0.8],
            intensity: 1.5,
        };

        let mut initial_data = vec![GpuPointLight::zeroed(); MAX_DYNAMIC_LIGHTS as usize];
        initial_data[0] = default_light;

        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light-array"),
            contents: bytemuck::cast_slice(&initial_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Ensure buffer is large enough even if init data was smaller
        debug_assert!(
            light_buffer.size() >= buffer_size as u64,
            "light buffer too small"
        );

        Self {
            light_buffer,
            lights: vec![default_light],
            config: LightConfig::default(),
        }
    }

    /// Upload current light data to GPU. Called once per frame.
    pub fn upload(&self, queue: &wgpu::Queue) {
        // Write light array (pad to MAX_DYNAMIC_LIGHTS with zeroed entries)
        let mut padded = vec![GpuPointLight::zeroed(); MAX_DYNAMIC_LIGHTS as usize];
        let count = self.lights.len().min(MAX_DYNAMIC_LIGHTS as usize);
        padded[..count].copy_from_slice(&self.lights[..count]);
        queue.write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&padded));
    }

    /// Get the current light count (clamped to MAX_DYNAMIC_LIGHTS).
    pub fn light_count(&self) -> u32 {
        self.lights.len().min(MAX_DYNAMIC_LIGHTS as usize) as u32
    }

    /// Set lights from an external source (e.g. emissive voxel scan).
    /// The first light is always the sun/default light. Additional lights
    /// are dynamic emissive sources.
    pub fn set_lights(&mut self, lights: Vec<GpuPointLight>) {
        self.lights = lights;
        self.config.light_count = self.light_count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_config_default() {
        let config = LightConfig::default();
        assert_eq!(config.light_count, 1);
        assert_eq!(config.max_shadow_lights, 4);
    }

    #[test]
    fn test_gpu_point_light_size() {
        assert_eq!(std::mem::size_of::<GpuPointLight>(), 32);
    }

    #[test]
    fn test_light_config_size() {
        assert_eq!(std::mem::size_of::<LightConfig>(), 48);
    }
}
