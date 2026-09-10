//! VCD 3.0 Interactive Kernel and Page Navigation State Machine.

use crate::assets::chm::{ChunkPayload, CompHtmlDoc, MapArea, MicroScriptOp};
use crate::assets::cls::AutoRunConfig;
use crate::assets::ybm::YbmImage;
use crate::audio::AudioManager;
use crate::core::script_ast::ScriptProgram;
use crate::core::script_vm::{UnrecognizedInstruction, VcdScriptVm, VmHost, VmState};
use crate::video::{VideoPlayState, VideoPlayer};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const CANVAS_WIDTH: u32 = 352;
pub const CANVAS_HEIGHT: u32 = 288;

const FONT_8X16_DIGITS: [[u8; 16]; 10] = [
    // '0'
    [0x00, 0x3c, 0x66, 0x66, 0x6e, 0x76, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00],
    // '1'
    [0x00, 0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7e, 0x00, 0x00, 0x00],
    // '2'
    [0x00, 0x3c, 0x66, 0x06, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x60, 0x66, 0x7e, 0x7e, 0x00, 0x00, 0x00],
    // '3'
    [0x00, 0x3c, 0x66, 0x06, 0x06, 0x1c, 0x06, 0x06, 0x06, 0x06, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00],
    // '4'
    [0x00, 0x0c, 0x1c, 0x3c, 0x6c, 0xcc, 0xfe, 0x0c, 0x0c, 0x0c, 0x0c, 0x1e, 0x00, 0x00, 0x00, 0x00],
    // '5'
    [0x00, 0x7e, 0x60, 0x60, 0x7c, 0x66, 0x06, 0x06, 0x06, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00],
    // '6'
    [0x00, 0x1c, 0x30, 0x60, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00],
    // '7'
    [0x00, 0x7e, 0x66, 0x06, 0x0c, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00],
    // '8'
    [0x00, 0x3c, 0x66, 0x66, 0x66, 0x3c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00],
    // '9'
    [0x00, 0x3c, 0x66, 0x66, 0x66, 0x66, 0x3e, 0x06, 0x06, 0x0c, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00],
];

const FONT_8X16_MINUS: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7e, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const FONT_8X16_PLUS: [u8; 16] = [
    0x00, 0x00, 0x00, 0x18, 0x18, 0x18, 0x7e, 0x7e, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Maps variable names like "i_a".."i_f" or single letter "a".."z" to index 0..25.
pub fn var_name_to_index(var: &str) -> Option<usize> {
    let trimmed = var.trim();
    if trimmed.is_empty() {
        return None;
    }
    let ch = if (trimmed.starts_with("i_") || trimmed.starts_with("I_")) && trimmed.len() >= 3 {
        trimmed.chars().nth(2)?
    } else {
        trimmed.chars().next()?
    };
    if ch.is_ascii_alphabetic() {
        Some((ch.to_ascii_lowercase() as u8 - b'a') as usize)
    } else {
        None
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KaraokePlaylist {
    pub slots: [i32; 19],
    pub count: usize,
}

impl KaraokePlaylist {
    pub fn new() -> Self {
        Self {
            slots: [0; 19],
            count: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn set(&mut self, index: i32, val: i32) {
        if index <= 0 {
            self.slots[0] = val;
            return;
        }
        let idx = index as usize;
        if idx <= self.count {
            self.slots[idx - 1] = val;
        } else {
            let next_slot = (self.count + 1).min(19);
            self.count = next_slot;
            self.slots[next_slot - 1] = val;
        }
    }

    pub fn get(&self, index: i32) -> i32 {
        if index <= 0 || (index as usize) > self.count {
            -1
        } else {
            self.slots[(index - 1) as usize]
        }
    }

    pub fn del(&mut self, index: i32) {
        if index <= 0 || (index as usize) > self.count {
            return;
        }
        let idx = (index - 1) as usize;
        for i in idx..self.count.saturating_sub(1) {
            self.slots[i] = self.slots[i + 1];
        }
        if self.count > 0 {
            self.slots[self.count - 1] = 0;
            self.count -= 1;
        }
    }

    pub fn ins(&mut self, index: i32, val: i32) {
        if index <= 0 || (index as usize) > self.count {
            return;
        }
        let idx = (index - 1) as usize;
        let new_count = (self.count + 1).min(19);
        self.count = new_count;
        for i in (idx + 1..self.count).rev() {
            self.slots[i] = self.slots[i - 1];
        }
        self.slots[idx] = val;
    }

    pub fn play(&mut self) -> Option<i32> {
        if self.count == 0 {
            return None;
        }
        let song_id = self.slots[0];
        for i in 0..self.count.saturating_sub(1) {
            self.slots[i] = self.slots[i + 1];
        }
        self.slots[self.count - 1] = 0;
        self.count -= 1;
        Some(song_id)
    }

    pub fn clear(&mut self) {
        self.slots = [0; 19];
        self.count = 0;
    }
}

impl std::ops::Index<usize> for KaraokePlaylist {
    type Output = i32;
    fn index(&self, index: usize) -> &Self::Output {
        &self.slots[index]
    }
}

impl Default for KaraokePlaylist {
    fn default() -> Self {
        Self::new()
    }
}

pub struct KernelHost<'a> {
    pub disc_root: &'a PathBuf,
    pub canvas: &'a mut Vec<u8>,
    pub cursor_pos: &'a mut Option<(i32, i32)>,
    pub audio: &'a mut AudioManager,
    pub sprite_cache: &'a mut HashMap<String, YbmImage>,
    pub start_time: Instant,
    pub pending_video: &'a mut Option<(String, i32, i32, Option<String>)>,
    pub karaoke_playlist: &'a mut KaraokePlaylist,
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

    fn karaoke_set(&mut self, index: i32, val: i32) {
        self.karaoke_playlist.set(index, val);
    }

    fn karaoke_get(&self, index: i32) -> i32 {
        self.karaoke_playlist.get(index)
    }

    fn karaoke_del(&mut self, index: i32) {
        self.karaoke_playlist.del(index);
    }

    fn karaoke_ins(&mut self, index: i32, val: i32) {
        self.karaoke_playlist.ins(index, val);
    }

    fn karaoke_play(&mut self) -> bool {
        let song_id = match self.karaoke_playlist.play() {
            Some(id) => id,
            None => return false,
        };
        let candidates = if song_id < 10 {
            vec![
                format!("MPEGAV/MUSIC0{}.DAT", song_id),
                format!("MPEGAV/AVSEQ0{}.DAT", song_id),
                format!("MUSIC0{}.DAT", song_id),
                format!("AVSEQ0{}.DAT", song_id),
            ]
        } else {
            vec![
                format!("MPEGAV/MUSIC{}.DAT", song_id),
                format!("MPEGAV/AVSEQ{}.DAT", song_id),
                format!("MUSIC{}.DAT", song_id),
                format!("AVSEQ{}.DAT", song_id),
            ]
        };
        for cand in &candidates {
            if self.find_file(cand).is_some() {
                *self.pending_video = Some((cand.clone(), 0, 0, None));
                return true;
            }
        }
        let fallback = candidates[0].clone();
        *self.pending_video = Some((fallback, 0, 0, None));
        true
    }

    fn get_time_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackMode {
    #[default]
    Sequential,
    ListRepeat,
    SingleRepeat,
}

impl PlaybackMode {
    pub fn cycle(&self) -> Self {
        match self {
            PlaybackMode::Sequential => PlaybackMode::ListRepeat,
            PlaybackMode::ListRepeat => PlaybackMode::SingleRepeat,
            PlaybackMode::SingleRepeat => PlaybackMode::Sequential,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PlaybackMode::Sequential => "➡ 顺序播放",
            PlaybackMode::ListRepeat => "🔁 列表循环",
            PlaybackMode::SingleRepeat => "🔂 单曲循环",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PlaybackMode::Sequential => "➡",
            PlaybackMode::ListRepeat => "🔁",
            PlaybackMode::SingleRepeat => "🔂",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PlaybackMode::Sequential => "顺序播放",
            PlaybackMode::ListRepeat => "列表循环",
            PlaybackMode::SingleRepeat => "单曲循环",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveDiscMode {
    #[default]
    Vcd30Interactive,
    Vcd20Classic,
    Vcd10Linear,
}

impl ActiveDiscMode {
    pub fn label(&self) -> &'static str {
        match self {
            ActiveDiscMode::Vcd30Interactive => "VCD 3.0 互动模式",
            ActiveDiscMode::Vcd20Classic => "VCD 2.0 经典模式",
            ActiveDiscMode::Vcd10Linear => "VCD 1.0 纯视频模式",
        }
    }
}

pub struct VcdKernel {
    pub disc_root: PathBuf,
    pub current_page_name: String,
    pub current_page: Option<CompHtmlDoc>,
    pub current_bg_image: Option<YbmImage>,
    pub overlay_doc: Option<CompHtmlDoc>,
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
    pub karaoke_playlist: KaraokePlaylist,
    pub disc_type: Option<crate::vcd::VcdDiscType>,
    pub channel_mode: crate::audio::AudioChannelMode,
    pub playback_mode: PlaybackMode,
    pub active_mode: ActiveDiscMode,
    pub tracks: Vec<crate::vcd::DiscTrackInfo>,
    pub current_track_index: Option<usize>,
    pub pbc: Option<crate::vcd::PbcEngine>,
    pub pbc_digit_buffer: Vec<u8>,
    pub pbc_digit_timestamp: Option<Instant>,
}

impl VcdKernel {
    pub fn new() -> Self {
        let mut canvas = vec![0u8; (CANVAS_WIDTH * CANVAS_HEIGHT * 4) as usize];
        for chunk in canvas.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
        Self {
            disc_root: PathBuf::new(),
            disc_type: None,
            current_page_name: String::new(),
            current_page: None,
            current_bg_image: None,
            overlay_doc: None,
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
            karaoke_playlist: KaraokePlaylist::new(),
            channel_mode: crate::audio::AudioChannelMode::Stereo,
            playback_mode: PlaybackMode::Sequential,
            active_mode: ActiveDiscMode::Vcd30Interactive,
            tracks: Vec::new(),
            current_track_index: None,
            pbc: None,
            pbc_digit_buffer: Vec::new(),
            pbc_digit_timestamp: None,
        }
    }

    pub fn get_variable_by_name(&self, var_name: &str) -> i32 {
        if let Some(idx) = var_name_to_index(var_name) {
            self.vm.variables[idx]
        } else {
            0
        }
    }

    pub fn set_variable_by_name(&mut self, var_name: &str, val: i32) {
        if let Some(idx) = var_name_to_index(var_name) {
            self.vm.variables[idx] = val;
        }
    }

    fn eval_micro_script_operand(&self, s: &str) -> i32 {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return 0;
        }
        if let Some(idx) = var_name_to_index(trimmed) {
            if trimmed.starts_with("i_")
                || trimmed.starts_with("I_")
                || (trimmed.len() == 1 && trimmed.chars().next().unwrap().is_ascii_alphabetic())
            {
                return self.vm.variables[idx];
            }
        }
        trimmed.parse::<i32>().unwrap_or(0)
    }

    pub fn execute_micro_script(&mut self, op: &MicroScriptOp) {
        let op1_val = self.eval_micro_script_operand(&op.op1);
        let op2_val = self.eval_micro_script_operand(&op.op2);
        let res = if op.op2.trim().is_empty() {
            op1_val
        } else {
            match op.opcode {
                0 => op1_val.saturating_add(op2_val),
                1 => op1_val.saturating_sub(op2_val),
                2 => op1_val.saturating_mul(op2_val),
                3 => {
                    if op2_val != 0 {
                        op1_val / op2_val
                    } else {
                        0
                    }
                }
                _ => op1_val,
            }
        };
        self.set_variable_by_name(&op.target_var, res);
    }

    pub fn draw_char_on_canvas(&mut self, x: i32, y: i32, ch: char, color: [u8; 4]) {
        let glyph = match ch {
            '0'..='9' => &FONT_8X16_DIGITS[(ch as u8 - b'0') as usize],
            '-' => &FONT_8X16_MINUS,
            '+' => &FONT_8X16_PLUS,
            _ => return,
        };

        for row in 0..16 {
            let py = y + row;
            if py < 0 || py >= CANVAS_HEIGHT as i32 {
                continue;
            }
            let mask = glyph[row as usize];
            for col in 0..8 {
                if (mask & (1 << (7 - col))) != 0 {
                    let px = x + col;
                    if px >= 0 && px < CANVAS_WIDTH as i32 {
                        let offset = ((py as u32 * CANVAS_WIDTH + px as u32) * 4) as usize;
                        if offset + 4 <= self.canvas.len() {
                            self.canvas[offset..offset + 4].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
    }

    pub fn draw_string_on_canvas(&mut self, x: i32, y: i32, text: &str, color: [u8; 4]) {
        let mut curr_x = x;
        for ch in text.chars() {
            if ch == ' ' {
                curr_x += 8;
                continue;
            }
            self.draw_char_on_canvas(curr_x, y, ch, color);
            curr_x += 9;
        }
    }

    /// Applies an overlay document on top of the current base page without wiping the canvas.
    pub fn apply_overlay_doc(&mut self, doc: CompHtmlDoc) {
        // 1. Re-blit base image if available to erase previous overlay drawings
        if let Some(ybm) = &self.current_bg_image {
            ybm.blit_to_rgba_canvas(
                &mut self.canvas,
                CANVAS_WIDTH,
                CANVAS_HEIGHT,
                0,
                0,
                None,
            );
        }

        // 2. Render any overlay images onto canvas
        for img in doc.get_images() {
            if let Some(img_path) = self.find_file(&img.filename) {
                if let Ok(bytes) = std::fs::read(&img_path) {
                    if let Ok(ybm) = YbmImage::decode(&bytes) {
                        ybm.blit_to_rgba_canvas(
                            &mut self.canvas,
                            CANVAS_WIDTH,
                            CANVAS_HEIGHT,
                            img.x,
                            img.y,
                            None,
                        );
                    }
                }
            }
        }

        // 3. Render any variable text elements onto canvas
        for (x, y, var_name) in doc.get_variable_texts() {
            let val = self.get_variable_by_name(var_name);
            let text = format!("{}", val);
            self.draw_string_on_canvas(x, y, &text, [0, 0, 0, 255]);
        }

        // 3. Play any overlay BGSOUND if defined
        if let Some((snd_name, loop_count)) = doc.get_background_sound_info() {
            if let Some(snd_path) = self.find_file(snd_name) {
                self.audio.play_bgm_file(snd_path, loop_count);
            }
        }

        // 4. Run any overlay script if defined
        if let Some(script_code) = doc.get_script() {
            let prog = ScriptProgram::parse(script_code);
            self.vm.load_program(prog);
            self.run_vm();
        }

        self.overlay_doc = Some(doc);
    }

    /// Loads an overlay page from disc by CHM filename and composites it on current base page.
    pub fn load_overlay_page(&mut self, chm_name: &str) -> Result<(), KernelError> {
        let path = self
            .find_file(chm_name)
            .ok_or_else(|| KernelError::FileNotFound(PathBuf::from(chm_name)))?;

        let bytes = std::fs::read(&path).map_err(|e| KernelError::ChmParseError(e.to_string()))?;
        let doc =
            CompHtmlDoc::parse(&bytes).map_err(|e| KernelError::ChmParseError(e.to_string()))?;

        self.apply_overlay_doc(doc);
        Ok(())
    }

    /// Dismisses any active overlay and restores base canvas.
    pub fn dismiss_overlay(&mut self) {
        if self.overlay_doc.is_some() {
            self.overlay_doc = None;
            if let Some(ybm) = &self.current_bg_image {
                ybm.blit_to_rgba_canvas(
                    &mut self.canvas,
                    CANVAS_WIDTH,
                    CANVAS_HEIGHT,
                    0,
                    0,
                    None,
                );
            }
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
        self.karaoke_playlist.clear();
        self.pbc = None;
        self.pbc_digit_buffer.clear();

        let disc_type = crate::vcd::detect_disc(&self.disc_root);
        self.disc_type = Some(disc_type.clone());
        self.tracks = crate::vcd::scan_disc_tracks(&self.disc_root);
        self.current_track_index = None;

        match &disc_type {
            crate::vcd::VcdDiscType::Vcd30Interactive { .. } => {
                self.active_mode = ActiveDiscMode::Vcd30Interactive;
            }
            crate::vcd::VcdDiscType::Vcd20WithPbc { .. } => {
                self.active_mode = ActiveDiscMode::Vcd20Classic;
            }
            _ => {
                self.active_mode = ActiveDiscMode::Vcd10Linear;
            }
        }

        if self.active_mode == ActiveDiscMode::Vcd30Interactive {
            let cls_path = self.find_file("AUTORUN.CLS");
            let mut initial_page = "HOMEPAGE.CHM".to_string();
            let mut opening_video = None;

            if let Some(cls_file) = cls_path {
                if let Ok(bytes) = std::fs::read(&cls_file) {
                    if let Ok(config) = AutoRunConfig::parse(&bytes) {
                        initial_page = config.homepage_chm.clone();
                        if let Some(ref mpeg) = config.opening_mpeg {
                            if !mpeg.is_empty() {
                                opening_video = Some(mpeg.clone());
                            }
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
        } else if self.active_mode == ActiveDiscMode::Vcd20Classic {
            self.current_page = None;
            self.current_page_name.clear();
            if let Err(_) = self.start_pbc() {
                if !self.tracks.is_empty() {
                    self.play_track(0)?;
                }
            }
        } else {
            // VCD 1.0 mode: start playing first track if available
            self.current_page = None;
            self.current_page_name.clear();
            if !self.tracks.is_empty() {
                self.play_track(0)?;
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

        if doc.is_overlay() {
            self.apply_overlay_doc(doc);
            return Ok(());
        }

        self.overlay_doc = None;

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

        // Stop any one-shot sound effect on page transition
        self.audio.stop_sound();

        // BGSOUND handling
        let mut bg_sound = None;
        for chunk in &doc.chunks {
            if let ChunkPayload::BgSound { filename, loop_count } = chunk {
                if !filename.is_empty() {
                    bg_sound = Some((filename.clone(), *loop_count));
                    break;
                }
            }
        }
        if let Some((snd_name, loop_count)) = bg_sound {
            if let Some(snd_path) = self.find_file(&snd_name) {
                self.audio.play_bgm_file(snd_path, loop_count);
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

        self.audio.stop_all();
        let sink = self.audio.create_video_sink();
        let mut player = VideoPlayer::new(
            &bytes,
            filename.to_string(),
            sink,
            start_frame,
            end_frame,
            exit_target,
        )
        .map_err(|e| KernelError::ChmParseError(format!("Video init error: {}", e)))?;
        player.set_channel_mode(self.channel_mode);

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
                    if let Some((snd, loop_count)) = doc.get_background_sound_info() {
                        if let Some(p) = self.find_file(snd) {
                            self.audio.play_bgm_file(p, loop_count);
                        }
                    }
                }
            }

            if matches!(self.vm.state, VmState::WaitingForVideo) || self.vm.is_karaoke_video {
                self.vm.on_video_finished();
                self.run_vm();
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Checks if current disc supports VCD 3.0 Interactive mode.
    pub fn supports_vcd30(&self) -> bool {
        self.disc_type.as_ref().map(|d| d.supports_vcd30()).unwrap_or(false)
    }

    /// Checks if current disc supports VCD 2.0 Classic mode.
    pub fn supports_vcd20(&self) -> bool {
        self.disc_type.as_ref().map(|d| d.supports_vcd20()).unwrap_or(false)
    }

    /// Checks if current disc supports VCD 1.0 Linear mode.
    pub fn supports_vcd10(&self) -> bool {
        !self.tracks.is_empty()
            || self.disc_type.as_ref().map(|d| d.supports_vcd10()).unwrap_or(false)
    }

    /// Switches active disc playback mode (VCD 3.0 Interactive, VCD 2.0 Classic, VCD 1.0 Linear).
    pub fn switch_active_mode(&mut self, target_mode: ActiveDiscMode) -> Result<(), KernelError> {
        self.active_mode = target_mode;
        match target_mode {
            ActiveDiscMode::Vcd30Interactive => {
                let root = self.disc_root.clone();
                self.open_disc(root)
            }
            ActiveDiscMode::Vcd20Classic => {
                if let Some(mut v) = self.active_video.take() {
                    v.stop();
                }
                self.audio.stop_all();
                self.vm.terminate();
                self.current_page = None;
                self.current_page_name.clear();
                self.pbc_digit_buffer.clear();
                for chunk in self.canvas.chunks_exact_mut(4) {
                    chunk[0] = 0;
                    chunk[1] = 0;
                    chunk[2] = 0;
                    chunk[3] = 255;
                }
                if let Err(_) = self.start_pbc() {
                    if !self.tracks.is_empty() {
                        self.play_track(0)?;
                    }
                }
                Ok(())
            }
            ActiveDiscMode::Vcd10Linear => {
                if let Some(mut v) = self.active_video.take() {
                    v.stop();
                }
                self.audio.stop_all();
                self.vm.terminate();
                self.current_page = None;
                self.current_page_name.clear();
                self.pbc_digit_buffer.clear();
                for chunk in self.canvas.chunks_exact_mut(4) {
                    chunk[0] = 0;
                    chunk[1] = 0;
                    chunk[2] = 0;
                    chunk[3] = 255;
                }
                if !self.tracks.is_empty() {
                    self.play_track(0)?;
                }
                Ok(())
            }
        }
    }

    /// Restarts playback from the beginning in the current active mode.
    /// Mode switching is never performed here; it strictly preserves `self.active_mode`.
    pub fn restart_current_mode(&mut self) -> Result<(), KernelError> {
        match self.active_mode {
            ActiveDiscMode::Vcd30Interactive => {
                if let Some(mut v) = self.active_video.take() {
                    v.stop();
                }
                self.audio.stop_all();
                self.vm.terminate();
                self.history_stack.clear();
                self.forward_stack.clear();
                self.karaoke_playlist.clear();
                self.active_alert = None;

                let initial_page = self
                    .autorun_config
                    .as_ref()
                    .map(|c| c.homepage_chm.clone())
                    .unwrap_or_else(|| "HOMEPAGE.CHM".to_string());
                let opening_video = self
                    .autorun_config
                    .as_ref()
                    .and_then(|c| c.opening_mpeg.clone());

                self.load_page(&initial_page, false)?;

                if let Some(ref mpeg_name) = opening_video {
                    if self.find_file(mpeg_name).is_some() {
                        let _ = self.start_video(mpeg_name, 0, 0, Some(initial_page));
                    }
                }
                Ok(())
            }
            ActiveDiscMode::Vcd20Classic => {
                if let Some(mut v) = self.active_video.take() {
                    v.stop();
                }
                self.audio.stop_all();
                self.vm.terminate();
                self.current_page = None;
                self.current_page_name.clear();
                for chunk in self.canvas.chunks_exact_mut(4) {
                    chunk[0] = 0;
                    chunk[1] = 0;
                    chunk[2] = 0;
                    chunk[3] = 255;
                }
                if let Err(_) = self.restart_pbc() {
                    if !self.tracks.is_empty() {
                        self.play_track(0)?;
                    }
                }
                Ok(())
            }
            ActiveDiscMode::Vcd10Linear => {
                if let Some(mut v) = self.active_video.take() {
                    v.stop();
                }
                self.audio.stop_all();
                self.vm.terminate();
                self.current_page = None;
                self.current_page_name.clear();
                for chunk in self.canvas.chunks_exact_mut(4) {
                    chunk[0] = 0;
                    chunk[1] = 0;
                    chunk[2] = 0;
                    chunk[3] = 255;
                }
                if !self.tracks.is_empty() {
                    self.play_track(0)?;
                }
                Ok(())
            }
        }
    }

    /// Initializes PBC interactive state machine from LOT.VCD and PSD.VCD.
    pub fn init_pbc(&mut self) -> Result<(), String> {
        let lot_path = crate::vcd::detector::find_path_ci(&self.disc_root, &["VCD", "LOT.VCD"])
            .ok_or_else(|| "未找到 LOT.VCD 映射文件".to_string())?;
        let psd_path = crate::vcd::detector::find_path_ci(&self.disc_root, &["VCD", "PSD.VCD"])
            .ok_or_else(|| "未找到 PSD.VCD 控制文件".to_string())?;

        let offset_mult = 8;
        let lot = crate::vcd::LotTable::from_file(&lot_path, offset_mult)?;
        let psd = crate::vcd::PsdTable::from_file(&psd_path, offset_mult)?;

        self.pbc = Some(crate::vcd::PbcEngine::new(lot, psd));
        self.pbc_digit_buffer.clear();
        Ok(())
    }

    /// Starts PBC playback according to VCD 2.0 White Book rules.
    pub fn start_pbc(&mut self) -> Result<(), String> {
        if self.pbc.is_none() {
            self.init_pbc()?;
        }
        let action = self
            .pbc
            .as_mut()
            .and_then(|p| p.start())
            .ok_or_else(|| "启动 PBC 状态机失败".to_string())?;
        self.execute_pbc_action(action)
    }

    /// Restarts PBC playback from the beginning (LID 1).
    pub fn restart_pbc(&mut self) -> Result<(), String> {
        self.start_pbc()
    }

    /// User presses the dedicated "PBC" button.
    /// Jumps directly to the primary/root selection menu or restarts PBC loop.
    pub fn trigger_pbc_menu(&mut self) -> Result<(), String> {
        if self.active_mode != ActiveDiscMode::Vcd20Classic {
            return Err("仅在 VCD 2.0 模式下支持 PBC 菜单".to_string());
        }
        if self.pbc.is_none() {
            self.init_pbc()?;
        }
        let action = self
            .pbc
            .as_mut()
            .and_then(|p| p.press_pbc())
            .ok_or_else(|| "未找到有效 PBC 菜单".to_string())?;
        self.execute_pbc_action(action)
    }

    /// Executes an action emitted by the PBC state machine.
    pub fn execute_pbc_action(&mut self, action: crate::vcd::PbcAction) -> Result<(), String> {
        match action {
            crate::vcd::PbcAction::PlayTrack { track_number, item_id, .. } => {
                self.current_page = None;
                self.current_page_name.clear();
                let track_idx = self.tracks.iter().position(|t| {
                    t.track_no == track_number
                        || (item_id >= 2 && item_id <= 99 && t.index == (item_id - 1) as usize)
                });
                if let Some(idx) = track_idx {
                    self.play_track(idx).map_err(|e| e.to_string())?;
                    Ok(())
                } else if !self.tracks.is_empty() {
                    let fallback_idx = (track_number.saturating_sub(2) as usize).min(self.tracks.len() - 1);
                    self.play_track(fallback_idx).map_err(|e| e.to_string())?;
                    Ok(())
                } else {
                    Err(format!("未找到 PBC 轨道: {}", track_number))
                }
            }
            crate::vcd::PbcAction::DisplayStillMenu { item_id, segment_index, .. } => {
                if let Some(mut v) = self.active_video.take() {
                    v.stop();
                }
                self.current_page = None;
                self.current_page_name = format!("PBC 菜单 (ITEM{:04}.DAT)", segment_index);
                if let Some(seg_path) = crate::vcd::resolve_segment_path(&self.disc_root, item_id) {
                    let (w, h, rgba) = crate::vcd::decode_segment_frame(&seg_path)?;
                    crate::vcd::blit_segment_to_canvas(w, h, &rgba, &mut self.canvas);
                    Ok(())
                } else {
                    Err(format!("未找到段菜单文件: ITEM{:04}.DAT", segment_index))
                }
            }
            crate::vcd::PbcAction::PlayMotionMenu { track_number, item_id, .. } => {
                self.current_page = None;
                let track_idx = self.tracks.iter().position(|t| {
                    t.track_no == track_number
                        || (item_id >= 2 && item_id <= 99 && t.index == (item_id - 1) as usize)
                });
                let res = if let Some(idx) = track_idx {
                    self.play_track(idx).map_err(|e| e.to_string())
                } else if !self.tracks.is_empty() {
                    let fallback_idx = (track_number.saturating_sub(2) as usize).min(self.tracks.len() - 1);
                    self.play_track(fallback_idx).map_err(|e| e.to_string())
                } else {
                    Err(format!("未找到 PBC 动态选单轨道: {}", track_number))
                };
                if res.is_ok() {
                    self.current_page_name = format!("PBC 动态选单 ({})", track_number);
                }
                res.map(|_| ())
            }
            crate::vcd::PbcAction::End => {
                let _ = self.stop_video_and_exit();
                for chunk in self.canvas.chunks_exact_mut(4) {
                    chunk[0] = 0;
                    chunk[1] = 0;
                    chunk[2] = 0;
                    chunk[3] = 255;
                }
                self.current_page_name.clear();
                Ok(())
            }
        }
    }

    /// Handles a numeric digit input in VCD 2.0 PBC mode.
    pub fn handle_pbc_digit(&mut self, digit: u8) -> Result<(), String> {
        let (max_sel, bsn) = if let Some(ref pbc) = self.pbc {
            if let crate::vcd::PbcState::InSelection { ref desc, .. } = pbc.state {
                let max = (desc.bsn as usize + desc.nos as usize).saturating_sub(1);
                (Some(max), desc.bsn as usize)
            } else {
                (Some(self.tracks.len()), 1)
            }
        } else {
            (Some(self.tracks.len()), 1)
        };

        let mut next_action = None;
        if let Some(max) = max_sel {
            if max == 0 {
                return Ok(());
            }
            if max <= 9 {
                let sel_num = digit as usize;
                if let Some(ref mut pbc) = self.pbc {
                    next_action = pbc.select_number(sel_num)?;
                }
            } else {
                self.pbc_digit_buffer.push(digit);
                self.pbc_digit_timestamp = Some(Instant::now());
                let val = self.pbc_digit_buffer.iter().fold(0usize, |acc, &d| acc * 10 + d as usize);
                if val * 10 > max || self.pbc_digit_buffer.len() >= 2 || (val >= bsn && val > max / 10 && val <= max) {
                    self.pbc_digit_buffer.clear();
                    self.pbc_digit_timestamp = None;
                    if let Some(ref mut pbc) = self.pbc {
                        next_action = pbc.select_number(val)?;
                    }
                }
            }
        } else if let Some(ref mut pbc) = self.pbc {
            let sel_num = digit as usize;
            next_action = pbc.select_number(sel_num)?;
        }

        if let Some(action) = next_action {
            return self.execute_pbc_action(action);
        }
        Ok(())
    }

    /// Handles default / confirm / Enter input in VCD 2.0 PBC mode.
    pub fn handle_pbc_enter(&mut self) -> Result<(), String> {
        let mut next_action = None;
        if !self.pbc_digit_buffer.is_empty() {
            let val = self.pbc_digit_buffer.iter().fold(0usize, |acc, &d| acc * 10 + d as usize);
            self.pbc_digit_buffer.clear();
            self.pbc_digit_timestamp = None;
            if let Some(ref mut pbc) = self.pbc {
                next_action = pbc.select_number(val)?;
            }
        } else if let Some(ref mut pbc) = self.pbc {
            next_action = pbc.press_default();
        }

        if let Some(action) = next_action {
            return self.execute_pbc_action(action);
        }
        Ok(())
    }

    /// Checks if the numeric input buffer has timed out (e.g. 2.0 seconds without Enter).
    /// If timed out, automatically confirms the buffered digits.
    pub fn check_pbc_digit_timeout(&mut self) -> Result<bool, String> {
        if self.pbc_digit_buffer.is_empty() {
            self.pbc_digit_timestamp = None;
            return Ok(false);
        }

        if let Some(ts) = self.pbc_digit_timestamp {
            if ts.elapsed() >= std::time::Duration::from_millis(2000) {
                if self.active_mode == ActiveDiscMode::Vcd20Classic {
                    self.handle_pbc_enter()?;
                } else if self.active_mode == ActiveDiscMode::Vcd10Linear {
                    self.handle_linear_enter()?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Handles a numeric digit input in VCD 1.0 linear video mode.
    pub fn handle_linear_digit(&mut self, digit: u8) -> Result<(), String> {
        let max = self.tracks.len();
        if max == 0 {
            return Ok(());
        }
        if max <= 9 {
            let sel_num = digit as usize;
            if sel_num >= 1 && sel_num <= max {
                let _ = self.play_track(sel_num - 1);
            }
        } else {
            self.pbc_digit_buffer.push(digit);
            self.pbc_digit_timestamp = Some(Instant::now());
            let val = self.pbc_digit_buffer.iter().fold(0usize, |acc, &d| acc * 10 + d as usize);
            if val * 10 > max || self.pbc_digit_buffer.len() >= 2 || (val >= 1 && val > max / 10 && val <= max) {
                self.pbc_digit_buffer.clear();
                self.pbc_digit_timestamp = None;
                if val >= 1 && val <= max {
                    let _ = self.play_track(val - 1);
                }
            }
        }
        Ok(())
    }

    /// Handles Enter / confirm input in VCD 1.0 linear video mode.
    pub fn handle_linear_enter(&mut self) -> Result<(), String> {
        if !self.pbc_digit_buffer.is_empty() {
            let val = self.pbc_digit_buffer.iter().fold(0usize, |acc, &d| acc * 10 + d as usize);
            self.pbc_digit_buffer.clear();
            self.pbc_digit_timestamp = None;
            let max = self.tracks.len();
            if val >= 1 && val <= max {
                let _ = self.play_track(val - 1);
            }
        }
        Ok(())
    }

    /// Handles return / back / Escape input in VCD 2.0 PBC mode.
    pub fn handle_pbc_return(&mut self) -> Result<(), String> {
        self.pbc_digit_buffer.clear();
        self.pbc_digit_timestamp = None;
        let action = self.pbc.as_mut().and_then(|pbc| pbc.press_return());
        if let Some(act) = action {
            return self.execute_pbc_action(act);
        }
        Ok(())
    }

    /// Handles previous track / previous menu in VCD 2.0 PBC mode.
    pub fn handle_pbc_prev(&mut self) -> Result<(), String> {
        let action = self.pbc.as_mut().and_then(|pbc| pbc.press_prev());
        if let Some(act) = action {
            return self.execute_pbc_action(act);
        }
        Ok(())
    }

    /// Handles next track / next menu in VCD 2.0 PBC mode.
    pub fn handle_pbc_next(&mut self) -> Result<(), String> {
        let action = self.pbc.as_mut().and_then(|pbc| pbc.press_next());
        if let Some(act) = action {
            return self.execute_pbc_action(act);
        }
        Ok(())
    }

    /// Plays the track at the specified index in `self.tracks`.
    pub fn play_track(&mut self, track_idx: usize) -> Result<bool, KernelError> {
        if track_idx >= self.tracks.len() {
            return Ok(false);
        }
        let track = &self.tracks[track_idx];
        let fname = track.file_name.clone();
        self.current_track_index = Some(track_idx);
        self.current_page = None;
        self.current_page_name.clear();
        self.start_video(&fname, 0, 0, None)
    }

    /// Navigates to the previous track (or restarts current if > 3 seconds in).
    pub fn play_prev_track(&mut self) -> Result<bool, KernelError> {
        if self.tracks.is_empty() {
            return Ok(false);
        }
        let cur_idx = self.current_track_index.unwrap_or(0);
        if let Some(ref mut player) = self.active_video {
            if player.current_time() > 3.0 {
                player.seek(0.0);
                return Ok(true);
            }
        }
        let new_idx = if cur_idx > 0 {
            cur_idx - 1
        } else {
            match self.playback_mode {
                PlaybackMode::ListRepeat => self.tracks.len().saturating_sub(1),
                _ => 0,
            }
        };
        self.play_track(new_idx)
    }

    /// Navigates to the next track.
    pub fn play_next_track(&mut self, allow_wrap: bool) -> Result<bool, KernelError> {
        if self.tracks.is_empty() {
            return Ok(false);
        }
        let cur_idx = self.current_track_index.unwrap_or(0);
        if cur_idx + 1 < self.tracks.len() {
            self.play_track(cur_idx + 1)
        } else if allow_wrap || self.playback_mode == PlaybackMode::ListRepeat {
            self.play_track(0)
        } else {
            let _ = self.stop_video_and_exit();
            Ok(false)
        }
    }

    pub fn update_video(&mut self) -> bool {
        if let Some(ref mut player) = self.active_video {
            let updated = player.update();
            if player.state == VideoPlayState::Ended {
                if self.active_mode == ActiveDiscMode::Vcd20Classic && self.pbc.is_some() {
                    let _ = self.stop_video_and_exit();
                    if let Some(ref mut pbc) = self.pbc {
                        if let Some(action) = pbc.on_item_finished() {
                            let _ = self.execute_pbc_action(action);
                        }
                    }
                } else if self.active_mode != ActiveDiscMode::Vcd30Interactive {
                    match self.playback_mode {
                        PlaybackMode::SingleRepeat => {
                            if let Some(idx) = self.current_track_index {
                                let _ = self.play_track(idx);
                            }
                        }
                        PlaybackMode::ListRepeat => {
                            let _ = self.play_next_track(true);
                        }
                        PlaybackMode::Sequential => {
                            let _ = self.play_next_track(false);
                        }
                    }
                } else {
                    let _ = self.stop_video_and_exit();
                }
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
                karaoke_playlist: &mut self.karaoke_playlist,
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
            VmState::Running
                | VmState::WaitingForDelay { .. }
                | VmState::WaitingForKeyWithTimeout { .. }
        )
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<&MapArea> {
        // If an overlay document is active, prioritize its hotspots
        if let Some(overlay) = &self.overlay_doc {
            let overlay_hotspots = overlay.get_all_hotspots();
            for area in &overlay_hotspots {
                if !area.is_point_hotspot() {
                    let (min_x, min_y, max_x, max_y) = area.raw_bounds();
                    if x >= min_x && x <= max_x && y >= min_y && y <= max_y {
                        return Some(area);
                    }
                }
            }
            for area in &overlay_hotspots {
                if !area.is_point_hotspot() {
                    let (min_x, min_y, max_x, max_y) = area.raw_bounds();
                    let height = max_y - min_y;
                    let width = max_x - min_x;
                    if height <= 8 && width > 8 {
                        if x >= min_x - 2 && x <= max_x + 2 && y >= min_y - 8 && y <= max_y + 3 {
                            return Some(area);
                        }
                    }
                }
            }
        }

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

        // Phase 1.5: Natural tolerance for thin underline hotspots (e.g. fill-in-the-blank lines where height <= 8).
        // Authoring in EX*.CHM places 4-5px tall lines strictly on the underline ___ bar (y=80..85),
        // while the user naturally clicks in the blank space or on the question text (y=72..88).
        for area in &all_hotspots {
            if !area.is_point_hotspot() {
                let (min_x, min_y, max_x, max_y) = area.raw_bounds();
                let height = max_y - min_y;
                let width = max_x - min_x;
                if height <= 8 && width > 8 {
                    if x >= min_x - 2 && x <= max_x + 2 && y >= min_y - 8 && y <= max_y + 3 {
                        return Some(area);
                    }
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
        // Execute any micro-scripts attached to this hotspot
        for op in &area.micro_scripts {
            self.execute_micro_script(op);
        }

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
            if area.is_overlay {
                self.load_overlay_page(target)?;
            } else {
                self.load_page(target, true)?;
            }
            return Ok(true);
        }

        if target.to_uppercase().ends_with(".WAV") {
            if let Some(wav_path) = self.find_file(target) {
                // When activating a WAV hotspot (e.g. instrument audio demo),
                // stop background narration/BGM so the user can clearly hear the sound sample.
                self.audio.stop_bgm();
                self.audio.play_sound_file(wav_path);
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn inject_remote_key(&mut self, key_code: i32) -> VmState {
        if self.active_mode == ActiveDiscMode::Vcd20Classic && self.pbc.is_some() {
            match key_code {
                0..=9 => {
                    let _ = self.handle_pbc_digit(key_code as u8);
                }
                31 => {
                    let _ = self.handle_pbc_enter();
                }
                32 => {
                    let _ = self.handle_pbc_return();
                }
                34 | 36 => {
                    let _ = self.handle_pbc_prev();
                }
                35 | 37 => {
                    let _ = self.handle_pbc_next();
                }
                _ => {}
            }
            return self.vm.state.clone();
        }

        if self.active_mode == ActiveDiscMode::Vcd10Linear {
            match key_code {
                0..=9 => {
                    let _ = self.handle_linear_digit(key_code as u8);
                }
                31 => {
                    let _ = self.handle_linear_enter();
                }
                32 => {
                    let _ = self.stop_video_and_exit();
                }
                34 | 36 => {
                    let _ = self.play_prev_track();
                }
                35 | 37 => {
                    let _ = self.play_next_track(true);
                }
                _ => {}
            }
            return self.vm.state.clone();
        }

        if self.is_video_active() {
            if key_code == 32 {
                let _ = self.stop_video_and_exit();
            }
            return self.vm.state.clone();
        }

        if matches!(
            self.vm.state,
            VmState::WaitingForKey { .. } | VmState::WaitingForKeyWithTimeout { .. }
        ) {
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
                    karaoke_playlist: &mut self.karaoke_playlist,
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
                karaoke_playlist: &mut self.karaoke_playlist,
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
        if self.active_mode != ActiveDiscMode::Vcd30Interactive {
            return Ok(false);
        }
        if self.overlay_doc.is_some() {
            self.dismiss_overlay();
            return Ok(true);
        }
        if let Some(prev) = self.history_stack.pop() {
            self.forward_stack.push(self.current_page_name.clone());
            self.load_page(&prev, false)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn go_forward(&mut self) -> Result<bool, KernelError> {
        if self.active_mode != ActiveDiscMode::Vcd30Interactive {
            return Ok(false);
        }
        if let Some(next) = self.forward_stack.pop() {
            self.history_stack.push(self.current_page_name.clone());
            self.load_page(&next, false)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn go_home(&mut self) -> Result<(), KernelError> {
        if self.active_mode != ActiveDiscMode::Vcd30Interactive {
            return Ok(());
        }
        let home_target = if self.find_file("HOME.CHM").is_some() {
            "HOME.CHM"
        } else {
            "HOMEPAGE.CHM"
        };
        self.load_page(home_target, true)
    }

    pub fn go_back_or_home(&mut self) -> Result<bool, KernelError> {
        if self.active_mode != ActiveDiscMode::Vcd30Interactive {
            return Ok(false);
        }
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
