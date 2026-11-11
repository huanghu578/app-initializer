//! # App Initializer
//!
//! 专为桌面应用（尤其是 Windows MSIX / Store 应用）设计的初始化与资源部署库。
//!
//! ## 核心机制
//! 采用“影子目录 (Staging) + 原子重命名 (Atomic Rename)”机制。
//! 保证无论程序在何时崩溃或断电，目标目录（AppData）绝对不会出现半截文件。
//!
//! ## 严格校验
//! 在初始化开始前，会严格校验 `assets` 和 `configs` 中指定的文件是否存在于安装目录。
//! 如果文件缺失，会抛出 `ResourceNotFound` 错误并阻断初始化，防止生成残缺的运行环境。

use directories::ProjectDirs;
use semver::Version;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

/// 初始化过程中可能遇到的各种错误
#[derive(Error, Debug)]
pub enum InitError {
    #[error("无法确定系统数据目录，可能是当前操作系统不支持或环境变量缺失")]
    DirNotFound,

    #[error("版本号解析错误: {0}")]
    VersionParseError(#[from] semver::Error),

    #[error("文件或目录操作失败: {0}")]
    IoError(#[from] io::Error),

    #[error("未找到当前可执行文件所在的目录")]
    ExeDirNotFound,

    #[error("提交流程失败，已自动回滚至旧版本")]
    CommitFailed(#[source] io::Error),

    #[error("初始化资源缺失: 找不到文件或目录 '{0}'。请检查打包配置！")]
    ResourceNotFound(String),
}

/// 应用初始化配置
pub struct AppInitConfig<'a> {
    /// 应用名称 (用于生成 AppData 文件夹名称，如 `%APPDATA%/Org/AppName`)
    pub app_name: &'a str,

    /// 组织/作者名称 (用于区分同名应用)
    pub org_name: &'a str,

    /// 当前的软件版本号 (例如 "1.0.0")，用于对比是否需要增量更新
    pub current_version: &'a str,

    /// **程序资源列表** (如 `["index.html", "assets/"]`)。
    /// 每次版本升级都会用安装目录中的最新文件覆盖 AppData 中的旧文件。
    pub assets: &'a [&'a str],

    /// **用户配置列表** (如 `["config.toml"]`)。
    /// 仅在**首次运行**时复制，后续升级绝不覆盖，保护用户数据。
    pub configs: &'a [&'a str],
}

/// 进度报告结构体
#[derive(Debug, Clone)]
pub struct Progress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
}

impl Progress {
    /// 获取当前进度的百分比 (0.0 - 100.0)
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        }
    }
}

/// 应用初始化器实例
#[derive(Debug)]
pub struct AppInitializer {
    target_dir: PathBuf,
}

impl AppInitializer {
    /// 创建实例并确定跨平台的 AppData 路径
    pub fn new(app_name: &str, org_name: &str) -> Result<Self, InitError> {
        let proj_dirs =
            ProjectDirs::from("com", org_name, app_name).ok_or(InitError::DirNotFound)?;
        Ok(Self {
            target_dir: proj_dirs.data_dir().to_path_buf(),
        })
    }

    /// 获取目标目录（AppData 下的路径），供主程序加载文件使用
    pub fn data_dir(&self) -> &Path {
        &self.target_dir
    }

    /// 快速检查是否需要初始化或更新（不执行任何文件复制）
    pub fn is_init_needed(&self, config: &AppInitConfig) -> bool {
        if !self.target_dir.exists() {
            return true;
        }

        let target_version_file = self.target_dir.join(".version");
        if let Ok(stored_ver_str) = fs::read_to_string(&target_version_file) {
            if let Ok(stored_ver) = Version::parse(stored_ver_str.trim()) {
                if let Ok(current_ver) = Version::parse(config.current_version) {
                    return current_ver > stored_ver;
                }
            }
        }
        true
    }

    /// 执行初始化或增量更新（核心方法）
    pub fn initialize<F>(&self, config: &AppInitConfig, mut progress_cb: F) -> Result<(), InitError>
    where
        F: FnMut(Progress),
    {
        if !self.is_init_needed(config) {
            return Ok(());
        }

        let is_first_run = !self.target_dir.exists();
        let tmp_dir = self.target_dir.with_extension("tmp");
        let bak_dir = self.target_dir.with_extension("bak");

        let _ = fs::remove_dir_all(&tmp_dir);
        let _ = fs::remove_dir_all(&bak_dir);

        fs::create_dir_all(&tmp_dir)?;

        let exe_path = std::env::current_exe()?;
        let mut source_dir = exe_path
            .parent()
            .ok_or(InitError::ExeDirNotFound)?
            .to_path_buf();

        // 兼容 cargo run 开发环境
        if !config.assets.is_empty() && !source_dir.join(config.assets[0]).exists() {
            if let Ok(cwd) = std::env::current_dir() {
                if cwd.join(config.assets[0]).exists() {
                    source_dir = cwd;
                }
            }
        }

        if !is_first_run {
            copy_dir_all(&self.target_dir, &tmp_dir)?;
        }

        // === 收集任务队列（包含严格校验）===
        let mut tasks = Vec::new();
        if is_first_run || !config.assets.is_empty() {
            self.collect_tasks_strict(&source_dir, config.assets, &tmp_dir, &mut tasks)?;
        }
        if is_first_run {
            self.collect_tasks_strict(&source_dir, config.configs, &tmp_dir, &mut tasks)?;
        }

        let total = tasks.len();
        let mut current = 0;
        let mut last_report_percent = 0;

        for (src, dest) in tasks {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dest)?;
            current += 1;

            let current_percent = if total == 0 {
                100
            } else {
                (current * 100) / total
            };
            if current_percent > last_report_percent + 4 || current == total || total <= 20 {
                last_report_percent = current_percent;
                let file_name = src
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                progress_cb(Progress {
                    current,
                    total,
                    current_file: file_name,
                });
            }
        }

        fs::write(tmp_dir.join(".version"), config.current_version)?;

        // 原子提交
        if !is_first_run {
            if let Err(e) = fs::rename(&self.target_dir, &bak_dir) {
                let _ = fs::remove_dir_all(&tmp_dir);
                return Err(InitError::CommitFailed(e));
            }
        }

        if let Err(e) = fs::rename(&tmp_dir, &self.target_dir) {
            if !is_first_run {
                let _ = fs::rename(&bak_dir, &self.target_dir);
            }
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(InitError::CommitFailed(e));
        }

        if !is_first_run {
            let _ = fs::remove_dir_all(&bak_dir);
        }

        Ok(())
    }

    /// 带严格校验的任务收集器
    /// 如果配置项在源目录中不存在，直接返回 ResourceNotFound 错误
    fn collect_tasks_strict(
        &self,
        source_dir: &Path,
        items: &[&str],
        tmp_dir: &Path,
        tasks: &mut Vec<(PathBuf, PathBuf)>,
    ) -> Result<(), InitError> {
        for item in items {
            let src_base = source_dir.join(item);
            let dest_base = tmp_dir.join(item);

            if !src_base.exists() {
                return Err(InitError::ResourceNotFound(item.to_string()));
            }

            if src_base.is_file() {
                tasks.push((src_base, dest_base));
            } else if src_base.is_dir() {
                for entry in WalkDir::new(&src_base).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        let relative = entry.path().strip_prefix(&src_base).unwrap();
                        tasks.push((entry.path().to_path_buf(), dest_base.join(relative)));
                    }
                }
            }
        }
        Ok(())
    }
}

/// 辅助函数：递归复制整个目录
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.as_ref().join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(entry.path(), dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
