# vcd30player

[![Rust](https://img.shields.io/badge/Language-Rust_2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Tests](https://img.shields.io/badge/Tests-47%20Passed-brightgreen.svg)](#testing)

**vcd30player** is a modern, cross-platform interactive media player for **VCD 3.0 format discs** written in safe Rust.

[简体中文文档](README.md)

---

## 📖 Introduction

This project is used to play VCD 3.0 format optical discs.

VCD 3.0 builds upon traditional Video CD playback by adding interactive, page-based menus and scripting:
- **`<COMPHTML>` (.CHM)**: Compiled page files defining visual layouts, button hotspots, and routing logic;
- **`<YUVBMP>` (.YBM)**: 8-bit indexed YUV palette image files compliant with ITU-R BT.601;
- **VCDSCRIPT**: An embedded BASIC-like script engine for handling user input, timing delays, variable computation, and navigation;
- **Audio and Video**: Works alongside MPEG-1 video (`.DAT`) and background audio (`.WAV`) to provide menu navigation, karaoke track selection, and interactive features.

Original 16/32-bit executables cannot run natively on modern 64-bit platforms (Windows 10/11, macOS, Linux). This project provides a cross-platform playback engine implemented in safe Rust, allowing these discs to be played directly on modern computers without virtual machines.

---

## ✨ Key Features

- **Disc Files & Format Decoders**
  - **CHM Page Parser**: Parses `<COMPHTML>` binaries, extracting metadata, palettes, layer chunks, full-screen/polygonal/rectangular hotspots, and embedded VCDSCRIPT code.
  - **YBM Image Decoder**: Decodes ITU-R BT.601 palette bitmaps into RGBA framebuffers; supports sub-image sprite blitting.
  - **CLS Config Decoder**: Parses `PROGRAM/JAVA/AUTORUN.CLS` for disc entry points, cover graphics, and opening videos.
- **VCDSCRIPT Virtual Machine**
  - Integrated tokenizer and AST parser handling arithmetic operations, control flows (`IF...THEN...ELSE`), and subroutines (`GOTO`, `GOSUB/RETURN`).
  - Coroutine/state machine architecture supporting 100ms-calibrated timers (`CALL TIME`), remote key waits with timeout fallback (`CALL IRKEY`), and pseudorandom generators (`CALL RAND`).
  - Karaoke subsystem supporting 19-slot playlist registers (`KARAOKE SET/GET/DEL/INS/PLAY`) with sequential and queued playback modes.
- **Media Decoding & A/V Sync**
  - **CD-XA Demuxer**: Demultiplexes RIFF CD-XA Mode 2 Form 2 interleaved multimedia packets.
  - **MPEG-1 Decoding**: Integrated `pl_mpeg` decoder supporting real-time video playback, seek bar controls, pause/resume, and 4:3 aspect-ratio preservation.
  - **Audio Pipeline**: Supports background loop audio (BGSOUND) and immediate WAV sound effects triggered by button clicks.
- **User Interface & Controls (`egui`)**
  - Interactive virtual remote control (D-pad, numeric keys, enter, exit, volume).
  - Bottom toolbar with disc load/eject SplitButton and runtime reset capability.
  - Debugging overlays: interactive hotspot visualization, OSD cursor indicator, and graceful error alerts for unknown instructions.
- **Self-contained Testing**
  - Packaged with a self-contained 300KB mock disc fixture (`tests/fixtures/mock_disc`), allowing 100% test pass rates without external physical drives.

---

## 🛠️ Build & Run

### Prerequisites

- **Rust Toolchain**: Rust 1.80+ (2024 Edition)
- **C Compiler**: MSVC (Windows) or GCC / Clang (Linux / macOS)

### Build from Source

```bash
git clone https://github.com/your-username/vcd30player.git
cd vcd30player

# Build debug binary
cargo build

# Build optimized release binary
cargo build --release
```

### Run the Player

```bash
# Launch player GUI
cargo run --release

# Launch and load disc directly
cargo run --release -- "D:\"
```

---

## 🧪 Testing

The project comes with a comprehensive test suite of 47 tests:

```bash
# Run all tests using self-contained fixtures
cargo test
```

### Test Against Live Physical Media

To test against an actual mounted disc volume:

```powershell
# Windows (PowerShell)
$env:VCD_TEST_DISC = "D:\"; cargo test
```

```bash
# Linux / macOS (Bash)
VCD_TEST_DISC="/media/cdrom" cargo test
```

---

## 📂 Repository Structure

```text
vcd30player/
├── c_src/                 # C dependencies (pl_mpeg wrapper)
├── src/
│   ├── assets/            # CHM, YBM, CLS format decoders
│   ├── audio/             # Audio mixer and sound effect player
│   ├── core/              # Kernel state machine, VCDSCRIPT AST and VM
│   ├── ui/                # egui user interface and remote control
│   ├── video/             # CD-XA demuxer and MPEG video playback
│   ├── lib.rs             # Library exports
│   └── main.rs            # Application entry point
├── tests/
│   ├── common/            # Shared test disc resolution
│   ├── fixtures/          # Curated, copyright-clean mock disc fixtures
│   └── *.rs               # Phase 1 to Phase 4 integration tests
├── Cargo.toml
├── README.md              # Chinese Documentation
└── README_EN.md           # English Documentation
```

---

## 📜 License

This project is dual-licensed under:
- [MIT License](LICENSE-MIT) or
- [Apache License, Version 2.0](LICENSE-APACHE)
