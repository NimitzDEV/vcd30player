# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Custom Window Title Bar**:
  - Custom borderless window title bar rendered with egui, matching standard single toolbar height (`26.0px`).
  - Centered window title, responsive middle drag area, and double-click window maximize/restore toggle.
  - Crisp vector-drawn window control buttons (minimize, maximize/restore, close with hover highlight) to guarantee sharp rendering across high-DPI displays without font glyph dependencies.
  - Full 8-direction window edge and corner resizing handles with borderless viewport commands.
  - Subtle 1px outer frame border stroke for borderless window mode.
- **Unified Status Space**:
  - Unified status zone dynamically showing idle guide text, active page and video stream metadata, 2-second transient action messages, and interactive cyan hover target indicators.

### Changed
- **Bottom Toolbar Layout & Player Controls**:
  - Reorganized the bottom panel into two functional rows: Row 1 dedicated to disc management and persistent video playback controls, Row 2 dedicated to navigation, debug toggles, and status information.
  - Playback buttons updated to clean icon-only controls (`▶`/`⏸`, `⏹`) with hover tooltips and keyboard shortcuts (`Space`, `ESC`).
  - Expanded video seek slider to fill remaining width with elapsed/total duration displayed at the right.
  - Balanced vertical panel margins and separator gaps to a uniform `6.0px`.
  - Unified horizontal spacing between toolbar elements and vertical separators to `8.0px`.

### Fixed
- **Interactive Script Animation Display Delay**:
  - Implemented the standard 300ms display delay on `DRAWIMAGE` statements in the script virtual machine, enabling multi-frame interactive animations (e.g. target hit sequences, star score tallying) to render smoothly and synchronize with audio effects.
  - Suppressed hotspot mouse hover and click interactions while the script virtual machine is actively executing delays or animations to prevent interrupting playback.
  - Optimized GUI canvas texture refresh to update in-place without redundant GPU texture handle reallocations.
- **Hover Target Cleanup**:
  - Cleared active hotspot hover indicators immediately upon clicking DAT video targets and during active video playback.
- **Audio Initialization in Headless Environments**:
  - Automatically bypass native audio driver initialization when running under headless CI environments (`CI=true` / `GITHUB_ACTIONS=true`) to prevent `0xc0000005` access violation crashes.

---

## [0.1.5] - 2026-09-08

### Added
- **Composite Page Overlay Images**:
  - Support parsing and compositing sub-images onto the base canvas for composite CHM pages (e.g. answer key overlay patch images `AS011_*.YBM` on exercise pages).
  - Preserved underlying background and base page hotspots when overlay documents have no conflicting click targets.
  - Automatic overlay state cleanup upon page navigation.
- **NTSC Video Stream Support**:
  - Extended MPEG-1 video decoder and player to support NTSC dimensions (`352x240` @ ~29.97/30.0 fps) alongside standard PAL (`352x288` @ 25.0 fps).

### Fixed
- **Underline Hotspot Hit-Testing**:
  - Added upward click tolerance for thin underline hotspots (e.g. fill-in-the-blank questions in exercise pages), allowing clicks in the blank space directly above the underline to register properly.
- **Video Playback Concurrency & ESC Handling**:
  - Resolved lock contention and deadlock when pressing the Escape key during active video playback, ensuring instant return to the preceding CHM page.

---

## [0.1.4] - 2026-09-07

### Added
- **Dynamic Variable Text & Score Displays (Chunk 14 `TEXT`)**:
  - Support parsing and rendering Chunk 14 dynamic variable text overlays directly onto the canvas.
  - Real-time variable formatting and score counter displays (e.g. tracking `i_e` scores in interactive game pages `GAME051` and `SCORE05X`).
- **Hotspot Micro-Scripts (Chunk 13 `MICRO`)**:
  - Support executing inline micro-scripts embedded directly inside hotspot records (e.g. variable assignments and arithmetic like `i_e=i_e+2`).

### Fixed
- **Conditional Opening Video Playback**:
  - Made opening video playback strictly conditional on the presence of `PlayMpeg` symbols or explicit `.DAT` paths in `AUTORUN.CLS`, preventing unwanted playback of warning/audio tracks on discs without opening videos.
- **CHM Buffer & Comment Parsing**:
  - Read length-prefixed image filenames at offset `0x60` in CHM Chunk 3 to prevent dirty buffer data corruption.
  - Preserved colons inside `REM` and `'` comment statements during script instruction splitting.
  - Prevented automatic navigation on finished asynchronous background scripts so interactive pages remain open.

### Changed
- Collapsed virtual remote control panel by default on startup for a cleaner viewing experience.
- Refreshed project documentation with pre-built binary installation instructions and development guides.

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

[Unreleased]: https://github.com/NimitzDEV/vcd30player/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/NimitzDEV/vcd30player/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/NimitzDEV/vcd30player/compare/v0.1.0...v0.1.4
[0.1.0]: https://github.com/NimitzDEV/vcd30player/releases/tag/v0.1.0
