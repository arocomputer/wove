use wove::elements::Pixels;
use wove_gpu::{wgpu, Gpu};

/// Clear a frame to one color through a render pass.
fn clear(gpu: &Gpu, pixels: &mut Pixels, color: wgpu::Color) {
    gpu.render(pixels, |device, queue, target| {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        queue.submit([encoder.finish()]);
    })
    .unwrap();
}

#[test]
fn a_rendered_frame_lands_in_the_pixels_with_rows_unpadded() {
    let gpu = Gpu::new().expect("a GPU or software adapter");
    // Three pixels a row is well short of the copy alignment, so the
    // readback must strip padding, and every pixel must still be exact.
    let mut pixels = Pixels::new(3, 5);
    clear(&gpu, &mut pixels, wgpu::Color::RED);
    for y in 0..5 {
        for x in 0..3 {
            assert_eq!(pixels.get(x, y), Some([255, 0, 0]), "pixel {x},{y}");
        }
    }
    // Linear half intensity comes back sRGB-encoded, as a terminal wants it.
    let mut pixels = Pixels::new(300, 2);
    clear(
        &gpu,
        &mut pixels,
        wgpu::Color {
            r: 0.0,
            g: 0.5,
            b: 0.0,
            a: 1.0,
        },
    );
    let [_, green, _] = pixels.get(299, 1).unwrap();
    assert!(
        (186..=189).contains(&green),
        "sRGB of 0.5 is about 188, got {green}"
    );
}
