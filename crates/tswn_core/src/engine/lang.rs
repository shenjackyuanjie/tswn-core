//! # 引擎语言包 (lang)
//!
//! 本模块参考 JS 产物的 `LangData` 机制，为引擎提供本地化字符串支持。
//!
//! ## JS 对应关系
//!
//! JS 中的 `LangData` 使用以下流程：
//! 1. `LangData.eQ(str)` — 将明文键哈希为 4 字符混淆码
//! 2. `LangData.load_lang(map)` — 将 `{明文键 → 字符串}` 表加载到内存（以混淆码为 key）
//! 3. `LangData.get_lang(hash)` — 以混淆码查询字符串
//!
//! Rust 实现**直接使用明文键**，无需混淆，通过 [`get_lang`] 函数查询静态字符串表。
//!
//! ## 消息占位符约定
//!
//! 与 JS 产物保持一致：
//! - `[0]` — 施法者（Caster）的显示名
//! - `[1]` — 目标（Target）的显示名
//! - `[2]` — 参数值（数值或多个目标拼接字符串）
//!
//! ## 用法示例
//!
//! ```rust,ignore
//! use tswn_core::engine::lang;
//!
//! // 取得字符串模板（含占位符）
//! let tmpl = lang::get_lang(lang::keys::SKL_FIRE);
//! assert_eq!(tmpl, "[0]使用[火球术]");
//!
//! // 配合 RunUpdate 使用
//! // RunUpdate::new_lang(lang::keys::SKL_FIRE, caster, target, 1)
//! ```
//!
//! ## 语言数据来源
//!
//! 数据对齐 `fast-namerena/branch/latest/assets/zh.json`。
//! 如需新增或修改翻译，请同步修改 `zh.json` 和此文件中的 `LANG_DATA` 数组。

use foldhash::HashMap as FastHashMap;
use std::sync::OnceLock;

// ─── 内部静态 Map ─────────────────────────────────────────────────────────────

/// 所有语言条目，格式为 `(key, value)`。
/// key 与 `zh.json` 中的 JSON 键名完全对应；value 为中文模板字符串。
const LANG_DATA: &[(&str, &str)] = &[
    // ── 战斗核心 ─────────────────────────────────────────────────────────────
    ("damage", "[1]受到[2]点伤害"),
    ("defend", "[0][防御]"),
    ("die", "[1]被击倒了"),
    ("dodge", "[0][回避]了攻击(通用)"),
    ("minionDie", "[1]消失了"),
    ("recover", "[1]回复体力[2]点"),
    ("win", "[2]获得胜利"),
    // ── 普通技能 - act（主动）─────────────────────────────────────────────
    ("sklAbsorb", "[0]发起[吸血攻击]"),
    ("sklAccumulate", "[0]开始[聚气]"),
    ("sklAccumulateCancel", "[1]的[聚气]被打消了"),
    ("sklAccumulated", "[1]攻击力上升"),
    ("sklAokijiDefend", "[0][吸收]所有冰冻伤害"),
    ("sklAokijiIceAge", "[0]使用[冰河时代]"),
    ("sklAssassinate1", "[0][潜行]到[1]身后"),
    ("sklAssassinate2", "[0]发动[背刺]"),
    ("sklAssassinateFailed", "[0]的[潜行]被识破"),
    ("sklAttack", "[0]发起攻击"),
    ("sklBerserk", "[0]使用[狂暴术]"),
    ("sklBerserkAttack", "[0]发起[狂暴攻击]"),
    ("sklBerserkEnd", "[1]从[狂暴]中解除"),
    ("sklBerserkHit", "[1]进入[狂暴]状态"),
    ("sklCharge", "[0]开始[蓄力]"),
    ("sklChargeCancel", "[1]的[蓄力]被中止了"),
    ("sklCharm", "[0]使用[魅惑]"),
    ("sklCharmEnd", "[1]从[魅惑]中解除"),
    ("sklCharmHit", "[1]被[魅惑]了"),
    ("sklClone", "[0]使用[分身]"),
    ("sklCloned", "出现一个新的[1]"),
    ("sklCritical", "[0]发动[会心一击]"),
    ("sklCurse", "[0]使用[诅咒]"),
    ("sklCurseDamage", "[诅咒]使伤害加倍"),
    ("sklCurseEnd", "[1]从[诅咒]中解除"),
    ("sklCurseHit", "[1]被[诅咒]了"),
    ("sklDisperse", "[0]使用[净化]"),
    ("sklExchange", "[0]使用[生命之轮]"),
    ("sklExchanged", "[1]的体力值与[0]互换"),
    ("sklExplode", "[0]使用[自爆]"),
    ("sklFire", "[0]使用[火球术]"),
    ("sklHalf", "[0]使用[瘟疫]"),
    ("sklHalfDamage", "[1]体力减少[2]%"),
    ("sklHaste", "[0]使用[加速术]"),
    ("sklHasteEnd", "[1]从[疾走]中解除"),
    ("sklHasteHit", "[1]进入[疾走]状态"),
    ("sklHeal", "[0]使用[治愈魔法]"),
    ("sklIce", "[0]使用[冰冻术]"),
    ("sklIceEnd", "[1]从[冰冻]中解除"),
    ("sklIceHit", "[1]被[冰冻]了"),
    ("sklIkarugaAttack", "[0]使用[能量释放]"),
    ("sklIkarugaDefend", "[0][吸收]所有奇数伤害"),
    ("sklIron", "[0]发动[铁壁]"),
    ("sklIronCancel", "[1]的[铁壁]被打消了"),
    ("sklIronEnd", "[0]从[铁壁]中解除"),
    ("sklIrond", "[0]防御力大幅上升"),
    ("sklMagicAttack", "[0]发起攻击"),
    ("sklPoison", "[0][投毒]"),
    ("sklPoisonDamage", "[1][毒性发作]"),
    ("sklPoisonEnd", "[1]从[中毒]中解除"),
    ("sklPoisonHit", "[1][中毒]"),
    ("sklPossess", "[0]使用[附体]"),
    ("sklQuake", "[0]使用[地裂术]"),
    ("sklRevive", "[0]使用[苏生术]"),
    ("sklRevived", "[1][复活]了"),
    ("sklShadow", "[0]使用[幻术]"),
    ("sklShadowName", "幻影"),
    ("sklShadowed", "召唤出[1]"),
    ("sklSlow", "[0]使用[减速术]"),
    ("sklSlowEnd", "[1]从[迟缓]中解除"),
    ("sklSlowHit", "[1]进入[迟缓]状态"),
    ("sklSummon", "[0]使用[血祭]"),
    ("sklSummonName", "使魔"),
    ("sklSummoned", "召唤出[1]"),
    ("sklThunder", "[0]使用[雷击术]"),
    ("sklThunderEnd", "[0][回避]了攻击(雷击)"),
    // ── 普通技能 - skl（防御/被动）────────────────────────────────────────
    ("sklCounter", "[0]发起[反击]"),
    ("sklHide", "[0]发动[隐匿]"),
    ("sklMerge", "[0][吞噬]了[1]"),
    ("sklMerged", "[0]属性上升"),
    ("sklProtect", "[0][守护][1]"),
    ("sklReflect", "[0]使用[伤害反弹]"),
    ("sklReraise", "[0]使用[护身符]抵挡了一次死亡"),
    ("sklUpgrade", "[0]做出[垂死]抗争"),
    ("sklUpgradeCancel", "[1]的[垂死]属性被打消"),
    ("sklUpgraded", "[0]所有属性上升"),
    ("sklZombie", "[0][召唤亡灵]"),
    ("sklZombieName", "丧尸"),
    ("sklZombied", "[2]变成了[1]"),
    // ── 连击系 ────────────────────────────────────────────────────────────
    ("SklRapid", "[0]发起攻击"),
    ("SklRapidNext", "[0][连击]"),
    // ── Boss：青雉 ────────────────────────────────────────────────────────
    ("bossName_aokiji", "青雉"),
    // ── Boss：柯南 ────────────────────────────────────────────────────────
    ("bossName_conan", "柯南"),
    ("sklConanKill", "[0]在一间密室中发现了[1]的尸体"),
    ("sklConanKillLast", "[1]"),
    ("sklConanKillUnknown", "[0]在一间密室中发现了一具无名尸体"),
    ("sklConanThinking", "[0]正在进行推理"),
    ("sklConanThinkingFinish", "[0]推理完毕"),
    ("sklConanThinkingFinish2", "真相只有一个"),
    ("sklConanThinkingFinish3", "凶手就是你"),
    // ── Boss：新冠病毒 ────────────────────────────────────────────────────
    ("bossName_covid", "新冠病毒"),
    ("sklCovidDamage", "[1][肺炎]发作"),
    ("sklCovidHit", "[1]感染了[新冠病毒]"),
    ("sklCovidICU", "[1]在重症监护室无法行动"),
    ("sklCovidInfect", "[0]和[1]近距离接触"),
    ("sklCovidMutate", "[1]所感染的病毒发生变异"),
    ("sklCovidPrevent", "但[1]没被感染"),
    ("sklCovidStayHome", "[1]在家中自我隔离"),
    // ── Boss：斑鸠 ────────────────────────────────────────────────────────
    ("bossName_ikaruga", "斑鸠"),
    // ── Boss：懒癌 ────────────────────────────────────────────────────────
    ("bossName_lazy", "懒癌"),
    ("sklLazyDamage", "[1][懒癌]发作"),
    ("sklLazyHit", "[1]感染了[懒癌]"),
    ("sklLazySkipTurn0", "这回合什么也没做"),
    ("sklLazySkipTurn1", "[0]打开了[Steam]"),
    ("sklLazySkipTurn2", "[0]打开了[守望先锋]"),
    ("sklLazySkipTurn3", "[0]打开了[文明6]"),
    ("sklLazySkipTurn4", "[0]打开了[英雄联盟]"),
    ("sklLazySkipTurn5", "[0]打开了[微博]"),
    ("sklLazySkipTurn6", "[0]打开了[朋友圈]"),
    // ── Boss：马里奥 ──────────────────────────────────────────────────────
    ("bossName_mario", "马里奥"),
    ("bossMarioGrow10", "[0]得到[蘑菇]"),
    ("bossMarioGrow11", "[0]攻击力上升"),
    ("bossMarioGrow20", "[0]得到[火焰花]"),
    ("bossMarioGrow21", "[0]学会[火球术]"),
    ("bossMarioGrow30", "[0]得到[奖命蘑菇]"),
    ("bossMarioLife", "[0]还剩[2]条命"),
    ("bossMarioRevive", "[0]满血复活"),
    // ── Boss：蚊 ─────────────────────────────────────────────────────────
    ("bossName_mosquito", "蚊"),
    // ── Boss：一拳超人 ────────────────────────────────────────────────────
    ("bossName_saitama", "一拳超人"),
    ("saitamaHungry", "[0]觉得有点饿"),
    ("saitamaLeave", "[0]离开了战场"),
    // ── Boss：史莱姆 ──────────────────────────────────────────────────────
    ("bossName_slime", "史莱姆"),
    ("sklSlimeSpawn", "[0][分裂]"),
    ("sklSlimeSpawned", "分成了[0] 和  [1]"),
    // ── Boss：索尼克 ──────────────────────────────────────────────────────
    ("bossName_sonic", "索尼克"),
    // ── Boss：尤里 ────────────────────────────────────────────────────────
    ("bossName_yuri", "尤里"),
    ("sklYuriControl", "[0]使用[心灵控制]"),
    // ── 武器 ─────────────────────────────────────────────────────────────
    ("weaponDeathNoteAtk", "[0]在[死亡笔记]写下[1]的名字"),
    ("weaponRModifierUse", "[0]使用[属性修改器]"),
    ("weaponS11_0", "[0]在促销日[购买]了武器"),
    ("weaponS11_1", "但是并没有什么用"),
    ("weaponS11_2", "增加了[2]点"),
    ("weaponS11Done1", "[0]信用卡刷爆"),
    ("weaponS11Done2", "[0]砍下了自己的右手"),
    ("weaponS11Done3", "[0]砍下了自己的左手"),
    // ── 实力评估 ─────────────────────────────────────────────────────────
    ("benchmarkRatio", "》 胜率: [2]%"),
    ("benchmarkScore", "》 实力评分: [2]"),
    ("benchmarkSkill", "频率: [2]%"),
    ("benchmarking", "实力评估中...[2]%"),
    // ── UI / 搜索（引擎层一般不直接用，但保留对齐 JS）────────────────────
    ("HP", "HP"),
    ("challengeLabel", "挑战Boss"),
    ("closeTitle", "关闭"),
    ("continueGame", "继续游戏"),
    ("detail", " 攻 [] 防 [] 速 [] 敏 [] 魔 [] 抗 [] 智 []"),
    ("endMessage", "你已经玩了[0]局了"),
    ("errorMaxPlayer", "错误，目前最多支持1000人PK"),
    ("errorMinPlayer", "错误，请至少输入两行名字"),
    ("fastTitle", "快进"),
    ("helpTitle", "帮助"),
    (
        "inputPlaceholder",
        "修改by shenjackyuanjie&超导体元素\n\n版本: latest\n可能会有一些问题, 稳定版请使用根目录下版本",
    ),
    ("inputTitle", "名字竞技场"),
    ("killedCount", "击杀"),
    ("killerName", "致命一击"),
    ("loserName", "败者"),
    ("navigationLink", "navigation.html"),
    ("returnTitle", "返回"),
    ("score", "得分"),
    ("searchEnd", "搜索结束"),
    ("searchFailed", "但是一无所获"),
    ("searchInvalid", "错误，目前最多支持8000人搜索"),
    ("searchStart", "搜索开始..."),
    ("selectBossHint", "选择Boss"),
    ("shareTitle", "分享"),
    ("startFight", "开 始"),
    ("welcome", "名字竞技场"),
    ("welcome2", "(MD5大作战10周年纪念)"),
    ("winnerName", "胜者"),
];

// ─── 全局懒初始化 Map ──────────────────────────────────────────────────────────

/// 全局语言包 HashMap，首次访问时由 [`OnceLock`] 初始化。
static LANG_MAP: OnceLock<FastHashMap<&'static str, &'static str>> = OnceLock::new();

/// 获取语言包 HashMap 的引用。
fn lang_map() -> &'static FastHashMap<&'static str, &'static str> {
    LANG_MAP.get_or_init(|| {
        let mut map = FastHashMap::with_capacity_and_hasher(LANG_DATA.len(), Default::default());
        for &(k, v) in LANG_DATA {
            map.insert(k, v);
        }
        map
    })
}

// ─── 公开接口 ──────────────────────────────────────────────────────────────────

/// 根据语言键查询对应的中文字符串模板。
///
/// - 键未找到时返回空字符串 `""`，语义与 JS 的 `LangData.get_lang` 一致。
/// - 返回的字符串可能包含 `[0]`/`[1]`/`[2]` 占位符，
///   使用 [`RunUpdate::msg`](crate::engine::update::RunUpdate::msg) 可完成替换。
///
/// # 示例
///
/// ```rust
/// # use tswn_core::engine::lang;
/// assert_eq!(lang::get_lang("sklFire"), "[0]使用[火球术]");
/// assert_eq!(lang::get_lang("不存在的键"), "");
/// ```
#[inline]
pub fn get_lang(key: &str) -> &'static str { lang_map().get(key).copied().unwrap_or("") }

/// 查询语言键并将 `[0]`/`[1]`/`[2]` 占位符替换为给定的字符串。
///
/// - `s0`：替换 `[0]`（施法者名）
/// - `s1`：替换 `[1]`（目标名）
/// - `s2`：替换 `[2]`（参数，可为空字符串）
///
/// 若键不存在，返回空字符串 `String::new()`。
pub fn format_lang(key: &str, s0: &str, s1: &str, s2: &str) -> String {
    let tmpl = get_lang(key);
    if tmpl.is_empty() {
        return String::new();
    }
    tmpl.replace("[0]", s0).replace("[1]", s1).replace("[2]", s2)
}

// ─── 语言键常量 ────────────────────────────────────────────────────────────────

/// 所有游戏内语言键的常量定义，与 `zh.json` 的键名完全对应。
///
/// 推荐在技能实现中使用这里的常量，而不是裸字符串，以便编译期检查键名拼写。
pub mod keys {
    // ── 战斗核心 ──────────────────────────────────────────────────────────
    pub const DAMAGE: &str = "damage";
    pub const DEFEND: &str = "defend";
    pub const DIE: &str = "die";
    pub const DODGE: &str = "dodge";
    pub const MINION_DIE: &str = "minionDie";
    pub const RECOVER: &str = "recover";
    pub const WIN: &str = "win";

    // ── act 技能 ──────────────────────────────────────────────────────────
    pub const SKL_ABSORB: &str = "sklAbsorb";
    pub const SKL_ACCUMULATE: &str = "sklAccumulate";
    pub const SKL_ACCUMULATE_CANCEL: &str = "sklAccumulateCancel";
    pub const SKL_ACCUMULATED: &str = "sklAccumulated";
    pub const SKL_AOKIJI_DEFEND: &str = "sklAokijiDefend";
    pub const SKL_AOKIJI_ICE_AGE: &str = "sklAokijiIceAge";
    pub const SKL_ASSASSINATE_1: &str = "sklAssassinate1";
    pub const SKL_ASSASSINATE_2: &str = "sklAssassinate2";
    pub const SKL_ASSASSINATE_FAILED: &str = "sklAssassinateFailed";
    pub const SKL_ATTACK: &str = "sklAttack";
    pub const SKL_BERSERK: &str = "sklBerserk";
    pub const SKL_BERSERK_ATTACK: &str = "sklBerserkAttack";
    pub const SKL_BERSERK_END: &str = "sklBerserkEnd";
    pub const SKL_BERSERK_HIT: &str = "sklBerserkHit";
    pub const SKL_CHARGE: &str = "sklCharge";
    pub const SKL_CHARGE_CANCEL: &str = "sklChargeCancel";
    pub const SKL_CHARM: &str = "sklCharm";
    pub const SKL_CHARM_END: &str = "sklCharmEnd";
    pub const SKL_CHARM_HIT: &str = "sklCharmHit";
    pub const SKL_CLONE: &str = "sklClone";
    pub const SKL_CLONED: &str = "sklCloned";
    pub const SKL_CRITICAL: &str = "sklCritical";
    pub const SKL_CURSE: &str = "sklCurse";
    pub const SKL_CURSE_DAMAGE: &str = "sklCurseDamage";
    pub const SKL_CURSE_END: &str = "sklCurseEnd";
    pub const SKL_CURSE_HIT: &str = "sklCurseHit";
    pub const SKL_DISPERSE: &str = "sklDisperse";
    pub const SKL_EXCHANGE: &str = "sklExchange";
    pub const SKL_EXCHANGED: &str = "sklExchanged";
    pub const SKL_EXPLODE: &str = "sklExplode";
    pub const SKL_FIRE: &str = "sklFire";
    pub const SKL_HALF: &str = "sklHalf";
    pub const SKL_HALF_DAMAGE: &str = "sklHalfDamage";
    pub const SKL_HASTE: &str = "sklHaste";
    pub const SKL_HASTE_END: &str = "sklHasteEnd";
    pub const SKL_HASTE_HIT: &str = "sklHasteHit";
    pub const SKL_HEAL: &str = "sklHeal";
    pub const SKL_ICE: &str = "sklIce";
    pub const SKL_ICE_END: &str = "sklIceEnd";
    pub const SKL_ICE_HIT: &str = "sklIceHit";
    pub const SKL_IKARUGA_ATTACK: &str = "sklIkarugaAttack";
    pub const SKL_IKARUGA_DEFEND: &str = "sklIkarugaDefend";
    pub const SKL_IRON: &str = "sklIron";
    pub const SKL_IRON_CANCEL: &str = "sklIronCancel";
    pub const SKL_IRON_END: &str = "sklIronEnd";
    pub const SKL_IROND: &str = "sklIrond";
    pub const SKL_MAGIC_ATTACK: &str = "sklMagicAttack";
    pub const SKL_POISON: &str = "sklPoison";
    pub const SKL_POISON_DAMAGE: &str = "sklPoisonDamage";
    pub const SKL_POISON_END: &str = "sklPoisonEnd";
    pub const SKL_POISON_HIT: &str = "sklPoisonHit";
    pub const SKL_POSSESS: &str = "sklPossess";
    pub const SKL_QUAKE: &str = "sklQuake";
    pub const SKL_REVIVE: &str = "sklRevive";
    pub const SKL_REVIVED: &str = "sklRevived";
    pub const SKL_SHADOW: &str = "sklShadow";
    pub const SKL_SHADOW_NAME: &str = "sklShadowName";
    pub const SKL_SHADOWED: &str = "sklShadowed";
    pub const SKL_SLOW: &str = "sklSlow";
    pub const SKL_SLOW_END: &str = "sklSlowEnd";
    pub const SKL_SLOW_HIT: &str = "sklSlowHit";
    pub const SKL_SUMMON: &str = "sklSummon";
    pub const SKL_SUMMON_NAME: &str = "sklSummonName";
    pub const SKL_SUMMONED: &str = "sklSummoned";
    pub const SKL_THUNDER: &str = "sklThunder";
    pub const SKL_THUNDER_END: &str = "sklThunderEnd";

    // ── skl 技能（防御/被动）──────────────────────────────────────────────
    pub const SKL_COUNTER: &str = "sklCounter";
    pub const SKL_HIDE: &str = "sklHide";
    pub const SKL_MERGE: &str = "sklMerge";
    pub const SKL_MERGED: &str = "sklMerged";
    pub const SKL_PROTECT: &str = "sklProtect";
    pub const SKL_REFLECT: &str = "sklReflect";
    pub const SKL_RERAISE: &str = "sklReraise";
    pub const SKL_UPGRADE: &str = "sklUpgrade";
    pub const SKL_UPGRADE_CANCEL: &str = "sklUpgradeCancel";
    pub const SKL_UPGRADED: &str = "sklUpgraded";
    pub const SKL_ZOMBIE: &str = "sklZombie";
    pub const SKL_ZOMBIE_NAME: &str = "sklZombieName";
    pub const SKL_ZOMBIED: &str = "sklZombied";

    // ── 连击 ──────────────────────────────────────────────────────────────
    pub const SKL_RAPID: &str = "SklRapid";
    pub const SKL_RAPID_NEXT: &str = "SklRapidNext";

    // ── Boss 名称 ──────────────────────────────────────────────────────────
    pub const BOSS_NAME_AOKIJI: &str = "bossName_aokiji";
    pub const BOSS_NAME_CONAN: &str = "bossName_conan";
    pub const BOSS_NAME_COVID: &str = "bossName_covid";
    pub const BOSS_NAME_IKARUGA: &str = "bossName_ikaruga";
    pub const BOSS_NAME_LAZY: &str = "bossName_lazy";
    pub const BOSS_NAME_MARIO: &str = "bossName_mario";
    pub const BOSS_NAME_MOSQUITO: &str = "bossName_mosquito";
    pub const BOSS_NAME_SAITAMA: &str = "bossName_saitama";
    pub const BOSS_NAME_SLIME: &str = "bossName_slime";
    pub const BOSS_NAME_SONIC: &str = "bossName_sonic";
    pub const BOSS_NAME_YURI: &str = "bossName_yuri";

    // ── Boss：柯南 ────────────────────────────────────────────────────────
    pub const SKL_CONAN_KILL: &str = "sklConanKill";
    pub const SKL_CONAN_KILL_LAST: &str = "sklConanKillLast";
    pub const SKL_CONAN_KILL_UNKNOWN: &str = "sklConanKillUnknown";
    pub const SKL_CONAN_THINKING: &str = "sklConanThinking";
    pub const SKL_CONAN_THINKING_FINISH: &str = "sklConanThinkingFinish";
    pub const SKL_CONAN_THINKING_FINISH2: &str = "sklConanThinkingFinish2";
    pub const SKL_CONAN_THINKING_FINISH3: &str = "sklConanThinkingFinish3";

    // ── Boss：新冠病毒 ────────────────────────────────────────────────────
    pub const SKL_COVID_DAMAGE: &str = "sklCovidDamage";
    pub const SKL_COVID_HIT: &str = "sklCovidHit";
    pub const SKL_COVID_ICU: &str = "sklCovidICU";
    pub const SKL_COVID_INFECT: &str = "sklCovidInfect";
    pub const SKL_COVID_MUTATE: &str = "sklCovidMutate";
    pub const SKL_COVID_PREVENT: &str = "sklCovidPrevent";
    pub const SKL_COVID_STAY_HOME: &str = "sklCovidStayHome";

    // ── Boss：懒癌 ────────────────────────────────────────────────────────
    pub const SKL_LAZY_DAMAGE: &str = "sklLazyDamage";
    pub const SKL_LAZY_HIT: &str = "sklLazyHit";
    pub const SKL_LAZY_SKIP_TURN_0: &str = "sklLazySkipTurn0";
    pub const SKL_LAZY_SKIP_TURN_1: &str = "sklLazySkipTurn1";
    pub const SKL_LAZY_SKIP_TURN_2: &str = "sklLazySkipTurn2";
    pub const SKL_LAZY_SKIP_TURN_3: &str = "sklLazySkipTurn3";
    pub const SKL_LAZY_SKIP_TURN_4: &str = "sklLazySkipTurn4";
    pub const SKL_LAZY_SKIP_TURN_5: &str = "sklLazySkipTurn5";
    pub const SKL_LAZY_SKIP_TURN_6: &str = "sklLazySkipTurn6";

    // ── Boss：马里奥 ──────────────────────────────────────────────────────
    pub const BOSS_MARIO_GROW_10: &str = "bossMarioGrow10";
    pub const BOSS_MARIO_GROW_11: &str = "bossMarioGrow11";
    pub const BOSS_MARIO_GROW_20: &str = "bossMarioGrow20";
    pub const BOSS_MARIO_GROW_21: &str = "bossMarioGrow21";
    pub const BOSS_MARIO_GROW_30: &str = "bossMarioGrow30";
    pub const BOSS_MARIO_LIFE: &str = "bossMarioLife";
    pub const BOSS_MARIO_REVIVE: &str = "bossMarioRevive";

    // ── Boss：一拳超人 ────────────────────────────────────────────────────
    pub const SAITAMA_HUNGRY: &str = "saitamaHungry";
    pub const SAITAMA_LEAVE: &str = "saitamaLeave";

    // ── Boss：史莱姆 ──────────────────────────────────────────────────────
    pub const SKL_SLIME_SPAWN: &str = "sklSlimeSpawn";
    pub const SKL_SLIME_SPAWNED: &str = "sklSlimeSpawned";

    // ── Boss：尤里 ────────────────────────────────────────────────────────
    pub const SKL_YURI_CONTROL: &str = "sklYuriControl";

    // ── 武器 ──────────────────────────────────────────────────────────────
    pub const WEAPON_DEATH_NOTE_ATK: &str = "weaponDeathNoteAtk";
    pub const WEAPON_R_MODIFIER_USE: &str = "weaponRModifierUse";
    pub const WEAPON_S11_0: &str = "weaponS11_0";
    pub const WEAPON_S11_1: &str = "weaponS11_1";
    pub const WEAPON_S11_2: &str = "weaponS11_2";
    pub const WEAPON_S11_DONE_1: &str = "weaponS11Done1";
    pub const WEAPON_S11_DONE_2: &str = "weaponS11Done2";
    pub const WEAPON_S11_DONE_3: &str = "weaponS11Done3";

    // ── 实力评估 ──────────────────────────────────────────────────────────
    pub const BENCHMARK_RATIO: &str = "benchmarkRatio";
    pub const BENCHMARK_SCORE: &str = "benchmarkScore";
    pub const BENCHMARK_SKILL: &str = "benchmarkSkill";
    pub const BENCHMARKING: &str = "benchmarking";
}

// ─── 测试 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_known_keys() {
        assert_eq!(get_lang("sklAbsorb"), "[0]发起[吸血攻击]");
        assert_eq!(get_lang("sklFire"), "[0]使用[火球术]");
        assert_eq!(get_lang("damage"), "[1]受到[2]点伤害");
        assert_eq!(get_lang("recover"), "[1]回复体力[2]点");
        assert_eq!(get_lang("bossName_covid"), "新冠病毒");
    }

    #[test]
    fn test_get_missing_key() {
        assert_eq!(get_lang("不存在的键"), "");
        assert_eq!(get_lang(""), "");
    }

    #[test]
    fn test_format_lang() {
        let result = format_lang("damage", "A", "B", "100");
        assert_eq!(result, "B受到100点伤害");
    }

    #[test]
    fn test_all_keys_constants() {
        // 确保每个 keys 常量都能在 LANG_DATA 中找到对应条目。
        for (k, v) in LANG_DATA {
            assert_eq!(get_lang(k), *v, "键 {:?} 查询结果不一致", k);
        }
    }

    #[test]
    fn test_no_duplicate_keys() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for &(k, _) in LANG_DATA {
            assert!(seen.insert(k), "LANG_DATA 中存在重复键: {:?}", k);
        }
    }
}
