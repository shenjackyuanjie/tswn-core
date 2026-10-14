# DIY 往返验证：当前工作流与历史记录

[返回指南索引](README.md)

本文验证普通名字导出为 DIY / OL 后能否保持构造与战斗行为。当前格式与限制见 [DIY / OL Overlay 参考](../reference/diy-overlay.md)，对局增量接口见 [公共 API](../reference/public-api.md)。

## 1. 当前工具状态

[track_diy_roundtrip.py](../../track_diy_roundtrip.py) 是保留的历史采样脚本，**尚未适配当前 CLI**：

- 对战比对仍调用已删除的 `fight --out-raw`；当前结构化战斗输出为 `fight --jsonl`。
- 转换仍把名字作为 `to-diy` 的位置参数；当前要求 `-r/--raw` 或 `-f/--file`。
- 初始状态仍通过旧人类可读输出的正则提取，并要求当前输出已不再打印的 `move_point` 字段。

所以脚本的 `--skip-fight` 也不能使当前初始状态验证恢复可用。先使用下文的现行命令与源码测试；历史采样结果单独保留，脚本适配属于后续工具工作。

## 2. 验证目标与层次

```text
原始名字与队伍 -> 构造 roster -> 原始八围 / 技能布局 / 加成元数据 / name_factor
        |
        +-> to-diy 导出 OL -> 保留原分组回读 -> 比较构造数据
                                      |
原始输入与回读输入分别运行 fight --jsonl -> 比较 initial / frame / result
```

验证时区分三层：

1. **格式与构造层**：原始八围、技能等级、行动顺序、boost 类型、队名与名字系数配置。OL 和紧凑 DIY 的前七围都以 +36 编码，HP 不偏移。
2. **展示状态层**：`initial` 事件的派生属性、HP、MP、`name_factor` 等。`BattlePlayerState` 不含完整技能 loadout，不能用这一层代替技能构造检查。
3. **战斗层**：逐帧 updates、状态、终止原因、胜者与推进次数。初始状态相同还不足以证明分身重建、行动顺序和召唤物行为相同。

源码层应对照 [PreparedPlayer 构造](../../crates/tswn_core/src/namerena/build.rs)、[SkillLoadoutSpec](../../crates/tswn_core/src/namerena/skill.rs)、[导出器](../../crates/tswn_core/src/cli_api/parse.rs#L83-L185) 与 [BattlePlayerState](../../crates/tswn_core/src/cli_api/battle/dto.rs#L40-L75)。

## 3. 当前 CLI 冒烟工作流

以下命令在仓库根目录的 PowerShell 中运行，使用无需外部号库的 1v1 小样本：

```powershell
cargo build -p tswn_core --bin tswn-cli
New-Item -ItemType Directory -Force target/diy_docs_smoke | Out-Null
$cli = './target/debug/tswn-cli.exe'

# 原始输入保留队伍间空行；此例没有武器后缀。
Set-Content -Encoding utf8 target/diy_docs_smoke/input_orig.txt "alpha`n`nbeta"

# 转换正文写入文件，避免 stdout 的欢迎 banner；按原分组回读。
& $cli to-diy -r alpha -o target/diy_docs_smoke/alpha.ol.txt | Out-Null
& $cli to-diy -r beta -o target/diy_docs_smoke/beta.ol.txt | Out-Null
$alpha = Get-Content -Encoding utf8 target/diy_docs_smoke/alpha.ol.txt
$beta = Get-Content -Encoding utf8 target/diy_docs_smoke/beta.ol.txt
Set-Content -Encoding utf8 target/diy_docs_smoke/input_ol.txt "$alpha`n`n$beta"

& $cli fight -f target/diy_docs_smoke/input_orig.txt --jsonl --max-rounds 20000 > target/diy_docs_smoke/fight_orig.jsonl
& $cli fight -f target/diy_docs_smoke/input_ol.txt --jsonl --max-rounds 20000 > target/diy_docs_smoke/fight_ol.jsonl
```

PowerShell 示例使用原生换行转义；也可以直接写多行字符串。`to-diy --no-details` 仅关闭属性附加详情，不关闭 CLI 欢迎 banner，所以机器回读应使用 `-o` 输出文件，见 [to_diy.rs](../../crates/tswn_core/src/bin/tswn_cli/to_diy.rs#L10-L28) 与 [main.rs](../../crates/tswn_core/src/bin/tswn_cli/main.rs#L75-L86)。自动化时请统一 UTF-8 输出，并检查每次 CLI 的退出码与 stderr。JSONL 外壳固定为 `{"type":"initial|frame|result","data":...}`，不是旧的 raw 文本；实现与逐行刷新测试见 [fight/driver.rs](../../crates/tswn_core/src/bin/tswn_cli/fight/driver.rs)。

> 文件批量导出使用 `to-diy -f names.txt -o diy.txt`，按非空行输出名字；原始战斗中的空行分组需要由调用方保留，不能把批量转换结果直接当作已保留队伍边界的战斗文件。需要模板时加 `--minions`；此选项只支持 OL，不能与 `--old` 同时使用。

### 比较什么

- JSONL 第一条应为 `initial`，最后一条应为 `result`；中间 `frame` 的顺序不可重排。
- 先按实体 ID 比较初始派生属性与 `name_factor`，再比较帧序列及结果。确有展示文本差异时，记录明确的忽略字段，不要一开始就广泛删字段。
- 另在构造层核对技能等级、非零技能行动顺序和 `SkillBoost`。导出器按行动顺序写 `skills`，省略 0 等级技能；回读时补齐固定槽位，不保证任意 40 槽位的完整顺序被逐项导出。
- 使用同一个 rq、相同的输入队伍边界和 seed。DIY 并不天然意味着“RNG 不同”；是否一致取决于原始身份、构造参数、顺序与 overlay 保留的信息，应比较实际输出。

## 4. 现有源码测试

```powershell
# 导出格式、准备后导出和 overlay / 武器后缀切分
cargo test -p tswn_core --lib cli_api::parse::tests

# 输入解析与召唤物模板构造
cargo test -p tswn_core --lib namerena::input::tests
cargo test -p tswn_core --lib namerena::build::tests

# 当前等级、构造基底与分身加成重建
cargo test -p tswn_core --lib skill_loadout_rebuilds_clone_levels_from_build_baseline_before_boosts
cargo test -p tswn_core --lib plain_clone_skill_tests
cargo test -p tswn_core --lib plain_raw_import_tests
```

这些是当前实现的定点覆盖，不等价于重新跑过所有历史号库 case。大样本工具恢复后，建议按 16 个冒烟、200 个小样本、2000 个中样本，再到更大规模递进，保留失败输入和逐层差异。

## 5. 差异分级与限制

| 现象 | 首要核对项 |
| --- | --- |
| 名字转换或回读失败 | `-r/-f` 参数、身份 / 队名 / quoted plus 的切分、overlay JSON 格式 |
| 原始七围差固定值 | 前七围 +36 / -36 编码与 HP 不偏移规则 |
| 展示属性不同 | `name_factor`、队名、rq、武器或其他构造修正；原始八围与派生属性不要混用 |
| 技能等级相同但分身不同 | 保存的 build 基底、LastBoost / SlotBoost 类型及 SlotBoost 上限 |
| 帧序列不同 | 非零技能行动顺序、seed、召唤物模板、武器行为与终止预算 |
| 提前截断 | 检查 `result.stop_reason`；`max_rounds` / `no_progress` 是保护性终止，不是正常获胜 |

当前确定的限制与边界：

- **武器行为不能完整还原**：提供 attrs 或 skills 的 DIY / OL 会关闭武器构造，导出器也不输出武器信息。即使八围可被保存，武器技能或运行期行为仍可能不同，见 [build.rs](../../crates/tswn_core/src/namerena/build.rs#L274-L294)。
- **加成由当前元数据表示**：`SkillBoost` 保留 Normal / LastBoost / SlotBoost；原生构造在技能已有 boosted 标记时跳过重复末端加成，见 [build.rs](../../crates/tswn_core/src/namerena/build.rs#L385-L408)。不要沿用旧 `Skill.diy_boost` 对象字段或把旧“双 boost 丢失”统计当作当前缺陷结论。
- **模板导出有范围**：`to-diy --minions` 导出当前可生成或显式配置的 shadow / summon / zombie 模板，并省略 0 等级技能；它不是任意运行期实体与状态的存档格式。

## 6. 历史验证记录

旧工作流记录过以下结果，使用当时的 CLI、输出格式和实现，保留为问题定位线索：

| 历史样本 | 当时记录 |
| --- | --- |
| sqp6000 对战比对 | 64 / 64 通过 |
| sqp5900 对战比对 | 498 / 500 通过（99.6%） |

这些数字不是当前版本的验收结果；本页不再宣称当前“全部通过”或 40 槽位完整 action order 往返。恢复采样脚本后，应重新生成报告，并记录提交版本、样本库、rq、seed、分组、忽略字段与命令。

旧脚本写入 `target/diy_roundtrip/summary.json` 及每 case 的输入、初始状态、对战日志和差异文件。这种留存结构仍有价值；迁移时应改用当前 JSONL / DTO，不再解析人类可读行。

## 7. 与 JS / Rust 对齐验证的关系

本文比较同一 Rust 实现中的原始名字与导出回读名字；JS / Rust 差分比较两种实现的行为。号库采样和失败定位方案见 [SBY Test](../archive/sby-test-plan.md)，历史差异复核工具见 [tswn_diff.py](../../scripts/README.md#tswn_diffpy)。冻结 JS exact trace 与压力 golden 的现行门禁见 [主 Runtime 迁移指南](runtime-0.5-migration.md#回归门禁)。两类验证分别提供往返保真和实现兼容性的证据。
