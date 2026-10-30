# Runtime 分配计数探针

独立小型 Cargo workspace，不增加生产 workspace 依赖。包装同版本 `mimalloc::MiMalloc`，core 关闭默认 allocator feature，保留 `no_debug,png_render`；release 使用 fat LTO、单 codegen unit。

```sh
cargo build --locked --release --manifest-path tools/perf-allocation-probe/Cargo.toml
tools/perf-allocation-probe/target/release/allocation-probe --input INPUT85 --start 0 --runs 1000 --threads 1
tools/perf-allocation-probe/target/release/allocation-probe --input STABLE32 --start 2000000 --runs 1000 --threads 1
```

Windows 可执行文件带 `.exe`。若设置 `CARGO_TARGET_DIR`，相应调整二进制路径。跨版本比较使用同一源码、锁文件、编译器、flags、输入、seed 起点与线程数，仅修改 core 路径。

输入按 `perf_runtime` 相同规则：单文件或非递归排序的 `.txt` 目录，公开解析器分组，丢弃文本附带 seed，使用 batch 原始轮次 seed。start=0 调用完整公开胜率 API；非零 start 限单线程，在计数窗口内构造 prepared、调用区间 API 并释放 prepared。排除读取解析及 JSON 输出，包含准备、模拟与内部释放。

输出 JSONL 包含 alloc/zeroed/realloc/dealloc 次数、成功请求/释放字节、当前及峰值请求 live，并保留 total/wins/errors/guard。必须精确核对行为摘要后才比较数值；guard 非零照实报告。

限制：

- 不测延迟或吞吐；原子计数本身扰动执行。
- alloc_calls 包含 alloc_zeroed，realloc_calls 独立；成功请求才累加字节，失败 realloc 保留旧 live。
- realloc 计完整新请求字节，不是实际复制量；看不到分配器内部短暂双缓冲峰值。
- 请求 live/peak 不是 RSS，不含 mimalloc 保留页、元数据或绕过全局分配器的 C 分配。
- 多线程快照非事务，推荐单线程；peak_extra 是相对窗口初始 live 的差，不是严格独立所有权图。
- 净 live 为零只描述该窗口，不是一般性无泄漏证明。
- 原始 input 字段可能包含本机路径，公开报告仅保留审核后的 basename 和派生数值，不上传原始日志或主机信息。
