# skillsLocalManager

![skillsLocalManager Logo](./image/logo.png)

`skillsLocalManager` 是一个面向本地 AI Agent / CLI 生态的桌面端技能管理器。它用统一界面扫描本机技能目录、查看不同 Agent 的安装状态，并支持安装、移除、覆盖同步、回滚、Registry 浏览和 Prompt 翻译，适合维护多套技能库的个人开发者与团队成员。

## 项目简介

这个项目基于 `Tauri 2 + React + TypeScript + Rust` 构建，目标是把分散在不同 Agent 配置目录中的 skills 管理动作收敛到一个跨平台桌面应用里，减少手工复制目录、比对版本和排查覆盖冲突的成本。

## 核心功能

- 自动识别本机 AI Agent、配置目录和技能目录
- 浏览本地 skills，并按 Agent 查看可用数量和安装状态
- 一键安装或移除某个 skill
- 在多个 Agent 之间覆盖同步同名 skill
- 记录覆盖历史，并支持按版本回滚
- 浏览和搜索 ClawHub Registry
- 提供 Claude Bootstrap 初始化入口，快速安装推荐 skills
- 对 skill Prompt 做中译支持，并在设置页查看翻译日志
- 打开本地技能目录和相关配置目录，便于联动排查

## 支持的平台与适用对象

- Windows x64
- macOS Apple Silicon

适用场景：

- 同时使用多个 AI Agent / CLI 工具，需要共享或对齐同一套 skills
- 本地 skills 经常调整，想要可视化查看差异与安装状态
- 需要更安全地做覆盖更新，并保留回滚能力

## 本地开发与部署

### 依赖要求

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) stable
- Windows 或 macOS 对应的 Tauri 构建环境

### 安装依赖

```bash
npm install
npm --prefix frontend install
```

### 开发启动

```bash
npm run dev
```

### 生产构建

```bash
npm run build
```

构建成功后，安装包产物位于：

```text
src-tauri/target/release/bundle/
```

如果只是使用应用，不需要自己构建，直接在 GitHub Release 页面下载对应平台安装包即可。

## 下载与安装

项目发布页：

```text
https://github.com/Eric-Ma-Q/skillsLocalManager/releases
```

当前提供的安装包类型：

- Windows：`.msi` 或 `-setup.exe`
- macOS Apple Silicon：`.dmg`

## 可选配置

### SiliconFlow 翻译

如果你希望在技能详情页中把英文 Prompt 翻译成中文，可以在应用内打开：

```text
Settings -> SiliconFlow Translator Settings
```

需要配置：

- `SiliconFlow API Key`
- `Model`，当前默认 `Qwen/Qwen3.5-4B`

固定接口地址：

```text
https://api.siliconflow.cn/v1
```

## 技术栈

- Tauri 2
- Rust
- React 18
- TypeScript
- Vite
- Tailwind CSS
