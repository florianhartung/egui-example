use egui::{Color32, Context, Visuals};
use egui_wgpu::renderer::ScreenDescriptor;
use wgpu::{Backends, Color, InstanceDescriptor, LoadOp, StoreOp};
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::Window,
};
use winit::event_loop::ControlFlow;

async fn run(event_loop: EventLoop<()>, window: Window) {
    let mut size = window.inner_size();
    size.width = size.width.max(1);
    size.height = size.height.max(1);

    let instance = wgpu::Instance::new(InstanceDescriptor { backends: Backends::PRIMARY, ..Default::default() });

    let surface = unsafe { instance.create_surface(&window) }.unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let mut config = surface
        .get_default_config(&adapter, size.width, size.height)
        .unwrap();
    surface.configure(&device, &config);

    // Egui stuff
    let context = Context::default();
    let mut winit_state = egui_winit::State::new(context.viewport_id(), &window, Some(window.scale_factor() as f32), None);

    let mut egui_renderer = egui_wgpu::Renderer::new(
        &device,
        config.format,
        None,
        1,
    );

    let mut screen_descriptor = ScreenDescriptor {
        pixels_per_point: window.scale_factor() as f32,
        size_in_pixels: [window.inner_size().width, window.inner_size().height],
    };

    let window = Box::leak(Box::new(window));

    let mut first_resize_happened = cfg!(not(target_os = "windows"));

    event_loop
        .run(move |event, target, control_flow| {
            // Have the closure take ownership of the resources.
            // `event_loop.run` never returns, therefore we must do this to ensure
            // the resources are properly cleaned up.
            let _ = (&instance, &adapter, &pipeline_layout);

            match event {
                Event::RedrawRequested(_) => {
                    // egui
                    let raw_input = winit_state.take_egui_input(&window);
                    context.begin_frame(raw_input);
                    context.set_visuals(Visuals {
                        window_fill: Color32::TRANSPARENT,
                        panel_fill: Color32::TRANSPARENT,
                        override_text_color: Some(Color32::RED),
                        faint_bg_color: Color32::RED,
                        extreme_bg_color: Color32::BLUE,
                        ..Default::default()
                    });
                    egui::CentralPanel::default().show(&context, |ui| {
                        ui.label("Hello world".to_owned());

                        ui.scope(|ui| {
                            ui.button("aaa");
                        });

                        if ui.button("Click me").clicked() {
                            println!("Clicked");
                        }
                    });
                    let output = context.end_frame();


                    let frame = surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: None,
                        });

                    // prepare egui frame
                    let paint_jobs = context
                        .tessellate(output.shapes, output.pixels_per_point);
                    let tdelta = output.textures_delta;

                    for (t_id, tdelta) in tdelta.set {
                        egui_renderer
                            .update_texture(&device, &queue, t_id, &tdelta);
                    }

                    egui_renderer.update_buffers(
                        &device,
                        &queue,
                        &mut encoder,
                        &paint_jobs,
                        &screen_descriptor,
                    );

                    {
                        let mut egui_render_pass = encoder
                            .begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: None,
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    ops: wgpu::Operations {
                                        load: LoadOp::Clear(Color::TRANSPARENT),
                                        store: StoreOp::Store,
                                    },
                                    resolve_target: None,
                                })],
                                depth_stencil_attachment: None,
                                occlusion_query_set: None,
                                timestamp_writes: None,
                            });

                        egui_renderer
                            .render(&mut egui_render_pass, &paint_jobs, &screen_descriptor);
                    }

                    queue.submit(Some(encoder.finish()));
                    frame.present();
                }

                Event::WindowEvent {
                    event: window_event, window_id: _
                } => {
                    match window_event {
                        WindowEvent::Resized(new_size) => {
                            if !first_resize_happened {
                                first_resize_happened = true;
                                return;
                            }
                            // egui resize
                            screen_descriptor.size_in_pixels = [new_size.width, new_size.height];
                            screen_descriptor.pixels_per_point = egui_winit::pixels_per_point(&context, &window);

                            // Reconfigure the surface with the new size
                            config.width = new_size.width.max(1);
                            config.height = new_size.height.max(1);
                            surface.configure(&device, &config);
                            // On macos the window needs to be redrawn manually after resizing
                            window.request_redraw();
                        }
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::ExitWithCode(0);
                        }
                        other => {
                            let result =
                                winit_state
                                    .on_window_event(&context, &other);
                            if result.repaint {
                                window.request_redraw();
                            }
                        }
                    };
                }
                _ => {}
            }
        });
}

pub fn main() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_transparent(true)
        .build(&event_loop)
        .unwrap();

    pollster::block_on(run(event_loop, window));
}
