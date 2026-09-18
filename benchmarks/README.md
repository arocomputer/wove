# Performance

`./x bench` builds release examples and records sizes, median warm process
launch, changed-frame rendering, and cached-frame access in `artifacts/bench.json`.
These are local measurements, not comparisons with another library or language.

The startup probe constructs a tree and renders one 100 by 30 frame in memory,
then exits. Timing includes Python subprocess overhead. It uses a warm executable
and does not measure cold-cache startup or time to first terminal output.

The rendering workload updates Unicode text and renders 1,000 changed frames.
A separate loop reads 100,000 cached frames. Both exclude terminal output.
Executable sizes cover the direct-core gallery and the Dioxus counter with their
terminal backends. Broad ceilings catch large regressions on shared CI machines.

Comparisons need equivalent applications, release settings, input, terminal,
platform, and cache conditions. Report first-frame latency separately from
process launch before drawing conclusions about Rust or Zig.
