import { runScoreTest } from "./show-score-test.js";
import { formatError } from "./show-utils.js";
import { ensureApi } from "./show-wasm.js";

self.onmessage = async ({ data }) => {
  self.onmessage = null;
  try {
    const info = { textContent: "" };
    const api = await ensureApi(info, info, info);
    runScoreTest(api, data, progress => self.postMessage({ type: "checkpoint", progress }));
  } catch (error) {
    self.postMessage({ type: "error", message: formatError(error) });
  }
};
