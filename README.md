# vcd30player

[![CI](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml/badge.svg)](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Language-Rust_2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#开源协议)
[![Tests](https://img.shields.io/badge/Tests-47%20Passed-brightgreen.svg)](#测试指南)

**vcd30player** 是一个使用现代 Rust 语言编写的 **VCD 3.0 格式光盘** 跨平台交互播放器。

[English Documentation](README_EN.md)

---

## 📖 项目介绍

本项目用于播放 VCD 3.0 格式光盘。

VCD 3.0 是在传统 VCD 视频播放的基础上，增加了基于页面的菜单与交互能力的一种光盘格式：
- **`<COMPHTML>` (.CHM)**：编译后的页面文件，包含界面布局、按钮热区及交互跳转逻辑；
- **`<YUVBMP>` (.YBM)**：基于 ITU-R BT.601 规范的 8-bit YUV 调色板图像文件；
- **VCDSCRIPT**：光盘内置的类 BASIC 脚本引擎，用于处理按键输入、延时控制、变量计算和流程跳转；
- **视频与音频**：配合光盘内的 MPEG-1 视频（`.DAT`）和背景音频（`.WAV`），实现菜单导航、卡拉OK点歌、答题互动等功能。

早期光盘自带的 16/32 位播放程序在现代 64 位操作系统（Windows 10/11、macOS、Linux）上已无法正常运行。本项目使用现代 Rust 语言实现了该格式的跨平台播放引擎，让光盘可以在现代电脑上免虚拟机直接运行播放。

---

## ✨ 核心特性

- **光盘文件与格式解析**
  - **CHM 页面解析**：支持 `<COMPHTML>` 二进制文件解析，读取页面元数据、调色板、图层块、全屏/多边形/矩形按钮热区以及内嵌脚本。
  - **YBM 图像解码**：支持 ITU-R BT.601 标准调色板转 RGBA 显示，支持精灵图（Sprite）局部绘制。
  - **CLS 启动配置**：读取 `PROGRAM/JAVA/AUTORUN.CLS`，自动识别封面图、开机片头视频与主页。
- **VCDSCRIPT 脚本引擎**
  - 完整的语法解析与抽象语法树（AST），支持基本算术运算、条件分支（`IF...THEN...ELSE`）与子程序跳转（`GOTO`, `GOSUB/RETURN`）。
  - 基于协程与状态机的执行机制，原生支持延时控制（`CALL TIME`，以 100ms 为单位）、遥控器按键等待与超时返回（`CALL IRKEY`）、随机数生成（`CALL RAND`）。
  - 完整实现卡拉OK指令集（`KARAOKE SET/GET/DEL/INS/PLAY`），支持 19 槽位歌曲列表管理、顺序点播与切歌。
- **音视频解码与同步**
  - **CD-XA 解复用**：支持光盘 RIFF CD-XA 格式音视频数据分离。
  - **MPEG-1 视频播放**：集成轻量级视频解码库（`pl_mpeg`），支持播放、暂停、进度条拖拽、画面比例自适应（4:3 保持）。
  - **音频播放与音效混音**：支持页面背景音频（BGSOUND）循环播放，以及按钮点击触发的即时短音效（WAV）。
- **界面与交互操作 (`egui`)**
  - 虚拟遥控器面板：提供数字按键、方向键、确认/返回键与音量调节。
  - 底部操作栏：支持直接加载光盘目录、加载单个 CHM 页面、弹出光盘及一键重置（Reset）。
  - 调试辅助：支持热区高亮显示开关、遥控器光标显示、未识别指令弹窗容错。
- **自动化测试支持**
  - 自带约 300KB 的无版权测试样本（`tests/fixtures/mock_disc`），不需要挂载任何物理光盘即可在本地和 CI 环境中一键跑通全部 47 项测试。

---

## 🛠️ 编译与运行

### 环境要求

- **Rust 工具链**：Rust 1.80+ (2024 Edition)
- **C 编译器**：MSVC (Windows) 或 GCC / Clang (Linux / macOS)

### 编译构建

```bash
# 克隆仓库（包含 submodule）
git clone --recurse-submodules https://github.com/NimitzDEV/vcd30player.git
cd vcd30player

# 若克隆时未添加 --recurse-submodules，可执行以下命令拉取子模块：
git submodule update --init --recursive

# 编译 Debug 版本
cargo build

# 编译 Release 高性能版本
cargo build --release
```

### 运行播放器

```bash
# 直接启动图形界面
cargo run --release

# 或在命令行直接指定已挂载的光盘根目录或解压目录
cargo run --release -- "D:\"
```

---

## 🧪 测试指南

项目自带完整的单元测试与集成测试（共 47 项测试）：

```bash
# 运行全部测试（使用内置的测试样本，无需真实光盘）
cargo test
```

### 使用真实光盘运行扩展测试

如需对本地挂载的真实光盘进行全量测试，可以通过环境变量指定盘符或挂载路径：

```powershell
# Windows (PowerShell)
$env:VCD_TEST_DISC = "D:\"; cargo test
```

```bash
# Linux / macOS (Bash)
VCD_TEST_DISC="/media/cdrom" cargo test
```

---

## 📂 项目结构

```text
vcd30player/
├── .github/
│   └── workflows/ci.yml   # GitHub Actions 自动化持续集成
├── c_src/                 # C 桥接代码（pl_mpeg_impl.c）
├── vendor/
│   └── pl_mpeg/           # Git Submodule（官方 phoboslab/pl_mpeg）
├── src/
│   ├── assets/            # CHM、YBM、CLS 格式解析器
│   ├── audio/             # 音频管理器与混音播放
│   ├── core/              # 核心状态机、VCDSCRIPT AST 与虚拟机
│   ├── ui/                # egui 渲染界面与虚拟遥控器
│   ├── video/             # CD-XA 解复用器与 MPEG 视频播放器
│   ├── lib.rs             # 库入口导出
│   └── main.rs            # 应用程序主入口
├── tests/
│   ├── common/            # 测试光盘路径解析辅助模块
│   ├── fixtures/          # 300KB 测试专用虚拟光盘样本
│   └── *.rs               # 完整测试套件
├── Cargo.toml
├── CHANGELOG.md           # 版本更新日志
├── CONTRIBUTING.md        # 贡献指南
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md              # 中文说明
└── README_EN.md           # English Documentation
```

---

## ⚠️ 免责声明

本项目仅作为 VCD 3.0 格式光盘的播放与学习研究工具：
- 本项目代码仓库不包含、亦不分发任何受版权保护的商业影视、音乐或游戏等光盘原盘媒体内容。
- 用户使用本播放器时，请确保使用的是您合法拥有的光盘介质或合法备份副本。

---

## 📜 开源协议

本项目采用双重开源协议授权：
- [MIT License](LICENSE-MIT) 或
- [Apache License, Version 2.0](LICENSE-APACHE)
