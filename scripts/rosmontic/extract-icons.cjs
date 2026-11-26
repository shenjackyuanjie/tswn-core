// 从固定参考文件提取原始图标，同时生成 Rust 可直接使用的 RGBA 像素。
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const cp = require('node:child_process');
const [input, python = 'python'] = process.argv.slice(2);
if (!input) throw Error('用法：node scripts/rosmontic/extract-icons.cjs MD5_JS [PYTHON]');
const bytes = fs.readFileSync(input);
const sha256 = crypto.createHash('sha256').update(bytes).digest('hex');
if (sha256 !== 'b7dc47862dc64b8ce9a2705b53d49784815f0ff9642235494918b983f3c888d7') throw Error('参考文件版本不符');
const source = bytes.toString('utf8');
const names = ['reimu','marisa','rumia','daiyousei','cirno','hong','koakuma','patchouli','sakuya','remilia','flandre','rin','letty','chen','alice','phantom','youmu','yuyuko','ran','yukari','AL1S','fox1','fox2','fox3','fox4','ribbit1','ribbit2','ribbit3','ribbit4','megia'].sort();
const root = path.resolve(__dirname, '../..');
const out = path.join(root, 'assets/rosmontic-boss-icons');
fs.mkdirSync(out, {recursive: true});
const icons = names.map(name => {
    const matches = [...source.matchAll(new RegExp(JSON.stringify(name) + ',\\s*"([A-Za-z0-9+/=]+)"', 'g'))].filter(m => m[1].startsWith('iVBOR'));
    if (matches.length !== 1) throw Error(name + ' 图标不唯一');
    const png = Buffer.from(matches[0][1], 'base64');
    fs.writeFileSync(path.join(out, name + '.png'), png);
    return {name, file: name + '.png', sha256: crypto.createHash('sha256').update(png).digest('hex')};
});
fs.writeFileSync(path.join(out, 'manifest.json'), JSON.stringify({source:'https://github.com/Rosmontic/namerena/blob/2ff2bbb700d3e42c0293c870277efa7fd2ab3aa2/md5.js', source_sha256:sha256, icons}, null, 2) + '\n');
// Pillow 解码一次，运行时无需携带 PNG 解码器。
cp.execFileSync(python, ['-c', 'import sys,json,pathlib; from PIL import Image; p=pathlib.Path(sys.argv[1]); m=json.loads((p/"manifest.json").read_text()); images=[Image.open(p/x["file"]).convert("RGBA") for x in m["icons"]]; assert all(i.size==(16,16) for i in images); pathlib.Path(sys.argv[2]).write_bytes(b"".join(i.tobytes() for i in images))', out, path.join(root, 'crates/tswn_core/src/namerena/icon_render/rosmontic_rgba.bin')]);
console.log('已提取 ' + icons.length + ' 个原始 Boss 图标');
