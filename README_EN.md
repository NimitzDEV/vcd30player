# vcd30player

[![CI](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/NimitzDEV/vcd30player?color=blue&label=release)](https://github.com/NimitzDEV/vcd30player/releases/latest)
[![Rust](https://img.shields.io/badge/Language-Rust_2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A cross-platform VCD 3.0 and VCD 2.0 interactive disc player written in Rust. It runs legacy interactive VCD titles on modern Windows, macOS, and Linux systems without virtual machines or legacy Windows environments.

[简体中文文档](README.md)

---

## What is VCD 3.0?

VCD 3.0 was an extended multimedia disc format introduced in the late 1990s that added page-based interactive menus on top of standard VCD video playback.

Unlike linear video VCDs, a VCD 3.0 disc typically contains:

* Compiled markup pages (`.CHM`) with layout definitions and clickable hotspots
* 8-bit indexed palette images (`.YBM`)
* Embedded control scripts for logic and navigation
* MPEG-1 video streams (`.DAT`)
* Background audio and sound effects (`.WAV`)
* Karaoke playlist selection and interactive applications

Original player software relied on proprietary Windows 9x components and runtimes that cannot run directly on modern operating systems. vcd30player parses the underlying disc formats and implements the runtime natively.

---

## Features

### VCD 3.0 Support

* Disc structure detection: automatically detects VCD 3.0 layout conventions, parsing `AUTORUN.CLS` and `ENREACH.CLS` configuration files to locate the startup page and intro sequence.
* Interactive page parsing: parses `<COMPHTML>` / `.CHM` binary page files, including layer layouts, polygonal and rectangular clickable hotspots, navigation tables, and overlay compositing.
* Palette image decoding: decodes `<YUVBMP>` / `.YBM` 8-bit indexed images into RGBA textures with chroma subsampling and alpha blending.
* VCDSCRIPT engine: includes an AST parser and coroutine virtual machine supporting arithmetic expressions, conditional branches (`IF...THEN...ELSE`), subroutines (`GOSUB`), random numbers (`CALL RAND`), key wait operations (`CALL IRKEY`), and animation delays.
* Audio and video: schedules MPEG-1 video playback (`.DAT`), looping background music (`BGSOUND`), WAV sound effects, and karaoke playlist commands.

### VCD 2.0 Playback Control (PBC)

* PBC state machine: parses `LOT.VCD` (Location Table) and `PSD.VCD` (Play Sequence Descriptor) to handle `PlayList`, `SelectionList`, and `EndList` workflows.
* Menus: decodes `/SEGMENT/ITEMxxxx.DAT` high-resolution still picture menu frames and plays standard MPEG-1 motion video menus.
* Remote and keyboard input: buffers multi-digit track numbers with a 2.0-second auto-confirmation timeout or `Enter` confirmation, track seeking, and a dedicated PBC return button.
* Playback modes: identifies and switches between VCD 3.0 interactive pages, VCD 2.0 PBC menus, and VCD 1.0 linear video playback.
* Track indexing: parses `ENTRIES.VCD` entry points, displaying track list timecodes in `MM:SS.FF` format.

### Audio, Video, and Platform Architecture

* Media synchronization: CD-XA sector demultiplexing, MPEG-1 playback with 4:3 aspect ratio correction, and PTS-based audio/video synchronization.
* Modern cross-platform runtime: written in safe Rust with no legacy system dependencies, running natively on 64-bit Windows, macOS, and Linux.

---

## Pre-built Binaries

Pre-compiled standalone archives are available on GitHub Releases:

[Download from GitHub Releases](https://github.com/NimitzDEV/vcd30player/releases)

* Windows: download `vcd30player-v*-windows-x64.zip`, extract, and launch `vcd30_player.exe`.
* Linux: download `vcd30player-v*-linux-x64.tar.gz`, extract, and launch the binary.

---

## Local Development and Building

### Requirements

* Rust 1.80+ (2024 Edition)
* A C compiler:
  * MSVC on Windows
  * GCC or Clang on Linux/macOS

### Build

```bash
git clone --recurse-submodules https://github.com/NimitzDEV/vcd30player.git
cd vcd30player

cargo build --release
```

If the repository was cloned without submodules:

```bash
git submodule update --init --recursive
```

### Run

Launch the graphical interface:

```bash
cargo run --release
```

Or pass a mounted disc letter or extracted VCD folder directly:

```bash
cargo run --release -- "D:\"
```

---

## Testing

Run unit and integration tests using the built-in test fixtures (no physical disc required):

```bash
cargo test
```

To run integration tests against a mounted disc or local dump:

Windows (PowerShell):

```powershell
$env:VCD_TEST_DISC = "D:\"
cargo test
```

Linux / macOS:

```bash
VCD_TEST_DISC="/media/cdrom" cargo test
```

---

## Format Architecture

The project originated from a VCD 3.0 disc whose bundled player no longer functions on modern operating systems. Instead of hosting legacy system runtimes, vcd30player directly parses the on-disc data formats and executes their presentation semantics in native code.

Data flow and format architecture:

```text
VCD 3.0 disc
    │
    ├── CHM / COMPHTML
    ├── YBM / YUVBMP
    ├── CLS / Java bytecode
    ├── VCD scripts
    └── MPEG / CD-XA
            │
            ▼
      Modern VCD 3.0 Runtime
```

---

## Project Structure

```text
vcd30player/
├── .github/
│   ├── ISSUE_TEMPLATE/    # Issue templates
│   ├── workflows/
│   │   ├── ci.yml         # CI and test workflow
│   │   └── release.yml    # Multi-platform release workflow
│   └── PULL_REQUEST_TEMPLATE.md
├── src/
│   ├── assets/            # CHM / YBM / CLS format decoders
│   ├── audio/             # Audio playback and mixing
│   ├── core/              # Runtime state machine, script AST, and VM
│   ├── ui/                # egui interface and virtual remote
│   └── video/             # CD-XA demuxer and MPEG playback
├── c_src/                 # C bridge for MPEG decoding
├── vendor/                # Git submodule (phoboslab/pl_mpeg)
├── tests/                 # Unit and integration tests with fixtures
├── Cargo.toml
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md              # Chinese Documentation
└── README_EN.md           # English Documentation
```

---

## Contributing

Public documentation on VCD 3.0 is sparse. Contributions in the following areas are welcome:

* Compatibility reports and sample disc feedback
* Technical documentation and format findings
* Bug reports and pull requests

Please review [CONTRIBUTING.md](CONTRIBUTING.md) before submitting contributions.

---

## Disclaimer

This project does not distribute copyrighted media or commercial disc images. Use the player only with discs or backups you have the legal right to access.

---

## License

Dual-licensed under either:

* [MIT License](LICENSE-MIT)
* [Apache License, Version 2.0](LICENSE-APACHE)
