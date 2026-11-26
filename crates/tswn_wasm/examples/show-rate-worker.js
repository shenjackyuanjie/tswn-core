import { ensureApi } from "./show-wasm.js";
import { runGroupWinRates } from "./show-win-rate.js";

self.onmessage = async ({ data }) => {
  // 每次测试独占 Worker；关闭/替换测试时由页面 terminate，包含进行中的一组。
  self.onmessage = null;
  try {
    const info = { textContent: "" };
    const api = await ensureApi(info, info, info);
    const results = await runGroupWinRates(api, data.groups, progress => self.postMessage({ type: "progress", progress }));
    self.postMessage({ type: "done", results });
  } catch (error) {
    self.postMessage({ type: "error", message: error?.message ?? String(error) });
  }
};
