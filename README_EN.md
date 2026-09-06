# vcd30player

[![CI](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/NimitzDEV/vcd30player?color=blue&label=release)](https://github.com/NimitzDEV/vcd30player/releases/latest)
[![Rust](https://img.shields.io/badge/Language-Rust_2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**A modern, cross-platform VCD 3.0 player written in Rust.**

vcd30player is an open-source reimplementation of the **VCD 3.0 interactive CD-ROM environment**, bringing interactive VCD 3.0 content from the late 1990s back to modern Windows, macOS, and Linux.

> **No Windows 98. No virtual machine. No original player required.**

[简体中文文档](README.md)

---

## What is VCD 3.0?

VCD 3.0 was an extended VCD format developed in the late 1990s, adding interactive multimedia capabilities on top of conventional VCD playback.

Unlike ordinary VCDs, a VCD 3.0 disc can contain:

* Interactive menus and clickable regions
* Compiled HTML-like pages (`.CHM`)
* Indexed/paletted images (`.YBM`)
* Embedded scripts and navigation logic
* MPEG-1 video (`.DAT`)
* Background music and sound effects (`.WAV`)
* Karaoke and other interactive applications

The original software stack depended on Windows-era multimedia components and proprietary runtime software that are no longer available on modern systems.

**vcd30player aims to preserve this software ecosystem without emulating the entire old PC.**

---

## ✨ Features

### Disc & Format Support

* VCD 3.0 disc/directory detection
* `<COMPHTML>` / `.CHM` parsing
* `.YBM` image decoding
* `AUTORUN.CLS` analysis
* VCD navigation and resource loading

### Interactive Runtime

* VCD 3.0 page rendering
* Interactive button/hotspot regions
* Embedded script execution
* Keyboard and virtual remote-control input
* Timers, navigation, and state management
* Karaoke command support

### Audio & Video

* CD-XA demultiplexing
* MPEG-1 video playback
* Aspect-ratio correction (4:3)
* Background music playback
* WAV sound effects
* Video/audio synchronization

### Modern Runtime

* Written entirely in modern safe Rust
* Runs directly on modern 64-bit operating systems (Windows, macOS, Linux)
* No Windows 9x environment required
* No legacy player installation required
* Designed to be cross-platform

---

## 💾 Download Pre-built Binaries

If you simply want to run the player without installing Rust or setting up a development environment, download the latest standalone pre-built binaries for your platform:

👉 **[Download from GitHub Releases](https://github.com/NimitzDEV/vcd30player/releases)**

* **Windows**: Download `vcd30player-v*-windows-x64.zip`, extract, and double-click `vcd30_player.exe`.
* **Linux**: Download `vcd30player-v*-linux-x64.tar.gz`, extract, and run.

---

## 🛠️ Local Development & Building from Source

If you wish to contribute to development, debug features, or build the player manually from source, follow the instructions below:

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

Start the player GUI:

```bash
cargo run --release
```

Or specify a mounted disc / extracted VCD directory:

```bash
cargo run --release -- "D:\"
```

---

## 🧪 Testing

The project includes unit and integration tests with a small self-contained test fixture, so all tests can run without a physical VCD.

```bash
cargo test
```

To run extended tests against a real disc:

```powershell
$env:VCD_TEST_DISC = "D:\"
cargo test
```

Linux/macOS:

```bash
VCD_TEST_DISC="/media/cdrom" cargo test
```

---

## 🔬 Format Analysis & Architecture

This project started from the investigation of a real VCD 3.0 disc whose original player software no longer works on modern systems.

The project is progressively replacing the original legacy runtime with modern, documented implementations of the underlying formats and execution environment.

Target formats and pipeline:

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

The long-term goal is not to reproduce the original Windows environment, but to **reimplement the semantics required to run VCD 3.0 content directly**.

---

## 📁 Project Structure

```text
vcd30player/
├── .github/
│   ├── ISSUE_TEMPLATE/    # Bug report and feature request templates
│   ├── workflows/
│   │   ├── ci.yml         # Automated continuous integration and test
│   │   └── release.yml    # Automated multi-platform release binary packaging
│   └── PULL_REQUEST_TEMPLATE.md
├── src/
│   ├── assets/            # CHM / YBM / CLS format handling
│   ├── audio/             # Audio playback and mixing
│   ├── core/              # Runtime, scripts and application state
│   ├── ui/                # egui interface and virtual remote
│   └── video/             # CD-XA and MPEG playback
├── c_src/                 # C bridge for MPEG decoding
├── vendor/                # Git submodule (phoboslab/pl_mpeg)
├── tests/                 # Unit/integration tests and fixtures
├── Cargo.toml
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md              # Chinese Documentation
└── README_EN.md           # English Documentation
```

---

## 🤝 Contributing

VCD 3.0 is a largely undocumented and poorly preserved multimedia format. If you have:

* VCD 3.0 discs
* Original VCD 3.0 player software
* Documentation or specifications
* Format analysis findings
* Format samples
* Compatibility information

they can be extremely useful for the project.

Issues, pull requests, format research, and compatibility reports are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting contributions.

---

## ⚠️ Disclaimer

This project does **not** distribute commercial VCD content, copyrighted media, or original commercial discs.

Use the player with discs or backups that you have the legal right to use.

---

## 📜 License

Licensed under either of:

* [MIT License](LICENSE-MIT)
* [Apache License, Version 2.0](LICENSE-APACHE)
