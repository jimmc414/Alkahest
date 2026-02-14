use alkahest_core::constants::{CHUNK_SIZE, VOXELS_PER_CHUNK};
use wgpu::util::DeviceExt;

use crate::debug_lines::DebugVertex;

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
    voxel_buffer: wgpu::Buffer,
    material_color_buffer: wgpu::Buffer,
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
            "const CHUNK_SIZE: u32 = {}u;\nconst VOXELS_PER_CHUNK: u32 = {}u;\n",
            CHUNK_SIZE, VOXELS_PER_CHUNK,
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

        // -- Voxel buffer (static for M1) --
        let voxel_data = Self::build_test_scene();
        let voxel_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("voxel-buffer"),
            contents: bytemuck::cast_slice(&voxel_data),
            usage: wgpu::BufferUsages::STORAGE,
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

        let scene_bind_group = Self::create_scene_bind_group(
            device,
            &scene_bgl,
            &voxel_buffer,
            &material_color_buffer,
            &render_texture_view,
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
            voxel_buffer,
            material_color_buffer,
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
            &self.voxel_buffer,
            &self.material_color_buffer,
            &self.render_texture_view,
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

    /// Count non-air voxels in the test scene (for debug panel).
    pub fn non_air_voxel_count(&self) -> u32 {
        // We know the scene layout, compute at init.
        // Stone floor: 32*32 = 1024
        // Pyramid layers 1-8
        let mut count = 1024u32;
        for layer in 1u32..=8 {
            let half = CHUNK_SIZE / 2;
            let min_coord = half as i32 - (9 - layer) as i32;
            let max_coord = half as i32 + (9 - layer) as i32;
            if max_coord > min_coord {
                let side = (max_coord - min_coord) as u32;
                count += side * side;
            }
        }
        count += 1; // emissive voxel
        count
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
        voxel_buffer: &wgpu::Buffer,
        material_color_buffer: &wgpu::Buffer,
        texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene-bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: voxel_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: material_color_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
            ],
        })
    }

    /// Build the M1 test scene: stone floor + sand pyramid + emissive voxel.
    fn build_test_scene() -> Vec<[u32; 2]> {
        use alkahest_core::math::pack_voxel;
        use alkahest_core::types::MaterialId;

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
                let idx = x + z * cs * cs; // y=0, so y*cs term is zero
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

    /// Build the material color table (C-DESIGN-1: no hardcoded materials in shaders).
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
            // 3: Emissive
            MaterialColor {
                color: [1.0, 0.9, 0.6],
                emission: 5.0,
            },
        ]
    }
}
