//! VCD 3.0 Interactive Kernel and Page Navigation State Machine.

use crate::assets::chm::{ChunkPayload, CompHtmlDoc, MapArea};
use crate::assets::cls::AutoRunConfig;
use crate::assets::ybm::YbmImage;
use crate::audio::AudioManager;
use crate::core::script_ast::ScriptProgram;
use crate::core::script_vm::{UnrecognizedInstruction, VcdScriptVm, VmHost, VmState};
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
}

impl VcdKernel {
    pub fn new() -> Self {
        Self {
            disc_root: PathBuf::new(),
            current_page_name: String::new(),
            current_page: None,
            current_bg_image: None,
            canvas: vec![0u8; (CANVAS_WIDTH * CANVAS_HEIGHT * 4) as usize],
            history_stack: Vec::new(),
            forward_stack: Vec::new(),
            autorun_config: None,
            audio: AudioManager::new(),
            vm: VcdScriptVm::new(),
            cursor_pos: None,
            active_alert: None,
            sprite_cache: HashMap::new(),
            start_time: Instant::now(),
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
        let initial_page = if let Some(cls_file) = cls_path {
            if let Ok(bytes) = std::fs::read(&cls_file) {
                if let Ok(config) = AutoRunConfig::parse(&bytes) {
                    let page = config.homepage_chm.clone();
                    self.autorun_config = Some(config);
                    page
                } else {
                    "HOMEPAGE.CHM".to_string()
                }
            } else {
                "HOMEPAGE.CHM".to_string()
            }
        } else {
            "HOMEPAGE.CHM".to_string()
        };

        self.load_page(&initial_page, false)?;
        Ok(())
    }

    pub fn load_page(&mut self, chm_name: &str, push_history: bool) -> Result<(), KernelError> {
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

        // Clear canvas with black
        self.canvas.fill(0);

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

    pub fn run_vm(&mut self) -> VmState {
        let mut host = KernelHost {
            disc_root: &self.disc_root,
            canvas: &mut self.canvas,
            cursor_pos: &mut self.cursor_pos,
            audio: &mut self.audio,
            sprite_cache: &mut self.sprite_cache,
            start_time: self.start_time,
        };
        let state = self.vm.run_until_yield(&mut host);
        if let VmState::PausedForAlert(ref alert) = state {
            self.active_alert = Some(alert.clone());
        }
        state
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<&MapArea> {
        let doc = self.current_page.as_ref()?;
        for area in doc.get_all_hotspots() {
            let min_x = area.x1.min(area.x2);
            let max_x = area.x1.max(area.x2);
            let min_y = area.y1.min(area.y2);
            let max_y = area.y1.max(area.y2);

            if x >= min_x && x <= max_x && y >= min_y && y <= max_y {
                return Some(area);
            }
        }
        None
    }

    pub fn activate_hotspot(&mut self, area: &MapArea) -> Result<bool, KernelError> {
        // If hotspot routes to a VCDSCRIPT line
        if let Some(line) = area.script_entry_line {
            self.vm.start_at_line(line);
            self.run_vm();
            return Ok(true);
        }

        let target = area.target.trim();
        if target.to_uppercase().ends_with(".CHM") && target != ".CHM" {
            self.load_page(target, true)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn inject_remote_key(&mut self, key_code: i32) -> VmState {
        let mut host = KernelHost {
            disc_root: &self.disc_root,
            canvas: &mut self.canvas,
            cursor_pos: &mut self.cursor_pos,
            audio: &mut self.audio,
            sprite_cache: &mut self.sprite_cache,
            start_time: self.start_time,
        };
        let state = self.vm.inject_key(key_code, &mut host);
        if let VmState::PausedForAlert(ref alert) = state {
            self.active_alert = Some(alert.clone());
        }
        state
    }

    pub fn skip_alert_and_continue(&mut self) -> VmState {
        self.active_alert = None;
        let mut host = KernelHost {
            disc_root: &self.disc_root,
            canvas: &mut self.canvas,
            cursor_pos: &mut self.cursor_pos,
            audio: &mut self.audio,
            sprite_cache: &mut self.sprite_cache,
            start_time: self.start_time,
        };
        let state = self.vm.skip_unrecognized_and_continue(&mut host);
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
