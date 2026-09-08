//! 仅供兼容旧版 UpdateView DTO 的状态标签。

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
    use super::*;
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
