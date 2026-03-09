# tswn-core v0.1.7 更新日志

> 提交范围: bb7b3f5..05ff54c

## 新功能

### 图标系统重构

- **图标生成算法**：完整复现 JS/Dart 的 `Sgl.createFromName()` 算法
  - 支持 `name@team` 格式解析（同队玩家共享图标）
  - RC4 密钥生成 + S 表映射 + 颜色选择
  - 颜色距离矩阵预计算（`OnceLock` 懒加载）
- **精灵数据**：新增 `src/player/sprite_data.rs`
  - 38 个前景形状 alpha 映射（每个 16x16 = 256 字节）
  - 8 个边框深色覆盖层
  - 8 个边框不透明度掩码
- **提取工具**：新增 `examples/extract_sprites.rs` 从 PNG 提取精灵数据

### CLI 工具增强

- **Benchmark 模式**：
  - `--bench [N]` — 自动检测模式（1组→评分，2+组→胜率）
  - `--bench-raw` / `--bench-file` — 支持命令行/文件输入
  - 评分测试：普通评分 + !评分
  - 胜率测试：team1 vs team2
- **图标生成**：
  - `--icon <名字>` — 显示图标信息 + ANSI 真彩色终端渲染
  - `--icon-b64 <名字>` — 输出 Base64 PNG data URL（需 `png_render` feature）
  - `--icon-path <目录> <名字>` — 保存 PNG 文件（需 `png_render` feature）
- **输入处理**：支持 `--raw`、`--file`、stdin 三种输入方式

### 玩家系统扩展

- **状态系统**：新增 `src/player/status.rs` - `PlayerStatus` 结构体
  - 完整属性：HP、MP、攻击、防御、速度、敏捷、魔法、抗性、智力等
  - Display trait 格式化输出
- **Boss 系统**：新增 `src/player/boss/mod.rs`
  - 4 种 Boss 类型：Covid、Lazy、Saitama、Generic
  - Boss 免疫阈值系统
  - Boss 默认行动逻辑
- **技能系统**：新增 `skill/act/mod.rs`、`skill/skl/mod.rs`、`skill/store.rs`
- **玩家实现**：新增 `impl_attr.rs`、`impl_ctor.rs`、`impl_runtime.rs`
- **其他**：新增 `weapons.rs` 武器系统、`eval_name.rs` 名称评估

## 改进

- **引擎优化**：简化 `engine/test/runner/large_41_45.rs` 测试代码
- **依赖更新**：Cargo.toml 新增 `png` 依赖用于精灵提取

## 统计

| 指标 | 数值 |
|------|------|
| 提交数 | 4 |
| 文件修改 | 23 |
| 新增行数 | +2083 |
| 删除行数 | -705 |
| 新增文件 | 12 |

## 新增文件列表

```
examples/extract_sprites.rs    - 精灵提取示例
src/player/sprite_data.rs      - 精灵数据常量
src/player/status.rs           - 玩家状态结构体
src/player/boss/mod.rs         - Boss 系统
src/player/eval_name.rs        - 名称评估
src/player/impl_attr.rs        - 玩家属性实现
src/player/impl_ctor.rs        - 玩家构造实现
src/player/impl_runtime.rs     - 玩家运行时实现
src/player/weapons.rs          - 武器系统
src/player/skill/act/mod.rs    - 技能行动
src/player/skill/skl/mod.rs    - 技能定义
src/player/skill/store.rs      - 技能存储
src/player/test.rs             - 玩家测试
src/player/icon_render/test.rs - 图标渲染测试
```
