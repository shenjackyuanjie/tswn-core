# CPU 搜索实验版

搜索入口 `CombatRuntime::run_search_round`，命令行 `clever-search`。默认战斗接口仍使用原版策略；Clever 展示页可显式启用搜索策略，其他默认战斗接口保持原版策略。

## 决策规则

- 保留强制行动、行动前后状态和正式结算流程。在完整回合起点克隆战场，不直接拼接伤害结果。
- 每个正熟练度主动技能独立抽取一次资格；保留原技能概率及武器特例，关闭原智慧低血阈值等战术偏好。MP 门槛仍为 8～23，最终选择技能时扣费。物理和可负担的魔法普攻也参与比较。
- 常见单体攻击（火球、雷击、吸血、毒、会心）和治疗枚举所有合法阵营的存活目标；其余技能暂沿用原有选目标过程。Boss、cards、effective_skills 暂退回原策略。
- 一步模拟按存活单位、HP 比例、输出属性、MP、行动条评分，保留 shortlist 个候选。所有入围候选使用相同的独立搜索种子集合续局，模拟不能读取真实战斗未来随机序列。
- 续局采用原版策略，禁止递归搜索。终局奖励为己方胜 +1、负 -1；未终局使用半幅启发式值。返回 value 是混合评价，不是校准胜率；wins/completed/samples 单独记录。
- 当前评价将其他队伍合并计入敌方资源，主要用于两队研究；多队博弈、完整状态估值和所有技能目标枚举仍待扩展。尚未证明相对原版有统计显著的胜率提升。

## SSH 服务器运行

需要 Rust 工具链和系统编译工具，在仓库根目录执行：

```bash
cargo build --release -p tswn_core --bin clever-search --features 'no_debug,mimalloc_alloc'
./target/release/clever-search --input roster.txt --matches 256 --workers 128 \
  --samples 16 --horizon 128 --shortlist 4 --seed 1 > search.jsonl
./target/release/clever-search --legacy --input roster.txt --matches 256 --workers 128 \
  --seed 1 > legacy.jsonl
```

输入沿用普通名字文本，空行分队；命令行用 seed 和对局编号派生显式战斗种子，忽略输入中的 seed 行。每行输出一场结果，线程完成顺序不固定，按 match 对齐。默认双方均搜索；可加 `--search-team 0` 或 `--search-team 1` 只接管指定运行时队伍，另一队使用原版。运行时队伍编号不保证等于输入队伍位置，严谨评测还需按 input_groups 映射并交换输入位置。legacy 运行双方均使用原策略。

先用 1/8/32/64/128 workers 测吞吐和内存，再选择服务器并发。128 核不意味着应在一个决策内部开 128 个线程；当前并行单位是独立对局，每个工作线程内部串行搜索，避免线程嵌套。

示例参数仅是起点，没有在远程机器测性能。先以 2 场、2 samples、16 horizon 冒烟，再增加预算。horizon 以引擎推进次数计，包含可能没有行动的调度轮，不等于角色出手次数。max-rounds 截断通过 truncated 标记，不能计为负场。

## 后续验收

使用单方搜索/单方原版的对照、交换队伍位置、独立名单和 seed 集，报告胜率区间、截断率、吞吐、内存与每次决策耗时。接入前端或训练网络前，需完成这一步。

## Clever 网页与发布来源

- 前端入口：https://kodu16.github.io/namerena-clever/ 。默认选择 Clever 搜索，可在输入面板切换原版策略。双方搜索预算为 16 samples、128 horizon、4 shortlist，模拟 seed 固定为 1；对照时输入相同的显式 seed 行。分享链接保留策略。
- 网页通过 WASM BattleSession.set_search 接入同一 run_search_round，每次只推进真实回合，试算不计入回放或回合上限。搜索耗时随阵容增长，尚未验证策略整体胜率优于原版。
- 发布源固定为本项目 KODU16/tswn-core-clever 的 main；不自动拉取或合并 shenjackyuanjie/tswn-core。后续修改应从这里验证、提交并发布至 Clever main。
- KODU16/kodu16.github.io 的 Pages 工作流每 5 分钟检查 Clever main，对比线上 build.json，只在版本变化时重建，WASM 和前端取同一提交。GitHub 定时任务可能延迟；手动运行主页部署工作流可立即同步。

## !test! 有限评分测试

```text
!test!

alpha
```

输入面板的策略决定全部 10000 场 profile 对局的战斗方式：Clever 使用与回放一致的 run_search_round 和 16/128/4 预算；原版使用原有策略。只在累计 1000 场（10%）和 10000 场（100%）展示结果，随后永久停止该会话。Profile 名单、空战斗 seed 和 WIN_RATE_EVAL_RQ 沿用现有评分接口；旧 score() API 仍为原版评分。

技能／动作频率沿用参考站口径：被测本体产生的正 score、含 [0] 的真实消息次数，除以已完成场数再乘 100。不计 profile、召唤物、搜索试算及兼容层的 [s_dmg0] 伤害消息。这是每百局发生次数，不是每次出手的发动概率，可以超过 100%。Worker 支持立即取消正在计算的对局。

参考：https://github.com/namerena/namerena.github.io/blob/master/md5.js 的 V.eQ 与 lang/zh.json。本实现按需求设置固定上限，不继续进入参考站的扩大样本循环。
