import { ensureApi } from "./show-wasm.js";
import { runScoreTest } from "./show-score-test.js";

self.onmessage = async ({ data }) => {
  // 每次测试独占 Worker；关闭/替换测试时由页面 terminate，包含进行中的一场。
  self.onmessage = null;
  try {
    const info = { textContent: "" };
    const api = await ensureApi(info, info, info);
    await runScoreTest(api, data.input, data.strategy, progress => self.postMessage({ type: "checkpoint", progress }));
    self.postMessage({ type: "done" });
  } catch (error) {
    self.postMessage({ type: "error", message: error?.message ?? String(error) });
  }
};
