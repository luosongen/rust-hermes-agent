//! 配置管理命令实现
//!
//! 提供读取、设置、显示和编辑配置的命令处理函数。

use anyhow::Result;
use hermes_core::config::{config_file, Config};

/// 显示完整配置
///
/// 加载配置并以格式化形式输出，敏感信息自动脱敏。
pub fn show_config() -> Result<()> {
    let config = Config::load()?;
    println!("{}", config.display());
    Ok(())
}

/// 获取单个配置值
///
/// 根据键读取配置，支持点号分隔的嵌套键（如 `defaults.model`）。
pub fn get_config(key: &str) -> Result<()> {
    let config = Config::load()?;
    match config.get(key) {
        Some(value) => {
            println!("{}", value);
            Ok(())
        }
        None => {
            anyhow::bail!("Config key not found: {}", key);
        }
    }
}

/// 设置配置值并保存
///
/// 根据键设置配置值，支持点号分隔的嵌套键，然后保存到文件。
pub fn set_config(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;
    if config.set(key, value.to_string()) {
        config.save()?;
        println!("{} set to: {}", key, value);
        Ok(())
    } else {
        anyhow::bail!(
            "Cannot set config key: {}. Use 'hermes config show' to see valid keys.",
            key
        );
    }
}

/// 在编辑器中编辑配置文件
///
/// 读取 $EDITOR 环境变量打开配置文件路径。
pub fn edit_config() -> Result<()> {
    let path = config_file();
    let editor = Config::editor();
    std::process::Command::new(&editor).arg(&path).status()?;
    Ok(())
}
