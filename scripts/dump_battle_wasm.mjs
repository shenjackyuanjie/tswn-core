// JSON stdin/stdout bridge for the actual wasm-bindgen Node package.
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
const require = createRequire(import.meta.url);
const wasm = require(resolve(process.env.TSWN_WASM_NODE_PACKAGE || 'target/battle_wasm_node/tswn_wasm.js'));
let input = '';
for await (const chunk of process.stdin) input += chunk;
const outputs = JSON.parse(input).map(({ raw, max_rounds }) => {
    const session = new wasm.BattleSession(raw, { include_icons: false, max_rounds });
    try {
        const initial = session.initial_states(), frames = [];
        for (let frame; (frame = session.next_frame()) !== null;) frames.push(frame);
        return { initial, frames, result: session.result() };
    } finally { session.free(); }
});
process.stdout.write(JSON.stringify(outputs));
