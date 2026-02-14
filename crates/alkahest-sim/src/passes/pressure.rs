use alkahest_core::constants::CHUNK_SIZE;

/// Create the pressure compute pipeline.
pub fn create_pressure_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    shader_source: &str,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("pressure-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pressure-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("pressure-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Dispatch the pressure pass over all active chunks.
/// Workgroup is 8x8x4, dispatch z = active_chunk_count * (CHUNK_SIZE / 4).
pub fn dispatch_pressure(
    pass: &mut wgpu::ComputePass,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    active_chunk_count: u32,
) {
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(
        CHUNK_SIZE / 8,
        CHUNK_SIZE / 8,
        active_chunk_count * (CHUNK_SIZE / 4),
    );
}
