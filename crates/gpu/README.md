# GPU

Render with the GPU into Wove's `Pixels` element. `Gpu::new` opens a device
with no window, preferring a real GPU and falling back to a software renderer
such as Mesa's or Windows' WARP. `Gpu::render` makes a texture the size of a
`Pixels`, lets the caller record and submit any wgpu work against it, and
copies the frame back into the element, which the tree draws with block
characters. wgpu is re-exported.

```rust
use wove::elements::Pixels;
use wove_gpu::{wgpu, Gpu};

let gpu = Gpu::new()?;
let mut pixels = Pixels::new(160, 90);
gpu.render(&mut pixels, |device, queue, target| {
    // Record a render pass against `target`, a Rgba8UnormSrgb view.
})?;
```

Run `cargo run -p wove-gpu --example shader` for a ray-marched scene drawn by
a fragment shader and animated in the terminal. The `terminal` feature, on by
default, is only what the example needs; the library itself works headless.
