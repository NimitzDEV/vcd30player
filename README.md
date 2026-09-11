# vcd30player

[![CI](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/NimitzDEV/vcd30player?color=blue&label=release)](https://github.com/NimitzDEV/vcd30player/releases/latest)
[![Rust](https://img.shields.io/badge/Language-Rust_2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#开源协议)

基于 Rust 开发的跨平台 VCD 3.0 与 VCD 2.0 交互式光盘播放器，无需配置旧版 Windows 或虚拟机，即可在现代 Windows、macOS 与 Linux 系统上直接播放。

[English Documentation](README_EN.md)

---

## 什么是 VCD 3.0？

VCD 3.0 是 20 世纪 90 年代末在传统 VCD 视频播放基础上扩展出的多媒体格式，增加了基于页面的交互能力。

与普通的线性视频 VCD 不同，VCD 3.0 光盘通常包含：

* 编译后的交互页面文件（`.CHM`），包含页面排版与可点击热区
* 8 位调色板索引图像（`.YBM`）
* 页面流程控制脚本与跳转逻辑
* MPEG-1 格式音视频（`.DAT`）
* 背景音乐与按键音效（`.WAV`）
* 卡拉 OK 点歌及其他互动逻辑

原版播放软件依赖旧版 Windows 专有运行时与组件，在现代操作系统上无法直接运行。vcd30player 通过解析光盘底层文件格式并重写运行时，在现代系统上直接还原其交互播放能力。

---

## 功能特性

### VCD 3.0 交互支持

* 光盘结构与启动识别：自动识别 VCD 3.0 目录规范，解析 `AUTORUN.CLS` 与 `ENREACH.CLS` 启动配置，确定光盘首页与开场视频配置。
* 复合交互页面：解析 `<COMPHTML>` / `.CHM` 二进制页面文件，支持页面图层排版、多边形与矩形热区交互、页面跳转表以及覆盖图层（Overlay）合成。
* 调色板图像渲染：解码 `<YUVBMP>` / `.YBM` 8 位索引图像为 RGBA 纹理，支持色度抽样与 Alpha 通道透明度合成。
* VCDSCRIPT 脚本引擎：内置协程虚拟机，支持变量运算、条件分支（`IF...THEN...ELSE`）、子程序调用（`GOSUB`）、伪随机数（`CALL RAND`）、按键等待（`CALL IRKEY`）与动画延时。
* 卡拉 OK 与多媒体调度：支持 MPEG-1 视频（`.DAT`）、循环背景音乐（`BGSOUND`）、按键音效（`.WAV`）以及卡拉 OK 播放列表操作指令。

### VCD 2.0 播放控制 (PBC)

* 白皮书 PBC 状态机：解析 `LOT.VCD`（位置查找表）与 `PSD.VCD`（播放序列描述表），支持 `PlayList`、`SelectionList` 与 `EndList` 交互流程。
* 菜单渲染：支持 `/SEGMENT/ITEMxxxx.DAT` 静态高分辨率菜单帧解码，以及 MPEG-1 动态交互式视频菜单回放。
* 遥控器与键盘输入：支持多位曲目编号输入、2 秒超时自动确认、回车确认、物理音轨寻道与专属 PBC 返回按键。
* 播放模式识别：自动区分 VCD 2.0 PBC 交互光盘与 VCD 1.0 纯线性流媒体光盘，并支持模式切换。
* 入口点索引：解析 `ENTRIES.VCD` 入口点列表，时间码按 `MM:SS.FF` 格式展示。

### 音画同步与跨平台架构

* 音视频分离与同步：支持 CD-XA 扇区解复用与基于 PTS 时间戳的音视频同步，画面保持 4:3 显示比例。
* 原生跨平台：基于 Rust 编写，无外部老旧运行时依赖，直接运行于 64 位 Windows、macOS 与 Linux。

---

## 下载预编译版本

Releases 页面提供免安装的发布包：

[前往 GitHub Releases 下载最新版本](https://github.com/NimitzDEV/vcd30player/releases)

* Windows: 下载 `vcd30player-v*-windows-x64.zip`，解压后运行 `vcd30_player.exe`。
* Linux: 下载 `vcd30player-v*-linux-x64.tar.gz`，解压后运行可执行文件。

---

## 本地开发与构建

### 环境要求

* Rust 1.80+（2024 Edition）
* C 编译器：
  * Windows: MSVC
  * Linux / macOS: GCC 或 Clang

### 编译构建

```bash
git clone --recurse-submodules https://github.com/NimitzDEV/vcd30player.git
cd vcd30player

cargo build --release
```

如果克隆时未拉取子模块，可执行：

```bash
git submodule update --init --recursive
```

### 运行

启动图形界面：

```bash
cargo run --release
```

直接指定光盘挂载盘符或提取目录：

```bash
cargo run --release -- "D:\"
```

---

## 测试

运行内置测试套件（使用脱敏测试样本，无需物理光驱）：

```bash
cargo test
```

针对本地光盘或光盘镜像目录运行测试：

Windows (PowerShell)：

```powershell
$env:VCD_TEST_DISC = "D:\"
cargo test
```

Linux / macOS：

```bash
VCD_TEST_DISC="/media/cdrom" cargo test
```

---

## 格式架构与设计

项目起源于一张原始播放程序已无法在现代系统运行的 VCD 3.0 光盘。播放器直接解析底层数据格式并执行交互语义，不依赖旧版操作系统环境。

数据流与处理流程：

```text
VCD 3.0 光盘
    │
    ├── CHM / COMPHTML (页面布局与热区)
    ├── YBM / YUVBMP   (图像与调色板)
    ├── CLS / Java 字节码 (启动配置)
    ├── VCD 脚本        (交互与逻辑)
    └── MPEG / CD-XA   (视频与音频)
            │
            ▼
     现代 VCD 3.0 运行时引擎
```

---

## 项目结构

```text
vcd30player/
├── .github/
│   ├── ISSUE_TEMPLATE/    # Issue 反馈模板
│   ├── workflows/
│   │   ├── ci.yml         # 持续集成与测试工作流
│   │   └── release.yml    # 二进制发布打包工作流
│   └── PULL_REQUEST_TEMPLATE.md
├── src/
│   ├── assets/            # CHM / YBM / CLS 文件格式解析
│   ├── audio/             # 音频播放与混音
│   ├── core/              # 运行时状态机、脚本 AST 与虚拟机
│   ├── ui/                # egui 渲染界面与虚拟遥控器
│   └── video/             # CD-XA 解复用与 MPEG 视频播放
├── c_src/                 # C 桥接层（pl_mpeg 解码绑定）
├── vendor/                # Git Submodule（上游 phoboslab/pl_mpeg）
├── tests/                 # 单元测试、集成测试与测试样本
├── Cargo.toml
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md              # 中文说明文档
└── README_EN.md           # 英文说明文档
```

---

## 参与贡献

VCD 3.0 相关的公开技术资料较少。欢迎提供以下帮助：

* VCD 3.0 光盘测试与兼容性反馈
* 相关技术规范或文档资料
* 格式解析与测试样本
* 问题反馈与补丁提交

提交前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

---

## 免责声明

本项目不分发任何受版权保护的光盘媒体或商业内容。请在拥有合法授权的光盘或其备份副本上使用本播放器。

---

## 开源协议

本项目采用双重开源协议授权，可任选其一：

* [MIT License](LICENSE-MIT)
* [Apache License, Version 2.0](LICENSE-APACHE)
