use alkahest_core::constants::CHUNK_SIZE;

/// Create the reaction compute pipeline.
/// Workgroup is 8x8x4 = 256. Chunk is 32x32x32. Dispatch is 4x4x8 workgroups.
pub fn create_reaction_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    shader_source: &str,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("reactions-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("reactions-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("reactions-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Dispatch the reactions pass over the full chunk.
/// Workgroup is 8x8x4, chunk is 32x32x32, so dispatch (4, 4, 8) workgroups.
pub fn dispatch_reactions(
    pass: &mut wgpu::ComputePass,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
) {
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(CHUNK_SIZE / 8, CHUNK_SIZE / 8, CHUNK_SIZE / 4);
}
