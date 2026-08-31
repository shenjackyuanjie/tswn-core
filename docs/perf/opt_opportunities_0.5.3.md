# 0.5.3 性能优化评估与落地记录（tswn_core）

> 本文只覆盖 `crates/tswn_core` 的战斗/批量热路径。所有数字都是本机实测，
> 包含**负面结果**和一次**测量方法翻车**的记录，避免后来者重复踩坑。

## 1. 环境与方法

- OS：Windows 11，AMD Ryzen 7 5800X（8C/16T）
- rustc 1.98.0（LLVM 22.1.8），`--release --features no_debug`（fat LTO / codegen-units=1 / mimalloc）
- 口径：`tswn-cli bench win-rate -f <input> -n <N> -s`，**同一会话内交替顺序**跑各个二进制，取中位数
- 样本：
  - `aaa` vs `bbb`（简单 1v1）
  - `喘际瞬爆@昀澤` vs `蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall`（复杂 1v1）
  - `docs/perf/fixed_cases_30` 第 29 号 case 改写成两队格式（2v2）

### 1.1 本机采不了样，改用 PGO 计数

`samply` 底层是 xperf/ETW 内核采样，需要管理员权限；非提权账户下它能跑完，
但产出的 profile 是空的（`threads: []`），`wpr` 同样要提权。

替代方案：用 **PGO instrumentation** 拿函数级执行计数——
`-Cprofile-generate` 构建 → 跑训练输入 → `llvm-profdata show` 统计。
它给的是执行次数而不是时间，但"哪些代码被反复执行"这件事比采样更精确，
而且顺带把 PGO 本身的收益量出来了。

结构体体积用 `cargo +nightly rustc -p tswn_core --release --lib -- -Zprint-type-sizes`。

### 1.2 ⚠️ 噪声带：本机 ±1.5%，不同会话之间可差 3%

这一轮最重要的教训。同一个二进制、同一条命令：

- 会话 A（`--perf`）：`5.266 / 5.303 / 5.398` s
- 会话 B（无 `--perf`，计算路径完全相同）：`5.103 / 5.141 / 5.226` s

**同一个二进制在两次会话之间差了约 3%。** 我最初用"先跑旧版三次，再跑新版三次"
的方式量到的 `-2.3% ~ -3.5%`，改成同会话交替 A/B 之后全部塌回 `±0.7%`。

结论：

- 单次比较必须**同会话交替顺序**跑，且至少 4~5 轮取中位数；
- 小于 **1.5%** 的差异不要写进任何长期表格，也不要作为"优化成功"的依据；
- 只有 PGO 这种 20% 量级的收益才可以用粗糙的方式确认。

## 2. 最终结果

同会话交替 A/B，60 万场单线程，4 轮取中位数：

| 样本 | 基线 | 位图优化后 | PGO |
| --- | ---: | ---: | ---: |
| `aaa` vs `bbb`（非训练集） | `3.179s` | `3.160s`（-0.6%） | `2.337s`（**-26.5%**） |
| 复杂 1v1（非训练集） | `4.949s` | `4.891s`（-1.2%） | `3.713s`（**-25.0%**） |
| 2v2 | `12.171s` | `12.163s`（-0.1%） | `9.538s`（**-21.6%**） |

三个构建在 13000 场上胜率完全一致（`48.99% (6369/13000)` / `52.41% (6813/13000)`）。

### 已落地

| 提交 | 内容 | 实测 |
| --- | --- | --- |
| `perf(build)` | `scripts/pgo_build.py` PGO 流水线 | **-21.6% ~ -26.5%** |
| `perf(core)` | 批量胜率/评分默认不再逐场计时（const 泛型 `TIMED` + `_timed` 入口） | 单线程持平，多线程 -1.2% |
| `perf(core)` | `state_hook_plan` 不再按 key 反查注册顺序（去掉 O(n²)） | 噪声内，属实现缺陷修复 |
| `perf(core)` | `StatePayloadKind` 位图做状态查询的快速否定 | -0.1% ~ -1.2% |

## 3. PGO：目前唯一的量级收益

```powershell
python scripts/pgo_build.py                    # 全流程
python scripts/pgo_build.py --train-runs 8000  # 加大训练量
python scripts/pgo_build.py --skip-train       # 复用已有 profdata 只重建
```

脚本要点：

1. 自动查找 `llvm-profdata`（PATH 或 `rustup component add llvm-tools-preview`），
   并**强制其 LLVM 大版本等于 rustc 的 LLVM 大版本**——版本不匹配会读出坏 profile，
   所以直接报错而不是继续；
2. 训练输入 = `docs/perf/fixed_cases_30` 全部 30 个 case（1v1 / 2v2 / ffa / 3v3v3）
   加 `docs/perf/pgo_training/score.txt`（score 路径），覆盖面比只训练 `aaa/bbb` 好得多；
3. 训练**强制单线程**：LLVM 的 IR 插桩计数器不是原子的，多线程训练会丢计数；
4. PGO 构建默认清掉 `RUSTC_WRAPPER`（插桩产物不该进编译缓存，且 wrapper 在
   `RUSTFLAGS` 变化时容易超时）。

泛化性验证：`aaa/bbb` 和复杂 1v1 都**不在训练集里**，收益仍有 -25% ~ -26.5%，
不是过拟合。用只训练 `aaa/bbb` 的旧 profile 时，非训练输入也有 -18% ~ -22%。

对照：`-Ctarget-cpu=x86-64-v3` 单独用是 ±0% / -1.5%，**不值得**牺牲可移植性。

PGO 能拿到 25% 这个事实本身说明：**当前热路径的瓶颈是代码布局、内联决策和分支
预测，而不是算法**。下面第 5 节剩下的候选大多是在"帮编译器做 PGO 已经在做的事"，
上了 PGO 之后它们的边际收益还会进一步缩水。

## 4. 热点数据（PGO 计数，40 万场合计）

`llvm-profdata show --topn` 的"最大内部基本块计数"。入口即热块的函数≈调用次数，
含循环的函数≈循环体执行次数。

| 函数 | 计数 | ≈每场 |
| --- | ---: | ---: |
| `RC4::new_with_key_schedule_prefix`（KSA 内层） | 98.8M | ~247（=1 次 KSA） |
| `CombatRuntime::scan_plain_action_skill_probabilities` | 37.1M | 93 |
| `StateStore::effective_speed` | 24.4M | 61 |
| `WorldArena::next_actor` / `StateStore::apply_ice_pre_step` | 24.3M | 61 |
| `PhaseScheduler::skill_hook_plan` | 21.1M | 53 |
| `PreparedBattleRoster::refill_seed_state` | 20.4M | 51 |
| `SkillLoadout::cached_hook_entries` | 19.5M | 49 |
| `PhaseScheduler::state_hook_plan` | 19.0M | 48 |
| `RC4::pick_skip_range` / `encrypt_bytes_no_change` | 16.3M / 13.6M | 41 / 34 |
| `CombatRuntime::lazy_boss_at_boost` | 11.6M | 29 |
| `RunUpdates::add` + `drop_glue::<RunUpdate>` | 8.9M + 8.9M | 22 + 22 |
| `SmallVec<[(..,DefendHookPlanEntry);8]>::try_reserve` | 8.7M | 22 |
| `drain_effects_into` | 6.9M | 17 |
| `has_alive_enemy_or_pending_spawn` | 5.8M | 15 |
| `plain_effective_team` | 5.1M | 13 |

关键类型体积：

| 类型 | 大小 | 说明 |
| --- | ---: | --- |
| `QueuedEffect` | 1264 B | 3 个 Spawn 变体内联 `PlayerTemplate`(1232B)；`Damage` 只要 20B |
| `PlayerTemplate` | 1232 B | `skills: SkillLoadout` 872B、`clone_build` 160B |
| `SkillLoadout` | 872 B | 6 个 80B 的顺序表合计 480B |
| `StateStore` | 1208 B | `SmallVec<[StateEntry; 8]>` 内联 1088B |
| `StateEntry` | 136 B | 由 `StatePayload`(104B) 决定 |
| `StatePayload` | 104 B | `SaitamaBoss` 104B、`CovidInfection` 81B；常见的 `Ice`/`Haste`/`LazyBoss` ≤16B |

## 5. 还没做、值得继续评估的

按"预期收益 ÷ 风险"排序。注意第 1.2 节：这些都在 1%~3% 量级，**必须交替 A/B 验证**，
否则量出来的全是噪声。

### 5.1 丢帧模式仍在构造并丢弃 `RunUpdate`（22 次/场）

批量胜率走 `RunUpdates::new_no_capture()`，但 110 处调用点仍然是
`updates.add(RunUpdate::new(...))`：先把结构体造出来，再在 `add` 里丢掉。
其中 13 处（`attack.rs` 5、`infection.rs` 6、`effects.rs` 2）带 `format!`，
还有 `display_name.clone()`，在丢帧模式下是纯堆分配浪费。

试过只给 `add` 加 `#[inline]`：复杂样本 -1.6%、简单样本 +0.3%，也就是噪声。
原因是 `add` 在 no-capture 分支仍要读 `update.message` 做 `== "[0][防御]"` 比较，
LLVM 消不掉构造。

真正的做法是把消息构造闭包化：`RunUpdates::emit` 这个 API 已经存在但**当前 0 处调用**。
建议补一个 `add_with(caster, target, is_plain_defense, f: impl FnOnce() -> RunUpdate)`，
把 no-capture 需要的语义用显式参数传，并把 `"[0][防御]"` 字符串比较换成枚举标记。

### 5.2 hook plan 每个阶段都重建

`skill_hook_plan`(53/场) 与 `state_hook_plan`(48/场) 在每次行动的
PRE_ACTION / PRE_DAMAGE / POST_DAMAGE / POST_ACTION / POST_DEFEND 各重建一次 SmallVec。
`skill_hook_plan` 即使命中 `cached_hook_entries` 也要整表复制，只为过滤
`level_at(fixed_lane) != Some(0)`；可以在 `SkillLoadout` 里缓存"已过滤的 plan"，
用 `hook_generation` + levels 版本号失效。`state_hook_plan` 每次都排序，
可以让 `StateStore` 维持有序。

注意：hook 执行过程中会改写 loadout / 状态表，所以现在是先快照再执行，
改缓存要保证快照语义不变。

### 5.3 `drain_post_defend_hooks_into` 每次防御重建 528B 数组并排序

每次防御事件都新建
`SmallVec<[(SkillPriority, u8, usize, RegistrationOrder, DefendHookPlanEntry); 8]>`（528B），
extend 两次 + `sort_by_key` + `Vec::contains` 去重。绝大多数防御只有 0~1 个 skill hook
+ 0~1 个 state hook + 可选护盾，值得加一条 ≤2 条目的快路径。

### 5.4 `StateEntry` 136B / `StateStore` 1208B

`StatePayload` 被两个 boss 专用变体撑到 104B。把 `SaitamaBoss`、`CovidInfection` 装箱，
再把 `Charm` 的两个 `Option<usize>`(各 16B) 和 `Poison` 的 `Option<u32>`(各 8B) 换成
哨兵值，`StatePayload` 能压到 ~24B、`StateEntry` 到 ~48B、`StateStore` 到 ~500B，
每场的状态扫描和 `reset_battle_state_from` 拷贝量都会明显下降。

（本轮已经用 `StatePayloadKind` 位图拿掉了"扫描本身"，装箱只影响命中后的遍历成本，
所以优先级降低了。）

### 5.5 190 处 `unwrap_or_else(|| panic!(...))`

非测试代码里有 190 处，每处都在调用点展开一段 `core::fmt` 格式化代码。
它们永不执行，但会撑大 LLVM 的内联代价估算、挤占 I-cache。建议收敛成
`#[cold] #[inline(never)]` 的统一 helper，或给 `EntityArena` 加
`expect_get(idx)`。这一项和 PGO 是同一类收益，做了 PGO 之后会缩水，
但对不方便上 PGO 的下游构建（wasm、ohos、C API）仍然有效。

### 5.6 init 路径（占总时间约 18%）

每场一次 RC4 KSA（256 步）。`Rc4KeySchedulePrefix` 已经做了共享前缀跳过，
但 batch 的 key 形如 `seed:1000@!`，共享前缀很短，实际还是要跑 250 步左右。
`RC4::update_interleaved` 里已经有 2/3/4 路交错 KSA 的实现——如果批量路径愿意
一次准备 2~4 场的 seed 状态（胜率批量各场之间没有依赖），就能把这条串行依赖链
交错起来，这是 init 里唯一的量级机会。

## 6. 明确**不要**做的（实测反例）

**把 `QueuedEffect` 的 `Spawn`/`SpawnSilent`/`SpawnWithMessage` 改成 `Box<PlayerTemplate>`。**

理论上很诱人：`QueuedEffect` 从 1264B 降到 ~48B，`VecDeque` 的 stride 缩 26 倍，
`pop_front` 的 move 从 1264B 变 48B。完整改完实测：

| 样本 | 基线 | 装箱后 |
| --- | ---: | ---: |
| 简单 1v1 | `5.303s` | `5.415s`（+2.1%） |
| 复杂 1v1 | `8.086s` | `8.223s`（+1.7%） |
| 2v2 | `3.997s` | `4.005s`（±0） |

没有稳定收益，主口径上甚至更慢。效果队列每场只 drain 17 次、元素个数很少，
1264B×少量元素仍然待在 L1 里；换来的是多一次间接寻址和潜在分配。

> 补丁在 `target/exp1_boxed_spawn.patch`（`target` 不入库，需要时重做即可），
> 后续如果要在召唤密集场景重新评估可以参考。

**结论：本仓库已经把明显的算法/分配问题清理得差不多了，"结构体太大""有额外拷贝"
都不构成充分理由，剩下的候选必须一条条交替 A/B 实测。**
