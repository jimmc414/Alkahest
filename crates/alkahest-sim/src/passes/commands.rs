/// Maximum number of player commands per frame.
pub const MAX_COMMANDS: u32 = 64;

/// GPU-uploadable player command. Must match SimCommand in commands.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimCommand {
    pub tool_type: u32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub pos_z: i32,
    pub material_id: u32,
    pub chunk_dispatch_idx: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

/// Tool type constants matching the shader.
#[allow(dead_code)]
pub const TOOL_PLACE: u32 = 1;
#[allow(dead_code)]
pub const TOOL_REMOVE: u32 = 2;
#[allow(dead_code)]
pub const TOOL_HEAT: u32 = 3;

/// GPU-uploadable simulation parameters. Must match SimParams in commands.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimParams {
    pub tick: u32,
    pub command_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
}

/// Create the command application compute pipeline.
pub fn create_command_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    shader_source: &str,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("commands-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("commands-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("commands-pipeline"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Dispatch the command application pass.
pub fn dispatch_commands(
    pass: &mut wgpu::ComputePass,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    command_count: u32,
) {
    if command_count == 0 {
        return;
    }
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(1, 1, 1);
}
