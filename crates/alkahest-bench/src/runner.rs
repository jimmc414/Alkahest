use std::time::Instant;

use alkahest_core::constants::*;
use alkahest_render::Renderer;
use alkahest_sim::pipeline::SimPipeline;
use alkahest_world::World;
use glam::IVec3;

use crate::scenes::SceneConfig;

/// Timing data for a single benchmark run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimingSeries {
    pub mean_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

/// Result of a single scene benchmark.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkResult {
    pub scene_name: String,
    pub active_voxels: u32,
    pub chunk_count: u32,
    pub tick_count: u32,
    pub timings: TimingSeries,
}

/// Runs benchmarks on native GPU (not WASM).
pub struct BenchmarkRunner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    tick_count: u32,
}

impl BenchmarkRunner {
    /// Initialize wgpu natively. Blocks on async adapter request.
    pub fn new(tick_count: u32) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no suitable GPU adapter found");

        log::info!("Benchmark adapter: {}", adapter.get_info().name);

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bench-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .expect("failed to create device");

        Self {
            device,
            queue,
            tick_count,
        }
    }

    /// Run a single benchmark scene and return timing results.
    pub fn run_scene(&self, config: &SceneConfig) -> BenchmarkResult {
        log::info!(
            "Running scene '{}' ({} target voxels)...",
            config.name,
            config.target_active_voxels
        );

        // Load rules
        let (rule_data, _material_table) = Self::load_rules(&self.device);

        // Create subsystems
        let mut sim = SimPipeline::new(&self.device, &self.queue, rule_data);
        let renderer = Renderer::new(
            &self.device,
            &self.queue,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            640,
            480,
        );

        let mut world = World::new();

        // Populate scene with bench data
        let num_chunks = crate::scenes::scene_chunk_count(config);
        let voxels_per_chunk = VOXELS_PER_CHUNK;
        let mut total_voxels_placed = 0u32;

        for chunk_idx in 0..num_chunks {
            // Map chunk index to a coordinate in the world grid
            let cx = chunk_idx % WORLD_CHUNKS_X;
            let cy = (chunk_idx / WORLD_CHUNKS_X) % WORLD_CHUNKS_Y;
            let cz = chunk_idx / (WORLD_CHUNKS_X * WORLD_CHUNKS_Y);
            if cz >= WORLD_CHUNKS_Z {
                break;
            }
            let coord = IVec3::new(cx as i32, cy as i32, cz as i32);

            let remaining = config
                .target_active_voxels
                .saturating_sub(total_voxels_placed);
            let fill = remaining.min(voxels_per_chunk);
            let data = crate::scenes::generate_bench_chunk(chunk_idx, fill);

            if let Some(pool_slot) = world.chunk_map().get(&coord).and_then(|c| c.pool_slot) {
                sim.chunk_pool()
                    .upload_chunk_data_both(&self.queue, pool_slot, &data);
                if let Some(chunk) = world.chunk_map_mut().get_mut(&coord) {
                    chunk.has_non_air = fill > 0;
                }
            }

            total_voxels_placed += fill;
        }

        log::info!(
            "  Populated {} voxels across {} chunks",
            total_voxels_placed,
            num_chunks
        );

        // Build initial chunk map for renderer
        let chunk_map_data = build_renderer_chunk_map(world.chunk_map());
        renderer.update_chunk_map(&self.queue, &chunk_map_data);

        // Run ticks and time each frame
        let mut frame_times = Vec::with_capacity(self.tick_count as usize);

        for _ in 0..self.tick_count {
            let camera_pos = glam::Vec3::new(
                config.camera_position[0],
                config.camera_position[1],
                config.camera_position[2],
            );
            let dispatch_list = world.update(camera_pos);
            let descriptor_data = dispatch_list.build_descriptor_data();
            let active_chunk_count = dispatch_list.len() as u32;
            let active_slots: Vec<u32> =
                dispatch_list.entries.iter().map(|e| e.pool_slot).collect();

            let frame_start = Instant::now();

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("bench-encoder"),
                });

            // Sim tick
            sim.upload_chunk_descriptors(&self.queue, &descriptor_data);
            sim.upload_commands(&self.queue);
            sim.tick(
                &self.device,
                &self.queue,
                &mut encoder,
                active_chunk_count,
                &active_slots,
            );

            // Render (compute ray march only, no surface blit needed for timing)
            let render_width = 640u32;
            let render_height = 480u32;
            renderer.render(
                &mut encoder,
                &renderer_dummy_view(&self.device),
                render_width,
                render_height,
            );

            self.queue.submit(std::iter::once(encoder.finish()));
            self.device.poll(wgpu::Maintain::Wait);

            let elapsed = frame_start.elapsed().as_secs_f64() * 1000.0;
            frame_times.push(elapsed);
        }

        let timings = compute_timings(&frame_times);
        log::info!(
            "  Done: mean={:.2}ms, p95={:.2}ms, p99={:.2}ms",
            timings.mean_ms,
            timings.p95_ms,
            timings.p99_ms
        );

        BenchmarkResult {
            scene_name: config.name.to_string(),
            active_voxels: total_voxels_placed,
            chunk_count: num_chunks,
            tick_count: self.tick_count,
            timings,
        }
    }

    /// Load and compile rule engine data (same pattern as app.rs).
    fn load_rules(
        device: &wgpu::Device,
    ) -> (
        alkahest_rules::GpuRuleData,
        alkahest_core::material::MaterialTable,
    ) {
        let naturals_ron = include_str!("../../../data/materials/naturals.ron");
        let organics_ron = include_str!("../../../data/materials/organics.ron");
        let energy_ron = include_str!("../../../data/materials/energy.ron");
        let explosives_ron = include_str!("../../../data/materials/explosives.ron");
        let metals_ron = include_str!("../../../data/materials/metals.ron");
        let synthetics_ron = include_str!("../../../data/materials/synthetics.ron");
        let exotic_ron = include_str!("../../../data/materials/exotic.ron");

        let combustion_ron = include_str!("../../../data/rules/combustion.ron");
        let structural_ron = include_str!("../../../data/rules/structural.ron");
        let phase_change_ron = include_str!("../../../data/rules/phase_change.ron");
        let dissolution_ron = include_str!("../../../data/rules/dissolution.ron");
        let displacement_ron = include_str!("../../../data/rules/displacement.ron");
        let biological_ron = include_str!("../../../data/rules/biological.ron");
        let thermal_ron = include_str!("../../../data/rules/thermal.ron");
        let synthesis_ron = include_str!("../../../data/rules/synthesis.ron");

        let materials = alkahest_rules::loader::load_all_materials(&[
            naturals_ron,
            organics_ron,
            energy_ron,
            explosives_ron,
            metals_ron,
            synthetics_ron,
            exotic_ron,
        ])
        .expect("failed to parse material RON data");

        let rules = alkahest_rules::loader::load_all_rules(&[
            combustion_ron,
            structural_ron,
            phase_change_ron,
            dissolution_ron,
            displacement_ron,
            biological_ron,
            thermal_ron,
            synthesis_ron,
        ])
        .expect("failed to parse rules RON data");

        if let Err(errors) = alkahest_rules::validator::validate_materials(&materials) {
            for e in &errors {
                log::error!("Material validation error: {e}");
            }
            panic!("Material validation failed with {} errors", errors.len());
        }
        if let Err(errors) = alkahest_rules::validator::validate_rules(&rules, &materials) {
            for e in &errors {
                log::error!("Rule validation error: {e}");
            }
            panic!("Rule validation failed with {} errors", errors.len());
        }

        let gpu_data = alkahest_rules::compiler::compile(device, &materials, &rules);
        (gpu_data, materials)
    }
}

/// Build a flat grid-indexed chunk map for the renderer (same logic as app.rs).
fn build_renderer_chunk_map(chunk_map: &alkahest_world::chunk_map::ChunkMap) -> Vec<u32> {
    let total = (WORLD_CHUNKS_X * WORLD_CHUNKS_Y * WORLD_CHUNKS_Z) as usize;
    let mut data = vec![SENTINEL_NEIGHBOR; total];

    for (coord, chunk) in chunk_map.iter() {
        if let Some(pool_slot) = chunk.pool_slot {
            let idx = coord.z as u32 * WORLD_CHUNKS_X * WORLD_CHUNKS_Y
                + coord.y as u32 * WORLD_CHUNKS_X
                + coord.x as u32;
            if (idx as usize) < total {
                data[idx as usize] = pool_slot * BYTES_PER_CHUNK;
            }
        }
    }

    data
}

/// Create a dummy texture view for render pass (bench doesn't present to screen).
fn renderer_dummy_view(device: &wgpu::Device) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench-dummy-surface"),
        size: wgpu::Extent3d {
            width: 640,
            height: 480,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Compute timing statistics from a list of frame times in milliseconds.
fn compute_timings(times: &[f64]) -> TimingSeries {
    if times.is_empty() {
        return TimingSeries {
            mean_ms: 0.0,
            median_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
        };
    }

    let mut sorted = times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = sorted.len();
    let mean = sorted.iter().sum::<f64>() / n as f64;
    let median = if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    };
    let p95_idx = ((n as f64) * 0.95).ceil() as usize;
    let p99_idx = ((n as f64) * 0.99).ceil() as usize;

    TimingSeries {
        mean_ms: mean,
        median_ms: median,
        p95_ms: sorted[p95_idx.min(n - 1)],
        p99_ms: sorted[p99_idx.min(n - 1)],
        min_ms: sorted[0],
        max_ms: sorted[n - 1],
    }
}
