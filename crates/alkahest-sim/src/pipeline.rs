use alkahest_core::constants::{CHUNK_DESC_STRIDE, CHUNK_SIZE, MAX_CHUNK_SLOTS, VOXELS_PER_CHUNK};

use alkahest_rules::GpuRuleData;

use crate::buffers::ChunkPool;
use crate::conflict::{build_movement_schedule, MovementUniforms, SubPass};
pub use crate::passes::commands::SimCommand;
use crate::passes::commands::{self, SimParams, MAX_COMMANDS};
use crate::passes::movement;
use crate::passes::reactions;
use crate::passes::thermal;

/// GPU debug buffer size (C-GPU-10).
const DEBUG_BUFFER_SIZE: u64 = 4096;

/// Reaction uniform struct uploaded each tick. Must match ReactionUniforms in reactions.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ReactionUniforms {
    tick: u32,
    material_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
    _pad4: u32,
    _pad5: u32,
}

/// Single public struct owning the entire simulation pipeline.
///
/// All GPU resources created at init time (C-PERF-2).
pub struct SimPipeline {
    chunk_pool: ChunkPool,
    material_props_buffer: wgpu::Buffer,
    rule_lookup_buffer: wgpu::Buffer,
    rule_data_buffer: wgpu::Buffer,
    command_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    chunk_desc_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    debug_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    command_pipeline: wgpu::ComputePipeline,
    movement_pipeline: wgpu::ComputePipeline,
    reaction_pipeline: wgpu::ComputePipeline,
    thermal_pipeline: wgpu::ComputePipeline,
    activity_pipeline: wgpu::ComputePipeline,
    activity_bind_group_layout: wgpu::BindGroupLayout,
    activity_flags_buffer: wgpu::Buffer,
    /// Double-buffered staging for async readback of activity flags (C-GPU-8).
    staging_buffers: [wgpu::Buffer; 2],
    staging_index: usize,
    movement_schedule: Vec<SubPass>,
    material_count: u32,
    pending_commands: Vec<SimCommand>,
    tick_count: u64,
    paused: bool,
    single_step_requested: bool,
}

impl SimPipeline {
    /// Create the simulation pipeline with all GPU resources (C-PERF-2).
    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue, rule_data: GpuRuleData) -> Self {
        let chunk_pool = ChunkPool::new(device);

        // Command buffer (fixed capacity, C-PERF-2)
        let command_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim-command-buffer"),
            size: (MAX_COMMANDS as u64) * std::mem::size_of::<SimCommand>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Uniform buffer (32 bytes, covers all pass uniform structs)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim-uniform-buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Chunk descriptor buffer: CHUNK_DESC_STRIDE * 4 bytes per chunk entry
        let chunk_desc_size = MAX_CHUNK_SLOTS as u64 * CHUNK_DESC_STRIDE as u64 * 4;
        let chunk_desc_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-descriptor-buffer"),
            size: chunk_desc_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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

        // Activity flags buffer: 1 u32 per chunk
        let activity_flags_size = MAX_CHUNK_SLOTS as u64 * 4;
        let activity_flags_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("activity-flags-buffer"),
            size: activity_flags_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Staging buffers for async readback (C-GPU-8)
        let staging_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("activity-staging-0"),
                size: activity_flags_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("activity-staging-1"),
                size: activity_flags_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];

        // Main sim bind group layout: 8 bindings (C-GPU-3: at limit)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sim-bind-group-layout"),
            entries: &[
                // binding 0: read pool (storage, read)
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
                // binding 1: write pool (storage, read_write)
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
                // binding 5: rule lookup (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 6: rule data (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 7: chunk descriptors (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Activity scan bind group layout (separate, 5 bindings)
        let activity_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("activity-bind-group-layout"),
                entries: &[
                    // binding 0: read pool (storage, read)
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
                    // binding 1: write pool (storage, read — post-sim state)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 2: activity flags (storage, read_write)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 3: uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 4: chunk descriptors (storage, read)
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Compose shader sources
        let constants_preamble = format!(
            "const CHUNK_SIZE: u32 = {}u;\nconst VOXELS_PER_CHUNK: u32 = {}u;\n\
             const DIFFUSION_RATE: f32 = {:.6};\n\
             const ENTROPY_DRAIN_RATE: u32 = {}u;\n\
             const CONVECTION_THRESHOLD: u32 = {}u;\n\
             const AMBIENT_TEMP_QUANTIZED: u32 = {}u;\n\
             const TEMP_QUANT_MAX_VALUE: u32 = {}u;\n\
             const CHUNK_DESC_STRIDE: u32 = {}u;\n\
             const SENTINEL_NEIGHBOR: u32 = {}u;\n\
             const WORLD_CHUNKS_X: u32 = {}u;\n\
             const WORLD_CHUNKS_Y: u32 = {}u;\n\
             const WORLD_CHUNKS_Z: u32 = {}u;\n",
            CHUNK_SIZE,
            VOXELS_PER_CHUNK,
            alkahest_core::constants::DIFFUSION_RATE,
            alkahest_core::constants::ENTROPY_DRAIN_RATE,
            alkahest_core::constants::CONVECTION_THRESHOLD,
            alkahest_core::constants::AMBIENT_TEMP_QUANTIZED,
            alkahest_core::constants::TEMP_QUANT_MAX_VALUE,
            CHUNK_DESC_STRIDE,
            alkahest_core::constants::SENTINEL_NEIGHBOR,
            alkahest_core::constants::WORLD_CHUNKS_X,
            alkahest_core::constants::WORLD_CHUNKS_Y,
            alkahest_core::constants::WORLD_CHUNKS_Z,
        );
        let types_wgsl = include_str!("../../../shaders/common/types.wgsl");
        let coords_wgsl = include_str!("../../../shaders/common/coords.wgsl");
        let rng_wgsl = include_str!("../../../shaders/common/rng.wgsl");
        let commands_wgsl = include_str!("../../../shaders/sim/commands.wgsl");
        let movement_wgsl = include_str!("../../../shaders/sim/movement.wgsl");
        let reactions_wgsl = include_str!("../../../shaders/sim/reactions.wgsl");
        let thermal_wgsl = include_str!("../../../shaders/sim/thermal.wgsl");
        let activity_wgsl = include_str!("../../../shaders/sim/activity.wgsl");

        let command_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{commands_wgsl}"
        );
        let movement_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{movement_wgsl}"
        );
        let reactions_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{reactions_wgsl}"
        );
        let thermal_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{thermal_wgsl}"
        );
        let activity_shader_source = format!("{constants_preamble}\n{activity_wgsl}");

        let command_pipeline =
            commands::create_command_pipeline(device, &bind_group_layout, &command_shader_source);
        let movement_pipeline =
            movement::create_movement_pipeline(device, &bind_group_layout, &movement_shader_source);
        let reaction_pipeline = reactions::create_reaction_pipeline(
            device,
            &bind_group_layout,
            &reactions_shader_source,
        );
        let thermal_pipeline =
            thermal::create_thermal_pipeline(device, &bind_group_layout, &thermal_shader_source);
        let activity_pipeline = crate::passes::activity::create_activity_pipeline(
            device,
            &activity_bind_group_layout,
            &activity_shader_source,
        );

        let movement_schedule = build_movement_schedule();

        Self {
            chunk_pool,
            material_props_buffer: rule_data.material_props_buffer,
            rule_lookup_buffer: rule_data.rule_lookup_buffer,
            rule_data_buffer: rule_data.rule_data_buffer,
            command_buffer,
            uniform_buffer,
            chunk_desc_buffer,
            debug_buffer,
            bind_group_layout,
            command_pipeline,
            movement_pipeline,
            reaction_pipeline,
            thermal_pipeline,
            activity_pipeline,
            activity_bind_group_layout,
            activity_flags_buffer,
            staging_buffers,
            staging_index: 0,
            movement_schedule,
            material_count: rule_data.material_count,
            pending_commands: Vec::new(),
            tick_count: 0,
            paused: false,
            single_step_requested: false,
        }
    }

    /// Get the chunk pool for uploading terrain data.
    pub fn chunk_pool(&self) -> &ChunkPool {
        &self.chunk_pool
    }

    /// Get the chunk pool's slot count.
    pub fn pool_slot_count(&self) -> u32 {
        self.chunk_pool.slot_count()
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

    /// Upload chunk descriptor data to the GPU.
    pub fn upload_chunk_descriptors(&self, queue: &wgpu::Queue, descriptor_data: &[u32]) {
        if !descriptor_data.is_empty() {
            queue.write_buffer(
                &self.chunk_desc_buffer,
                0,
                bytemuck::cast_slice(descriptor_data),
            );
        }
    }

    /// Run one simulation tick over all active chunks.
    ///
    /// `active_slots` is the list of pool slot indices for active chunks.
    /// Returns true if the simulation actually ticked (not paused).
    pub fn tick(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        active_chunk_count: u32,
        active_slots: &[u32],
    ) -> bool {
        // Check pause state
        if self.paused && !self.single_step_requested {
            self.pending_commands.clear();
            return false;
        }
        self.single_step_requested = false;

        if active_chunk_count == 0 {
            self.pending_commands.clear();
            return true;
        }

        let command_count = self.pending_commands.len() as u32;

        // Copy read pool → write pool for each active chunk slot
        for &slot in active_slots {
            self.chunk_pool.copy_slot_read_to_write(encoder, slot);
        }

        // Create bind group for this tick (references current read/write pools)
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sim-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.chunk_pool.read_pool().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.chunk_pool.write_pool().as_entire_binding(),
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
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.rule_lookup_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.rule_data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.chunk_desc_buffer.as_entire_binding(),
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

        // Pass 2: Movement sub-passes (28 sub-passes, batched over all active chunks)
        for sub_pass in &self.movement_schedule {
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
            movement::dispatch_movement(
                &mut pass,
                &self.movement_pipeline,
                &bind_group,
                active_chunk_count,
            );
        }

        // Pass 3: Reactions (batched)
        {
            let uniforms = ReactionUniforms {
                tick: self.tick_count as u32,
                material_count: self.material_count,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
                _pad3: 0,
                _pad4: 0,
                _pad5: 0,
            };
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("sim-reactions-pass"),
                timestamp_writes: None,
            });
            reactions::dispatch_reactions(
                &mut pass,
                &self.reaction_pipeline,
                &bind_group,
                active_chunk_count,
            );
        }

        // Pass 4: Thermal diffusion (batched)
        {
            let uniforms = ReactionUniforms {
                tick: self.tick_count as u32,
                material_count: self.material_count,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
                _pad3: 0,
                _pad4: 0,
                _pad5: 0,
            };
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("sim-thermal-pass"),
                timestamp_writes: None,
            });
            thermal::dispatch_thermal(
                &mut pass,
                &self.thermal_pipeline,
                &bind_group,
                active_chunk_count,
            );
        }

        // Pass 5: Activity scan (separate bind group)
        {
            let activity_uniforms = ReactionUniforms {
                tick: self.tick_count as u32,
                material_count: active_chunk_count,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
                _pad3: 0,
                _pad4: 0,
                _pad5: 0,
            };
            queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::bytes_of(&activity_uniforms),
            );

            // Clear activity flags before scan (each workgroup atomicOr's into these)
            encoder.clear_buffer(&self.activity_flags_buffer, 0, None);

            let activity_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("activity-bind-group"),
                layout: &self.activity_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.chunk_pool.read_pool().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.chunk_pool.write_pool().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.activity_flags_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.chunk_desc_buffer.as_entire_binding(),
                    },
                ],
            });

            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("sim-activity-pass"),
                timestamp_writes: None,
            });
            crate::passes::activity::dispatch_activity(
                &mut pass,
                &self.activity_pipeline,
                &activity_bind_group,
                active_chunk_count,
            );
        }

        // Swap pool read/write indices
        self.chunk_pool.swap();
        self.tick_count += 1;
        self.pending_commands.clear();

        // Request async readback of activity flags
        self.request_readback(encoder, active_chunk_count);

        true
    }

    /// Copy activity flags to staging buffer and initiate async readback (C-GPU-8).
    fn request_readback(&mut self, encoder: &mut wgpu::CommandEncoder, active_chunk_count: u32) {
        let copy_size = (active_chunk_count as u64 * 4).min(MAX_CHUNK_SLOTS as u64 * 4);
        if copy_size == 0 {
            return;
        }

        let staging = &self.staging_buffers[self.staging_index];
        encoder.copy_buffer_to_buffer(&self.activity_flags_buffer, 0, staging, 0, copy_size);

        self.staging_index = 1 - self.staging_index;
    }

    /// Poll for readback completion and return activity flags if available.
    /// Returns None if no readback has completed yet.
    pub fn poll_readback(
        &self,
        device: &wgpu::Device,
        active_chunk_count: u32,
    ) -> Option<Vec<u32>> {
        // Read from the buffer we're NOT currently writing to
        let read_idx = 1 - self.staging_index;
        let staging = &self.staging_buffers[read_idx];

        let slice = staging.slice(..active_chunk_count as u64 * 4);

        // Try to map synchronously (non-blocking check)
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Poll once (non-blocking)
        device.poll(wgpu::Maintain::Poll);

        match rx.try_recv() {
            Ok(Ok(())) => {
                let data = slice.get_mapped_range();
                let flags: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
                drop(data);
                staging.unmap();
                Some(flags)
            }
            _ => None,
        }
    }

    /// Get the read pool buffer (for renderer to sample from).
    pub fn get_read_pool(&self) -> &wgpu::Buffer {
        self.chunk_pool.read_pool()
    }

    /// Get the chunk descriptor buffer (for renderer).
    pub fn get_chunk_desc_buffer(&self) -> &wgpu::Buffer {
        &self.chunk_desc_buffer
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn single_step(&mut self) {
        self.single_step_requested = true;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }
}
