// 验证 battle_replay_forced：玩家指定技能后，日志里应当出现对应技能。
import { readFileSync } from 'node:fs';
import * as path from 'node:path';
import { pathToFileURL } from 'node:url';

const dir = process.argv[2];
const mod = await import(pathToFileURL(path.join(dir, 'tswn_wasm.js')).href);
mod.initSync({ module: readFileSync(path.join(dir, 'tswn_wasm_bg.wasm')) });

const NAME = '半城烟沙@青';
const NAMES = {
  0: '火球术', 1: '冰冻术', 2: '雷击术', 3: '地裂术', 4: '吸血', 5: '投毒', 6: '连击',
  7: '会心一击', 8: '瘟疫', 9: '生命之轮', 10: '狂暴术', 11: '魅惑', 12: '加速术',
  13: '减速术', 14: '诅咒', 15: '治愈魔法', 16: '苏生术', 17: '净化', 18: '铁壁',
  19: '蓄力', 20: '聚气', 21: '潜行', 22: '血祭', 23: '分身', 24: '幻术', 25: '防御',
  26: '守护', 27: '伤害反弹', 28: '护身符', 29: '蓄盾', 30: '反击', 31: '吞噬',
  32: '召唤亡灵', 33: '垂死抗争', 34: '隐匿',
};

const diy = mod.to_diy(NAME);
const ol = JSON.parse(diy.slice(diy.indexOf('ol:') + 3));
const skills = Object.entries(ol.skills || {}).filter(([, v]) => {
  const n = typeof v === 'number' ? v : String(v).includes('*') ? v.split('*').map(Number).reduce((a, b) => a * b, 1) : v.split('+').map(Number).reduce((a, b) => a + b, 0);
  return n > 0;
});
console.log('玩家技能', skills.map(([k, v]) => `${k}:${v}`).join(', '));

const input = `${NAME}\n\n木桩@test\nseed:forced-demo@!`;
const lines = (rep) => {
  const out = [];
  for (const f of rep.frames) for (const row of f.rows) for (const clip of row.clips) out.push(clip.parts.map((p) => p.text).join('').trim());
  return out;
};

const base = lines(mod.battle_replay(input, { max_rounds: 20000 }));
console.log('\n--- 基线（自动）前 12 行 ---');
console.log(base.slice(0, 12).join('\n'));

const target = skills.find(([k]) => k === 'sklfire') || skills[0];
const id = Number(target[0].replace('skl', '').length ? Object.entries(NAMES).find(([, n]) => n === (NAMES[0] && ''))?.[0] ?? 0 : 0);
// 直接从名字表反查技能 id
const idOf = (key) => {
  const map = { sklfire: 0, sklice: 1, sklthunder: 2, sklquake: 3, sklabsorb: 4, sklpoison: 5, sklrapid: 6, sklcritical: 7, sklhalf: 8, sklexchange: 9, sklberserk: 10, sklcharm: 11, sklhaste: 12, sklslow: 13, sklcurse: 14, sklheal: 15, sklrevive: 16, skldisperse: 17, skliron: 18, sklcharge: 19, sklaccumulate: 20, sklassassinate: 21, sklsummon: 22, sklclone: 23, sklshadow: 24, skldefend: 25, sklprotect: 26, sklreflect: 27, sklreraise: 28, sklshield: 29, sklcounter: 30, sklmerge: 31, sklzombie: 32, sklupgrade: 33, sklhide: 34 };
  return map[key];
};
const skillId = idOf(target[0]);
console.log(`\n强制技能：${target[0]} (id=${skillId}, ${NAMES[skillId]})，连续 8 次`);
const forced = mod.battle_replay_forced(input, Array(8).fill(skillId), 0, { max_rounds: 20000 });
const fl = lines(forced);
console.log('--- 强制后前 16 行 ---');
console.log(fl.slice(0, 16).join('\n'));
const used = fl.filter((l) => l.includes(`使用【${NAMES[skillId]}】`)).length;
console.log(`\n出现「使用【${NAMES[skillId]}】」${used} 次；函数存在：${typeof mod.battle_replay_forced === 'function'}`);
