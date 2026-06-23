//! WASM 消息渲染与色调分类。
//!
//! 将 [`RunUpdate`](tswn_core::RunUpdate) 消息模板中的占位符（`[0]`/`[1]`/`[2]`）
//! 替换为实际玩家名称，并根据消息关键词判定 `MessageTone`（普通/伤害/回复/击倒），
//! 避免 JavaScript 侧重复实现字符串匹配。

use std::collections::HashMap;

use tswn_core::RunUpdate;
use tswn_core::player::PlrId;

use crate::model::MessageTone;

fn render_name(id: PlrId, names: &HashMap<PlrId, String>) -> String { names.get(&id).cloned().unwrap_or_else(|| id.to_string()) }

pub fn render_update_message(update: &RunUpdate, names: &HashMap<PlrId, String>) -> String {
    let mut message = update.message.to_string();
    message = message.replace("[0]", &render_name(update.caster, names));
    message = message.replace("[1]", &render_name(update.target, names));

    let param = if let Some(value) = update.param {
        value.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|target| render_name(*target, names))
            .collect::<Vec<String>>()
            .join(",")
    };

    message.replace("[2]", &param)
}

/// 根据消息模板中的中文关键词判定消息色调，避免 JS 侧重复实现字符串匹配。
pub fn classify_message_tone(template: &str) -> MessageTone {
    if template.contains("回复体力") {
        MessageTone::Recover
    } else if template.contains("被击倒") {
        MessageTone::Knockout
    } else if template.contains("点伤害") || template.contains("体力减少") {
        MessageTone::Damage
    } else {
        MessageTone::Normal
    }
}

fn push_unique_token(tokens: &mut Vec<String>, token: &str) {
    if token.is_empty() || tokens.iter().any(|existing| existing == token) {
        return;
    }
    tokens.push(token.to_string());
}

fn collect_between(tokens: &mut Vec<String>, template: &str, prefix: &str, suffix: &str) {
    let mut rest = template;
    while let Some(start) = rest.find(prefix) {
        let token_start = start + prefix.len();
        let after_prefix = &rest[token_start..];
        let Some(end) = after_prefix.find(suffix) else {
            break;
        };
        push_unique_token(tokens, &after_prefix[..end]);
        rest = &after_prefix[end + suffix.len()..];
    }
}

pub fn status_change_tokens(template: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    collect_between(&mut tokens, template, "从[", "]中解除");
    collect_between(&mut tokens, template, "从[", "]状态中解除");
    collect_between(&mut tokens, template, "的[", "]被识破");
    collect_between(&mut tokens, template, "的[", "]被中止了");
    collect_between(&mut tokens, template, "的[", "]被中止");
    collect_between(&mut tokens, template, "的[", "]被打消了");
    collect_between(&mut tokens, template, "的[", "]被打消");
    collect_between(&mut tokens, template, "的[", "]属性被打消");
    for token in ["解除", "中止", "打消"] {
        if template.contains(&format!("[{token}]")) {
            push_unique_token(&mut tokens, token);
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::{classify_message_tone, render_update_message, status_change_tokens};
    use std::collections::HashMap;
    use tswn_core::RunUpdate;

    #[test]
    fn falls_back_to_score_for_damage_placeholder() {
        let mut names = HashMap::new();
        names.insert(0usize, "施法者".to_string());
        names.insert(1usize, "目标".to_string());

        let update = RunUpdate::new("[1]受到[2]点伤害", 0, 1, 42);

        assert_eq!(render_update_message(&update, &names), "目标受到42点伤害");
    }

    #[test]
    fn half_damage_message_is_damage_tone() {
        assert_eq!(classify_message_tone("[1]体力减少[2]%"), crate::model::MessageTone::Damage);
    }

    #[test]
    fn extracts_status_change_tokens_from_known_templates() {
        assert_eq!(status_change_tokens("[0]的[潜行]被识破"), vec!["潜行"]);
        assert_eq!(status_change_tokens("[1]的[蓄力]被中止了"), vec!["蓄力"]);
        assert_eq!(status_change_tokens("[1]的[铁壁]被打消了"), vec!["铁壁"]);
        assert_eq!(status_change_tokens("[1]的[垂死]属性被打消"), vec!["垂死"]);
        assert_eq!(status_change_tokens("[1]从[狂暴]中解除"), vec!["狂暴"]);
        assert_eq!(status_change_tokens("[1]从[无实体]状态中解除"), vec!["无实体"]);
    }
}
