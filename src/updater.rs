//! Application self-update and version check module.
//!
//! Provides automated version checking against Cloudflare R2 / S3 storage,
//! bilingual changelog rendering, cancellable background downloads with progress tracking,
//! SHA-256 integrity verification, and in-place binary self-replacement.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// Default public URL for fetching version metadata from Cloudflare R2
pub const VERSION_JSON_URL: &str = "https://content.vcd30player.app.nimitz.io/version.json";

/// Version metadata structure matching the CI-generated version.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionInfo {
    pub version: String,
    pub tag: String,
    pub release_date: Option<String>,
    pub changelog: Changelog,
    pub platforms: HashMap<String, PlatformAsset>,
}

/// Bilingual changelog container
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Changelog {
    pub zh: Option<String>,
    pub en: Option<String>,
}

/// Asset metadata for each supported platform
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformAsset {
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// Tab selector for bilingual changelogs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangelogTab {
    Chinese,
    English,
}

/// Detects system language to default to the appropriate changelog tab
pub fn detect_default_tab() -> ChangelogTab {
    if let Some(locale) = sys_locale::get_locale() {
        if locale.to_lowercase().starts_with("zh") {
            return ChangelogTab::Chinese;
        }
    }
    ChangelogTab::English
}

/// Returns the key for current operating system platform
pub fn get_current_platform_key() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows-x64"
    }
    #[cfg(target_os = "linux")]
    {
        "linux-x64"
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "unknown"
    }
}

/// Compares two SemVer strings (e.g. "0.3.0" vs "0.2.0")
pub fn is_newer_version(remote: &str, current: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(|c: char| c == '.' || c == '-' || c == '+')
            .filter_map(|s| s.parse::<u64>().ok())
            .collect()
    };
    let r = parse_parts(remote);
    let c = parse_parts(current);
    let max_len = std::cmp::max(r.len(), c.len());
    for i in 0..max_len {
        let rv = r.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if rv > cv {
            return true;
        } else if rv < cv {
            return false;
        }
    }
    false
}

/// Status of update checking
#[derive(Debug, Clone)]
pub enum UpdateCheckStatus {
    /// Has not checked yet
    Idle,
    /// Currently querying remote server
    Checking,
    /// Already running the latest version
    UpToDate,
    /// New version is available
    UpdateAvailable(VersionInfo),
    /// Failed to check (network error, parse error, etc.)
    CheckFailed(String),
}

/// Progress / state of download
#[derive(Debug, Clone)]
pub enum DownloadState {
    NotStarted,
    Downloading {
        downloaded: u64,
        total: u64,
    },
    Downloaded {
        temp_file: PathBuf,
        sha256: String,
    },
    Failed(String),
    Installing,
    InstallFailed(String),
}

/// Internal progress messages sent from background worker threads
enum DownloadProgress {
    Progress { downloaded: u64, total: u64 },
    Done { temp_file: PathBuf, sha256: String },
    Failed(String),
    Cancelled,
}

/// Central manager for application updates
pub struct UpdateManager {
    pub check_status: UpdateCheckStatus,
    pub download_state: DownloadState,
    pub active_tab: ChangelogTab,
    pub show_details_window: bool,

    has_auto_checked: bool,
    cancel_signal: Arc<AtomicBool>,
    check_receiver: Option<Receiver<Result<VersionInfo, String>>>,
    download_receiver: Option<Receiver<DownloadProgress>>,
    install_receiver: Option<Receiver<Result<(), String>>>,
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateManager {
    pub fn new() -> Self {
        Self {
            check_status: UpdateCheckStatus::Idle,
            download_state: DownloadState::NotStarted,
            active_tab: detect_default_tab(),
            show_details_window: false,
            has_auto_checked: false,
            cancel_signal: Arc::new(AtomicBool::new(false)),
            check_receiver: None,
            download_receiver: None,
            install_receiver: None,
        }
    }

    /// Called when the user opens the About dialog.
    /// Triggers automatic update check on first open of the session.
    pub fn on_about_opened(&mut self) {
        if !self.has_auto_checked {
            self.has_auto_checked = true;
            self.check_for_updates();
        }
    }

    /// Initiates a background check for updates
    pub fn check_for_updates(&mut self) {
        self.check_status = UpdateCheckStatus::Checking;
        let (tx, rx) = channel();
        self.check_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = (|| -> Result<VersionInfo, String> {
                let resp = ureq::get(VERSION_JSON_URL)
                    .timeout(std::time::Duration::from_secs(10))
                    .call()
                    .map_err(|e| format!("网络请求失败: {}", e))?;
                let info: VersionInfo = resp
                    .into_json()
                    .map_err(|e| format!("解析版本信息失败: {}", e))?;
                Ok(info)
            })();
            let _ = tx.send(result);
        });
    }

    /// Finds the asset for the current operating system
    pub fn get_current_platform_asset(&self) -> Option<PlatformAsset> {
        let info = match &self.check_status {
            UpdateCheckStatus::UpdateAvailable(info) => info,
            _ => return None,
        };

        let plat_key = get_current_platform_key();
        if let Some(asset) = info.platforms.get(plat_key) {
            return Some(asset.clone());
        }

        // Fallback: search by substring in filename
        info.platforms
            .values()
            .find(|a| a.filename.contains(plat_key))
            .cloned()
    }

    /// Starts downloading the update package in background
    pub fn start_download(&mut self) {
        let Some(asset) = self.get_current_platform_asset() else {
            let plat_key = get_current_platform_key();
            self.download_state =
                DownloadState::Failed(format!("未找到适用于当前平台 ({}) 的更新包", plat_key));
            return;
        };

        self.cancel_signal.store(false, Ordering::SeqCst);
        let cancel = Arc::clone(&self.cancel_signal);
        let (tx, rx) = channel();
        self.download_receiver = Some(rx);
        self.download_state = DownloadState::Downloading {
            downloaded: 0,
            total: asset.size,
        };

        std::thread::spawn(move || {
            let worker_tx = tx.clone();
            if let Err(e) = Self::download_worker(&asset, cancel, worker_tx) {
                let _ = tx.send(DownloadProgress::Failed(e));
            }
        });
    }

    /// Cancels an in-progress download
    pub fn cancel_download(&mut self) {
        self.cancel_signal.store(true, Ordering::SeqCst);
        self.download_state = DownloadState::NotStarted;
        self.download_receiver = None;
    }

    /// Worker function running in background thread for stream downloading & hashing
    fn download_worker(
        asset: &PlatformAsset,
        cancel: Arc<AtomicBool>,
        tx: Sender<DownloadProgress>,
    ) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        use std::io::{Read, Write};

        let resp = ureq::get(&asset.url)
            .timeout(std::time::Duration::from_secs(30))
            .call()
            .map_err(|e| format!("连接更新服务器失败: {}", e))?;

        let total_size = resp
            .header("Content-Length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(asset.size);

        let mut reader = resp.into_reader();
        let temp_dir = std::env::temp_dir();
        let temp_filename = format!("vcd30_update_{}.pkg", std::process::id());
        let temp_path = temp_dir.join(temp_filename);

        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut downloaded: u64 = 0;
        let mut last_progress_time = std::time::Instant::now();

        loop {
            if cancel.load(Ordering::SeqCst) {
                drop(file);
                let _ = std::fs::remove_file(&temp_path);
                let _ = tx.send(DownloadProgress::Cancelled);
                return Ok(());
            }

            let bytes_read = reader
                .read(&mut buffer)
                .map_err(|e| format!("读取下载数据失败: {}", e))?;
            if bytes_read == 0 {
                break;
            }

            file.write_all(&buffer[..bytes_read])
                .map_err(|e| format!("保存下载数据失败: {}", e))?;
            hasher.update(&buffer[..bytes_read]);

            downloaded += bytes_read as u64;

            if last_progress_time.elapsed() >= std::time::Duration::from_millis(80)
                || downloaded == total_size
            {
                let _ = tx.send(DownloadProgress::Progress {
                    downloaded,
                    total: total_size,
                });
                last_progress_time = std::time::Instant::now();
            }
        }

        file.flush()
            .map_err(|e| format!("刷新临时文件失败: {}", e))?;
        drop(file);

        let computed_hash = format!("{:x}", hasher.finalize());
        if !computed_hash.eq_ignore_ascii_case(&asset.sha256) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(format!(
                "SHA-256 完整性校验失败！期望值: {}, 实际值: {}",
                asset.sha256, computed_hash
            ));
        }

        let _ = tx.send(DownloadProgress::Done {
            temp_file: temp_path,
            sha256: computed_hash,
        });

        Ok(())
    }

    /// Extracts executable and performs in-place binary replacement, then restarts
    pub fn apply_update(&mut self) {
        let (temp_file, _sha256) = match &self.download_state {
            DownloadState::Downloaded { temp_file, sha256 } => (temp_file.clone(), sha256.clone()),
            _ => return,
        };

        self.download_state = DownloadState::Installing;
        let (tx, rx) = channel();
        self.install_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let new_exe = extract_executable(&temp_file)?;
                self_replace::self_replace(&new_exe)
                    .map_err(|e| format!("就地替换程序失败: {}", e))?;

                // Clean up temporary downloaded archive
                let _ = std::fs::remove_file(&temp_file);

                // Spawn new process
                let current_exe = std::env::current_exe()
                    .map_err(|e| format!("获取程序路径失败: {}", e))?;

                std::process::Command::new(&current_exe)
                    .args(std::env::args().skip(1))
                    .spawn()
                    .map_err(|e| format!("启动新版本程序失败: {}", e))?;

                // Clean exit
                std::process::exit(0);
            })();

            let _ = tx.send(result);
        });
    }

    /// Polls background workers each frame (must be called in egui update loop)
    pub fn poll(&mut self) {
        // 1. Check update status receiver
        if let Some(rx) = &self.check_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(info) => {
                        let current_ver = env!("CARGO_PKG_VERSION");
                        if is_newer_version(&info.version, current_ver) {
                            self.check_status = UpdateCheckStatus::UpdateAvailable(info);
                        } else {
                            self.check_status = UpdateCheckStatus::UpToDate;
                        }
                    }
                    Err(err) => {
                        self.check_status = UpdateCheckStatus::CheckFailed(err);
                    }
                }
                self.check_receiver = None;
            }
        }

        // 2. Download status receiver
        if let Some(rx) = &self.download_receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    DownloadProgress::Progress { downloaded, total } => {
                        self.download_state = DownloadState::Downloading { downloaded, total };
                    }
                    DownloadProgress::Done { temp_file, sha256 } => {
                        self.download_state = DownloadState::Downloaded { temp_file, sha256 };
                    }
                    DownloadProgress::Failed(err) => {
                        self.download_state = DownloadState::Failed(err);
                    }
                    DownloadProgress::Cancelled => {
                        self.download_state = DownloadState::NotStarted;
                    }
                }
            }
        }

        // 3. Install status receiver
        if let Some(rx) = &self.install_receiver {
            if let Ok(result) = rx.try_recv() {
                if let Err(err) = result {
                    self.download_state = DownloadState::InstallFailed(err);
                }
                self.install_receiver = None;
            }
        }
    }
}

/// Helper to extract executable from zip or tar.gz archive
fn extract_executable(archive_path: &Path) -> Result<PathBuf, String> {
    let target_bin_name = if cfg!(target_os = "windows") {
        "vcd30_player.exe"
    } else {
        "vcd30_player"
    };

    let temp_dir = std::env::temp_dir().join(format!("vcd30_extract_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建解压目录失败: {}", e))?;
    let extracted_bin_path = temp_dir.join(target_bin_name);

    let filename = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if filename.ends_with(".zip") || cfg!(target_os = "windows") {
        let file = std::fs::File::open(archive_path)
            .map_err(|e| format!("打开更新压缩包失败: {}", e))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("解析 ZIP 压缩包失败: {}", e))?;
        let mut found = false;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("读取压缩包条目失败: {}", e))?;
            let name = entry.name().to_string();

            let is_match = name.ends_with(target_bin_name)
                || (cfg!(target_os = "windows") && name.to_lowercase().ends_with(".exe"));

            if is_match && !entry.is_dir() {
                let mut out_file = std::fs::File::create(&extracted_bin_path)
                    .map_err(|e| format!("创建解压目标文件失败: {}", e))?;
                std::io::copy(&mut entry, &mut out_file)
                    .map_err(|e| format!("解压可执行文件数据失败: {}", e))?;
                found = true;
                break;
            }
        }

        if !found {
            return Err(format!("压缩包中未找到可执行文件 {}", target_bin_name));
        }
    } else {
        // Linux tar.gz
        let file = std::fs::File::open(archive_path)
            .map_err(|e| format!("打开更新压缩包失败: {}", e))?;
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);
        let mut found = false;

        for entry in archive
            .entries()
            .map_err(|e| format!("读取 TAR 条目失败: {}", e))?
        {
            let mut entry = entry.map_err(|e| format!("解析 TAR 条目失败: {}", e))?;
            let path = entry
                .path()
                .map_err(|e| format!("读取条目路径失败: {}", e))?;
            if path.file_name().and_then(|s| s.to_str()) == Some(target_bin_name) {
                entry
                    .unpack(&extracted_bin_path)
                    .map_err(|e| format!("解压 TAR 文件失败: {}", e))?;
                found = true;
                break;
            }
        }

        if !found {
            return Err(format!("压缩包中未找到可执行文件 {}", target_bin_name));
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(
            &extracted_bin_path,
            std::fs::Permissions::from_mode(0o755),
        );
    }

    Ok(extracted_bin_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.3.0", "0.2.0"));
        assert!(is_newer_version("v0.3.0", "0.2.0"));
        assert!(is_newer_version("0.2.1", "0.2.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(is_newer_version("1.2.3", "1.2.2"));

        assert!(!is_newer_version("0.2.0", "0.2.0"));
        assert!(!is_newer_version("v0.2.0", "0.2.0"));
        assert!(!is_newer_version("0.1.9", "0.2.0"));
        assert!(!is_newer_version("0.2.0", "0.3.0"));
    }

    #[test]
    fn test_version_json_deserialize() {
        let sample_json = r#"{
            "version": "0.3.0",
            "tag": "v0.3.0",
            "release_date": "2026-09-12T03:00:00Z",
            "changelog": {
                "zh": "1. 修复音频播放\n2. 优化界面",
                "en": "1. Fix audio playback\n2. UI optimizations"
            },
            "platforms": {
                "windows-x64": {
                    "filename": "vcd30player-v0.3.0-windows-x64.zip",
                    "url": "https://content.vcd30player.app.nimitz.io/v0.3.0/vcd30player-v0.3.0-windows-x64.zip",
                    "sha256": "5f4dcc3b5aa765d61d8327deb882cf99",
                    "size": 18234567
                },
                "linux-x64": {
                    "filename": "vcd30player-v0.3.0-linux-x64.tar.gz",
                    "url": "https://content.vcd30player.app.nimitz.io/v0.3.0/vcd30player-v0.3.0-linux-x64.tar.gz",
                    "sha256": "2c624232cdd221771294dfbb310aca00",
                    "size": 15234567
                }
            }
        }"#;

        let info: VersionInfo = serde_json::from_str(sample_json).expect("deserialize failed");
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.tag, "v0.3.0");
        assert_eq!(
            info.changelog.zh.as_deref(),
            Some("1. 修复音频播放\n2. 优化界面")
        );
        assert_eq!(
            info.changelog.en.as_deref(),
            Some("1. Fix audio playback\n2. UI optimizations")
        );
        assert!(info.platforms.contains_key("windows-x64"));
        assert!(info.platforms.contains_key("linux-x64"));
    }

    #[test]
    fn test_platform_key() {
        let key = get_current_platform_key();
        #[cfg(target_os = "windows")]
        assert_eq!(key, "windows-x64");
        #[cfg(target_os = "linux")]
        assert_eq!(key, "linux-x64");
    }
}
