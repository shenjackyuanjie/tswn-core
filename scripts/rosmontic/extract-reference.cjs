// 从固定版本的 Dart2JS 参考实现导出 Boss 机制与初始化快照；解码文本仅供分析。
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const crypto = require("node:crypto");

const SOURCE_SHA256 = "b7dc47862dc64b8ce9a2705b53d49784815f0ff9642235494918b983f3c888d7";
const BOSSES = ["reimu","marisa","rumia","daiyousei","cirno","hong","koakuma","patchouli","sakuya","remilia","flandre","rin","letty","chen","alice","phantom","youmu","yuyuko","ran","yukari","AL1S","fox1","fox2","fox3","fox4","ribbit1","ribbit2","ribbit3","ribbit4","megia"];
const [input, output] = process.argv.slice(2);
if (!input || !output) throw Error("用法：node extract-reference.cjs SOURCE_MD5_JS OUTPUT_DIR");
const bytes = fs.readFileSync(input);
const sha256 = crypto.createHash("sha256").update(bytes).digest("hex");
if (sha256 !== SOURCE_SHA256) throw Error("参考版本摘要不符，不能混用不同版本的技能与数值");
const source = bytes.toString("utf8");
const marker = "console.log($.md)";
if (source.split(marker).length !== 2) throw Error("参考导出锚点必须唯一");
const exportLine = "globalThis.__reference = {C,H,J,P,W,Y,L,V,X,S,Z,F,O,T,Q,A,M,$}";
const context = {console: {log() {}}, dartMainRunner() {}, setTimeout, clearTimeout};
vm.runInNewContext(source.replace(marker, exportLine), context, {filename: input, timeout: 10000});
const ref = context.__reference;
if (!ref || typeof ref.T.hd !== "function") throw Error("参考初始化失败");

// 捕获成功求值的原始常量；失败和对象保留 getter，绝不把未知值改写为 undefined。
const lazy = {};
for (const [,property,getter] of source.matchAll(/u\(\$,\s*"(\w+)",\s*"(\w+)",\s*function/g)) {
    try {
        const value = ref.$[getter]();
        if (typeof value === "string" || typeof value === "boolean" ||
            (typeof value === "number" && Number.isFinite(value))) {
            lazy[getter] = {property, value};
        }
    } catch {}
}
function decode(text) {
    return text.replace(/\$\.(\w+)\(\)/g, (original, key) =>
        key in lazy ? JSON.stringify(lazy[key].value) : original
    ).replace(/O\.([ic])\(("(?:[^"\\]|\\.)*")\)/g, (original, key, literal) => {
        try {
            const value = ref.O[key](JSON.parse(literal));
            return typeof value === "string" ? JSON.stringify(value) : original;
        } catch { return original; }
    });
}
function methodRecord(prototype) {
    const result = {};
    for (const key of Object.getOwnPropertyNames(prototype)) {
        if (key === "constructor" || key.startsWith("$i")) continue;
        const descriptor = Object.getOwnPropertyDescriptor(prototype, key);
        if (typeof descriptor.value === "function") result[key] = decode(descriptor.value.toString());
    }
    return result;
}
function ownScalars(value) {
    const result = {};
    for (const [key,item] of Object.entries(value)) {
        if (item === null || ["string","boolean"].includes(typeof item) ||
            (typeof item === "number" && Number.isFinite(item))) result[key] = item;
    }
    return result;
}
fs.mkdirSync(output, {recursive:true});
const records = [];
for (const name of BOSSES) {
    // 每名 Boss 使用独立 VM，避免编号、lazy 单例与初始化计数相互污染。
    const isolated = {console:{log() {}}, dartMainRunner() {}, setTimeout, clearTimeout};
    vm.runInNewContext(source.replace(marker, exportLine), isolated, {filename:input,timeout:10000});
    const api = isolated.__reference;
    const player = api.T.hd(name, "!", null, null);
    player.av();
    const slots = player.id.map((skill, index) => ({
        index, class:skill.constructor.name, level:skill.f,
        scalar_state:ownScalars(skill), methods:methodRecord(Object.getPrototypeOf(skill))
    }));
    const linked = {};
    // 收集技能持有的延迟、状态、死亡与反击 hook；它们可能不在 id 数组中。
    for (const skill of player.id) {
        for (const value of Object.values(skill)) {
            if (!value || typeof value !== "object" || value === player || Array.isArray(value)) continue;
            const ctor = value.constructor;
            if (typeof ctor !== "function" || ctor === Object || !api.T[ctor.name]) continue;
            linked[ctor.name] = methodRecord(Object.getPrototypeOf(value));
        }
    }
    const record = {
        name, raw:name+"@!", attrs:Array.from(player.u),
        runtime:{attack:player.Q,defense:player.ch,speed:player.cx,agility:player.cy,
                 magic:player.db,resistance:player.dx,wisdom:player.dy,hp:player.fr,
                 max_hp:player.fx,mp:player.fy},
        append_attrs:Array.from(player.gah()),
        immune_keys:Array.from(player.gaQ()),
        weak_immune_keys:Array.from(player.gaB()),
        actor_scalar_state:ownScalars(player),
        default_attack:{class:player.k2.constructor.name,methods:methodRecord(Object.getPrototypeOf(player.k2))},
        order:player.k1.map(skill => player.id.indexOf(skill)), slots, linked,
        actor_methods:methodRecord(Object.getPrototypeOf(player))
    };
    if (record.order.some(index=>index<0)) throw Error(name+": 技能顺序引用错误");
    records.push(record);
    fs.writeFileSync(path.join(output,name+".json"),JSON.stringify(record,null,2)+"\n","utf8");
}
const manifest = {
    source_repository:"https://github.com/Rosmontic/namerena",
    source_commit:"2ff2bbb700d3e42c0293c870277efa7fd2ab3aa2", source_path:"md5.js",
    source_sha256:sha256, boss_count:records.length,
    note:"初始化快照使用完整原代码执行；解码后的方法文本仅用于人工分析，不能替代差分执行。",
    bosses:records.map(({name,attrs,runtime,order,slots})=>({
        name,attrs,runtime,order,slots:slots.map(({index,class:cls,level})=>({index,class:cls,level}))
    }))
};
fs.writeFileSync(path.join(output,"manifest.json"),JSON.stringify(manifest,null,2)+"\n","utf8");
fs.writeFileSync(path.join(output,"lazy-values.json"),JSON.stringify(lazy,null,2)+"\n","utf8");
console.log(JSON.stringify({bosses:records.length,source_sha256:sha256,output:path.resolve(output)}));
