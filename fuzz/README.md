# Fuzz checks

The input decoder must behave identically across transport fragmentation and
keep paste events bounded. The sanitizer must prevent arbitrary text from
placing control characters in rendered cells. These targets exercise those
contracts with malformed bytes, escape reports, paste sequences, and Unicode.

`./x fuzz-check` compiles both targets on the pinned stable toolchain. The weekly
workflow runs each for two minutes using pinned cargo-fuzz and nightly. Crashes
retain their input for fourteen days and the scheduled reporter opens one issue
until a successful scheduled run resolves it. Replay a retained input with:

```sh
cargo +nightly-2026-08-20 fuzz run decoder path/to/crash
```

Keep discovered crashes as deterministic regression tests in the core suite.
