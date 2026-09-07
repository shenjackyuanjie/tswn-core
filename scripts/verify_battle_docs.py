"""Run executable examples extracted from docs/public_api.md against local artifacts."""
from pathlib import Path
import os
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
text = (ROOT / 'docs/public_api.md').read_text(encoding='utf8')
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
subprocess.run(['cargo', 'run', '--offline', '--quiet', '--manifest-path', str(smoke / 'Cargo.toml'),
                '--target-dir', str(ROOT / 'target')], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
env = {**os.environ, 'PYTHONPATH': str(ROOT / 'target/py_cli_api_verify/import')}
subprocess.run([sys.executable, '-c', blocks['python']], cwd=ROOT, env=env, check=True)
js = '''import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const wasm = require('./target/battle_wasm_node/tswn_wasm.js');
''' + blocks['js']
subprocess.run(['node', '--input-type=module', '-e', js], cwd=ROOT, check=True)
print('PASS: public API Rust/Python/WASM documentation examples')
