// Optional integration check against an installed pi-ai SDK; no real keys or upstream calls.
// node tests/pi-thinking.mjs target/debug/loom /path/to/@earendil-works/pi-ai
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";
import { createServer } from "node:http";

const [binary, sdk] = process.argv.slice(2);
if (!binary || !sdk) throw new Error("Usage: node tests/pi-thinking.mjs <loom-binary> <pi-ai-directory>");
const { streamSimple } = await import(pathToFileURL(join(resolve(sdk), "dist/api/openai-completions.js")));
const root = await mkdtemp(join(tmpdir(), "loom-pi-thinking-"));
const data = join(root, "data");
const captured = [];
const server = createServer(async (req, res) => {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  captured.push(JSON.parse(Buffer.concat(chunks).toString()));
  res.writeHead(200, { "content-type": "text/event-stream" });
  res.end('data: {"id":"fixture","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"ok"},"finish_reason":null}]}\n\ndata: {"id":"fixture","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}\n\ndata: [DONE]\n\n');
});
try {
  await mkdir(data);
  await writeFile(join(data, "providers.json"), JSON.stringify({ providers: { wallet: {
    enabled: true, baseURL: "https://api.deepseek.com", keyFile: join(data, "unused.key"),
    models: ["deepseek-flash", "deepseek-v4-pro"].map(id => ({ id, reasoning: true, input: ["text"], maxTokens: 128, contextWindow: 8192, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } })),
  } } }));
  execFileSync(resolve(binary), ["sync"], { env: { ...process.env, HOME: root, MODEL_HUB_DATA_DIR: data }, stdio: "pipe" });
  const pi = JSON.parse(await readFile(join(root, ".pi/agent/models.json"), "utf8"));
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  for (const entry of pi.providers.hub.models) {
    const model = { ...entry, provider: "hub", api: "openai-completions", baseUrl: `http://127.0.0.1:${server.address().port}/v1` };
    for (const [level, effort] of [["off", null], ["minimal", "low"], ["low", "low"], ["medium", "high"], ["high", "high"], ["xhigh", "high"], ["max", "max"]]) {
      const stream = streamSimple(model, { messages: [{ role: "user", content: "fixture", timestamp: 0 }] }, { apiKey: "FIXTURE_ONLY", reasoning: level, maxTokens: 16 });
      const result = await stream.result();
      assert.equal(result.stopReason, "stop", result.errorMessage);
      const body = captured.at(-1);
      assert.equal(body.model, entry.id);
      assert.equal(body.thinking?.type, level === "off" ? "disabled" : "enabled");
      assert.equal(body.reasoning_effort ?? null, effort);
    }
  }
  assert.equal(captured.length, 14);
  console.log("PASS: real pi SDK sent 14 requests to a loopback fixture; off/on and all effort levels agree.");
} finally {
  await new Promise(resolve => server.close(resolve));
  await rm(root, { recursive: true, force: true });
}
