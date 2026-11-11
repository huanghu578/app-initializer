# App Initializer

一个专为桌面应用（尤其是 **Windows MSIX / Windows Store** 应用）设计的初始化与资源部署库。

## 💡 为什么需要这个库？

在 Windows MSIX 生态中，应用的安装目录（`C:\Program Files\WindowsApps`）是**严格只读的**，且受到 WebView 等组件的沙盒限制。直接在安装目录中读写配置会导致权限错误或白屏。

本库遵循微软规范，自动将安装包中的只读资源（如 `index.html`、前端 `dist` 目录）和默认配置文件（如 `config.toml`）**安全地复制到用户的 AppData 目录**。

## ✨ 核心特性

- **原子提交机制 (Atomic Commits)**：采用“影子目录”复制，完美成功后才瞬间替换目标目录。**哪怕复制到 99% 断电，目标目录也不会出现半截文件。**
- **纯逻辑，零 UI 依赖**：只接收进度回调闭包，命令行、FLTK、Tauri 等框架均可轻松接入。
- **自动增量更新**：对比语义化版本号，升级时只覆盖 `assets`（静态资源），绝不覆盖 `configs`（用户配置）。
- **安全回滚**：升级覆盖时，若旧文件被系统占用，会自动放弃替换并恢复原状，保护用户数据。
- **跨平台**：自动适配 Windows（`%APPDATA%`）、macOS 和 Linux。

## 📦 安装

```toml
[dependencies]
app-initializer = "0.1.0"
```
🚀 快速开始
```rust
use app_initializer::{AppInitializer, AppInitConfig};

let config = AppInitConfig {
    app_name: "MyApp",
    org_name: "MyCompany",
    current_version: env!("CARGO_PKG_VERSION"), // 自动读取 Cargo.toml 版本号
    assets: &["index.html", "assets/"],
    configs: &["config.toml"],
};

let initializer = AppInitializer::new(config.app_name, config.org_name).unwrap();

if initializer.is_init_needed(&config) {
    // 传入一个闭包接收进度数据，自己决定如何绘制 UI
    initializer.initialize(&config, |progress| {
        println!("进度: {:.2}% - {}", progress.percentage(), progress.current_file);
    }).unwrap();
}

// 获取目标路径用于加载
let app_data_path = initializer.data_dir();
println!("目标路径: {:?}", app_data_path);
```
🖥 框架接入示例
命令行 (CLI)：使用 print!("\r...") 覆盖打印，见 examples/cli_demo.rs。

FLTK / Webview：通过 app::channel 或 app::awake_callback 将进度安全地更新到主线程 UI 控件。

Tauri / Egui：通过 std::sync::mpsc 转发进度给前端/状态管理器。

📄 License
MIT