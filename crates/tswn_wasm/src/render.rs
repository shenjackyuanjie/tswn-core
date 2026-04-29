use std::collections::HashMap;

use tswn_core::RunUpdate;
use tswn_core::player::PlrId;

fn render_name(id: PlrId, names: &HashMap<PlrId, String>) -> String {
    names.get(&id).cloned().unwrap_or_else(|| id.to_string())
}

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

#[cfg(test)]
mod tests {
    use super::render_update_message;
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
}