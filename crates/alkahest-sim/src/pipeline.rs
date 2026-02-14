use alkahest_core::constants::{CHUNK_SIZE, VOXELS_PER_CHUNK};
use alkahest_core::math::pack_voxel;
use alkahest_core::types::MaterialId;
use wgpu::util::DeviceExt;

use crate::buffers::DoubleBuffer;
use crate::conflict::{build_gravity_schedule, MovementUniforms, SubPass};
// Re-export SimCommand for external use (web crate needs it for player tools)
pub use crate::passes::commands::SimCommand;
use crate::passes::commands::{self, SimParams, MAX_COMMANDS};
use crate::passes::movement;

/// Material properties uploaded to the GPU. Must match `materials` buffer layout in shaders.
/// vec4<f32>(density, phase, pad, pad)
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialProps {
    density: f32,
    phase: f32,
    _pad0: f32,
    _pad1: f32,
}

/// Phase constants matching the shader.
const PHASE_GAS: f32 = 0.0;
#[allow(dead_code)] // Used in M3 for liquid movement
const PHASE_LIQUID: f32 = 1.0;
const PHASE_SOLID: f32 = 2.0;
const PHASE_POWDER: f32 = 3.0;

/// GPU debug buffer size (C-GPU-10).
const DEBUG_BUFFER_SIZE: u64 = 4096;

/// Single public struct owning the entire simulation pipeline.
///
/// All GPU resources created at init time (C-PERF-2).
pub struct SimPipeline {
    double_buffer: DoubleBuffer,
    material_props_buffer: wgpu::Buffer,
    command_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)] // Read back in debug builds for shader diagnostics (C-GPU-10)
    debug_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    command_pipeline: wgpu::ComputePipeline,
    movement_pipeline: wgpu::ComputePipeline,
    gravity_schedule: Vec<SubPass>,
    pending_commands: Vec<SimCommand>,
    tick_count: u64,
    paused: bool,
    single_step_requested: bool,
}

impl SimPipeline {
    /// Create the simulation pipeline with all GPU resources (C-PERF-2).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let double_buffer = DoubleBuffer::new(device);

        // Upload initial scene to both buffers
        let scene_data = Self::build_initial_scene();
        let scene_bytes: &[u8] = bytemuck::cast_slice(&scene_data);
        queue.write_buffer(double_buffer.read_buffer(), 0, scene_bytes);
        queue.write_buffer(double_buffer.write_buffer(), 0, scene_bytes);

        // Material properties buffer (C-DESIGN-1: density-driven, not material ID checks)
        let material_props = Self::build_material_props();
        let material_props_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material-props"),
            contents: bytemuck::cast_slice(&material_props),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Command buffer (fixed capacity, C-PERF-2)
        let command_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim-command-buffer"),
            size: (MAX_COMMANDS as u64) * std::mem::size_of::<SimCommand>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Uniform buffer for sim params / movement params (reused each sub-pass)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim-uniform-buffer"),
            size: 32, // max(sizeof(SimParams), sizeof(MovementUniforms)) = 32
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Debug buffer (C-GPU-10)
        let debug_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim-debug-buffer"),
            size: DEBUG_BUFFER_SIZE,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shared bind group layout: both command and movement passes use the same layout.
        // Storage buffers per shader: read(0) + write(1) + materials(2) + commands(3) = 4 storage
        // + 1 uniform(4) = 5 bindings total, well within C-GPU-3 limit of 8.
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sim-bind-group-layout"),
            entries: &[
                // binding 0: read buffer (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: write buffer (storage, read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: material properties (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: command buffer (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 4: uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Compose shader sources
        let constants_preamble = format!(
            "const CHUNK_SIZE: u32 = {}u;\nconst VOXELS_PER_CHUNK: u32 = {}u;\n",
            CHUNK_SIZE, VOXELS_PER_CHUNK,
        );
        let types_wgsl = include_str!("../../../shaders/common/types.wgsl");
        let coords_wgsl = include_str!("../../../shaders/common/coords.wgsl");
        let rng_wgsl = include_str!("../../../shaders/common/rng.wgsl");
        let commands_wgsl = include_str!("../../../shaders/sim/commands.wgsl");
        let movement_wgsl = include_str!("../../../shaders/sim/movement.wgsl");

        let command_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{commands_wgsl}"
        );
        let movement_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{movement_wgsl}"
        );

        let command_pipeline =
            commands::create_command_pipeline(device, &bind_group_layout, &command_shader_source);
        let movement_pipeline =
            movement::create_movement_pipeline(device, &bind_group_layout, &movement_shader_source);

        let gravity_schedule = build_gravity_schedule();

        Self {
            double_buffer,
            material_props_buffer,
            command_buffer,
            uniform_buffer,
            debug_buffer,
            bind_group_layout,
            command_pipeline,
            movement_pipeline,
            gravity_schedule,
            pending_commands: Vec::new(),
            tick_count: 0,
            paused: false,
            single_step_requested: false,
        }
    }

    /// Enqueue a player command for the next tick.
    pub fn enqueue_command(&mut self, cmd: SimCommand) {
        if self.pending_commands.len() < MAX_COMMANDS as usize {
            self.pending_commands.push(cmd);
        }
    }

    /// Upload pending commands to the GPU command buffer.
    pub fn upload_commands(&mut self, queue: &wgpu::Queue) {
        if !self.pending_commands.is_empty() {
            queue.write_buffer(
                &self.command_buffer,
                0,
                bytemuck::cast_slice(&self.pending_commands),
            );
        }
    }

    /// Run one simulation tick: dispatch Pass 1 (commands) + Pass 2 (movement sub-passes).
    ///
    /// Returns true if the simulation actually ticked (not paused).
    pub fn tick(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> bool {
        // Check pause state
        if self.paused && !self.single_step_requested {
            // Still process commands even while paused (they'll apply on resume)
            self.pending_commands.clear();
            return false;
        }
        self.single_step_requested = false;

        let command_count = self.pending_commands.len() as u32;

        // Copy read buffer to write buffer first, so the write buffer starts
        // with the current state. Passes then modify the write buffer in place.
        encoder.copy_buffer_to_buffer(
            self.double_buffer.read_buffer(),
            0,
            self.double_buffer.write_buffer(),
            0,
            crate::buffers::VOXEL_BUFFER_SIZE,
        );

        // Create bind group for this tick (references current read/write buffers)
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sim-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.double_buffer.read_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.double_buffer.write_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.material_props_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.command_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Pass 1: Apply player commands
        if command_count > 0 {
            let params = SimParams {
                tick: self.tick_count as u32,
                command_count,
                _pad0: 0,
                _pad1: 0,
            };
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&params));

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("sim-commands-pass"),
                timestamp_writes: None,
            });
            commands::dispatch_commands(
                &mut pass,
                &self.command_pipeline,
                &bind_group,
                command_count,
            );
        }

        // Pass 2: Movement sub-passes (C-SIM-2: fixed order)
        for sub_pass in &self.gravity_schedule {
            let uniforms = MovementUniforms {
                direction: sub_pass.direction,
                parity: sub_pass.parity,
                tick: self.tick_count as u32,
                _pad: [0; 3],
            };
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("sim-movement-pass"),
                timestamp_writes: None,
            });
            movement::dispatch_movement(&mut pass, &self.movement_pipeline, &bind_group);
        }

        // Swap buffers and advance tick
        self.double_buffer.swap();
        self.tick_count += 1;
        self.pending_commands.clear();

        true
    }

    /// Get the buffer the renderer should read from (most recently written).
    pub fn get_read_buffer(&self) -> &wgpu::Buffer {
        self.double_buffer.read_buffer()
    }

    /// Pause the simulation.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume the simulation.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Toggle pause state.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Request a single simulation step (advances by exactly 1 tick).
    pub fn single_step(&mut self) {
        self.single_step_requested = true;
    }

    /// Whether the simulation is paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Build the M2 initial scene: stone floor + sand pyramid + emissive voxel.
    /// Same layout as the M1 test scene in the renderer.
    fn build_initial_scene() -> Vec<[u32; 2]> {
        let cs = CHUNK_SIZE as usize;
        let total = cs * cs * cs;
        let air = pack_voxel(MaterialId(0), 0, 0, 0, 0, 0, 0);
        let mut data = vec![[air.low, air.high]; total];

        let stone = pack_voxel(MaterialId(1), 150, 0, 0, 0, 0, 0);
        let sand = pack_voxel(MaterialId(2), 150, 0, 0, 0, 0, 0);
        let emissive = pack_voxel(MaterialId(3), 150, 0, 0, 0, 0, 0);

        // Stone floor at y=0
        for z in 0..cs {
            for x in 0..cs {
                let idx = x + z * cs * cs;
                data[idx] = [stone.low, stone.high];
            }
        }

        // Sand pyramid: y=1..=8, centered at chunk center
        let half = cs / 2;
        for layer in 1u32..=8 {
            let radius = (9 - layer) as i32;
            let min_c = half as i32 - radius;
            let max_c = half as i32 + radius;
            for z in min_c..max_c {
                for x in min_c..max_c {
                    if x >= 0 && x < cs as i32 && z >= 0 && z < cs as i32 {
                        let idx = x as usize + layer as usize * cs + z as usize * cs * cs;
                        data[idx] = [sand.low, sand.high];
                    }
                }
            }
        }

        // Emissive voxel at (28, 20, 28)
        {
            let idx = 28 + 20 * cs + 28 * cs * cs;
            data[idx] = [emissive.low, emissive.high];
        }

        data
    }

    /// Build M2 material properties table.
    fn build_material_props() -> Vec<MaterialProps> {
        vec![
            // 0: Air
            MaterialProps {
                density: 0.0,
                phase: PHASE_GAS,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            // 1: Stone
            MaterialProps {
                density: 5000.0,
                phase: PHASE_SOLID,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            // 2: Sand
            MaterialProps {
                density: 2500.0,
                phase: PHASE_POWDER,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            // 3: Emissive
            MaterialProps {
                density: 1000.0,
                phase: PHASE_SOLID,
                _pad0: 0.0,
                _pad1: 0.0,
            },
        ]
    }
}
