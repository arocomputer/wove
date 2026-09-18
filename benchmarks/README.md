# Performance

Run `./x bench` to build release examples and measure their size, median warm
process launch, and pure buffer drawing. Results include the machine and are
written to `artifacts/bench.json`. These are local measurements, not claims
that weft beats another library or language.

The startup probe returns before terminal initialization. It measures process
launch on a warm executable, including Python subprocess overhead. It is not a
cold-cache benchmark and does not measure time to first terminal frame.

The rendering workload clears a 120 by 40 buffer and paints three Unicode text
rows 10,000 times. It excludes terminal output. The explorer executable size
includes its terminal backend. Broad ceilings catch large regressions without
pretending shared CI machines give precise performance comparisons.

Future comparisons must use equivalent applications, release settings, input,
terminal, platform, and cache conditions. Report actual first-frame latency
separately from process startup before drawing conclusions about Rust or Zig.
