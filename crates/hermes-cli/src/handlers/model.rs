//! 模型管理命令实现
//!
//! 提供列出可用模型、设置默认模型、查看模型信息的命令处理函数。

use anyhow::Result;
use hermes_core::config::Config;

/// 可用模型列表
const AVAILABLE_MODELS: &[(&str, &str)] = &[
    ("openai/gpt-4o", "OpenAI GPT-4o - Most capable model"),
    ("openai/gpt-4-turbo", "OpenAI GPT-4 Turbo - Faster, cheaper than GPT-4"),
    ("openai/gpt-3.5-turbo", "OpenAI GPT-3.5 Turbo - Fastest, cheapest"),
    ("anthropic/claude-3-5-sonnet-20241022", "Anthropic Claude 3.5 Sonnet"),
    ("anthropic/claude-3-5-haiku-20241022", "Anthropic Claude 3.5 Haiku"),
];

/// 列出所有可用的模型
pub fn list_models() -> Result<()> {
    println!("Available models:\n");
    for (id, description) in AVAILABLE_MODELS {
        println!("  {:<45} {}", id, description);
    }
    Ok(())
}

/// 设置默认模型
///
/// 验证模型格式（必须包含 `/`），然后更新配置文件。
pub fn set_default_model(model: &str) -> Result<()> {
    // 验证模型格式
    if !model.contains('/') {
        anyhow::bail!("Invalid model format: '{}'. Expected format: 'provider/model-name' (e.g., 'openai/gpt-4o')", model);
    }

    // 检查模型是否在可用列表中
    let is_valid = AVAILABLE_MODELS.iter().any(|(id, _)| *id == model);
    if !is_valid {
        anyhow::bail!(
            "Unknown model: '{}'. Use 'hermes model list' to see available models.",
            model
        );
    }

    // 加载配置并更新
    let mut config = Config::load()?;
    config.set("defaults.model", model.to_string());
    config.save()?;

    println!("Default model set to: {}", model);
    Ok(())
}

/// 显示指定模型的详细信息
pub fn model_info(model: &str) -> Result<()> {
    // 验证模型格式
    if !model.contains('/') {
        anyhow::bail!("Invalid model format: '{}'. Expected format: 'provider/model-name' (e.g., 'openai/gpt-4o')", model);
    }

    // 查找模型信息
    let info = AVAILABLE_MODELS
        .iter()
        .find(|(id, _)| *id == model);

    match info {
        Some((id, description)) => {
            println!("Model: {}", id);
            println!("Description: {}", description);

            // 显示提供商
            if let Some((provider, _)) = id.split_once('/') {
                println!("Provider: {}", provider);
            }

            Ok(())
        }
        None => {
            anyhow::bail!(
                "Unknown model: '{}'. Use 'hermes model list' to see available models.",
                model
            );
        }
    }
}
