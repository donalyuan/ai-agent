//! 提供多个 Agent 能力共用的 Prompt 上下文格式化与长度控制工具。

use crate::repositories::Project;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AccountStrategyContextField {
    pub(super) key: &'static str,
    pub(super) rendered: String,
}

/// 将账号策略拆为稳定字段，使各 Agent 可逐字段声明来源和版本。
pub(super) fn account_strategy_context_fields(
    project: &Project,
) -> Vec<AccountStrategyContextField> {
    let profile = &project.strategy_profile;
    vec![
        AccountStrategyContextField {
            key: "name",
            rendered: format!("- 账号名称：{}", non_empty_text(&project.name)),
        },
        AccountStrategyContextField {
            key: "positioning",
            rendered: format!("- 定位摘要：{}", non_empty_text(&project.positioning)),
        },
        AccountStrategyContextField {
            key: "description",
            rendered: format!("- 账号描述：{}", non_empty_text(&project.description)),
        },
        AccountStrategyContextField {
            key: "target-audience",
            rendered: format!("- 目标受众：{}", non_empty_text(&profile.target_audience)),
        },
        AccountStrategyContextField {
            key: "content-pillars",
            rendered: format!(
                "- 内容支柱：{}",
                format_context_list(&profile.content_pillars)
            ),
        },
        AccountStrategyContextField {
            key: "tone-style",
            rendered: format!("- 表达风格：{}", non_empty_text(&profile.tone_style)),
        },
        AccountStrategyContextField {
            key: "forbidden-topics",
            rendered: format!(
                "- 禁区方向：{}",
                format_context_list(&profile.forbidden_topics)
            ),
        },
        AccountStrategyContextField {
            key: "reference-accounts",
            rendered: format!(
                "- 参考账号：{}",
                format_context_list(&profile.reference_accounts)
            ),
        },
        AccountStrategyContextField {
            key: "topic-preferences",
            rendered: format!("- 选题偏好：{}", non_empty_text(&profile.topic_preferences)),
        },
    ]
}

/// 统一格式化账号策略，确保生成、质量闸门和主题评审读取同一上下文。
pub fn format_account_strategy_context(project: &Project) -> String {
    account_strategy_context_fields(project)
        .into_iter()
        .map(|field| field.rendered)
        .collect::<Vec<_>>()
        .join("\n")
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
