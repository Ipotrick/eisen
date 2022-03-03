use std::{time::{SystemTime, Instant}};

use async_std::sync::Mutex;
use wgpu::{util::{DeviceExt, RenderEncoder}, BindGroupDescriptor, RenderPipelineDescriptor};
use winit::{dpi::PhysicalSize, window::Window};

const QUADS_PER_BATCH: usize = 1024;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadDrawInfo{
    pub color: [f32; 4],
    pub scale: [f32; 2],
    pub position: [f32; 2],
    pub orientation: [f32; 2],
    pub _pad: [f32; 2],
}

pub struct SharedRenderRessources 
{
    instance:       wgpu::Instance,
    surface:        wgpu::Surface,
    surf_size:      PhysicalSize<u32>,
    surf_config:    wgpu::SurfaceConfiguration,
    adapter:        wgpu::Adapter,

    device: wgpu::Device,
    main_queue: wgpu::Queue,
}

pub trait RenderRoutine: Send + Sync
{
    fn render(&mut self, shareed: &mut SharedRenderRessources);
}

pub enum RenderPass 
{
    Main,
}

pub struct RenderState 
{
    shared_ressources: SharedRenderRessources,
    main_pass_render_routines: Vec<Box<dyn RenderRoutine>>,
    rect_draw_buffers: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    last_buffer_index: usize,
    last_buffer_fill_len: usize,
    rect_index_buffer: wgpu::Buffer,
    rect_pipeline_binding_group_layout: wgpu::BindGroupLayout,
    rect_pipeline_layout: wgpu::PipelineLayout,
    rect_pipeline: wgpu::RenderPipeline,
}

pub struct Renderer 
{
    state: Mutex<RenderState>,
    start_time: SystemTime,
} 

impl Renderer 
{
    pub async fn new(window: &Window) -> Self 
    {
        let instance = wgpu::Instance::new(wgpu::Backends::VULKAN);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        let (device, main_queue) = adapter.request_device(
            &wgpu::DeviceDescriptor{
                features:   wgpu::Features::empty(),
                limits:     wgpu::Limits::default(),
                label:      Some("main device"),
            }, 
            None,
        ).await.unwrap();

        let surf_size = window.inner_size();
        let surf_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width:  surf_size.width,
            height: surf_size.height,
            present_mode: wgpu::PresentMode::Immediate,
        };
        surface.configure(&device, &surf_config);

        // 0       1
        // ---------
        // |       |
        // |       |
        // |       |
        // ---------
        // 2       4
        // 2 -> 0 -> 1
        // 2 -> 1 -> 3
        let mut indices = Vec::<u32>::new();
        indices.reserve(QUADS_PER_BATCH*6);
        for i in (0 as u32..(QUADS_PER_BATCH*4) as u32).step_by(4) {
            indices.push(i + 2);
            indices.push(i + 0);
            indices.push(i + 1);
            indices.push(i + 2);
            indices.push(i + 1);
            indices.push(i + 3);
        }

        let batched_quad_index_buffer = device.create_buffer(
            &wgpu::BufferDescriptor{
                label: Some("batched quad index buffer"),
                size: (std::mem::size_of::<u32>() * indices.len()) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }
        );

        main_queue.write_buffer(&batched_quad_index_buffer, 0, bytemuck::cast_slice(&indices[..]));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer{
                        has_dynamic_offset: false,
                        ty: wgpu::BufferBindingType::Uniform,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("quad render bind group layout"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{
            label: Some("quad render pipeline"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor{
            label: Some("quad render shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("rendering/quad_shader.wgsl"))),
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor{
            vertex: wgpu::VertexState{
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState{
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[
                    wgpu::ColorTargetState{
                        blend: None,
                        format: surf_config.format,
                        write_mask: wgpu::ColorWrites::ALL,
                    }
                ],
            }),
            label: Some("quad render pipeline"),
            layout: Some(&pipeline_layout),
            primitive: wgpu::PrimitiveState{
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(), 
            multiview: None,
        });

        let shared = SharedRenderRessources{
            instance,
            surface,
            surf_size,
            surf_config,
            adapter,
            device,
            main_queue,
        };

        let state = Mutex::new(RenderState{
            shared_ressources: shared,
            main_pass_render_routines: Vec::new(),
            rect_draw_buffers: Vec::new(),
            last_buffer_index: 0,
            last_buffer_fill_len: 0,
            rect_index_buffer: batched_quad_index_buffer,
            rect_pipeline_binding_group_layout: bind_group_layout,
            rect_pipeline_layout: pipeline_layout,
            rect_pipeline: pipeline,
        });

        Self{
            state,
            start_time: SystemTime::now(),
        }
    }

    pub async fn resize(&self, new_size: winit::dpi::PhysicalSize<u32>) 
    {
        let mut state = self.state.lock().await;
        if new_size.width > 0 && new_size.height > 0 {
            state.shared_ressources.surf_size = new_size;
            state.shared_ressources.surf_config.width = new_size.width;
            state.shared_ressources.surf_config.height = new_size.height;
            state.shared_ressources.surface.configure(&state.shared_ressources.device, &state.shared_ressources.surf_config);
            println!("resized to: ({},{})", new_size.width, new_size.height);
        }
    }

    pub async fn add_render_routine(&self, routine: impl RenderRoutine + 'static, pass: RenderPass) 
    {
        let mut state = self.state.lock().await;
        match pass {
            RenderPass::Main => state.main_pass_render_routines.push(Box::new(routine)),
        }
    }

    pub async fn push_quads(&self, quads: &[QuadDrawInfo]) {
        let mut state = self.state.lock().await;

        if quads.len() > 0 {
            state.last_buffer_index = (quads.len() - 1) / QUADS_PER_BATCH;
            state.last_buffer_fill_len = (quads.len() - 1) % QUADS_PER_BATCH + 1;
    
            while state.rect_draw_buffers.len() <= state.last_buffer_index {
                let buff = state.shared_ressources.device.create_buffer(&wgpu::BufferDescriptor{
                    label: Some("quad draw buffer"),
                    mapped_at_creation: false,
                    size: (QUADS_PER_BATCH * std::mem::size_of::<QuadDrawInfo>()) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM
                });
                let bind_group = state.shared_ressources.device.create_bind_group(&wgpu::BindGroupDescriptor{
                    label: Some("quad render pipeline bind group"),
                    layout: &state.rect_pipeline_binding_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry{
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding{
                                buffer: &buff,
                                offset: 0,
                                size: None,
                            }),
                        }
                    ],
                });
                state.rect_draw_buffers.push((buff, bind_group));
            }
    
            for i in 0..state.last_buffer_index {
                let slice = bytemuck::cast_slice(&quads[i*QUADS_PER_BATCH..(i+1)*state.last_buffer_index]);
                state.shared_ressources.main_queue.write_buffer(&state.rect_draw_buffers[i].0, 0, slice);
            }
            let slice = bytemuck::cast_slice(&quads[state.last_buffer_index*QUADS_PER_BATCH..]);
            state.shared_ressources.main_queue.write_buffer(&state.rect_draw_buffers[state.last_buffer_index].0, 0, slice);
        } else {
            state.last_buffer_index = 0;
            state.last_buffer_fill_len = 0;
        }
    }

    pub async fn render(&self) -> Result<(), wgpu::SurfaceError> 
    {
        let mut state = self.state.lock().await;
        let state = &mut*state;

        let output = state.shared_ressources.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = state.shared_ressources.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Rect Renderpass Encoder"),
        });

        {
            let val = self.start_time.elapsed().unwrap().as_secs_f64();

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&state.rect_pipeline);

            render_pass.set_index_buffer(
                state.rect_index_buffer.slice(..), 
                wgpu::IndexFormat::Uint32,
            );

            if state.last_buffer_fill_len > 0 {
                for i in 0..state.last_buffer_index {
                    render_pass.set_bind_group(0, &state.rect_draw_buffers[i].1, &[]);
                    render_pass.draw_indexed(0..(QUADS_PER_BATCH*6) as u32, 0, 0..1);
                }
                render_pass.set_bind_group(0, &state.rect_draw_buffers[state.last_buffer_index].1, &[]);
                render_pass.draw_indexed(0..(state.last_buffer_fill_len*6) as u32, 0, 0..1);
            }
        }
        let shared = &mut state.shared_ressources;

        for render_routine in &mut state.main_pass_render_routines {
            render_routine.render(shared);
        }
    
        // submit will accept anything that implements IntoIter
        state.shared_ressources.main_queue.submit(std::iter::once(encoder.finish()));
        output.present();
    
        Ok(())
    }
}