# tswn_core

名字竞技场的 Rust 核心引擎。

## 功能

- **战斗模拟**：解析 namerena 输入格式，完整模拟回合制对战
- **胜率统计**：支持单线程/多线程批量胜率计算，含 JS profile seed 调度
- **CLI 工具**：`tswn-cli` 提供 bench、win-rate、fight 等子命令
- **DIY 转换**：`tswn-cli to-diy -r NAME` 默认输出 `+ol`，加 `--old` 输出 `+diy`，也支持 `-f/--file` 和 `-o/--out-file`
- **图标渲染**：玩家名称 → 16×16 像素头像（PNG / Base64 / RGBA）
- **结构化回放视图**：`replay_view` 提供跨 WASM / Python / C 等包装层复用的分行、分帧、延迟、文本片段、血条和死亡特效规则
- **DS3 兼容**：通过 `tswn_ds3` crate 提供 DS3_demo3 流程兼容

## 快速开始

### CLI

```powershell
# 构建
cargo build -p tswn_core --bin tswn-cli --release

# 胜率基准测试
./target/release/tswn-cli bench win-rate --total 100000 --perf

# 单局对战（stdin 输入）
echo '<your raw input>' | ./target/release/tswn-cli fight

# fight/diff/raw/bench 均使用主 Runtime
./target/release/tswn-cli fight -f input.txt
./target/release/tswn-cli diff -f input.txt
./target/release/tswn-cli raw -f input.txt
./target/release/tswn-cli runtime normalized-run -f input.txt --max-rounds 20000

# DIY/OL 导出
./target/release/tswn-cli to-diy -r "mario@team+fire"
./target/release/tswn-cli to-diy -r "mario@team+fire" --old
./target/release/tswn-cli to-diy -f names.txt -o diy.txt
./target/release/tswn-cli to-diy -f names.txt --minions -o diy.txt

# 批量胜率输出筛选与导出格式
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt --min-screen 60
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt -o out.txt --min-file 65
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt -o out.jsonl --log
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt -o names.txt --pure
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt --wr-precision 5
./target/release/tswn-cli bench batch-rate -l weighted-targets.toml -p players.txt --target-factored

# namer-pf 四项评分；--precision 默认 0，控制分数输出的小数位数
./target/release/tswn-cli namer-pf -r "mario\nluigi" --mode pp qd --precision 2

# 二人组队友筛选；player-list 和 teammate-list 都是每行一个名字
./target/release/tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3
./target/release/tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 5 -o pair.txt --min-file 250
./target/release/tswn-cli bench pair -l weighted-targets.toml -p players.txt --teammate-list teammates.txt --head 3 --target-factored
```

`raw` 输入以 `!test!` 开头时会进入主 Runtime 批量评分/胜率路径。CLI 不再提供 `--runtime` 执行器选择器或 `runtime parity`；独立 `bench` 也已迁移到同一数据模型。

`to-diy --minions` 会额外导出 shadow / summon / zombie 模板。OL/DIY 的 `attrs` 都使用前七围 +36、HP 原样的编码；summon 的两个火球分别用 `sklfire1`、`sklfire2` 表示，自爆用 `sklexplode`，`skills` 保持普通 JSON object 形态，字段顺序就是行动顺序。0 熟练度技能会省略输出，解析时未带前缀的 `summon.skills` 只接受这三个 `skl` 槽位名。

OL 召唤物模板支持继续嵌套 `shadow` / `summon` / `zombie` 子模板，用于配置“召唤物的召唤物”。给使魔模板配置普通玩家技能时需要显式写 `normal:` 前缀，例如 `{"normal:sklsummon":255,"sklfire1":9}`；这样普通技能、使魔固定技能和幻影附体会保留在不同技能编号通道中，吞噬时也不会互相串槽。使魔召唤出的子使魔会按直接来源链路传导伤害；使魔分身仍沿用 root owner 命名/清理规则，但伤害分摊会直接传到主名字。

`bench pair` 会为每个选手组合与每个队友组合组成对局，分别计算 batch rate，并取最高的 `--head <N>` 个 batch rate 求和作为最终分数。选手每行默认按 `+` 分隔，使用 `--player-list-double-plus` 时改按 `++`；队友每行默认按 `++` 分隔，使用 `--teammate-list-single-plus` 时改按 `+`。选中带权靶子时加 `--target-factored`，即可按 TOML 中的 `factor` 计算加权平均。

### 作为库使用

```toml
[dependencies]
tswn_core = { path = "crates/tswn_core" }
```

```rust
use tswn_core::Runner;

let mut runner = Runner::new_from_namerena_raw(raw_input).unwrap();
let summary = runner.run_to_completion(20_000);
```

根级 `Runner` / `PreparedRunner` 是主 Runtime 的稳定入口。0.5.0 已删除旧
`engine` / `player` 对象模型及其兼容别名；名字解析、属性与 overlay 数据位于
`namerena`，运行期实体、调度和更新类型位于 `runtime`。

`tswn_core::replay_view` 暴露公共的 replay view 构建结构：一个 frame 包含多行 `ReplayRow`，
一行包含多个 `ReplayClip`。clip 只承载播放与布局信息：展示前 `delay`、结构化文本 `parts`、`[]`
高亮文字颜色码 `color`、语义分类 `tone`、关联玩家 id、侧栏状态快照和胜利片段标记。
文本、玩家、数值、HP 条、死亡特效与 emoji 占位字段都下沉到 `ReplayTextPart`。包装层只需要把
自己的玩家快照类型实现 `ReplayState`，即可复用同一套回放推演规则。
同一句包含多个玩家时，每个玩家 part 独立携带 HP 前后值；生命之轮互换会强制双方都展示血条，即使某一方或双方 HP 没有变化。
复活提示会先将目标的回放状态重置为 `0`，因此“复活”句为 `0 -> 0`，紧随其后的回复句才产生
`0 -> x` 的血条变化，即使运行时帧快照仍携带此前动作的 HP。“`[2]变成了[1]`”等实体转化句会将
旧对象和新对象都输出为 `player` part，调用方可对旧对象继续应用普通名字、头像和编号显示。

当前 delay 规则按优先级依次为：frame 首句 `900ms`，雷击/地裂行首句 `150ms`，展示血条的句子
`600ms`，其他句子 `500ms`。血条只在帧前后 HP 不同且该玩家 part 没有死亡特效时展示；死亡特效只在“被击倒”或“消失”等死亡句且该句后 HP 为 `0`
时渲染，所有带死亡特效的句子都不展示血条；附体、自爆、owner 死亡牵连等机制死亡会在死亡句同步为 `0`。分身展示序号由 Runtime minion handler 提供：本体为 `0`，
后续同名分身为 `1`、`2`……，供上层在名字内展示；唯一对象编号仍使用玩家 id。
默认 `[]` 高亮文字颜色为 `0077BB`；解除、识破、中止、打消等状态离开消息使用 `bb7700`。普通文本不使用该颜色码。

## 构建配置

| feature      | 默认 | 说明                            |
| ------------ | ---- | ------------------------------- |
| `png_render` | ✅   | PNG 图标渲染支持                |
| `no_debug`   | ❌   | 关闭调试输出，用于 release 分发 |

## 测试

```powershell
cargo test -p tswn_core
python scripts/check_runtime_release.py --corpus
```

第二条命令会检查主 Runtime 独立性并执行 87 个 JS exact trace 与 37 个冻结压力 golden。

## 版本

当前版本见 [CHANGELOG.md](./CHANGELOG.md)。
