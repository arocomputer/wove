//! A ray-marched scene drawn by a fragment shader on the GPU and shown in
//! the terminal as blocks. Esc quits.
use std::time::{Duration, Instant};
use wove::{
    elements::{Blocks, Pixels, Text},
    layout::*,
    terminal::{self, Terminal},
    Event, Id, Key, Style, Tree,
};
use wove_gpu::{wgpu, Gpu};

const SHADER: &str = r#"
struct Frame { time: f32, width: f32, height: f32, _pad: f32 };
@group(0) @binding(0) var<uniform> frame: Frame;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle that covers the screen.
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index & 2u) * 2 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn scene(p: vec3<f32>) -> f32 {
    let ball = length(p - vec3<f32>(0.0, 1.0, 0.0)) - 1.0;
    let floor = p.y;
    return min(ball, floor);
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.001, 0.0);
    return normalize(vec3<f32>(
        scene(p + e.xyy) - scene(p - e.xyy),
        scene(p + e.yxy) - scene(p - e.yxy),
        scene(p + e.yyx) - scene(p - e.yyx)));
}

fn shadow(origin: vec3<f32>, direction: vec3<f32>) -> f32 {
    var t = 0.05;
    var light = 1.0;
    for (var i = 0; i < 48; i++) {
        let d = scene(origin + direction * t);
        if (d < 0.001) { return 0.0; }
        light = min(light, 12.0 * d / t);
        t += d;
        if (t > 12.0) { break; }
    }
    return light;
}

@fragment
fn fragment(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (at.xy - 0.5 * vec2<f32>(frame.width, frame.height)) / frame.height;
    let angle = frame.time * 0.4;
    let eye = vec3<f32>(3.5 * sin(angle), 2.0, 3.5 * cos(angle));
    let look = vec3<f32>(0.0, 0.8, 0.0);
    let forward = normalize(look - eye);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), forward));
    let up = cross(forward, right);
    let ray = normalize(forward * 1.6 + right * uv.x - up * uv.y);
    var t = 0.0;
    var hit = false;
    for (var i = 0; i < 96; i++) {
        let d = scene(eye + ray * t);
        if (d < 0.001) { hit = true; break; }
        t += d;
        if (t > 30.0) { break; }
    }
    let sky = mix(vec3<f32>(0.10, 0.12, 0.18), vec3<f32>(0.35, 0.45, 0.65), 0.5 - 0.5 * ray.y);
    if (!hit) { return vec4<f32>(sky, 1.0); }
    let p = eye + ray * t;
    let n = normal(p);
    let sun = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let checker = f32((i32(floor(p.x)) + i32(floor(p.z))) & 1);
    var albedo = mix(vec3<f32>(0.80, 0.75, 0.65), vec3<f32>(0.25, 0.22, 0.20), checker);
    if (p.y > 0.01) { albedo = vec3<f32>(0.85, 0.35, 0.25); }
    let diffuse = max(dot(n, sun), 0.0) * shadow(p + n * 0.02, sun);
    let half = normalize(sun - ray);
    let gloss = pow(max(dot(n, half), 0.0), 48.0) * f32(p.y > 0.01);
    let color = albedo * (0.15 + 0.85 * diffuse) + vec3<f32>(gloss);
    let fog = exp(-0.04 * t);
    return vec4<f32>(mix(sky, color, fog), 1.0);
}
"#;

/// The scene's pipeline and the buffer its frame uniform lives in.
struct Scene {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}

impl Scene {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wove_gpu::FORMAT.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            bind,
            uniform,
        }
    }

    /// Draw one frame at `time` into `image`, which is `width` by `height` pixels.
    fn draw(&self, gpu: &Gpu, image: &mut Pixels, time: f32) -> Result<(), wove_gpu::Error> {
        let (width, height) = image.size();
        let mut frame = [0u8; 16];
        for (slot, value) in [time, width as f32, height as f32, 0.0].iter().enumerate() {
            frame[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        gpu.queue().write_buffer(&self.uniform, 0, &frame);
        gpu.render(image, |device, queue, target| {
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind, &[]);
                pass.draw(0..3, 0..1);
            }
            queue.submit([encoder.finish()]);
        })
    }
}

/// A title above an image that fills the rest of the screen.
fn screen(tree: &mut Tree, title: String, blocks: Blocks) -> Result<Id, wove::Error> {
    let root = tree.root();
    tree.set_layout(
        root,
        wove::Layout {
            flex_direction: FlexDirection::Column,
            ..Default::default()
        },
    )?;
    tree.add(
        root,
        Text {
            content: title,
            style: Style {
                bold: true,
                ..Style::default()
            },
            ..Text::default()
        },
    )?;
    let mut image = Pixels::new(1, 1);
    image.blocks = blocks;
    let pixels = tree.add(root, image)?;
    tree.set_layout(
        pixels,
        wove::Layout {
            flex_grow: 1.0,
            min_size: Size {
                width: length(0.0),
                height: length(0.0),
            },
            ..Default::default()
        },
    )?;
    Ok(pixels)
}

fn main() -> Result<(), wove_gpu::Error> {
    let gpu = Gpu::new()?;
    let scene = Scene::new(gpu.device());
    // Sextants where the terminal draws them, half blocks elsewhere.
    let term = std::env::var("TERM").unwrap_or_default().to_lowercase();
    let program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_lowercase();
    let fine = ["ghostty", "wezterm", "kitty", "foot", "contour"]
        .iter()
        .any(|name| term.contains(name) || program.contains(name));
    let blocks = if fine { Blocks::Sextant } else { Blocks::Half };
    let (across, down) = blocks.per_cell();
    let mut tree = Tree::new();
    let title = format!("Wove · wgpu on {} · Esc quits", gpu.info().name);
    let pixels = screen(&mut tree, title, blocks)?;
    let mut terminal = Terminal::new()?;
    let started = Instant::now();
    loop {
        let (columns, rows) = terminal.size()?;
        let (width, height) = (
            usize::from(columns) * across,
            usize::from(rows.saturating_sub(1)) * down,
        );
        let time = started.elapsed().as_secs_f32();
        let mut drawn = Ok(());
        tree.update::<Pixels>(pixels, |image| {
            image.resize(width, height);
            drawn = scene.draw(&gpu, image, time);
        })?;
        drawn?;
        terminal.draw(tree.frame(columns, rows)?)?;
        // Draw again after a short wait, or as soon as a key arrives.
        if terminal::poll(Duration::from_millis(33))? {
            if let Some(event) = terminal::read()? {
                match event {
                    Event::Key(Key::Escape, _) => return Ok(()),
                    Event::Resize(..) => terminal.invalidate(),
                    _ => {}
                }
                tree.dispatch(event)?;
            }
        }
    }
}
