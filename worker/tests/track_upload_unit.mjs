import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

const sources = await Promise.all([
  "upload-target.js", "upload.js", "edit/tracks.js",
].map((name) => readFile(new URL(`../../static/js/${name}`, import.meta.url), "utf8")));
const MiB = 1024 * 1024;
const audio = new File([new Uint8Array(120 * MiB)], "large.wav", { type: "audio/wav" });

function uploadPage({ worker = true, limit = "100", healthStatus = 200 } = {}) {
  const requests = [];
  const alerts = [];
  const uploadedParts = new Map();
  const sessionId = "a".repeat(32);
  const context = vm.createContext({
    Response, FormData, Blob, File, URLSearchParams, console,
    document: { getElementById: () => null },
    alert: (message) => alerts.push(message),
    offerToApplyAudioDuration: async () => {},
    fetch: async (url, init = {}) => {
      requests.push({ url, ...init });
      if (url === "/api/health") {
        return Response.json(worker
          ? { runtime: "cloudflare-worker" }
          : { ok: true, version: "0.2.2" }, { status: healthStatus });
      }
      if (url === "/api/settings") return Response.json({ "upload.audio_max_mb": limit });
      if (url.endsWith("/audio/multipart")) {
        return Response.json({ id: sessionId, part_size: 8 * MiB });
      }
      const sign = /\/parts\/(\d+)\/sign$/.exec(url);
      if (sign) return Response.json({ upload_url: `https://storage.test/parts/${sign[1]}` });
      if (url.startsWith("https://storage.test/parts/")) {
        uploadedParts.set(Number(url.split("/").at(-1)), init.body.size);
        return new Response(null, { headers: { ETag: `"part-${url.split("/").at(-1)}"` } });
      }
      if (url.endsWith("/complete")) {
        const parts = JSON.parse(init.body).parts;
        assert.deepEqual(parts.map((part) => part.part_number), [...uploadedParts.keys()].sort((a, b) => a - b));
        return Response.json({ status: "complete" });
      }
      if (url.endsWith("/audio/finalize") || /\/audio(?:\?|$)/.test(url)) {
        return Response.json({ file_hash: "uploaded.wav" });
      }
      throw new Error(`Unexpected request: ${url}`);
    },
  });
  for (const source of sources) vm.runInContext(source, context);
  return {
    context, requests, alerts, uploadedParts,
    upload: (file = audio) => context.uploadTrackAudio("cd", 11, 191, { files: [file], value: "selected" }),
  };
}

test("Worker track input accepts audio above the Native limit and uploads only direct parts", async () => {
  const page = uploadPage();
  await page.upload();
  assert.deepEqual(page.alerts, []);
  assert.equal([...page.uploadedParts.values()].reduce((sum, size) => sum + size, 0), audio.size);
  assert.ok(page.requests.some(({ url }) => url.endsWith("/audio/finalize")));
  assert.equal(page.requests.some(({ body }) => body instanceof FormData), false);
  assert.equal(page.requests.some(({ url }) => url === "/api/settings"), false);
});

test("Native track input still rejects audio above its configured application limit", async () => {
  const page = uploadPage({ worker: false });
  await page.upload();
  assert.equal(page.alerts.length, 1);
  assert.equal(page.requests.some(({ method }) => method === "POST"), false);
});

test("Native custom limits still permit large audio through the Native chunk protocol", async () => {
  const page = uploadPage({ worker: false, limit: "128" });
  await page.upload();
  assert.deepEqual(page.alerts, []);
  const chunks = page.requests.filter(({ body }) => body instanceof FormData);
  assert.equal(chunks.reduce((sum, { body }) => sum + body.get("audio").size, 0), audio.size);
  assert.equal(page.uploadedParts.size, 0);
});

test("an unavailable runtime check reports failure without sending the selected audio", async () => {
  const page = uploadPage({ healthStatus: 503 });
  await page.upload();
  assert.equal(page.alerts.length, 1);
  assert.equal(page.requests.some(({ method }) => method === "POST"), false);
  assert.equal(vm.runInContext("trackUploadsInFlight.size", page.context), 0);
});
