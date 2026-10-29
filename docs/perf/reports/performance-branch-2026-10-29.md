# 无武器路径成本简报（2026-10-29）

## 同步与改动

本分支合入 main `3eea3322`（含 `25865902` 的属性修改器技能改写及身份复活/分身）；保留此前非 PGO 优化，不改 main，不改上游武器规则。

**已确认减少常驻空间和无效扫描；尚未证明稳定 E2E 提速，也没有做到绝对零开销。**

本轮保留两项结构优化：
- Rinick 分身的大技能快照按需 `Box`，没有该装备就不分配快照；不再让所有普通角色内嵌整张技能表。
- 普通反击结算后，仅在玄月确有本批次待发反击时扫描对应技能。保留反击顺序、RNG 与寄存器清理行为。

| 64位本机构建的类型大小 | 新 main 合入后 | 冷数据候选 |
|---|---:|---:|
| PreparedPlayer | 1216 B | 1080 B |
| PlayerTemplate | 2144 B | 1280 B |
| EntityRecord | 3776 B | 2912 B |

这不是“全部武器成本为零”：S11 初始化查找、小型运行态字段和部分分支仍保留。两个公开 `rinick_clone_skills` Rust 字段由 `Option<T>` 改为 `Option<Box<T>>`，构造方需相应 `Box::new`；`PlayerRuntime` 字段/序列化不变，不引入 unsafe。

## 实测与限制

Rust 1.99.1、Xeon Platinum 8573C；release/fat LTO、默认 mimalloc、`no_debug`，无 PGO/额外 RUSTFLAGS。每版本独立 target，核验源码及二进制 SHA；串行绑定一个 CPU，预热不计入，顺序交替/轮换，核验输出与统计一致。初次共享 target 误复用产物的运行已排除，不能作为版本差异证据。

有效的旧版本对照中，早期武器运行态扩展（`3c540d41 → 7f6a7b17`）没有稳定的全局降速证据；最近两次身份移植（`e94a0f47 → 合入3eea`）在普通16例观察到吞吐中位 −3.79%，但同码 A/A 也出现约2%偏差，不能泛化成固定回退率。

下表 Runtime 使用 API 墙钟（含初始化/战斗，不含输入读取/JSON输出），namer-pf 使用完整 CLI 墙钟；CPU时间另行标注。

| 组合改动相对最新合并基线 | 吞吐中位变化 | 说明 |
|---|---:|---|
| 普通主集16例，5轮 | +1.31% | MAD约4.5个百分点，不能确认稳定收益 |
| 独立留出8例，3轮 | −17.67% | 不利观察保留，不能直接忽略 |
| 同8例五方平衡分解，5轮 | +0.20% | 组合CPU约+0.28%、MAD约7.6个百分点；未稳定复现上述回退 |
| namer-pf 12配置，5轮 | +2.19% | 分项有正有负，不能宣称四项均加速 |

五方分解同时比较基线、仅Box、仅guard、组合及**同码基线副本**；同码副本CPU也出现−4.17%中位假差异，足以说明该机器不足以精确断言小幅提速。仅Box/guard的CPU中位约−1.33%/−4.90%，也不能脱离噪声解释。这里保留的是可验证的空间/工作量减少，不是“5% E2E达标”。

主集16个普通无武器布局（FFA4/6/8/12/16及团队），独立留出8个新roster；每例13000场。namer-pf取1/4/8名、pp/pd/qp/qd共12配置，每例5000次。另有3个Generic武器输入，不能代表全部特殊武器。中位数/MAD不是置信区间，保留不利结果，不宣称所有场景达到5%。PC采样确认主要时间仍在回合推进、目标选择、初始化及hook调度，但有效样本稀疏，不能据此精确归因百分比。

## 验证与上游缺口

- 默认核心703、no_debug核心698、CLI91、corpus124通过；nightly格式检查通过。27个普通/留出/Generic输入的 normalized trace 与基线逐字节一致。
- workspace：1023通过、5失败、7 ignored；失败为DS4三组golden；精确上游`3eea3322`独立复现同5项，首个断言的字节数组经CRLF归一化后一致，未改golden掩盖问题。不同测试调用有重叠，不合计为唯一用例数。
- S11、死亡笔记、玄月及Rinick公开输入在基线/候选均因缺handler而 `NotReady`，没有冒充可运行的完整武器E2E覆盖。Rinick字段遗漏也在精确上游与合并版本的公开分步初始化中复现：复活属性/分身快照未复制，生命次数为空；本轮不擅自补武器功能。特殊武器的本轮行为覆盖主要依靠现有/新增单元测试。

## 复现入口

```bash
# 每个源码 worktree 必须有独立 target；先核对 git rev-parse HEAD 与 sha256sum。
CARGO_TARGET_DIR="/tmp/tswn-$REV" cargo build --locked --release -p tswn_core \
  --features no_debug --bin tswn-cli --example perf_runtime
python scripts/compare_perf_runtime.py --a "$BASELINE" --b "$CANDIDATE" \
  --input "$INPUTS" --runs 13000 --rounds 5 --out "$OUT"
cargo +nightly fmt --check
cargo test --locked -p tswn_core
cargo test --locked -p tswn_core --features no_debug
cargo test -p tswn_test --features runtime-corpus --test runtime
```

普通矩阵来自 `scripts/generate_hook_perf_matrix.py` 的 plain 家族；留出盐为 `tswn-cold-weapon-heldout-20261029`，取FFA4/8/16及4v4、每种两个roster。仅保留本简报，不提交逐轮表、环境日志或采样目录。此前性能记录仍可在 `9aa6dbc3` / `befd56a7` 历史提交追溯，不能当作当前 main 的实测保证。

## 同日追加：合并剩余 Rinick 快照

在已发布 `43c31234` 上，把构建态/模板的复活属性与分身技能合成一个按需分配的冷载荷；普通角色不再内嵌 `Option<[u32; 8]>`。实测 PreparedPlayer **1080→1048 B**、PlayerTemplate **1280→1240 B**、EntityRecord **2912→2872 B**；PlayerRuntime 仍为360 B。`rinick_reraise_lives` 保留：单独装箱在64位仍占8 B，只会多一次分配；统一武器运行态冷载荷不属本次改动。

公开 Rust API：旧字段 `rinick_revive_attrs` / `rinick_clone_skills` 改经 `rinick: Option<Box<PreparedRinickData/RinickTemplateData>>` 的 `revive_attrs` / `clone_skills` 访问，内部两项仍独立可选，技能不再二次装箱。保留深克隆、自定义部分载荷语义、运行态 serde 形状；未顺带修复上文上游武器缺口。

同 Rust 1.99.1 / release / no_debug、独立 target 和不同二进制 SHA、无PGO，8个新普通布局（FFA4/6/8/12/16、2v2/4v4/3v3v3），每例13000场、1轮预热+5轮交错配对：总吞吐中位 **+1.30%**（耗时变化−1.28%，耗时MAD0.69个百分点）；FFA **+1.19%**，团队 **−1.20%**；单例范围 **−2.01%～+3.82%**。仅3/5轮总体更快，所有统计签名一致；小幅结果不能证明稳定提速，确定收益是上述空间减少。输入用 `generate_hook_perf_matrix.build_cases()`，盐改为 `tswn-rinick-cold-20261029`，取plain家族每布局第一例；比较命令同上。没有重跑前文旧实验，其数字仅保留历史范围。

本次默认核心706、no_debug核心701、CLI91、integration4、corpus124、doctests6通过；nightly格式检查通过。新增回归覆盖部分载荷、导入与深克隆。完整workspace：1026通过、5失败、7 ignored；同5项DS4失败已在基线 `43c31234` 的独立target复现，首个断言左右字节数组逐项一致，未修改golden。最初workspace构建因临时盘满中断，迁移到有空间的工作区后已完整重跑。
