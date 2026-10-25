//! 跨 crate 行为验收、引擎适配与冻结 golden/corpus 的测试辅助库。
//!
//! # 使用方式
//!
//! [`EngineAdapter`] 约定构造、单轮推进、胜者与可选 RNG 游标查询；
//! [`RuntimeEngine`] 接入当前主 Runtime。[`test_engine_suite!`] 为适配器生成
//! 共享行为测试，[`suite`] 保存 case 的断言，[`golden`] 读取基线消息。
//! [`EventSnapshot`] 提供行为比较需要的事件摘要。
//!
//! ```text
//! cargo test -p tswn_test --features runtime-corpus
//! python scripts/check_runtime_release.py --corpus
//! ```
//!
//! `runtime-corpus` 启用完整 corpus 集成测试。基线位于 `cases/`，语义更新时结合
//! 差分明确审阅。测试验证实际行为、兼容性、数据格式和资源生命周期；模块布局与
//! 维护约定由开发文档说明。此 crate 以 `publish = false` 作为项目测试辅助维护。

pub mod golden;
pub mod suite;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 测试快照中的事件类别，区分普通事件、排版换行和胜利通知。
///
/// NextLine 只影响日志分组，不计入战斗总分；golden 摘要则保留类别本身以发现排版语义变化。
pub enum SnapshotKind {
    Event,
    NextLine,
    Win,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 一次可比较的渲染事件：完整消息、施法者名称、积分和类别。
///
/// 字符串由适配器持有，不借用 runner；名称用于日志/积分断言，不是跨局稳定实体 ID。
pub struct EventSnapshot {
    pub message: String,
    pub caster_name: String,
    pub score: u32,
    pub kind: SnapshotKind,
}

/// 共享黑盒回放用例所需的最小引擎接口，Runner 的所有权交给测试函数持有。
///
/// main_round 推进一个引擎轮次，允许无可见消息；调用方用独立轮数上限防止无限运行。
/// 普通用例只需要消息/分数/胜者，冻结 stress 比较另外要求胜者队伍和 RC4 游标可用。
/// 构造失败以文本错误返回，推进阶段的机制错误如何呈现由适配器决定。
pub trait EngineAdapter {
    type Runner;

    /// 消费完整对战文本并构建独立 runner，输入解释失败返回便于断言的错误文本。
    fn new_from_raw(raw: String) -> Result<Self::Runner, String>;
    /// 可变推进一轮并移出本轮快照；空向量不表示已经产生胜者，调用方仍须查询 have_winner。
    fn main_round(runner: &mut Self::Runner) -> Vec<EventSnapshot>;
    fn have_winner(runner: &Self::Runner) -> bool;
    fn winner_names(runner: &Self::Runner) -> Vec<String>;

    /// 返回引擎胜者的队伍编号；默认不支持，需比较 stress golden 的适配器必须覆盖。
    fn winner_team_index(_runner: &Self::Runner) -> Option<usize> { None }

    /// 读取当前 RC4 的 i/j 游标，不消费随机数；不支持时返回 None。
    fn rc4_state(_runner: &Self::Runner) -> Option<(usize, usize)> { None }
}

/// 当前 Runtime 的无状态适配器类型，实际 runner 在 trait 实现中创建并由测试持有。
///
/// 使用默认自定义机制导入配置和输入名解析，不复用其他测试的 RNG 或全局战斗状态。
pub struct RuntimeEngine;

/// 在调用位置生成指定 EngineAdapter 的共享回放测试，不读取或更新磁盘基线。
///
/// 生成的模块名固定，同一作用域应只展开一次；stress corpus 的额外入口另行注册。
#[macro_export]
macro_rules! test_engine_suite {
    ($engine:ty) => {
        mod small {
            #[test]
            fn small_seed() { $crate::suite::small::small_seed::<$engine>(); }
        }

        mod simple {
            #[test]
            fn simple_fight() { $crate::suite::simple::simple_fight::<$engine>(); }
            #[test]
            fn simple_fight_scores() { $crate::suite::simple::simple_fight_scores::<$engine>(); }
            #[test]
            fn small_seed_scores() { $crate::suite::simple::small_seed_scores::<$engine>(); }
            #[test]
            fn case_d8c6_opening_matches_js_trace() { $crate::suite::simple::case_d8c6_opening_matches_js_trace::<$engine>(); }
            #[test]
            fn case_large_67_summon_opening_matches_js_trace() {
                $crate::suite::simple::case_large_67_summon_opening_matches_js_trace::<$engine>();
            }
        }

        mod fight_large {
            #[test]
            fn large_full() { $crate::suite::fight_large::large_full::<$engine>(); }
        }

        mod fight_multi {
            #[test]
            fn fight_multi_1() { $crate::suite::fight_multi_1::fight_multi_1::<$engine>(); }
            #[test]
            fn fight_multi_2() { $crate::suite::fight_multi_2::fight_multi_2::<$engine>(); }
            #[test]
            fn fight_multi_3() { $crate::suite::fight_multi_3::fight_multi_3::<$engine>(); }
            #[test]
            fn fight_multi_4() { $crate::suite::fight_multi_4::fight_multi_4::<$engine>(); }
            #[test]
            fn fight_multi_5() { $crate::suite::fight_multi_5::fight_multi_5::<$engine>(); }
            #[test]
            fn fight_multi_6() { $crate::suite::fight_multi_6::fight_multi_6::<$engine>(); }
            #[test]
            fn fight_multi_7() { $crate::suite::fight_multi_7::fight_multi_7::<$engine>(); }
            #[test]
            fn fight_multi_8() { $crate::suite::fight_multi_8::fight_multi_8::<$engine>(); }
        }

        mod large_01_10 {
            #[test]
            fn large_01() { $crate::suite::large_01_10::large_01::<$engine>(); }
            #[test]
            fn large_02() { $crate::suite::large_01_10::large_02::<$engine>(); }
            #[test]
            fn large_03() { $crate::suite::large_01_10::large_03::<$engine>(); }
            #[test]
            fn large_04() { $crate::suite::large_01_10::large_04::<$engine>(); }
            #[test]
            fn large_05() { $crate::suite::large_01_10::large_05::<$engine>(); }
            #[test]
            fn large_06() { $crate::suite::large_01_10::large_06::<$engine>(); }
            #[test]
            fn large_07() { $crate::suite::large_01_10::large_07::<$engine>(); }
            #[test]
            fn large_08() { $crate::suite::large_01_10::large_08::<$engine>(); }
            #[test]
            fn large_09() { $crate::suite::large_01_10::large_09::<$engine>(); }
            #[test]
            fn large_10() { $crate::suite::large_01_10::large_10::<$engine>(); }
        }

        mod large_11_17 {
            #[test]
            fn large_11() { $crate::suite::large_11_17::large_11::<$engine>(); }
            #[test]
            fn large_12() { $crate::suite::large_11_17::large_12::<$engine>(); }
            #[test]
            fn large_13() { $crate::suite::large_11_17::large_13::<$engine>(); }
            #[test]
            fn large_14() { $crate::suite::large_11_17::large_14::<$engine>(); }
            #[test]
            fn large_15() { $crate::suite::large_11_17::large_15::<$engine>(); }
            #[test]
            fn large_16() { $crate::suite::large_11_17::large_16::<$engine>(); }
            #[test]
            fn case_17() { $crate::suite::large_11_17::case_17::<$engine>(); }
        }

        mod large_18_22 {
            #[test]
            fn large_18() { $crate::suite::large_18_22::large_18::<$engine>(); }
            #[test]
            fn large_19() { $crate::suite::large_18_22::large_19::<$engine>(); }
            #[test]
            fn large_20() { $crate::suite::large_18_22::large_20::<$engine>(); }
            #[test]
            fn large_21() { $crate::suite::large_18_22::large_21::<$engine>(); }
            #[test]
            fn large_22() { $crate::suite::large_18_22::large_22::<$engine>(); }
        }

        mod large_23_30 {
            #[test]
            fn large_23() { $crate::suite::large_23_30::large_23::<$engine>(); }
            #[test]
            fn large_24() { $crate::suite::large_23_30::large_24::<$engine>(); }
            #[test]
            fn large_25() { $crate::suite::large_23_30::large_25::<$engine>(); }
            #[test]
            fn large_26() { $crate::suite::large_23_30::large_26::<$engine>(); }
            #[test]
            fn large_27() { $crate::suite::large_23_30::large_27::<$engine>(); }
            #[test]
            fn large_28() { $crate::suite::large_23_30::large_28::<$engine>(); }
            #[test]
            fn large_29() { $crate::suite::large_23_30::large_29::<$engine>(); }
            #[test]
            fn large_30() { $crate::suite::large_23_30::large_30::<$engine>(); }
        }

        mod large_31_40 {
            #[test]
            fn large_31() { $crate::suite::large_31_40::large_31::<$engine>(); }
            #[test]
            fn large_32() { $crate::suite::large_31_40::large_32::<$engine>(); }
            #[test]
            fn large_33() { $crate::suite::large_31_40::large_33::<$engine>(); }
            #[test]
            fn large_34() { $crate::suite::large_31_40::large_34::<$engine>(); }
            #[test]
            fn large_35() { $crate::suite::large_31_40::large_35::<$engine>(); }
            #[test]
            fn large_36() { $crate::suite::large_31_40::large_36::<$engine>(); }
            #[test]
            fn large_37() { $crate::suite::large_31_40::large_37::<$engine>(); }
            #[test]
            fn large_38() { $crate::suite::large_31_40::large_38::<$engine>(); }
            #[test]
            fn large_39() { $crate::suite::large_31_40::large_39::<$engine>(); }
            #[test]
            fn large_40() { $crate::suite::large_31_40::large_40::<$engine>(); }
        }

        mod large_41_45 {
            #[test]
            fn large_41() { $crate::suite::large_41_45::large_41::<$engine>(); }
            #[test]
            fn large_42() { $crate::suite::large_41_45::large_42::<$engine>(); }
            #[test]
            fn large_43() { $crate::suite::large_41_45::large_43::<$engine>(); }
            #[test]
            fn large_44() { $crate::suite::large_41_45::large_44::<$engine>(); }
            #[test]
            fn large_45() { $crate::suite::large_41_45::large_45::<$engine>(); }
        }

        mod large_46_50 {
            #[test]
            fn large_46() { $crate::suite::large_46_50::large_46::<$engine>(); }
            #[test]
            fn large_47() { $crate::suite::large_46_50::large_47::<$engine>(); }
            #[test]
            fn large_48() { $crate::suite::large_46_50::large_48::<$engine>(); }
            #[test]
            fn large_49() { $crate::suite::large_46_50::large_49::<$engine>(); }
            #[test]
            fn large_50() { $crate::suite::large_46_50::large_50::<$engine>(); }
        }

        mod large_51_55 {
            #[test]
            fn large_51() { $crate::suite::large_51_55::large_51::<$engine>(); }
            #[test]
            fn large_52() { $crate::suite::large_51_55::large_52::<$engine>(); }
            #[test]
            fn large_53() { $crate::suite::large_51_55::large_53::<$engine>(); }
            #[test]
            fn large_54() { $crate::suite::large_51_55::large_54::<$engine>(); }
            #[test]
            fn large_55() { $crate::suite::large_51_55::large_55::<$engine>(); }
        }

        mod large_56_61 {
            #[test]
            fn large_56() { $crate::suite::large_56_61::large_56::<$engine>(); }
            #[test]
            fn large_57() { $crate::suite::large_56_61::large_57::<$engine>(); }
            #[test]
            fn large_58() { $crate::suite::large_56_61::large_58::<$engine>(); }
            #[test]
            fn large_59() { $crate::suite::large_56_61::large_59::<$engine>(); }
            #[test]
            fn large_60() { $crate::suite::large_56_61::large_60::<$engine>(); }
            #[test]
            fn large_61() { $crate::suite::large_56_61::large_61::<$engine>(); }
        }

        mod large_62_65 {
            #[test]
            fn large_62() { $crate::suite::large_62_65::large_62::<$engine>(); }
            #[test]
            fn large_63() { $crate::suite::large_62_65::large_63::<$engine>(); }
            #[test]
            fn large_64() { $crate::suite::large_62_65::large_64::<$engine>(); }
            #[test]
            fn large_65() { $crate::suite::large_62_65::large_65::<$engine>(); }
        }

        mod large_66_70 {
            #[test]
            fn large_66() { $crate::suite::large_66_70::large_66::<$engine>(); }
            #[test]
            fn large_67() { $crate::suite::large_66_70::large_67::<$engine>(); }
            #[test]
            fn large_68() { $crate::suite::large_66_70::large_68::<$engine>(); }
            #[test]
            fn large_69() { $crate::suite::large_66_70::large_69::<$engine>(); }
            #[test]
            fn large_70() { $crate::suite::large_66_70::large_70::<$engine>(); }
        }

        mod large_71_80 {
            #[test]
            fn large_71() { $crate::suite::large_71_80::large_71::<$engine>(); }
            #[test]
            fn large_72() { $crate::suite::large_71_80::large_72::<$engine>(); }
        }
    };
}

#[cfg(test)]
mod battle_session;
