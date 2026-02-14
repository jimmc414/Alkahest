/// GPU pick buffer for voxel hover info via async readback.
/// Stores 8 u32 values written by the ray march shader for the cursor pixel:
///   [0] world_x, [1] world_y, [2] world_z, [3] material_id,
///   [4] temperature, [5] pressure, [6] velocity_packed, [7] flags

/// Size of the pick buffer in bytes (8 × u32 = 32 bytes).
pub const PICK_BUFFER_SIZE: u64 = 8 * 4;

/// Decoded pick result from GPU readback.
#[derive(Debug, Clone, Default)]
pub struct PickResult {
    pub valid: bool,
    pub world_x: i32,
    pub world_y: i32,
    pub world_z: i32,
    pub material_id: u32,
    pub temperature: u32,
    pub pressure: u32,
    pub vel_x: i32,
    pub vel_y: i32,
    pub vel_z: i32,
    pub flags: u32,
}

/// GPU pick buffer + staging buffer for async readback.
pub struct PickBuffer {
    /// Storage buffer bound to the ray march shader (binding 5, read_write).
    pub pick_buffer: wgpu::Buffer,
    /// Staging buffer for CPU readback (MAP_READ + COPY_DST).
    staging_buffer: wgpu::Buffer,
    /// Whether a readback is in progress.
    pending: bool,
}

impl PickBuffer {
    /// Create the pick buffer and staging buffer at init time (C-PERF-2).
    pub fn new(device: &wgpu::Device) -> Self {
        let pick_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pick-buffer"),
            size: PICK_BUFFER_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pick-staging"),
            size: PICK_BUFFER_SIZE,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pick_buffer,
            staging_buffer,
            pending: false,
        }
    }

    /// Request a readback: copy pick buffer → staging buffer.
    /// Called once per frame after the ray march dispatch.
    pub fn request_readback(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if self.pending {
            return;
        }
        encoder.copy_buffer_to_buffer(&self.pick_buffer, 0, &self.staging_buffer, 0, PICK_BUFFER_SIZE);
        self.pending = true;
    }

    /// Poll the staging buffer for readback results. Non-blocking (C-GPU-8).
    /// Returns Some(PickResult) if data is ready, None otherwise.
    pub fn poll_readback(&mut self, device: &wgpu::Device) -> Option<PickResult> {
        if !self.pending {
            return None;
        }

        let slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Poll);

        match rx.try_recv() {
            Ok(Ok(())) => {
                let data = slice.get_mapped_range();
                let values: &[u32] = bytemuck::cast_slice(&data);
                let result = if values.len() >= 8 && values[3] != 0 {
                    // Decode velocity_packed: 3 i8 values packed as u32
                    let vp = values[6];
                    let vx_u8 = vp & 0xFF;
                    let vy_u8 = (vp >> 8) & 0xFF;
                    let vz_u8 = (vp >> 16) & 0xFF;
                    let vx = (vx_u8 as i32) - if vx_u8 >= 128 { 256 } else { 0 };
                    let vy = (vy_u8 as i32) - if vy_u8 >= 128 { 256 } else { 0 };
                    let vz = (vz_u8 as i32) - if vz_u8 >= 128 { 256 } else { 0 };
                    PickResult {
                        valid: true,
                        world_x: values[0] as i32,
                        world_y: values[1] as i32,
                        world_z: values[2] as i32,
                        material_id: values[3],
                        temperature: values[4],
                        pressure: values[5],
                        vel_x: vx,
                        vel_y: vy,
                        vel_z: vz,
                        flags: values[7],
                    }
                } else {
                    PickResult::default()
                };
                drop(data);
                self.staging_buffer.unmap();
                self.pending = false;
                Some(result)
            }
            _ => None,
        }
    }
}
