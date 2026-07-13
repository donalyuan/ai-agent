//! 提供多个 Agent 能力共用的 Prompt 上下文格式化与长度控制工具。

use crate::repositories::Project;

/// 统一格式化账号策略，确保生成、质量闸门和主题评审读取同一上下文。
pub fn format_account_strategy_context(project: &Project) -> String {
    let profile = &project.strategy_profile;
    format!(
        r#"- 账号名称：{name}
- 定位摘要：{positioning}
- 账号描述：{description}
- 目标受众：{target_audience}
- 内容支柱：{content_pillars}
- 表达风格：{tone_style}
- 禁区方向：{forbidden_topics}
- 参考账号：{reference_accounts}
- 选题偏好：{topic_preferences}"#,
        name = non_empty_text(&project.name),
        positioning = non_empty_text(&project.positioning),
        description = non_empty_text(&project.description),
        target_audience = non_empty_text(&profile.target_audience),
        content_pillars = format_context_list(&profile.content_pillars),
        tone_style = non_empty_text(&profile.tone_style),
        forbidden_topics = format_context_list(&profile.forbidden_topics),
        reference_accounts = format_context_list(&profile.reference_accounts),
        topic_preferences = non_empty_text(&profile.topic_preferences),
    )
}

fn format_context_list(values: &[String]) -> String {
    if values.is_empty() {
        return "未填写".to_string();
    }
    values.join("、")
}

fn non_empty_text(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "未填写"
    } else {
        trimmed
    }
}

pub(super) fn truncate_for_prompt(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    format!("{}...", trimmed.chars().take(max_chars).collect::<String>())
}
