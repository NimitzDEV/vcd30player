//! VCD 3.0 Interactive Kernel and Page Navigation State Machine.

use crate::assets::chm::{CompHtmlDoc, MapArea};
use crate::assets::cls::AutoRunConfig;
use crate::assets::ybm::YbmImage;
use std::fmt;
use std::path::PathBuf;

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

pub struct VcdKernel {
    pub disc_root: PathBuf,
    pub current_page_name: String,
    pub current_page: Option<CompHtmlDoc>,
    pub current_bg_image: Option<YbmImage>,
    pub canvas: Vec<u8>,
    pub history_stack: Vec<String>,
    pub forward_stack: Vec<String>,
    pub autorun_config: Option<AutoRunConfig>,
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
        }
    }

    /// Resolves a filename case-insensitively within candidate directories on the disc.
    pub fn find_file(&self, filename: &str) -> Option<PathBuf> {
        let clean_name = filename.trim_start_matches('/').trim_start_matches('\\');

        // Direct check
        let direct = self.disc_root.join(clean_name);
        if direct.exists() {
            return Some(direct);
        }

        // Candidate search directories on VCD 3.0 disc
        let candidate_dirs = [
            self.disc_root.clone(),
            self.disc_root.join("DATA").join("VCD_DATA"),
            self.disc_root.join("PROGRAM").join("JAVA"),
            self.disc_root.join("MPEGAV"),
        ];

        for dir in &candidate_dirs {
            if !dir.exists() {
                continue;
            }

            // Case-insensitive directory scan
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

    /// Initializes disc from root path, reading AUTORUN.CLS or defaulting to HOMEPAGE.CHM.
    pub fn open_disc(&mut self, disc_root: PathBuf) -> Result<(), KernelError> {
        if !disc_root.exists() {
            return Err(KernelError::DiscNotFound(disc_root));
        }

        self.disc_root = disc_root;
        self.history_stack.clear();
        self.forward_stack.clear();

        // Check for AUTORUN.CLS
        let cls_path = self.find_file("AUTORUN.CLS");
        let initial_page = if let Some(p) = cls_path {
            if let Ok(bytes) = std::fs::read(&p) {
                if let Ok(cfg) = AutoRunConfig::parse(&bytes) {
                    let page = cfg.homepage_chm.clone();
                    self.autorun_config = Some(cfg);
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

        self.load_page(&initial_page, false)
    }

    /// Loads and renders a .CHM interactive page.
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

        // Clear canvas with background color or black
        self.canvas.fill(0);

        // Load background image if specified
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

        self.current_page = Some(doc);
        Ok(())
    }

    /// Tests if a canvas coordinate hits any interactive hotspot.
    pub fn hit_test(&self, x: i32, y: i32) -> Option<&MapArea> {
        let doc = self.current_page.as_ref()?;
        for area in doc.get_all_hotspots() {
            // Note: area coordinates (x1, y1) to (x2, y2)
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

    /// Activates a clicked hotspot area.
    pub fn activate_hotspot(&mut self, area: &MapArea) -> Result<bool, KernelError> {
        let target = area.target.trim();
        if target.to_uppercase().ends_with(".CHM") {
            self.load_page(target, true)?;
            Ok(true)
        } else {
            // Internal script or action ID
            Ok(false)
        }
    }

    /// Navigates back in history stack.
    pub fn go_back(&mut self) -> Result<bool, KernelError> {
        if let Some(prev) = self.history_stack.pop() {
            self.forward_stack.push(self.current_page_name.clone());
            self.load_page(&prev, false)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Navigates forward in forward stack.
    pub fn go_forward(&mut self) -> Result<bool, KernelError> {
        if let Some(next) = self.forward_stack.pop() {
            self.history_stack.push(self.current_page_name.clone());
            self.load_page(&next, false)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Navigates to main menu / home page.
    pub fn go_home(&mut self) -> Result<(), KernelError> {
        let home_target = if self.find_file("HOME.CHM").is_some() {
            "HOME.CHM"
        } else {
            "HOMEPAGE.CHM"
        };
        self.load_page(home_target, true)
    }
}
