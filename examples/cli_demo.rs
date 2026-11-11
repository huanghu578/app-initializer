use app_initializer::{AppInitConfig, AppInitializer};
use std::fs::File;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== 启动程序 ===");

    let config = AppInitConfig {
        app_name: "MyDemoApp",
        org_name: "DemoCorp",
        current_version: "1.0.0", // 修改这里可以测试升级逻辑
        assets: &["test_assets", "bigfile.zip"],
        configs: &["test_config.toml"],
    };

    let initializer = AppInitializer::new(config.app_name, config.org_name).expect("无法解析目录");

    if initializer.is_init_needed(&config) {
        println!("检测到首次运行或版本升级，准备初始化...");

        // 生成模拟文件（如果已存在则跳过）
        //create_dummy_files();

        // 显示进度的终端动画
        match initializer.initialize(&config, |progress| {
            let bar_width = 30;
            let filled = (progress.percentage() / 100.0 * bar_width as f64) as usize;
            let bar = format!(
                "[{}{}] {:.0}%",
                "=".repeat(filled),
                " ".repeat(bar_width - filled),
                progress.percentage()
            );
            print!("\r{} 正在复制: {}", bar, progress.current_file);
            io::stdout().flush().unwrap();
        }) {
            Ok(_) => println!("\n初始化完成！请去 AppData 目录检查文件。"),
            Err(e) => {
                println!("\n❌ 初始化失败: {}", e);
                // 在这里你可以接入 UI 报错弹窗
                std::process::exit(1);
            }
        }

        println!("目标目录: {:?}", initializer.data_dir());
    } else {
        println!("资源已是最新，无需初始化。");
        println!("目标目录: {:?}", initializer.data_dir());
    }

    println!("=== 执行业务逻辑 ===");
    thread::sleep(Duration::from_millis(500));
}

/// 生成模拟的原始文件（包含大文件，用于观察耗时）
fn create_dummy_files() {
    // 1. 生成配置文件
    let _ = std::fs::write("test_config.toml", "[settings]\ntheme = 'dark'\n");

    // 2. 生成资源文件夹
    let _ = std::fs::create_dir_all("test_assets");
    for i in 0..200 {
        let _ = std::fs::write(format!("test_assets/file_{}.txt", i), "dummy content");
    }

    // 3. 生成 100MB 大文件
    if !std::path::Path::new("bigfile.zip").exists() {
        println!("正在生成本地测试大文件 (100MB)，请稍候...");
        let mut file = File::create("bigfile.zip").unwrap();
        let chunk = vec![0u8; 1024 * 1024];
        for _ in 0..100 {
            file.write_all(&chunk).unwrap();
        }
    }
}
