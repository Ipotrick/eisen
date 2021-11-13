use winit::{dpi::PhysicalSize, window::Window};

pub struct Renderer 
{
    instance:       wgpu::Instance,
    surface:        wgpu::Surface,
    surf_size:      PhysicalSize<u32>,
    surf_config:    wgpu::SurfaceConfiguration,
    adapter:        wgpu::Adapter,

    device: wgpu::Device,
    main_queue: wgpu::Queue,
}

impl Renderer {
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

        Self{
            instance,
            surface,
            surf_size,
            surf_config,
            adapter,
            device,
            main_queue
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) 
    {
        if new_size.width > 0 && new_size.height > 0 {
            self.surf_size = new_size;
            self.surf_config.width = new_size.width;
            self.surf_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surf_config);
            println!("resized to: ({},{})", new_size.width, new_size.height);
        }
    }

    pub async fn render(&mut self) -> Result<(), wgpu::SurfaceError> 
    {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.3,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
        }
    
        // submit will accept anything that implements IntoIter
        self.main_queue.submit(std::iter::once(encoder.finish()));
        output.present();
    
        Ok(())
    }
}