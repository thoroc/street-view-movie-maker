---
title: "ADR-013: Native Rust encode/mux pipeline — pure-Rust AV1 default, opt-in FFI H.264"
status: accepted
date: 2026-08-24
context:
  - path: ".context/findings/2026-08-24-replacing-ffmpeg-by-rust-native-crate.md"
  - path: ".context/plans/2026-08-24-replace-ffmpeg-with-native-rust-encode-pipeline.md"
---

**Status:** Accepted
**Date:** 2026-08-24

## Context

Every prebuilt `svmm` release binary requires `ffmpeg` on `PATH` at runtime for the final
frame-sequence-to-video encode. Two project goals turned out to be in tension: shipping a genuinely
dependency-free, pure-Rust binary, and having the output play natively everywhere (macOS, Windows,
Linux) with no extra codec install. The only mature pure-Rust video encoder is AV1 (`rav1e`) — but
AV1 isn't reliably natively playable on macOS pre-M3, stock Windows, or most Linux desktops. H.264,
by contrast, is close to universally native, but no mature pure-Rust H.264 encoder exists; the only
way to produce it is `openh264` (Cisco, BSD-licensed) via Rust FFI, which is not pure Rust.

## Decision

- **Default build stays pure Rust end to end**: `image` (JPEG decode, already a dependency) →
  `rav1e` (AV1 encode) → `muxide` (MP4 mux). This is what release CI builds and publishes for
  macOS/Linux/Windows — one self-contained binary, zero `ffmpeg` dependency.
- **H.264 output is an opt-in build feature, not a prebuilt release.** Users who need guaranteed
  native playback build it themselves via `cargo install --features h264 --no-default-features`,
  which pulls in `openh264` via FFI and requires a local C toolchain. This path is never shipped as
  a prebuilt binary.
- A `FrameEncoder` trait abstracts the encoder so the rest of the pipeline (frame loading, muxing)
  doesn't branch on codec; `#[cfg(feature = "av1")]` and `#[cfg(feature = "h264")]` gate the two
  implementations, both feeding `muxide`. A `compile_error!` enforces exactly one of `av1`/`h264` is
  enabled, so a build can never silently end up with no encoder compiled in.
- Statically linking a self-compiled `openh264` into a binary distributed by this project sits
  outside Cisco's prebuilt-binary MPEG LA royalty umbrella (the reason Firefox downloads Cisco's own
  binary at runtime instead) — but this is a flag for Legal/Compliance to confirm, not a legal
  conclusion this decision settles unilaterally. The `h264` feature never being a prebuilt release
  binary is itself part of what keeps this project in a materially safer licensing position.

## Consequences

**Easier:** No runtime dependency on `ffmpeg` being installed and discoverable on `PATH` — one
fewer install step and failure mode for every user of the prebuilt binaries. The codec choice is
isolated behind one trait, so a future third encoder (or a mature pure-Rust H.264 encoder, should
one appear) can be added without touching the rest of the pipeline.

**Harder:** The default output codec is AV1, which a meaningful share of users' devices cannot play
natively out of the box (pre-M3 Macs, stock Windows without the Microsoft Store AV1 extension, most
Linux desktops) — this is a user-visible behavior change from today's typically-H.264 ffmpeg output,
not merely an internal refactor. Users who need guaranteed native playback must build from source
with a C toolchain rather than using a prebuilt binary. `muxide`, a young (v0.2.x) pure-Rust MP4
muxer with no long track record, turned out to have a real AV1 sequence-header parsing bug against
genuine `rav1e` output (see `docs/RISK_REGISTER.md` row 2); the default build currently depends on a
fork with the fix, pending an upstream release.
