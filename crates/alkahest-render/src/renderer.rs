use alkahest_core::constants::*;
use wgpu::util::DeviceExt;

use crate::debug_lines::DebugVertex;
use crate::pick::PickBuffer;

/// Maximum number of debug line vertices the buffer can hold.
const MAX_DEBUG_VERTICES: u64 = 1024;

/// GPU-uploadable camera uniforms. Must match CameraUniforms in ray_march.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniforms {
    pub inv_view_proj: [[f32; 4]; 4],
    pub position: [f32; 4],
    pub screen_size: [f32; 2],
    pub near: f32,
    pub fov: f32,
    /// Render mode: 0 = normal, 1 = heatmap.
    pub render_mode: u32,
    /// Cross-section clip axis: 0 = off, 1 = X, 2 = Y, 3 = Z.
    pub clip_axis: u32,
    /// Cross-section clip position (bitcast from f32).
    pub clip_position: u32,
    /// Packed cursor pixel: cursor_x | (cursor_y << 16).
    pub cursor_packed: u32,
}

/// GPU-uploadable light uniforms. Must match LightUniforms in ray_march.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub ambient: [f32; 4],
}

/// GPU-uploadable material color. Must match MaterialColor in ray_march.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialColor {
    pub color: [f32; 3],
    pub emission: f32,
}

/// GPU-uploadable debug line view-projection uniform.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DebugUniforms {
    pub view_proj: [[f32; 4]; 4],
}

/// Single public struct owning all rendering GPU resources (C-DESIGN-4: concrete, no traits).
/// All GPU resources are created at init time (C-PERF-2: no per-frame allocation).
pub struct Renderer {
    // Compute ray march
    ray_march_pipeline: wgpu::ComputePipeline,
    uniform_bind_group: wgpu::BindGroup,
    scene_bind_group: wgpu::BindGroup,
    // Bind group layouts (needed for resize recreation)
    #[allow(dead_code)] // Kept for future uniform bind group recreation on resize
    uniform_bgl: wgpu::BindGroupLayout,
    scene_bgl: wgpu::BindGroupLayout,
    // Storage texture (compute -> blit)
    render_texture: wgpu::Texture,
    render_texture_view: wgpu::TextureView,
    // Blit
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group: wgpu::BindGroup,
    blit_bgl: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    // Debug lines
    debug_lines_pipeline: wgpu::RenderPipeline,
    debug_vertex_buffer: wgpu::Buffer,
    debug_vertex_count: u32,
    debug_uniform_bind_group: wgpu::BindGroup,
    debug_uniform_buffer: wgpu::Buffer,
    // Uniform buffers
    camera_uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)] // Kept for future dynamic light updates
    light_uniform_buffer: wgpu::Buffer,
    // Scene
    voxel_pool_buffer: wgpu::Buffer,
    material_color_buffer: wgpu::Buffer,
    chunk_map_buffer: wgpu::Buffer,
    octree_buffer: wgpu::Buffer,
    // Pick
    pub pick: PickBuffer,
}

impl Renderer {
    /// Build all GPU resources at init time (C-PERF-2).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        // -- Shader source composition --
        // Inject constants (C-DESIGN-3: single source of truth from Rust)
        let constants_preamble = format!(
            "const CHUNK_SIZE: u32 = {}u;\nconst VOXELS_PER_CHUNK: u32 = {}u;\n\
             const WORLD_CHUNKS_X: u32 = {}u;\nconst WORLD_CHUNKS_Y: u32 = {}u;\n\
             const WORLD_CHUNKS_Z: u32 = {}u;\n\
             const SENTINEL_NEIGHBOR: u32 = {}u;\n\
             const CHUNK_DESC_STRIDE: u32 = {}u;\n",
            CHUNK_SIZE,
            VOXELS_PER_CHUNK,
            WORLD_CHUNKS_X,
            WORLD_CHUNKS_Y,
            WORLD_CHUNKS_Z,
            SENTINEL_NEIGHBOR,
            CHUNK_DESC_STRIDE,
        );

        let types_wgsl = include_str!("../../../shaders/common/types.wgsl");
        let coords_wgsl = include_str!("../../../shaders/common/coords.wgsl");
        let ray_march_wgsl = include_str!("../../../shaders/render/ray_march.wgsl");
        let blit_wgsl = include_str!("../../../shaders/render/blit.wgsl");
        let debug_lines_wgsl = include_str!("../../../shaders/render/debug_lines.wgsl");

        // Compose ray march shader: constants + types + coords + ray_march
        let ray_march_source =
            format!("{constants_preamble}\n{types_wgsl}\n{coords_wgsl}\n{ray_march_wgsl}");

        // -- Create shader modules --
        let ray_march_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ray-march-shader"),
            source: wgpu::ShaderSource::Wgsl(ray_march_source.into()),
        });

        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit-shader"),
            source: wgpu::ShaderSource::Wgsl(blit_wgsl.into()),
        });

        let debug_lines_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("debug-lines-shader"),
            source: wgpu::ShaderSource::Wgsl(debug_lines_wgsl.into()),
        });

        // -- Uniform buffers --
        let camera_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-uniforms"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light-uniforms"),
            contents: bytemuck::bytes_of(&LightUniforms {
                position: [28.5, 22.0, 28.5, 1.0],
                color: [1.0, 0.95, 0.8, 1.0],
                ambient: [0.15, 0.15, 0.18, 1.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // -- Voxel pool buffer placeholder (will be replaced by sim pipeline's pool buffer) --
        let voxel_pool_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxel-pool-placeholder"),
            size: (VOXELS_PER_CHUNK as u64) * 8,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // -- Chunk map buffer: WORLD_CHUNKS_X * WORLD_CHUNKS_Y * WORLD_CHUNKS_Z u32 entries --
        // Each entry is a pool_slot_byte_offset (0xFFFFFFFF for unloaded).
        let chunk_map_size = (WORLD_CHUNKS_X * WORLD_CHUNKS_Y * WORLD_CHUNKS_Z * 4) as u64;
        let chunk_map_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-map"),
            size: chunk_map_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // -- Octree buffer placeholder (will be filled later) --
        let octree_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("octree-nodes"),
            size: 1024 * 8, // 8192 bytes initial placeholder
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // -- Material color buffer --
        let material_colors = Self::build_material_colors();
        let material_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material-colors"),
            contents: bytemuck::cast_slice(&material_colors),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // -- Storage texture for compute output --
        let (render_texture, render_texture_view) =
            Self::create_storage_texture(device, width, height);

        // -- Bind group layouts --
        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
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

        let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene-bgl"),
            entries: &[
                // binding 0: voxel_pool (storage, read)
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
                // binding 1: material_colors (storage, read)
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
                // binding 2: output_texture (storage texture, write, rgba8unorm)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 3: chunk_map (storage, read)
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
                // binding 4: octree_nodes (storage, read)
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
                // binding 5: pick_result (storage, read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // -- Bind groups --
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bg"),
            layout: &uniform_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let pick = PickBuffer::new(device);

        let scene_bind_group = Self::create_scene_bind_group(
            device,
            &scene_bgl,
            &voxel_pool_buffer,
            &material_color_buffer,
            &render_texture_view,
            &chunk_map_buffer,
            &octree_buffer,
            &pick.pick_buffer,
        );

        // -- Compute pipeline --
        let ray_march_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ray-march-pipeline-layout"),
                bind_group_layouts: &[&uniform_bgl, &scene_bgl],
                push_constant_ranges: &[],
            });

        let ray_march_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ray-march-pipeline"),
            layout: Some(&ray_march_pipeline_layout),
            module: &ray_march_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // -- Blit pipeline --
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create a filterable view of the storage texture for sampling in blit
        let blit_texture_view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bg"),
            layout: &blit_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blit_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&blit_sampler),
                },
            ],
        });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit-pipeline-layout"),
            bind_group_layouts: &[&blit_bgl],
            push_constant_ranges: &[],
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit-pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        // -- Debug lines pipeline --
        let debug_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("debug-uniforms"),
            size: std::mem::size_of::<DebugUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let debug_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("debug-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let debug_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("debug-uniform-bg"),
            layout: &debug_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: debug_uniform_buffer.as_entire_binding(),
            }],
        });

        let debug_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("debug-pipeline-layout"),
                bind_group_layouts: &[&debug_bgl],
                push_constant_ranges: &[],
            });

        let debug_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("debug-vertex-buffer"),
            size: MAX_DEBUG_VERTICES * std::mem::size_of::<DebugVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let debug_lines_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("debug-lines-pipeline"),
            layout: Some(&debug_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &debug_lines_module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<DebugVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 12,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &debug_lines_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        // Upload initial debug wireframe
        let wireframe = crate::debug_lines::chunk_wireframe();
        let debug_vertex_count = wireframe.len() as u32;
        queue.write_buffer(&debug_vertex_buffer, 0, bytemuck::cast_slice(&wireframe));

        Self {
            ray_march_pipeline,
            uniform_bind_group,
            scene_bind_group,
            uniform_bgl,
            scene_bgl,
            render_texture,
            render_texture_view,
            blit_pipeline,
            blit_bind_group,
            blit_bgl,
            blit_sampler,
            debug_lines_pipeline,
            debug_vertex_buffer,
            debug_vertex_count,
            debug_uniform_bind_group,
            debug_uniform_buffer,
            camera_uniform_buffer,
            light_uniform_buffer,
            voxel_pool_buffer,
            material_color_buffer,
            chunk_map_buffer,
            octree_buffer,
            pick,
        }
    }

    /// Recreate the storage texture and affected bind groups on window resize.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (texture, view) = Self::create_storage_texture(device, width, height);
        self.render_texture = texture;
        self.render_texture_view = view;

        // Recreate scene bind group (references the storage texture view)
        self.scene_bind_group = Self::create_scene_bind_group(
            device,
            &self.scene_bgl,
            &self.voxel_pool_buffer,
            &self.material_color_buffer,
            &self.render_texture_view,
            &self.chunk_map_buffer,
            &self.octree_buffer,
            &self.pick.pick_buffer,
        );

        // Recreate blit bind group (references the texture view for sampling)
        let blit_texture_view = self
            .render_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bg"),
            layout: &self.blit_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blit_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
            ],
        });
    }

    /// Encode compute + blit + debug line passes into the command encoder.
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        // 1. Compute pass: ray march
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ray-march-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.ray_march_pipeline);
            pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            pass.set_bind_group(1, &self.scene_bind_group, &[]);
            // Dispatch enough workgroups to cover every pixel (8x8 workgroup size)
            let wg_x = width.div_ceil(8);
            let wg_y = height.div_ceil(8);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        // 2. Blit render pass: fullscreen triangle
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &self.blit_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // 3. Debug lines render pass
        if self.debug_vertex_count > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("debug-lines-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.debug_lines_pipeline);
            pass.set_bind_group(0, &self.debug_uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, self.debug_vertex_buffer.slice(..));
            pass.draw(0..self.debug_vertex_count, 0..1);
        }
    }

    /// Upload camera uniforms each frame.
    pub fn update_camera(&self, queue: &wgpu::Queue, uniforms: CameraUniforms) {
        queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );
    }

    /// Upload debug line view-projection matrix each frame.
    pub fn update_debug_uniforms(&self, queue: &wgpu::Queue, view_proj: [[f32; 4]; 4]) {
        let uniforms = DebugUniforms { view_proj };
        queue.write_buffer(&self.debug_uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Update debug line vertices.
    pub fn update_debug_lines(&mut self, queue: &wgpu::Queue, vertices: &[DebugVertex]) {
        let count = vertices.len().min(MAX_DEBUG_VERTICES as usize);
        self.debug_vertex_count = count as u32;
        if count > 0 {
            queue.write_buffer(
                &self.debug_vertex_buffer,
                0,
                bytemuck::cast_slice(&vertices[..count]),
            );
        }
    }

    /// Rebind the scene bind group to use an external voxel pool buffer (from the sim pipeline).
    /// Called each frame (or when the pool buffer changes) to point at the sim's read pool.
    pub fn update_voxel_pool(&mut self, device: &wgpu::Device, voxel_pool_buffer: &wgpu::Buffer) {
        self.scene_bind_group = Self::create_scene_bind_group(
            device,
            &self.scene_bgl,
            voxel_pool_buffer,
            &self.material_color_buffer,
            &self.render_texture_view,
            &self.chunk_map_buffer,
            &self.octree_buffer,
            &self.pick.pick_buffer,
        );
    }

    /// Upload chunk map entries to the GPU.
    pub fn update_chunk_map(&self, queue: &wgpu::Queue, chunk_map_data: &[u32]) {
        queue.write_buffer(
            &self.chunk_map_buffer,
            0,
            bytemuck::cast_slice(chunk_map_data),
        );
    }

    /// Upload octree node data to the GPU.
    pub fn update_octree(&self, queue: &wgpu::Queue, octree_data: &[u32]) {
        queue.write_buffer(&self.octree_buffer, 0, bytemuck::cast_slice(octree_data));
    }

    /// Update material colors from compiled rule data. Called once at init (C-PERF-2).
    pub fn update_material_colors(&mut self, device: &wgpu::Device, colors: &[MaterialColor]) {
        self.material_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material-colors"),
            contents: bytemuck::cast_slice(colors),
            usage: wgpu::BufferUsages::STORAGE,
        });
        // Recreate scene bind group with new material color buffer
        self.scene_bind_group = Self::create_scene_bind_group(
            device,
            &self.scene_bgl,
            &self.voxel_pool_buffer,
            &self.material_color_buffer,
            &self.render_texture_view,
            &self.chunk_map_buffer,
            &self.octree_buffer,
            &self.pick.pick_buffer,
        );
    }

    // -- Private helpers --

    fn create_storage_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-storage-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_scene_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        voxel_pool_buffer: &wgpu::Buffer,
        material_color_buffer: &wgpu::Buffer,
        texture_view: &wgpu::TextureView,
        chunk_map_buffer: &wgpu::Buffer,
        octree_buffer: &wgpu::Buffer,
        pick_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene-bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: voxel_pool_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: material_color_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: chunk_map_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: octree_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: pick_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Build the material color table (C-DESIGN-1: no hardcoded materials in shaders).
    /// Provides initial fallback colors for 12 materials. Updated at init by rule engine output.
    fn build_material_colors() -> Vec<MaterialColor> {
        vec![
            // 0: Air (never rendered, but need valid entry)
            MaterialColor {
                color: [0.0, 0.0, 0.0],
                emission: 0.0,
            },
            // 1: Stone
            MaterialColor {
                color: [0.5, 0.5, 0.55],
                emission: 0.0,
            },
            // 2: Sand
            MaterialColor {
                color: [0.76, 0.70, 0.50],
                emission: 0.0,
            },
            // 3: Water
            MaterialColor {
                color: [0.2, 0.4, 0.8],
                emission: 0.1,
            },
            // 4: Oil
            MaterialColor {
                color: [0.3, 0.2, 0.05],
                emission: 0.0,
            },
            // 5: Fire
            MaterialColor {
                color: [1.0, 0.5, 0.1],
                emission: 5.0,
            },
            // 6: Smoke
            MaterialColor {
                color: [0.3, 0.3, 0.3],
                emission: 0.0,
            },
            // 7: Steam
            MaterialColor {
                color: [0.85, 0.85, 0.9],
                emission: 0.05,
            },
            // 8: Wood
            MaterialColor {
                color: [0.45, 0.28, 0.12],
                emission: 0.0,
            },
            // 9: Ash
            MaterialColor {
                color: [0.7, 0.7, 0.65],
                emission: 0.0,
            },
            // 10: Ice
            MaterialColor {
                color: [0.7, 0.85, 0.95],
                emission: 0.1,
            },
            // 11: Lava
            MaterialColor {
                color: [1.0, 0.3, 0.0],
                emission: 4.0,
            },
        ]
    }
}
