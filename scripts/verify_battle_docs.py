"""提取公共 API 文档中的 Rust、Python、WASM 示例并实际执行。

Rust 示例在 target/battle_docs_smoke 生成临时 crate，并离线运行；Python 使用
target/py_cli_api_verify/import，WASM 使用 target/battle_wasm_node 中的 Node 包。
后两者需提前构建且与源码一致；脚本不安装依赖，也不提供 --help 参数。
"""
from pathlib import Path
import os
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
text = (ROOT / 'docs/reference/public-api.md').read_text(encoding='utf8')
blocks = dict(re.findall(r'```(rust|python|js)\n(.*?)\n```', text, re.S))
smoke = ROOT / 'target/battle_docs_smoke'
(smoke / 'src').mkdir(parents=True, exist_ok=True)
(smoke / 'Cargo.toml').write_text('''[package]
name = "battle-docs-smoke"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
tswn_core = { path = "../../crates/tswn_core" }
''', encoding='utf8')
(smoke / 'src/main.rs').write_text(blocks['rust'], encoding='utf8')
# 保留 Rust 示例输出用于失败排查，也避免依赖受限 Windows 环境中的 NUL 设备。
with (smoke / 'rust.stdout.log').open('w', encoding='utf8') as output:
    subprocess.run(['cargo', 'run', '--offline', '--quiet', '--manifest-path', str(smoke / 'Cargo.toml'),
                    '--target-dir', str(ROOT / 'target')], cwd=ROOT, check=True, stdout=output)
env = {**os.environ, 'PYTHONPATH': str(ROOT / 'target/py_cli_api_verify/import')}
subprocess.run([sys.executable, '-c', blocks['python']], cwd=ROOT, env=env, check=True)
js = '''import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const wasm = require('./target/battle_wasm_node/tswn_wasm.js');
''' + blocks['js']
subprocess.run(['node', '--input-type=module', '-e', js], cwd=ROOT, check=True)
print('PASS: public API Rust/Python/WASM documentation examples')
