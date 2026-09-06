# vcd30player

[![CI](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/NimitzDEV/vcd30player/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/NimitzDEV/vcd30player?color=blue&label=release)](https://github.com/NimitzDEV/vcd30player/releases/latest)
[![Rust](https://img.shields.io/badge/Language-Rust_2024_Edition-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](#开源协议)

**基于 Rust 编写的现代化跨平台 VCD 3.0 播放器。**

vcd30player 是 **VCD 3.0 交互式光盘环境** 的开源现代重实现，让诞生于 20 世纪 90 年代末的 VCD 3.0 互动多媒体光盘能够免除繁琐配置，直接在现代 Windows、macOS 与 Linux 系统上重获新生。

> **无需 Windows 98，无需虚拟机，亦无需安装原版老旧播放软件。**

[English Documentation](README_EN.md)

---

## 什么是 VCD 3.0？

VCD 3.0 是 20 世纪 90 年代末在传统 VCD 视频播放基础上发展出的一种扩展多媒体光盘格式，加入了基于页面的交互能力。

与普通的 VCD 光盘不同，一张 VCD 3.0 光盘通常包含：

* 交互式菜单与可点击热区
* 类似 HTML 编译后的页面文件（`.CHM`）
* 索引与调色板图像（`.YBM`）
* 内嵌的流程控制脚本与跳转逻辑
* MPEG-1 格式视频（`.DAT`）
* 背景音乐与按键音效（`.WAV`）
* 卡拉OK点歌及其他互动应用程序

原始软件依赖旧时代 Windows 的多媒体组件及专有运行时，在现代操作系统上早已无法运行。

**vcd30player 的目标是在不模拟整台旧电脑的前提下，完整保护并还原这一软件生态。**

---

## ✨ 功能特性

### 光盘与格式支持

* VCD 3.0 光盘 / 目录自动识别
* `<COMPHTML>` / `.CHM` 页面解析
* `.YBM` 调色板图像高效解码
* `AUTORUN.CLS` 启动配置分析
* 光盘资源加载与多层级页面路由

### 交互运行时

* VCD 3.0 页面排版与渲染
* 交互式按钮与多边形热区响应
* 内嵌脚本解释与虚拟机执行
* 键盘快捷键与虚拟遥控器操作
* 定时延时、流程跳转与变量状态管理
* 完整支持卡拉OK点歌指令集

### 音频与视频

* CD-XA 扇区音视频解复用
* MPEG-1 视频流畅播放
* 4:3 画面显示比例校正
* 背景音乐循环播放
* WAV 即时按钮音效
* 基于时间戳（PTS）的音画同步

### 现代技术栈

* 全程基于现代安全 Rust 语言与生态打造
* 直接原生运行于现代 64 位操作系统（Windows, macOS, Linux）
* 无需 Windows 9x 兼容环境
* 无需安装任何历史遗留播放程序
* 跨平台原生架构设计

---

## 💾 下载预编译版本 (Pre-built Binaries)

如果您只想直接播放光盘，无需安装 Rust 编译环境，可直接前往 Releases 页面下载为您操作系统构建的免安装绿色版本：

👉 **[前往 GitHub Releases 下载最新版本](https://github.com/NimitzDEV/vcd30player/releases)**

* **Windows**: 下载 `vcd30player-v*-windows-x64.zip`，解压后双击 `vcd30_player.exe` 即可直接运行。
* **Linux**: 下载 `vcd30player-v*-linux-x64.tar.gz` 解压运行。

---

## 🛠️ 本地开发与从源码构建

如果您希望参与项目开发、调试功能，或自行从源码编译，请参考以下指南：

### 开发环境要求

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

如果克隆时未添加子模块参数，可执行：

```bash
git submodule update --init --recursive
```

### 运行播放器

直接启动图形界面：

```bash
cargo run --release
```

或在启动时直接指定光盘挂载盘符 / 解压目录：

```bash
cargo run --release -- "D:\"
```

---

## 🧪 测试指南

项目内置了小巧、脱敏且完全自包含的测试样本，无需物理光驱即可运行全部单元测试与集成测试：

```bash
cargo test
```

如需针对本地真实的物理光盘运行扩展测试：

```powershell
$env:VCD_TEST_DISC = "D:\"
cargo test
```

Linux / macOS：

```bash
VCD_TEST_DISC="/media/cdrom" cargo test
```

---

## 🔬 格式分析与兼容设计

本项目的起点是针对一张原始播放程序已无法在现代系统运行的 VCD 3.0 真实光盘展开的分析。

项目通过现代、规范的实现，逐步替代原有的历史遗留运行时，重新还原底层数据格式与执行环境。

核心数据流与格式架构：

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

项目的长远目标并非重现旧版 Windows 系统的所有细节，而是**直接重新实现播放 VCD 3.0 光盘内容所需的完整交互语义**。

---

## 📁 项目结构

```text
vcd30player/
├── .github/
│   ├── ISSUE_TEMPLATE/    # Issue 反馈模板
│   ├── workflows/
│   │   ├── ci.yml         # 自动化持续集成与测试
│   │   └── release.yml    # 多平台二进制打包与发布流水线
│   └── PULL_REQUEST_TEMPLATE.md
├── src/
│   ├── assets/            # CHM / YBM / CLS 文件格式解析
│   ├── audio/             # 音频播放与音效混音
│   ├── core/              # 运行时状态机、脚本 AST 与虚拟机
│   ├── ui/                # egui 渲染界面与虚拟遥控器
│   └── video/             # CD-XA 解复用与 MPEG 视频播放
├── c_src/                 # C 桥接层（pl_mpeg 解码绑定）
├── vendor/                # Git Submodule（上游 phoboslab/pl_mpeg）
├── tests/                 # 单元测试、集成测试与脱敏测试样本
├── Cargo.toml
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md              # 中文说明文档
└── README_EN.md           # 英文说明文档
```

---

## 🤝 参与贡献

VCD 3.0 属于缺乏公开技术规范且较少被数字保存的多媒体格式。如果您拥有：

* VCD 3.0 实体光盘
* 原始光盘播放软件
* 相关的格式文档或技术资料
* 格式分析与测试发现
* 典型测试样本
* 兼容性测试反馈

这些对本项目都是极为宝贵的帮助。

欢迎提交 Issue、Pull Request、格式分析成果与兼容性报告。提交前请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

---

## ⚠️ 免责声明

本项目**不分发**任何商业 VCD 光盘内容、受版权保护的音视频媒体或原版商业光盘。

请在您拥有合法使用权的光盘或其合法备份副本上使用本播放器。

---

## 📜 开源协议

本项目采用双重开源协议授权，您可以任选其一：

* [MIT License](LICENSE-MIT)
* [Apache License, Version 2.0](LICENSE-APACHE)
