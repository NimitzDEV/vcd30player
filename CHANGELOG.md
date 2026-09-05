# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-09-06

### Added
- **Disc Format Parsing Engine**:
  - Support parsing disc root class descriptor files (`AUTORUN.CLS` / `ENREACH.CLS`) and resolving the initial startup page.
  - Support parsing compiled `<COMPHTML>` (.CHM) pages, including background image references, navigation jump tables, and polygon/rectangular button hotspots.
  - Support decoding `<YUVBMP>` (.YBM) 8-bit YUV palette images with conversion to RGBA.
- **VCDSCRIPT Scripting Engine**:
  - Lexer and parser (AST) supporting arithmetic, comparison expressions, conditional branching (`IF...THEN...ELSE`), subroutine jumps (`GOSUB...RETURN`), and delay loops.
  - Virtual machine runtime (VM) supporting variable store, remote key waiting (`CALL IRKEY`), pseudo-random number generation (`CALL RAND`), and karaoke playlist commands (`KARAOKE` command set).
  - Built-in typo tolerance and alias support for legacy disc script syntax variations.
- **Multimedia Playback**:
  - CD-XA sector demuxer (RIFF CD-XA and RAW PS stream demuxing) for MPEG-1 video stream extraction.
  - C bridge integration with `pl_mpeg` for MPEG-1 video frame decoding and MPEG-1 Layer II audio decoding.
  - Timestamp-based (PTS) audio/video synchronization, playback control (play/pause), and seek functionality.
  - Background audio loop playback (`BGSOUND`) and on-demand button sound effects (`.WAV`).
- **Modern User Interface**:
  - Cross-platform graphical user interface built with `egui` and `eframe`, supporting DPI scaling and aspect-ratio-preserving rendering.
  - Interactive hotspot visual highlighting and mouse hover/click interaction.
  - Floating video control overlay with play/pause, seek slider, and elapsed/total time display.
  - Collapsible virtual remote control panel supporting physical keyboard shortcuts and simulated remote key codes.
  - Disc folder picker and empty-disc placeholder screen.
- **Testing & Continuous Integration**:
  - Self-contained, copyright-clean mock disc fixtures (`tests/fixtures/mock_disc`) allowing 100% of test suites to run offline without physical disc drives.
  - Comprehensive test suite covering 47 unit and integration tests across format parsing, VM execution, audio mixing, and video playback.
  - GitHub Actions CI workflow supporting automated check, test, and clippy runs on Windows and Ubuntu.
