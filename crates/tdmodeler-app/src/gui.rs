//! Interactive viewer (behind the `gui` feature). Wires `winit` + `wgpu` +
//! `egui` together and renders the demo part with orbit controls.
//!
//! Build/run on a desktop target with `--features gui`. This module is not
//! compiled in headless/CI builds and requires a GPU + display libraries.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use glam::Vec3;
use winit::event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes};

use tdmodeler_core::features;
use tdmodeler_core::solid;
use tdmodeler_mesh::TriangleMesh;
use tdmodeler_render::camera::OrbitCamera;
use tdmodeler_render::renderer::Renderer;

pub fn run() -> Result<()> {
    let event_loop = EventLoop::builder().build()?;
    let window = Arc::new(
        event_loop
            .create_window(WindowAttributes::default().with_title("TDModeler"))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance.create_surface(&*window)?;
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        ..Default::default()
    })) {
        Ok(a) => a,
        Err(e) => return Err(anyhow::anyhow!("no suitable GPU adapter found: {e}")),
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..Default::default()
    }))?;

    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats[0];
    let size = window.inner_size();
    let mut config = surface
        .get_default_config(&adapter, size.width.max(1), size.height.max(1))
        .ok_or_else(|| anyhow::anyhow!("surface not supported by adapter"))?;
    config.view_formats = vec![];
    surface.configure(&device, &config);

    // Build the demo part (base plate with holes + peg).
    let mut part = solid::box_(30.0, 30.0, 6.0, true);
    for (x, y) in [(-9.0, -9.0), (9.0, -9.0), (-9.0, 9.0), (9.0, 9.0)] {
        let hole = features::translate(&solid::cylinder(8.0, 2.0, 2.0, 32), x, y, 0.0);
        part = features::difference(&part, &hole);
    }
    let peg = features::translate(&solid::cylinder(10.0, 3.0, 3.0, 32), 0.0, 0.0, 3.0);
    let solid_part = features::union(&part, &peg);
    let mesh: TriangleMesh = solid_part.to_mesh();

    let mut renderer = Renderer::new(device, queue, format, (config.width, config.height));
    renderer.set_mesh(&mesh);

    let mut camera = OrbitCamera::default();
    camera.frame(Vec3::new(0.0, 0.0, 3.0), 40.0);

    let egui_ctx = egui::Context::default();
    let mut egui_state = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &event_loop,
        None,
        None,
        None,
    );
    let mut egui_renderer =
        egui_wgpu::Renderer::new(renderer.device(), format, egui_wgpu::RendererOptions::default());

    // `surface` borrows the `Window` behind the `Arc` above, so we must NOT move that
    // `Arc` into the event loop closure. Wrap it in an `Arc<Mutex<..>>` so the closure
    // only moves its own owning handle and reaches the `Window` through a lock.
    let window = Arc::new(Mutex::new(window));

    let mut dragging = false;

    event_loop.run(move |event, elwt| {
        let window_guard = window.lock().unwrap();
        let window_ref: &Window = window_guard.as_ref();
        match event {
        Event::WindowEvent { event, .. } => {
            if egui_state.on_window_event(window_ref, &event).consumed {
                return;
            }
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(s) => {
                    config.width = s.width.max(1);
                    config.height = s.height.max(1);
                    surface.configure(renderer.device(), &config);
                    renderer.resize(config.width, config.height);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if button == MouseButton::Left {
                        dragging = state == ElementState::Pressed;
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let amt = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    camera.zoom(amt);
                }
                WindowEvent::RedrawRequested => {
                    let aspect = config.width as f32 / config.height.max(1) as f32;
                    let frame = match surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(t)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                        _ => return,
                    };
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    // egui overlay
                    let raw = egui_state.take_egui_input(window_ref);
                    let full = egui_ctx.run(raw, |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            ui.heading("TDModeler");
                            ui.label("Left-drag: orbit · Wheel: zoom");
                            ui.label(format!(
                                "verts: {}  tris: {}",
                                mesh.num_vert(),
                                mesh.num_tri()
                            ));
                        });
                    });
                    egui_state.handle_platform_output(window_ref, full.platform_output);
                    let tris = egui_ctx.tessellate(full.shapes, full.pixels_per_point);

                    let mut encoder = renderer
                        .device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("frame"),
                        });
                    renderer.record(&mut encoder, &view, &camera, aspect);

                    let screen = egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [config.width, config.height],
                        pixels_per_point: egui_winit::pixels_per_point(&egui_ctx, window_ref),
                    };
                    egui_renderer.update_buffers(
                        renderer.device(),
                        renderer.queue(),
                        &mut encoder,
                        &tris,
                        &screen,
                    );
                    {
                        let mut pass = encoder
                            .begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("egui-pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                                multiview_mask: None,
                            })
                            .forget_lifetime();
                        egui_renderer.render(&mut pass, &tris, &screen);
                    }

                    renderer.queue().submit(Some(encoder.finish()));
                    frame.present();
                }
                _ => {}
            }
        }
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion { delta },
            ..
        } => {
            if dragging {
                camera.drag(delta.0 as f32, delta.1 as f32);
            }
        }
        Event::AboutToWait => window_ref.request_redraw(),
        _ => {}
        }
    }
    });
    Ok(())
}
