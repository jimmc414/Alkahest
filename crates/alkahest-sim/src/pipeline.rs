use alkahest_core::constants::{CHUNK_SIZE, VOXELS_PER_CHUNK};
use alkahest_core::math::pack_voxel;
use alkahest_core::types::MaterialId;

use alkahest_rules::GpuRuleData;

use crate::buffers::DoubleBuffer;
use crate::conflict::{build_movement_schedule, MovementUniforms, SubPass};
// Re-export SimCommand for external use (web crate needs it for player tools)
pub use crate::passes::commands::SimCommand;
use crate::passes::commands::{self, SimParams, MAX_COMMANDS};
use crate::passes::movement;
use crate::passes::reactions;

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
    double_buffer: DoubleBuffer,
    material_props_buffer: wgpu::Buffer,
    rule_lookup_buffer: wgpu::Buffer,
    rule_data_buffer: wgpu::Buffer,
    command_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)] // Read back in debug builds for shader diagnostics (C-GPU-10)
    debug_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    command_pipeline: wgpu::ComputePipeline,
    movement_pipeline: wgpu::ComputePipeline,
    reaction_pipeline: wgpu::ComputePipeline,
    movement_schedule: Vec<SubPass>,
    material_count: u32,
    pending_commands: Vec<SimCommand>,
    tick_count: u64,
    paused: bool,
    single_step_requested: bool,
}

impl SimPipeline {
    /// Create the simulation pipeline with all GPU resources (C-PERF-2).
    ///
    /// Accepts `GpuRuleData` from the rule engine compiler instead of building materials internally.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, rule_data: GpuRuleData) -> Self {
        let double_buffer = DoubleBuffer::new(device);

        // Upload initial scene to both buffers
        let scene_data = Self::build_initial_scene();
        let scene_bytes: &[u8] = bytemuck::cast_slice(&scene_data);
        queue.write_buffer(double_buffer.read_buffer(), 0, scene_bytes);
        queue.write_buffer(double_buffer.write_buffer(), 0, scene_bytes);

        // Command buffer (fixed capacity, C-PERF-2)
        let command_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim-command-buffer"),
            size: (MAX_COMMANDS as u64) * std::mem::size_of::<SimCommand>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Uniform buffer for sim params / movement params / reaction params (reused each sub-pass)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim-uniform-buffer"),
            size: 32, // max(sizeof(SimParams), sizeof(MovementUniforms), sizeof(ReactionUniforms)) = 32
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

        // Shared bind group layout: 7 bindings (C-GPU-3: under limit of 8)
        // 6 storage + 1 uniform = 7 total. One slot remains for M5.
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
                // binding 2: material properties (storage, read) — expanded to 32 bytes/material
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
                // binding 5: rule lookup (storage, read) — NEW for M3
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
                // binding 6: rule data (storage, read) — NEW for M3
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
        let reactions_wgsl = include_str!("../../../shaders/sim/reactions.wgsl");

        let command_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{commands_wgsl}"
        );
        let movement_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{movement_wgsl}"
        );
        let reactions_shader_source = format!(
            "{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{rng_wgsl}\n{reactions_wgsl}"
        );

        let command_pipeline =
            commands::create_command_pipeline(device, &bind_group_layout, &command_shader_source);
        let movement_pipeline =
            movement::create_movement_pipeline(device, &bind_group_layout, &movement_shader_source);
        let reaction_pipeline = reactions::create_reaction_pipeline(
            device,
            &bind_group_layout,
            &reactions_shader_source,
        );

        let movement_schedule = build_movement_schedule();

        Self {
            double_buffer,
            material_props_buffer: rule_data.material_props_buffer,
            rule_lookup_buffer: rule_data.rule_lookup_buffer,
            rule_data_buffer: rule_data.rule_data_buffer,
            command_buffer,
            uniform_buffer,
            debug_buffer,
            bind_group_layout,
            command_pipeline,
            movement_pipeline,
            reaction_pipeline,
            movement_schedule,
            material_count: rule_data.material_count,
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

    /// Run one simulation tick: Pass 1 (commands) + Pass 2 (movement) + Pass 3 (reactions).
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
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.rule_lookup_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.rule_data_buffer.as_entire_binding(),
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

        // Pass 2: Movement sub-passes (C-SIM-2: fixed order, 28 sub-passes)
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
            movement::dispatch_movement(&mut pass, &self.movement_pipeline, &bind_group);
        }

        // Pass 3: Reactions (C-SIM-3: reactions after movement)
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
            reactions::dispatch_reactions(&mut pass, &self.reaction_pipeline, &bind_group);
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

    /// Build the M3 initial scene: stone floor, sand pyramid, wood block, water pool, fire starter.
    fn build_initial_scene() -> Vec<[u32; 2]> {
        let cs = CHUNK_SIZE as usize;
        let total = cs * cs * cs;
        let air = pack_voxel(MaterialId(0), 0, 0, 0, 0, 0, 0);
        let mut data = vec![[air.low, air.high]; total];

        let stone = pack_voxel(MaterialId(1), 150, 0, 0, 0, 0, 0);
        let sand = pack_voxel(MaterialId(2), 150, 0, 0, 0, 0, 0);
        let water = pack_voxel(MaterialId(3), 150, 0, 0, 0, 0, 0);
        let wood = pack_voxel(MaterialId(8), 150, 0, 0, 0, 0, 0);
        let fire = pack_voxel(MaterialId(5), 1536, 0, 0, 0, 0, 0); // ~3000K quantized

        // Stone floor at y=0
        for z in 0..cs {
            for x in 0..cs {
                let idx = x + z * cs * cs;
                data[idx] = [stone.low, stone.high];
            }
        }

        // Sand pyramid: y=1..=5, centered at (8, _, 8)
        for layer in 1u32..=5 {
            let radius = (6 - layer) as i32;
            let cx = 8i32;
            let cz = 8i32;
            for z in (cz - radius)..(cz + radius) {
                for x in (cx - radius)..(cx + radius) {
                    if x >= 0 && x < cs as i32 && z >= 0 && z < cs as i32 {
                        let idx = x as usize + layer as usize * cs + z as usize * cs * cs;
                        data[idx] = [sand.low, sand.high];
                    }
                }
            }
        }

        // Water pool: y=1..=3 at (20-24, _, 20-24)
        for y in 1..=3u32 {
            for z in 20..24 {
                for x in 20..24 {
                    let idx = x + y as usize * cs + z * cs * cs;
                    data[idx] = [water.low, water.high];
                }
            }
        }

        // Wood block: 3x3x3 at (16, 1, 16)
        for y in 1..=3u32 {
            for z in 16..19 {
                for x in 16..19 {
                    let idx = x + y as usize * cs + z * cs * cs;
                    data[idx] = [wood.low, wood.high];
                }
            }
        }

        // Fire starter: single voxel adjacent to wood at (15, 2, 17)
        {
            let idx = 15 + 2 * cs + 17 * cs * cs;
            data[idx] = [fire.low, fire.high];
        }

        data
    }
}
