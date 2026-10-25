//! 低分与折叠组合归档规则。
//!
//! 依据赛道阈值及按排名折叠后的子项上限生成候选理由，不直接写数据库。
//! 手动屏蔽组合不自动封存，并优先折叠到更低的未屏蔽重叠项；同组多条理由合并去重。
//! 输入 rows 的排名顺序会影响折叠父子关系，调用方须使用本轮完整有序结果。

use super::*;

#[derive(Debug, Clone)]
pub(super) struct ArchiveCandidate {
    pub(super) group_id: GroupId,
    pub(super) reason: String,
    pub(super) average_cqd: f64,
}

pub(super) fn find_archive_candidates(rows: &[LaneResultRow], nodes: &[RankNode]) -> Vec<ArchiveCandidate> {
    let members_by_group_id: HashMap<GroupId, Vec<String>> =
        nodes.iter().map(|node| (node.group.id, node.group.members.clone())).collect();

    let mut by_group_id: HashMap<GroupId, ArchiveCandidate> = HashMap::new();

    for row in rows {
        // 手动屏蔽的组合仍要长期参与 CQD/Score 计算；
        // 自动封存会在“跳过封存组合”时把它从运行集合移除，所以这里不再自动封存手动屏蔽项。
        if row.is_blocked {
            continue;
        }

        if row.average_cqd < KICK_AVG_CQD_THRESHOLD {
            push_archive_reason(
                &mut by_group_id,
                row.group_id,
                row.average_cqd,
                format!("avg_cqd_below_kick_threshold_{KICK_AVG_CQD_THRESHOLD:.3}"),
            );
        } else {
            let archive_threshold = archive_avg_cqd_threshold(row.lane_size);
            if row.average_cqd < archive_threshold {
                push_archive_reason(
                    &mut by_group_id,
                    row.group_id,
                    row.average_cqd,
                    format!("avg_cqd_below_archive_threshold_{archive_threshold:.3}"),
                );
            }
        }
    }

    for group_id in folded_children_outside_keep_limit(rows, &members_by_group_id, FOLDED_CHILD_KEEP_LIMIT) {
        if let Some(row) = rows.iter().find(|row| row.group_id == group_id) {
            push_archive_reason(
                &mut by_group_id,
                row.group_id,
                row.average_cqd,
                format!("folded_child_outside_top_{FOLDED_CHILD_KEEP_LIMIT}"),
            );
        }
    }

    let mut out: Vec<_> = by_group_id.into_values().collect();
    out.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    out
}

pub(super) fn push_archive_reason(
    by_group_id: &mut HashMap<GroupId, ArchiveCandidate>,
    group_id: GroupId,
    average_cqd: f64,
    reason: String,
) {
    by_group_id
        .entry(group_id)
        .and_modify(|candidate| {
            if !candidate.reason.split(';').any(|part| part == reason.as_str()) {
                candidate.reason.push(';');
                candidate.reason.push_str(&reason);
            }
            candidate.average_cqd = average_cqd;
        })
        .or_insert(ArchiveCandidate {
            group_id,
            reason,
            average_cqd,
        });
}

pub(super) fn folded_children_outside_keep_limit(
    rows: &[LaneResultRow],
    members_by_group_id: &HashMap<GroupId, Vec<String>>,
    keep_limit: usize,
) -> Vec<GroupId> {
    let groups = folded_result_groups_for_archive(rows, members_by_group_id);
    let mut archived = Vec::new();

    for (_parent_idx, child_indices) in groups {
        let mut child_count = 0usize;
        for child_idx in child_indices {
            child_count += 1;
            if child_count > keep_limit && !rows[child_idx].is_blocked {
                archived.push(rows[child_idx].group_id);
            }
        }
    }

    archived
}

pub(super) fn folded_result_groups_for_archive(
    rows: &[LaneResultRow],
    members_by_group_id: &HashMap<GroupId, Vec<String>>,
) -> Vec<(usize, Vec<usize>)> {
    let mut consumed = vec![false; rows.len()];
    let mut pending: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut groups = Vec::new();

    for i in 0..rows.len() {
        if consumed[i] {
            continue;
        }

        let Some(parent_members) = members_by_group_id.get(&rows[i].group_id) else {
            consumed[i] = true;
            groups.push((i, pending.remove(&i).unwrap_or_default()));
            continue;
        };

        // 新折叠规则：如果一个被屏蔽组合下面有未屏蔽且成员重复的组合，
        // 它不再当父行，而是折叠到那个更低的未屏蔽组合下。
        if rows[i].is_blocked {
            if let Some(target_idx) = find_lower_unblocked_overlap(rows, members_by_group_id, &consumed, i, parent_members) {
                pending.entry(target_idx).or_default().push(i);
                consumed[i] = true;
                continue;
            }
        }

        consumed[i] = true;
        let mut children = pending.remove(&i).unwrap_or_default();

        for j in (i + 1)..rows.len() {
            if consumed[j] {
                continue;
            }

            let Some(child_members) = members_by_group_id.get(&rows[j].group_id) else {
                continue;
            };

            if !has_member_overlap(parent_members, child_members) {
                continue;
            }

            if rows[j].is_blocked {
                if let Some(target_idx) = find_lower_unblocked_overlap(rows, members_by_group_id, &consumed, j, child_members) {
                    pending.entry(target_idx).or_default().push(j);
                    consumed[j] = true;
                    continue;
                }
            }

            consumed[j] = true;
            children.push(j);

            if let Some(mut pending_for_child) = pending.remove(&j) {
                children.append(&mut pending_for_child);
            }
        }

        groups.push((i, children));
    }

    groups
}

pub(super) fn find_lower_unblocked_overlap(
    rows: &[LaneResultRow],
    members_by_group_id: &HashMap<GroupId, Vec<String>>,
    consumed: &[bool],
    start_idx: usize,
    members: &[String],
) -> Option<usize> {
    for target_idx in (start_idx + 1)..rows.len() {
        if consumed[target_idx] || rows[target_idx].is_blocked {
            continue;
        }

        let Some(target_members) = members_by_group_id.get(&rows[target_idx].group_id) else {
            continue;
        };

        if has_member_overlap(members, target_members) {
            return Some(target_idx);
        }
    }

    None
}

pub(super) fn has_member_overlap(a: &[String], b: &[String]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }

    let seen: HashSet<&str> = a.iter().map(String::as_str).collect();
    b.iter().any(|member| seen.contains(member.as_str()))
}
