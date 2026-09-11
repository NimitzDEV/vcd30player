# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

## [0.3.0] - 2026-09-12

### Added
- **Application Version Check & In-Place Self-Update**:
  - Lightweight version checking and distribution via Cloudflare R2 / S3-compatible storage and edge CDN.
  - Integrated update status row and trigger button in the "About" dialog, maintaining zero network overhead on startup.
  - Bilingual changelog modal window with auto-detection of system locale (Chinese/English).
  - Background streaming download with real-time percentage/MB progress bar and cancellation support.
  - SHA-256 checksum integrity verification prior to applying updates.
  - Atomic in-place binary self-replacement and seamless restart using `self-replace`.
- **CI Release Automation**:
  - Automated artifact archiving, bilingual changelog extraction, and generation of `version.json` and `checksums.txt`.
  - Dual distribution to both Cloudflare R2 storage and GitHub Releases.

## [0.2.0] - 2026-09-11

### Added
- **VCD 2.0 Classic PBC (Playback Control) State Machine**:
  - Full binary parser for `VCD/LOT.VCD` (Location Table) and `VCD/PSD.VCD` (Play Sequence Descriptor), supporting `PlayList`, `SelectionList`, and `EndList` structures according to the White Book standard.
  - Event-driven PBC state machine supporting multi-level menus, track chains, wait delays, and automatic timeout transitions.
  - High-resolution still picture menu support (`/SEGMENT/ITEMxxxx.DAT` demuxing, decoding, and canvas rendering).
  - MPEG-1 motion video menu playback.
  - Dedicated "PBC" root menu button in the bottom control bar to return to selection menus.
- **Multi-Mode Adaptive Disc Architecture**:
  - Automatic detection and switching between VCD 3.0 Interactive mode, VCD 2.0 Classic PBC mode, and VCD 1.0 Linear video playback.
  - Independent lifecycle management when switching modes.
- **Multi-Digit Numeric Keypad Buffering**:
  - Numeric key buffering for remote control and keyboard navigation (buffers single digits if track count exceeds 9, waiting for `Enter`, a second digit, or a 2.0-second auto-confirm timeout).
  - Real-time status bar feedback during digit entry (e.g. `输入曲目: 1 (按 Enter 确认或等待)`).
  - Direct numeric track selection in VCD 1.0 Linear mode.

### Fixed
- **Track Playback Numeric Keypad Intro Video Jump**:
  - Resolved an issue in VCD 2.0 mode where entering a track number (such as `1`) during playback incorrectly jumped to the opening intro video (LID 1) via LOT lookup instead of the physical track. Now resolves matching PSD descriptors and preserves PBC return semantics.
- **Track List Timecode Separator**:
  - Formatted track list timecode separator between seconds and frames/milliseconds as a dot `.` instead of a colon `:` (e.g. `[05:02.70]`), avoiding confusion with HH:MM:SS format.
- **PBC Menu State Lingering**:
  - Cleared lingering PBC menu titles from the window title bar and status bar upon navigating into track video playback.
- **Home Button Activation Guard**:
  - Disabled home button click activation when running outside of VCD 3.0 Interactive mode.
- **Drawer Close Button Glyph**:
  - Replaced drawer close glyph with high-compatibility symbols to prevent missing glyph rendering on non-standard font systems.

### Changed
- **Side Panel Expansion Window Sizing**:
  - Automatically expands window width when opening the side panel drawer and restores width when closing, preventing canvas aspect ratio compression.
- **Bottom Control Bar Layout**:
  - Moved playback mode switcher after the audio channel selector.
  - Compacted audio channel, playback order, and about buttons into icon controls.
  - Reset button now restarts the current active mode from the beginning.
- **Documentation**:
  - Streamlined Chinese and English README documentation, removing duplicate feature listings and promotional phrasing.

## [0.1.6] - 2026-09-10

### Added
- **Custom Window Title Bar**:
  - Borderless window title bar rendered with egui, matching standard toolbar height (`26.0px`).
  - Centered window title, responsive middle drag area, and double-click window maximize/restore toggle.
  - Vector-drawn window control buttons (minimize, maximize/restore, close with hover highlight) for high-DPI displays without font glyph dependencies.
  - 8-direction window edge and corner resizing handles with borderless viewport commands.
  - 1px outer frame border stroke for borderless window mode.
- **Unified Status Space**:
  - Unified status zone showing idle guide text, active page and video stream metadata, 2-second transient action messages, and hover target indicators.

### Changed
- **Bottom Toolbar Layout & Player Controls**:
  - Reorganized the bottom panel into two functional rows: Row 1 for disc management and video playback controls, Row 2 for navigation, debug toggles, and status information.
  - Updated playback buttons to icon-only controls (`▶`/`⏸`, `⏹`) with tooltips and keyboard shortcuts (`Space`, `ESC`).
  - Expanded video seek slider to fill remaining width with elapsed/total duration displayed at the right.
  - Set vertical panel margins and separator gaps to `6.0px`.
  - Set horizontal spacing between toolbar elements and vertical separators to `8.0px`.

### Fixed
- **Interactive Script Animation Display Delay**:
  - Implemented 300ms display delay on `DRAWIMAGE` statements in the script virtual machine to pace multi-frame animations (such as target hit sequences and score tallying) with audio effects.
  - Suppressed hotspot hover and click interactions while the script virtual machine is actively executing delays or animations.
  - Canvas texture refresh updates in-place without redundant GPU texture handle reallocations.
- **Hover Target Cleanup**:
  - Cleared active hotspot hover indicators immediately upon clicking DAT video targets and during active video playback.
- **Audio Initialization in Headless Environments**:
  - Bypassed native audio driver initialization when running under headless CI environments (`CI=true` / `GITHUB_ACTIONS=true`) to avoid access violation crashes.

---

## [0.1.5] - 2026-09-08

### Added
- **Composite Page Overlay Images**:
  - Support parsing and compositing sub-images onto the base canvas for composite CHM pages (such as answer key overlay patch images `AS011_*.YBM` on exercise pages).
  - Preserved underlying background and base page hotspots when overlay documents have no conflicting click targets.
  - Automatic overlay state cleanup upon page navigation.
- **NTSC Video Stream Support**:
  - Extended MPEG-1 video decoder and player to support NTSC dimensions (`352x240` @ ~29.97/30.0 fps) alongside standard PAL (`352x288` @ 25.0 fps).

### Fixed
- **Underline Hotspot Hit-Testing**:
  - Added upward click tolerance for thin underline hotspots (e.g. fill-in-the-blank questions in exercise pages), allowing clicks in the blank space directly above the underline to register properly.
- **Video Playback Concurrency & ESC Handling**:
  - Resolved lock contention when pressing the Escape key during active video playback, ensuring return to the preceding CHM page.

---

## [0.1.4] - 2026-09-07

### Added
- **Dynamic Variable Text & Score Displays (Chunk 14 `TEXT`)**:
  - Support parsing and rendering Chunk 14 dynamic variable text overlays directly onto the canvas.
  - Variable formatting and score counter displays (such as tracking `i_e` scores in interactive game pages `GAME051` and `SCORE05X`).
- **Hotspot Micro-Scripts (Chunk 13 `MICRO`)**:
  - Support executing inline micro-scripts embedded directly inside hotspot records (such as variable assignments and arithmetic like `i_e=i_e+2`).

### Fixed
- **Conditional Opening Video Playback**:
  - Made opening video playback conditional on the presence of `PlayMpeg` symbols or explicit `.DAT` paths in `AUTORUN.CLS`, avoiding unwanted playback on discs without opening videos.
- **CHM Buffer & Comment Parsing**:
  - Read length-prefixed image filenames at offset `0x60` in CHM Chunk 3 to prevent dirty buffer data corruption.
  - Preserved colons inside `REM` and `'` comment statements during script instruction splitting.
  - Prevented automatic navigation on finished asynchronous background scripts so interactive pages remain open.

### Changed
- Collapsed virtual remote control panel by default on startup.
- Updated project documentation with pre-built binary installation instructions and development guides.

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
- **User Interface**:
  - Cross-platform graphical user interface built with `egui` and `eframe`, supporting DPI scaling and aspect-ratio-preserving rendering.
  - Interactive hotspot visual highlighting and mouse hover/click interaction.
  - Floating video control overlay with play/pause, seek slider, and elapsed/total time display.
  - Collapsible virtual remote control panel supporting physical keyboard shortcuts and simulated remote key codes.
  - Disc folder picker and empty-disc placeholder screen.
- **Testing & Continuous Integration**:
  - Self-contained mock disc fixtures (`tests/fixtures/mock_disc`) allowing test suites to run offline without physical disc drives.
  - Test suite covering unit and integration tests across format parsing, VM execution, audio mixing, and video playback.
  - GitHub Actions CI workflow supporting automated check, test, and clippy runs on Windows and Ubuntu.

[Unreleased]: https://github.com/NimitzDEV/vcd30player/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/NimitzDEV/vcd30player/compare/v0.1.6...v0.2.0
[0.1.6]: https://github.com/NimitzDEV/vcd30player/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/NimitzDEV/vcd30player/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/NimitzDEV/vcd30player/compare/v0.1.0...v0.1.4
[0.1.0]: https://github.com/NimitzDEV/vcd30player/releases/tag/v0.1.0
