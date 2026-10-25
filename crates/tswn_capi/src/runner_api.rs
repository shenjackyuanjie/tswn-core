//! 低层 runner、预构建模板及回合更新的 C 句柄适配。
//!
//! 所有非空句柄必须来自同一份库，且不能在被借用时释放或并发修改；输入字符串
//! 与输出缓冲在调用期间有效，所有数组索引均从零开始。返回句柄与字符串分别由
//! 对应 free 函数释放，标量快照无需释放。状态型调用会清除旧错误并捕获可展开的
//! panic；直接返回布尔值、长度或消息的兼容接口各有明确约定，不能据此假定
//! 所有低层入口都提供相同的错误边界。胜率 `thread=0/1/n` 表示自动/单线程/指定线程。

use std::ffi::c_char;

use tswn_core::runtime::PlrId;
use tswn_core::runtime::update::{RunUpdate, UpdateType};
use tswn_core::runtime::{EntityIdx, RuntimePlayerSnapshot};
use tswn_core::{PreparedRunner, Runner};

use crate::{
    FfiResult, ffi_boundary, ffi_error, into_tswn_str, read_utf8, set_last_error, tswn_player_snapshot_t, tswn_prepared_runner_t,
    tswn_runner_t, tswn_status_t, tswn_str_t, tswn_update_snapshot_t, tswn_update_type_t, tswn_updates_t, tswn_win_rate_result_t,
};

fn read_optional_utf8(ptr: *const c_char, name: &str) -> FfiResult<Vec<String>> {
    if ptr.is_null() {
        Ok(Vec::new())
    } else {
        Ok(vec![unsafe { read_utf8(ptr, name)? }])
    }
}

fn ensure_groups_for_win_rate(groups: &[Vec<String>]) -> FfiResult<()> {
    let count = groups.iter().filter(|group| !group.is_empty()).count();
    if count < 2 {
        Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            "win_rate requires at least two non-empty groups",
        ))
    } else {
        Ok(())
    }
}

fn update_type_to_c(update_type: UpdateType) -> tswn_update_type_t {
    match update_type {
        UpdateType::Win => tswn_update_type_t::Win,
        UpdateType::None => tswn_update_type_t::None,
        UpdateType::NextLine => tswn_update_type_t::NextLine,
    }
}

fn player_snapshot(player: RuntimePlayerSnapshot) -> tswn_player_snapshot_t {
    tswn_player_snapshot_t {
        id: player.id as u64,
        ptr: player.id as u64,
        hp: player.hp,
        max_hp: player.max_hp,
        magic_point: player.magic_point,
        move_point: player.move_point,
        attack: player.attack,
        defense: player.defense,
        speed: player.speed,
        agility: player.agility,
        magic: player.magic,
        resistance: player.resistance,
        wisdom: player.wisdom,
        point: player.point,
        all_sum: player.all_sum,
        name_factor: player.name_factor,
        at_boost: player.at_boost,
        attract: player.attract,
        frozen: u8::from(player.frozen),
    }
}

fn update_snapshot(update: &RunUpdate) -> tswn_update_snapshot_t {
    tswn_update_snapshot_t {
        score: update.score,
        param: update.param.unwrap_or_default(),
        delay0: update.delay0,
        delay1: update.delay1,
        caster_id: update.caster as u64,
        target_id: update.target as u64,
        target_count: update.targets.len(),
        has_param: u8::from(update.param.is_some()),
        update_type: update_type_to_c(update.update_type),
    }
}

fn run_prepared_win_rate(prepared: &PreparedRunner, n: usize, thread: u32) -> FfiResult<tswn_win_rate_result_t> {
    let summary = tswn_core::runtime::prepared_runtime_win_rate(prepared, n, thread)
        .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
    Ok(tswn_win_rate_result_t {
        wins: summary.wins as u64,
        total: summary.total as u64,
    })
}

fn copy_ids(ids: &[PlrId], out: *mut u64, cap: usize, name: &str) -> FfiResult<()> {
    if ids.len() > cap {
        return Err(ffi_error(
            tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
            format!("{name} buffer too small"),
        ));
    }
    if ids.is_empty() {
        return Ok(());
    }
    if out.is_null() {
        return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, format!("{name} output is null")));
    }
    for (index, id) in ids.iter().enumerate() {
        unsafe {
            *out.add(index) = *id as u64;
        }
    }
    Ok(())
}

/// 使用模板创建时固定的评估口径，累计 `n` 次第一组获胜统计；不修改模板。
///
/// # Safety
///
/// 非空 `prepared` 为存活且可共享借用的本库模板；非空 `out_result` 对齐且可写。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_win_rate(
    prepared: *const tswn_prepared_runner_t,
    n: usize,
    thread: u32,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if prepared.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "prepared is null"));
        }
        if out_result.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_result is null"));
        }
        let result = run_prepared_win_rate(unsafe { &(*prepared).inner }, n, thread)?;
        unsafe {
            *out_result = result;
        }
        Ok(())
    })
}

/// 计算模板胜率，并按浮点位模式验证 `eval_rq` 与创建时完全一致；不一致返回参数错误。
///
/// # Safety
///
/// 非空 `prepared` 为存活且可共享借用的本库模板；非空 `out_result` 对齐且可写。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_win_rate_with_eval_rq(
    prepared: *const tswn_prepared_runner_t,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if prepared.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "prepared is null"));
        }
        if out_result.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_result is null"));
        }
        if unsafe { (*prepared).eval_rq.to_bits() } != eval_rq.to_bits() {
            return Err(ffi_error(
                tswn_status_t::TSWN_ERR_INVALID_ARGUMENT,
                "eval_rq is fixed when the PreparedRunner is created",
            ));
        }
        let result = run_prepared_win_rate(unsafe { &(*prepared).inner }, n, thread)?;
        unsafe {
            *out_result = result;
        }
        Ok(())
    })
}

/// 释放 runner 及其持有的运行时资源；空指针无操作，不影响已返回的独立更新句柄。
///
/// # Safety
///
/// `ptr` 为空或为本库创建的存活 runner；调用方拥有唯一释放权，调用后不得再使用或重复释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_free(ptr: *mut tswn_runner_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// 释放预构建模板；已从模板创建的 runner 独立持有自身状态。
///
/// # Safety
///
/// `ptr` 为空或为本库创建的存活模板；必须等待所有借用结束，且只能释放一次。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_free(ptr: *mut tswn_prepared_runner_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// 释放一个回合的更新集合及其消息数据；复制出的字符串和标量快照仍独立有效。
///
/// # Safety
///
/// `ptr` 为空或为本库创建的存活更新句柄；必须等待所有读取结束，且只能释放一次。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_free(ptr: *mut tswn_updates_t) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// 解析队伍与种子文本，使用普通对局默认口径创建可变 runner。
///
/// # Safety
///
/// 输入是调用期间有效的 NUL 结尾 C 字符串；非空 `out_runner` 对齐且可写。成功句柄需用 `tswn_runner_free` 释放；失败不改写输出。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_new_from_raw(
    raw_text_utf8: *const c_char,
    out_runner: *mut *mut tswn_runner_t,
) -> tswn_status_t {
    unsafe { tswn_runner_new_from_raw_with_eval_rq(raw_text_utf8, crate::tswn_default_eval_rq(), out_runner) }
}

/// 解析原始名竞文本并以显式 `eval_rq` 构建 runner；输入仅在本次调用借用。
///
/// # Safety
///
/// 输入是调用期间有效的 NUL 结尾 C 字符串；非空 `out_runner` 对齐且可写，且不覆盖未释放句柄。成功句柄需用 `tswn_runner_free` 释放；失败不改写输出。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_new_from_raw_with_eval_rq(
    raw_text_utf8: *const c_char,
    eval_rq: f64,
    out_runner: *mut *mut tswn_runner_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_runner is null"));
        }
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let (groups, seed) = Runner::split_namerena_into_groups(raw);
        let runner = Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, eval_rq)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_runner = Box::into_raw(Box::new(tswn_runner_t { inner: runner }));
        }
        Ok(())
    })
}

/// 使用普通对局默认口径准备阵容模板；原始文本中的种子不存入模板。
///
/// # Safety
///
/// 输入是 NUL 结尾 C 字符串；非空 `out_prepared` 对齐且可写。成功句柄用 `tswn_prepared_runner_free` 释放；失败不改写输出。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_new_from_raw(
    raw_text_utf8: *const c_char,
    out_prepared: *mut *mut tswn_prepared_runner_t,
) -> tswn_status_t {
    unsafe { tswn_prepared_runner_new_from_raw_with_eval_rq(raw_text_utf8, crate::tswn_default_eval_rq(), out_prepared) }
}

/// 以固定 `eval_rq` 准备可复用阵容，种子需在创建每场 runner 时另行提供。
///
/// # Safety
///
/// 输入是 NUL 结尾 C 字符串；非空 `out_prepared` 对齐且可写，不覆盖未释放句柄。成功句柄用 `tswn_prepared_runner_free` 释放；失败不改写输出。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_prepared_runner_new_from_raw_with_eval_rq(
    raw_text_utf8: *const c_char,
    eval_rq: f64,
    out_prepared: *mut *mut tswn_prepared_runner_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_prepared.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_prepared is null"));
        }
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let groups = Runner::split_namerena_into_groups(raw).0;
        let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_prepared = Box::into_raw(Box::new(tswn_prepared_runner_t {
                inner: prepared,
                eval_rq,
            }));
        }
        Ok(())
    })
}

/// 从模板创建独立对局；`seed_utf8=NULL` 表示无额外种子，非空字符串作为一条种子。
///
/// # Safety
///
/// 非空 `prepared` 为存活模板；非空种子为有效 C 字符串。非空 `out_runner` 对齐且可写；成功句柄需单独释放，模板所有权不转移。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_new_from_prepared(
    prepared: *const tswn_prepared_runner_t,
    seed_utf8: *const c_char,
    out_runner: *mut *mut tswn_runner_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if prepared.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "prepared is null"));
        }
        if out_runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_runner is null"));
        }
        let seed = read_optional_utf8(seed_utf8, "seed_utf8")?;
        let runner = Runner::new_from_prepared_with_seed(unsafe { &(*prepared).inner }, &seed)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_runner = Box::into_raw(Box::new(tswn_runner_t { inner: runner }));
        }
        Ok(())
    })
}

/// 查询是否已有胜者；空 runner 返回 0，且不更新线程错误通道。
///
/// # Safety
///
/// 非空 `runner` 是存活且可共享借用的本库句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_have_winner(runner: *const tswn_runner_t) -> u8 {
    if runner.is_null() {
        0
    } else {
        u8::from(unsafe { (*runner).inner.have_winner() })
    }
}

/// 同步推进至胜者、无进展或绑定层回合上限；返回 0 不等同于输入错误。
///
/// 此低层布尔入口不提供状态码或 sticky failure；需要明确截断原因时使用战斗会话。
///
/// # Safety
///
/// 非空 `runner` 是存活且独占借用的本库句柄；空指针返回 0。此入口不经过 panic 捕获边界。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_run_to_completion(runner: *mut tswn_runner_t) -> u8 {
    if runner.is_null() {
        0
    } else {
        u8::from(unsafe { (*runner).inner.run_binding_to_completion() })
    }
}

/// 推进一个主回合，返回独立拥有的更新集合；调用方用 `tswn_updates_free` 释放。
///
/// # Safety
///
/// 非空 `runner` 存活且被独占借用；非空 `out_updates` 对齐且可写，不覆盖未释放句柄。失败不保证回滚已经推进的状态。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_main_round(
    runner: *mut tswn_runner_t,
    out_updates: *mut *mut tswn_updates_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        if out_updates.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_updates is null"));
        }
        let updates = unsafe { (*runner).inner.main_round() };
        unsafe {
            *out_updates = Box::into_raw(Box::new(tswn_updates_t { inner: updates }));
        }
        Ok(())
    })
}

/// 读取原始输入的队伍数量，不受运行时行动顺序影响；空 runner 返回 0。
///
/// # Safety
///
/// 非空 `runner` 是存活且可共享借用的本库句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_input_group_count(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.input_groups.len() }
    }
}

/// 读取零基输入队伍的成员数；runner 为空或索引越界时返回 0，不写入错误通道。
///
/// # Safety
///
/// 非空 `runner` 是存活且可共享借用的本库句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_input_group_len(runner: *const tswn_runner_t, group_index: usize) -> usize {
    if runner.is_null() {
        0
    } else {
        let input_groups = unsafe { &(*runner).inner.input_groups };
        input_groups.get(group_index).map(|g| g.len()).unwrap_or(0)
    }
}

/// 复制零基输入队伍的 roster；越界队伍按空列表处理，不会产生参数错误。
///
/// # Safety
///
/// 非空 `runner` 存活且可共享借用；非空 `out_ids` 对齐且可写入 `cap` 个 u64，不与句柄重叠。空列表允许空输出；容量不足时不复制。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_input_group_copy(
    runner: *const tswn_runner_t,
    group_index: usize,
    out_ids: *mut u64,
    cap: usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        let input_groups = unsafe { &(*runner).inner.input_groups };
        let group = input_groups
            .get(group_index)
            .into_iter()
            .flatten()
            .map(|entity| entity.0 as PlrId)
            .collect::<Vec<_>>();
        copy_ids(&group, out_ids, cap, "out_ids")
    })
}

/// 查询玩家对应的零基原始输入队伍；不存在或无法表示的标识返回参数错误。
///
/// # Safety
///
/// 非空 `runner` 存活且可共享借用；非空 `out_group_index` 对齐且可写。标识仅用于所属对局。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_player_input_group_index(
    runner: *const tswn_runner_t,
    player_id: u64,
    out_group_index: *mut usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        if out_group_index.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_group_index is null"));
        }

        let player_id = EntityIdx(
            player_id
                .try_into()
                .map_err(|_| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "player id out of range"))?,
        );
        let input_groups = unsafe { &(*runner).inner.input_groups };
        let group_index = input_groups
            .iter()
            .position(|group| group.contains(&player_id))
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "player not found in input groups"))?;

        unsafe {
            *out_group_index = group_index;
        }
        Ok(())
    })
}

/// 读取胜者 roster 长度；尚无胜者或 runner 为空时返回 0。
///
/// # Safety
///
/// 非空 `runner` 是存活且可共享借用的本库句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_winner_len(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.winner_ids().len() }
    }
}

/// 复制当前胜者 roster；未结束时可成功返回空列表。
///
/// # Safety
///
/// 非空 `runner` 存活且可共享借用；非空 `out_ids` 对齐且可写入 `cap` 个 u64，不与句柄重叠。空列表允许空输出；容量不足时不复制。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_winner_copy(runner: *const tswn_runner_t, out_ids: *mut u64, cap: usize) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        let winners = unsafe { (*runner).inner.winner_ids() };
        copy_ids(&winners, out_ids, cap, "out_ids")
    })
}

/// 读取包括死亡实体在内的当前实体总数；空 runner 返回 0。
///
/// # Safety
///
/// 非空 `runner` 是存活且可共享借用的本库句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_all_player_count(runner: *const tswn_runner_t) -> usize {
    if runner.is_null() {
        0
    } else {
        unsafe { (*runner).inner.runtime.entities.len() }
    }
}

/// 复制当前对局全部玩家标识，包括死亡实体；缓冲容量可先用 count 查询。
///
/// # Safety
///
/// 非空 `runner` 存活且可共享借用；非空 `out_ids` 对齐且可写入 `cap` 个 u64，不与句柄重叠。空列表允许空输出；容量不足时不复制。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_all_player_ids_copy(
    runner: *const tswn_runner_t,
    out_ids: *mut u64,
    cap: usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        let ids = unsafe { (*runner).inner.all_player_ids() };
        copy_ids(&ids, out_ids, cap, "out_ids")
    })
}

/// 复制指定玩家的当前属性；`ptr` 是兼容字段，与 `id` 相同，不是可解引用地址。
///
/// # Safety
///
/// 非空 `runner` 存活且可共享借用；非空 `out_snapshot` 对齐且可写，不与句柄重叠。找不到玩家返回参数错误，快照本身无需释放。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_runner_player_snapshot(
    runner: *const tswn_runner_t,
    player_id: u64,
    out_snapshot: *mut tswn_player_snapshot_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if runner.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "runner is null"));
        }
        if out_snapshot.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_snapshot is null"));
        }
        let player = unsafe { (*runner).inner.player_snapshot(player_id as PlrId) }
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "player not found"))?;
        unsafe {
            *out_snapshot = player_snapshot(player);
        }
        Ok(())
    })
}

/// 读取集合中实际保存的更新数；空句柄返回 0，不更新错误通道。
///
/// # Safety
///
/// 非空 `updates` 是本库创建的存活且可共享借用的更新句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_len(updates: *const tswn_updates_t) -> usize {
    if updates.is_null() {
        0
    } else {
        unsafe { (*updates).inner.updates.len() }
    }
}

/// 复制零基更新的轻量快照；`has_param=0` 时忽略 param，targets 需另行复制。
///
/// # Safety
///
/// 非空 `updates` 存活且可共享借用；非空 `out_update` 对齐且可写。越界返回参数错误，快照不持有需释放的资源。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_get(
    updates: *const tswn_updates_t,
    index: usize,
    out_update: *mut tswn_update_snapshot_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if updates.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "updates is null"));
        }
        if out_update.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_update is null"));
        }
        let updates_ref = unsafe { &(*updates).inner.updates };
        let update = updates_ref
            .get(index)
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "update index out of range"))?;
        unsafe {
            *out_update = update_snapshot(update);
        }
        Ok(())
    })
}

/// 复制零基更新的全部目标标识；所需容量由更新快照的 `target_count` 给出。
///
/// # Safety
///
/// 非空 `updates` 存活且可共享借用；非空 `out_ids` 对齐且可写入 `cap` 个 u64，不与句柄重叠。空目标允许空输出；越界或容量不足返回参数错误。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_targets_copy(
    updates: *const tswn_updates_t,
    index: usize,
    out_ids: *mut u64,
    cap: usize,
) -> tswn_status_t {
    ffi_boundary(|| {
        if updates.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "updates is null"));
        }
        let updates_ref = unsafe { &(*updates).inner.updates };
        let update = updates_ref
            .get(index)
            .ok_or_else(|| ffi_error(tswn_status_t::TSWN_ERR_INVALID_ARGUMENT, "update index out of range"))?;
        copy_ids(update.targets.as_slice(), out_ids, cap, "out_ids")
    })
}

/// 复制原始占位符消息模板，调用方按 len 读取并用 `tswn_str_free` 释放。
///
/// 空句柄或越界返回 `{NULL, 0}` 并记录错误；成功不会清除旧错误，此低层入口不捕获 panic。
///
/// # Safety
///
/// 非空 `updates` 是存活且可共享借用的本库更新句柄。返回值独立于句柄存活期。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_message(updates: *const tswn_updates_t, index: usize) -> tswn_str_t {
    if updates.is_null() {
        set_last_error("updates is null");
        return tswn_str_t::default();
    }
    let updates_ref = unsafe { &(*updates).inner.updates };
    match updates_ref.get(index) {
        Some(update) => into_tswn_str(update.message.to_string()),
        None => {
            set_last_error("update index out of range");
            tswn_str_t::default()
        }
    }
}

/// 复制 core 展开占位符后的消息；模板本身可通过 `tswn_updates_message` 获取。
///
/// 成功结果用 `tswn_str_free` 释放；空句柄或越界返回 `{NULL, 0}` 并记录错误。
/// 成功不清除旧错误，此低层入口不捕获 panic。
///
/// # Safety
///
/// 非空 `updates` 是存活且可共享借用的本库更新句柄。返回字符串不保证 NUL 结尾。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_updates_message_rendered(updates: *const tswn_updates_t, index: usize) -> tswn_str_t {
    if updates.is_null() {
        set_last_error("updates is null");
        return tswn_str_t::default();
    }
    let updates_ref = unsafe { &(*updates).inner.updates };
    match updates_ref.get(index) {
        Some(update) => into_tswn_str(update.msg()),
        None => {
            set_last_error("update index out of range");
            tswn_str_t::default()
        }
    }
}

/// 使用胜率默认评估口径，计算原始输入第一组对其余组的 wins/total。
///
/// # Safety
///
/// 输入指向 NUL 结尾队伍文本；非空 `out_result` 对齐且可写。至少需两个非空队伍。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate(
    raw_text_utf8: *const c_char,
    n: usize,
    thread: u32,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe { tswn_win_rate_with_eval_rq(raw_text_utf8, n, thread, crate::tswn_win_rate_eval_rq(), out_result) }
}

/// 按显式 `eval_rq` 准备阵容并累计 `n` 次试验；输入文本中的固定种子不用于该批任务。
///
/// # Safety
///
/// 输入指向 NUL 结尾队伍文本；非空 `out_result` 对齐且可写。至少需两个非空队伍。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_win_rate_with_eval_rq(
    raw_text_utf8: *const c_char,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_result: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        if out_result.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_result is null"));
        }
        let raw = unsafe { read_utf8(raw_text_utf8, "raw_text_utf8")? };
        let groups = Runner::split_namerena_into_groups(raw).0;
        ensure_groups_for_win_rate(&groups)?;
        let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq)
            .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
        unsafe {
            *out_result = run_prepared_win_rate(&prepared, n, thread)?;
        }
        Ok(())
    })
}

/// 使用默认胜率口径，按 against 顺序计算 target 与每个对手的统计。
///
/// # Safety
///
/// target 为有效 C 字符串；against 有 `against_len` 个有效 C 字符串指针，输出可写同等数量的统计结构。长度为零时数组可为空。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
    thread: u32,
    out_results: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    unsafe {
        tswn_group_win_rate_with_eval_rq(
            target_utf8,
            against_utf8,
            against_len,
            n,
            thread,
            crate::tswn_win_rate_eval_rq(),
            out_results,
        )
    }
}

/// 按显式评估口径逐个比较 target 与 against；中途失败时此前写入的结果不回滚。
///
/// # Safety
///
/// target 为有效 C 字符串；against 有 `against_len` 个有效 C 字符串指针，输出对齐且可写同等数量的统计结构。数组不得与输入重叠，零长可为空。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tswn_group_win_rate_with_eval_rq(
    target_utf8: *const c_char,
    against_utf8: *const *const c_char,
    against_len: usize,
    n: usize,
    thread: u32,
    eval_rq: f64,
    out_results: *mut tswn_win_rate_result_t,
) -> tswn_status_t {
    ffi_boundary(|| {
        let target = unsafe { read_utf8(target_utf8, "target_utf8")? };
        if against_len > 0 && against_utf8.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "against_utf8 is null"));
        }
        if against_len > 0 && out_results.is_null() {
            return Err(ffi_error(tswn_status_t::TSWN_ERR_NULL, "out_results is null"));
        }
        for index in 0..against_len {
            let opponent_ptr = unsafe { *against_utf8.add(index) };
            let opponent = unsafe { read_utf8(opponent_ptr, "against_utf8[index]")? };
            let raw = format!("{target}\n\n{opponent}");
            let groups = Runner::split_namerena_into_groups(raw).0;
            ensure_groups_for_win_rate(&groups)?;
            let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq)
                .map_err(|err| ffi_error(tswn_status_t::TSWN_ERR_RUNNER, err.to_string()))?;
            unsafe {
                *out_results.add(index) = run_prepared_win_rate(&prepared, n, thread)?;
            }
        }
        Ok(())
    })
}
