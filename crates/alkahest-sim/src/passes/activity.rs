use alkahest_core::constants::CHUNK_SIZE;

/// Create the activity scan compute pipeline.
/// Uses a separate bind group layout (5 bindings) from the main sim passes.
pub fn create_activity_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    shader_source: &str,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("activity-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("activity-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("activity-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Dispatch the activity scan pass over all active chunks.
/// Workgroup is 8×8×4, dispatch z = active_chunk_count * (CHUNK_SIZE / 4).
pub fn dispatch_activity(
    pass: &mut wgpu::ComputePass,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    active_chunk_count: u32,
) {
    if active_chunk_count == 0 {
        return;
    }
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(
        CHUNK_SIZE / 8,
        CHUNK_SIZE / 8,
        active_chunk_count * (CHUNK_SIZE / 4),
    );
}
