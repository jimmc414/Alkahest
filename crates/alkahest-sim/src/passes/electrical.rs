use alkahest_core::constants::CHUNK_SIZE;

/// Create the electrical propagation bind group layout (7 bindings).
///
/// This is a separate layout from the main sim bind group because the electrical
/// pass needs charge_read + charge_write buffers, which would exceed the 8-binding
/// limit (C-GPU-3) if added to the main layout.
pub fn create_electrical_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("electrical-bind-group-layout"),
        entries: &[
            // binding 0: read pool (storage, read) — for cross_chunk_voxel material lookups
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
            // binding 1: write pool (storage, read_write) — for Joule heating temp updates
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
            // binding 3: charge_read (storage, read)
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
            // binding 4: charge_write (storage, read_write)
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 5: uniforms
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 6: chunk descriptors (storage, read)
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
    })
}

/// Create the electrical propagation compute pipeline.
pub fn create_electrical_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    shader_source: &str,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("electrical-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("electrical-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("electrical-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Create the charge-augmented reaction pipeline layout.
///
/// Reactions need @group(0) for the main sim bind group (8 bindings) and
/// @group(1) @binding(0) for charge_read (1 binding), totaling 9 bindings
/// across 2 groups (staying within C-GPU-3 per-group limit).
pub fn create_charge_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("charge-read-bind-group-layout"),
        entries: &[
            // binding 0: charge_read (storage, read)
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
        ],
    })
}

/// Create a reaction pipeline that uses two bind groups: main sim (@group(0)) + charge (@group(1)).
pub fn create_charge_reaction_pipeline(
    device: &wgpu::Device,
    main_bind_group_layout: &wgpu::BindGroupLayout,
    charge_bind_group_layout: &wgpu::BindGroupLayout,
    shader_source: &str,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("reactions-charge-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("reactions-charge-pipeline-layout"),
        bind_group_layouts: &[main_bind_group_layout, charge_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("reactions-charge-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Dispatch the electrical propagation pass over all active chunks.
/// Workgroup is 8x8x4, dispatch z = active_chunk_count * (CHUNK_SIZE / 4).
pub fn dispatch_electrical(
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

/// Dispatch reactions with charge condition support (two bind groups).
pub fn dispatch_reactions_with_charge(
    pass: &mut wgpu::ComputePass,
    pipeline: &wgpu::ComputePipeline,
    main_bind_group: &wgpu::BindGroup,
    charge_bind_group: &wgpu::BindGroup,
    active_chunk_count: u32,
) {
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, main_bind_group, &[]);
    pass.set_bind_group(1, charge_bind_group, &[]);
    pass.dispatch_workgroups(
        CHUNK_SIZE / 8,
        CHUNK_SIZE / 8,
        active_chunk_count * (CHUNK_SIZE / 4),
    );
}
