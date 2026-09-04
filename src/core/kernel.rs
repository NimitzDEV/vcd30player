//! VCD 3.0 Interactive Kernel and Page Navigation State Machine.

use crate::assets::chm::{ChunkPayload, CompHtmlDoc, MapArea};
use crate::assets::cls::AutoRunConfig;
use crate::assets::ybm::YbmImage;
use crate::audio::AudioManager;
use crate::core::script_ast::ScriptProgram;
use crate::core::script_vm::{UnrecognizedInstruction, VcdScriptVm, VmHost, VmState};
use crate::video::VideoPlayer;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const CANVAS_WIDTH: u32 = 352;
pub const CANVAS_HEIGHT: u32 = 288;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    DiscNotFound(PathBuf),
    FileNotFound(PathBuf),
    ChmParseError(String),
    YbmDecodeError(String),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::DiscNotFound(p) => write!(f, "Disc directory not found: {}", p.display()),
            KernelError::FileNotFound(p) => write!(f, "File not found: {}", p.display()),
            KernelError::ChmParseError(s) => write!(f, "CHM parse error: {}", s),
            KernelError::YbmDecodeError(s) => write!(f, "YBM decode error: {}", s),
        }
    }
}

impl std::error::Error for KernelError {}

pub struct KernelHost<'a> {
    pub disc_root: &'a PathBuf,
    pub canvas: &'a mut Vec<u8>,
    pub cursor_pos: &'a mut Option<(i32, i32)>,
    pub audio: &'a mut AudioManager,
    pub sprite_cache: &'a mut HashMap<String, YbmImage>,
    pub start_time: Instant,
    pub pending_video: &'a mut Option<(String, i32, i32, Option<String>)>,
}

impl<'a> KernelHost<'a> {
    fn find_file(&self, filename: &str) -> Option<PathBuf> {
        find_file_on_disc(self.disc_root, filename)
    }
}

impl<'a> VmHost for KernelHost<'a> {
    fn draw_image(&mut self, filename: &str, x: i32, y: i32, _mode: i32) {
        let clean_name = filename.to_uppercase();
        if !self.sprite_cache.contains_key(&clean_name) {
            if let Some(p) = self.find_file(filename) {
                if let Ok(bytes) = std::fs::read(&p) {
                    if let Ok(img) = YbmImage::decode(&bytes) {
                        self.sprite_cache.insert(clean_name.clone(), img);
                    }
                }
            }
        }

        if let Some(img) = self.sprite_cache.get(&clean_name) {
            img.blit_to_rgba_canvas(self.canvas, CANVAS_WIDTH, CANVAS_HEIGHT, x, y, None);
        }
    }

    fn draw_cursor(&mut self, x: i32, y: i32) {
        *self.cursor_pos = Some((x, y));
    }

    fn play_sound(&mut self, filename: &str) {
        if let Some(p) = self.find_file(filename) {
            self.audio.play_sound_file(p);
        }
    }

    fn play_video(&mut self, filename: &str, start_frame: i32, end_frame: i32, exit_page: Option<&str>) {
        *self.pending_video = Some((
            filename.to_string(),
            start_frame,
            end_frame,
            exit_page.map(|s| s.to_string()),
        ));
    }

    fn karaoke_set(&mut self, _channel: i32, _mode: i32) {
        // Karaoke channel settings (recorded or log)
    }

    fn get_time_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

pub struct VcdKernel {
    pub disc_root: PathBuf,
    pub current_page_name: String,
    pub current_page: Option<CompHtmlDoc>,
    pub current_bg_image: Option<YbmImage>,
    pub canvas: Vec<u8>,
    pub history_stack: Vec<String>,
    pub forward_stack: Vec<String>,
    pub autorun_config: Option<AutoRunConfig>,
    pub audio: AudioManager,
    pub vm: VcdScriptVm,
    pub cursor_pos: Option<(i32, i32)>,
    pub active_alert: Option<UnrecognizedInstruction>,
    pub sprite_cache: HashMap<String, YbmImage>,
    pub start_time: Instant,
    pub active_video: Option<VideoPlayer>,
}

impl VcdKernel {
    pub fn new() -> Self {
        let mut canvas = vec![0u8; (CANVAS_WIDTH * CANVAS_HEIGHT * 4) as usize];
        for chunk in canvas.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
        Self {
            disc_root: PathBuf::new(),
            current_page_name: String::new(),
            current_page: None,
            current_bg_image: None,
            canvas,

            history_stack: Vec::new(),
            forward_stack: Vec::new(),
            autorun_config: None,
            audio: AudioManager::new(),
            vm: VcdScriptVm::new(),
            cursor_pos: None,
            active_alert: None,
            sprite_cache: HashMap::new(),
            start_time: Instant::now(),
            active_video: None,
        }
    }

    pub fn find_file(&self, filename: &str) -> Option<PathBuf> {
        find_file_on_disc(&self.disc_root, filename)
    }

    pub fn open_disc(&mut self, disc_root: PathBuf) -> Result<(), KernelError> {
        if !disc_root.exists() {
            return Err(KernelError::DiscNotFound(disc_root));
        }

        self.disc_root = disc_root;
        self.history_stack.clear();
        self.forward_stack.clear();
        self.sprite_cache.clear();

        let cls_path = self.find_file("AUTORUN.CLS");
        let mut initial_page = "HOMEPAGE.CHM".to_string();
        let mut opening_video = None;

        if let Some(cls_file) = cls_path {
            if let Ok(bytes) = std::fs::read(&cls_file) {
                if let Ok(config) = AutoRunConfig::parse(&bytes) {
                    initial_page = config.homepage_chm.clone();
                    if !config.opening_mpeg.is_empty() {
                        opening_video = Some(config.opening_mpeg.clone());
                    }
                    self.autorun_config = Some(config);
                }
            }
        }

        // Always load initial page first so page state and background are fully ready
        self.load_page(&initial_page, false)?;

        // If an opening video is configured and exists, start playing it
        if let Some(ref mpeg_name) = opening_video {
            if self.find_file(mpeg_name).is_some() {
                let _ = self.start_video(mpeg_name, 0, 0, Some(initial_page.clone()));
            }
        }

        Ok(())
    }

    pub fn load_page(&mut self, chm_name: &str, push_history: bool) -> Result<(), KernelError> {
        if let Some(mut v) = self.active_video.take() {
            v.stop();
        }

        let path = self
            .find_file(chm_name)
            .ok_or_else(|| KernelError::FileNotFound(PathBuf::from(chm_name)))?;

        let bytes = std::fs::read(&path).map_err(|e| KernelError::ChmParseError(e.to_string()))?;
        let doc =
            CompHtmlDoc::parse(&bytes).map_err(|e| KernelError::ChmParseError(e.to_string()))?;

        if push_history && !self.current_page_name.is_empty() {
            self.history_stack.push(self.current_page_name.clone());
            self.forward_stack.clear();
        }

        self.current_page_name = chm_name.to_string();
        self.cursor_pos = None;
        self.active_alert = None;

        // Clear canvas with opaque black
        for chunk in self.canvas.chunks_exact_mut(4) {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
            chunk[3] = 255;
        }


        // Load background image
        if let Some(bg_name) = doc.get_background_image() {
            if let Some(bg_path) = self.find_file(bg_name) {
                if let Ok(bg_bytes) = std::fs::read(&bg_path) {
                    if let Ok(ybm) = YbmImage::decode(&bg_bytes) {
                        ybm.blit_to_rgba_canvas(
                            &mut self.canvas,
                            CANVAS_WIDTH,
                            CANVAS_HEIGHT,
                            0,
                            0,
                            None,
                        );
                        self.current_bg_image = Some(ybm);
                    }
                }
            }
        }

        // BGSOUND handling
        let mut bg_sound = None;
        for chunk in &doc.chunks {
            if let ChunkPayload::BgSound(snd) = chunk {
                if !snd.is_empty() {
                    bg_sound = Some(snd.clone());
                    break;
                }
            }
        }
        if let Some(snd_name) = bg_sound {
            if let Some(snd_path) = self.find_file(&snd_name) {
                self.audio.play_bgm_file(snd_path);
            }
        } else {
            self.audio.stop_bgm();
        }

        // VCDSCRIPT handling
        if let Some(script_code) = doc.get_script() {
            let prog = ScriptProgram::parse(script_code);
            self.vm.load_program(prog);
            self.run_vm();
        } else {
            self.vm.terminate();
        }

        self.current_page = Some(doc);
        Ok(())
    }

    pub fn is_video_active(&self) -> bool {
        self.active_video.is_some()
    }

    pub fn start_video(
        &mut self,
        filename: &str,
        start_frame: u32,
        end_frame: u32,
        exit_target: Option<String>,
    ) -> Result<bool, KernelError> {
        let video_path = self
            .find_file(filename)
            .ok_or_else(|| KernelError::FileNotFound(PathBuf::from(filename)))?;

        let bytes = std::fs::read(&video_path)
            .map_err(|e| KernelError::ChmParseError(format!("Failed to read video file: {}", e)))?;

        self.audio.stop_bgm();
        let sink = self.audio.create_video_sink();
        let player = VideoPlayer::new(
            &bytes,
            filename.to_string(),
            sink,
            start_frame,
            end_frame,
            exit_target,
        )
        .map_err(|e| KernelError::ChmParseError(format!("Video init error: {}", e)))?;

        self.active_video = Some(player);
        Ok(true)
    }

    pub fn stop_video_and_exit(&mut self) -> Result<bool, KernelError> {
        if let Some(mut player) = self.active_video.take() {
            player.stop();
            let exit_target = player.exit_target.clone();

            if let Some(target) = exit_target {
                self.load_page(&target, true)?;
            } else {
                // Resume current page BGSOUND if present
                if let Some(ref doc) = self.current_page {
                    for chunk in &doc.chunks {
                        if let ChunkPayload::BgSound(snd) = chunk {
                            if !snd.is_empty() {
                                if let Some(p) = self.find_file(snd) {
                                    self.audio.play_bgm_file(p);
                                }
                                break;
                            }
                        }
                    }
                }
            }

            if matches!(self.vm.state, VmState::WaitingForVideo) {
                self.vm.state = VmState::Running;
                self.run_vm();
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn update_video(&mut self) -> bool {
        if let Some(ref mut player) = self.active_video {
            let updated = player.update();
            if player.is_ended() {
                let _ = self.stop_video_and_exit();
                false
            } else {
                updated
            }
        } else {
            false
        }
    }

    pub fn run_vm(&mut self) -> VmState {
        let mut pending_video = None;
        let state = {
            let mut host = KernelHost {
                disc_root: &self.disc_root,
                canvas: &mut self.canvas,
                cursor_pos: &mut self.cursor_pos,
                audio: &mut self.audio,
                sprite_cache: &mut self.sprite_cache,
                start_time: self.start_time,
                pending_video: &mut pending_video,
            };
            self.vm.run_until_yield(&mut host)
        };

        if let Some((file, start, end, exit_page)) = pending_video {
            let _ = self.start_video(&file, start.max(0) as u32, end.max(0) as u32, exit_page);
        }

        if let VmState::PausedForAlert(ref alert) = state {
            self.active_alert = Some(alert.clone());
        }
        state
    }

    pub fn is_vm_active(&self) -> bool {
        matches!(
            self.vm.state,
            VmState::Running | VmState::WaitingForDelay { .. }
        )
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<&MapArea> {
        let doc = self.current_page.as_ref()?;
        let all_hotspots = doc.get_all_hotspots();

        // Phase 1: Exact hit test on real (non-point) hotspots.
        // Zero expansion! Strict adherence to authored geometry.
        for area in &all_hotspots {
            if !area.is_point_hotspot() {
                let (min_x, min_y, max_x, max_y) = area.raw_bounds();
                if x >= min_x && x <= max_x && y >= min_y && y <= max_y {
                    return Some(area);
                }
            }
        }

        // Phase 2: Proximity fallback for degenerate point-like hotspots (e.g. return icon at (300, 263)).
        // Only triggers if mouse is not inside any real button.
        const MAX_POINT_RADIUS_SQ: i32 = 16 * 16; // 16-pixel radius
        let mut best_candidate: Option<(&MapArea, i32)> = None;

        for area in &all_hotspots {
            if area.is_point_hotspot() {
                let (min_x, min_y, max_x, max_y) = area.raw_bounds();
                let mid_x = (min_x + max_x) / 2;
                let mid_y = (min_y + max_y) / 2;
                let dist_sq = (x - mid_x) * (x - mid_x) + (y - mid_y) * (y - mid_y);

                if dist_sq <= MAX_POINT_RADIUS_SQ {
                    if best_candidate.map_or(true, |(_, d)| dist_sq < d) {
                        best_candidate = Some((area, dist_sq));
                    }
                }
            }
        }

        best_candidate.map(|(area, _)| area)
    }

    pub fn activate_hotspot(&mut self, area: &MapArea) -> Result<bool, KernelError> {
        // If hotspot routes to a VCDSCRIPT line
        if let Some(line) = area.script_entry_line {
            self.vm.start_at_line(line);
            self.run_vm();
            return Ok(true);
        }

        let target = area.target.trim();
        if target.to_uppercase().ends_with(".DAT") {
            let started = self.start_video(target, 0, 0, None)?;
            return Ok(started);
        }

        if target.to_uppercase().ends_with(".CHM") && target != ".CHM" {
            self.load_page(target, true)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn inject_remote_key(&mut self, key_code: i32) -> VmState {
        if self.is_video_active() {
            if key_code == 32 {
                let _ = self.stop_video_and_exit();
            }
            return self.vm.state.clone();
        }

        if matches!(self.vm.state, VmState::WaitingForKey { .. }) {
            let mut pending_video = None;
            let state = {
                let mut host = KernelHost {
                    disc_root: &self.disc_root,
                    canvas: &mut self.canvas,
                    cursor_pos: &mut self.cursor_pos,
                    audio: &mut self.audio,
                    sprite_cache: &mut self.sprite_cache,
                    start_time: self.start_time,
                    pending_video: &mut pending_video,
                };
                self.vm.inject_key(key_code, &mut host)
            };

            if let Some((file, start, end, exit_page)) = pending_video {
                let _ = self.start_video(&file, start.max(0) as u32, end.max(0) as u32, exit_page);
            }

            if let VmState::PausedForAlert(ref alert) = state {
                self.active_alert = Some(alert.clone());
            }
            if matches!(state, VmState::Finished) && key_code == 32 {
                let _ = self.go_back_or_home();
            }
            state
        } else {
            if key_code == 32 {
                let _ = self.go_back_or_home();
            }
            self.vm.state.clone()
        }
    }

    pub fn skip_alert_and_continue(&mut self) -> VmState {
        self.active_alert = None;
        let mut pending_video = None;
        let state = {
            let mut host = KernelHost {
                disc_root: &self.disc_root,
                canvas: &mut self.canvas,
                cursor_pos: &mut self.cursor_pos,
                audio: &mut self.audio,
                sprite_cache: &mut self.sprite_cache,
                start_time: self.start_time,
                pending_video: &mut pending_video,
            };
            self.vm.skip_unrecognized_and_continue(&mut host)
        };

        if let Some((file, start, end, exit_page)) = pending_video {
            let _ = self.start_video(&file, start.max(0) as u32, end.max(0) as u32, exit_page);
        }

        if let VmState::PausedForAlert(ref alert) = state {
            self.active_alert = Some(alert.clone());
        }
        state
    }

    pub fn terminate_script(&mut self) {
        self.active_alert = None;
        self.vm.terminate();
    }

    pub fn go_back(&mut self) -> Result<bool, KernelError> {
        if let Some(prev) = self.history_stack.pop() {
            self.forward_stack.push(self.current_page_name.clone());
            self.load_page(&prev, false)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn go_forward(&mut self) -> Result<bool, KernelError> {
        if let Some(next) = self.forward_stack.pop() {
            self.history_stack.push(self.current_page_name.clone());
            self.load_page(&next, false)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn go_home(&mut self) -> Result<(), KernelError> {
        let home_target = if self.find_file("HOME.CHM").is_some() {
            "HOME.CHM"
        } else {
            "HOMEPAGE.CHM"
        };
        self.load_page(home_target, true)
    }

    pub fn go_back_or_home(&mut self) -> Result<bool, KernelError> {
        if self.go_back()? {
            Ok(true)
        } else {
            let home_target = if self.find_file("HOME.CHM").is_some() {
                "HOME.CHM"
            } else {
                "HOMEPAGE.CHM"
            };
            if !self.current_page_name.eq_ignore_ascii_case(home_target) {
                self.load_page(home_target, true)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

impl Default for VcdKernel {
    fn default() -> Self {
        Self::new()
    }
}

fn find_file_on_disc(disc_root: &Path, filename: &str) -> Option<PathBuf> {
    let clean_name = filename.trim_start_matches('/').trim_start_matches('\\');

    let direct = disc_root.join(clean_name);
    if direct.exists() {
        return Some(direct);
    }

    let candidate_dirs = [
        disc_root.to_path_buf(),
        disc_root.join("DATA").join("VCD_DATA"),
        disc_root.join("PROGRAM").join("JAVA"),
        disc_root.join("MPEGAV"),
    ];

    for dir in &candidate_dirs {
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if file_name.eq_ignore_ascii_case(clean_name) {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}
