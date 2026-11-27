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

test("overlapping track uploads save both durations even when the second finishes first", async () => {
  const savedDurations = new Map();
  const inputs = new Map([[190, { value: "" }], [191, { value: "" }]]);
  const list = {
    innerHTML: "",
    querySelector: (selector) => inputs.get(Number(/data-track-id="(\d+)"/.exec(selector)?.[1])),
  };
  let releaseFirst;
  let firstStarted;
  const firstUpload = new Promise((resolve) => { releaseFirst = resolve; });
  const started = new Promise((resolve) => { firstStarted = resolve; });
  const context = vm.createContext({
    Response, FormData, Blob, File, URLSearchParams, console,
    document: { getElementById: (id) => id === "edit-tracks-list" ? list : null },
    escapeAttr: String, escapeHtml: String, jsStringAttr: JSON.stringify,
    alert: (message) => assert.fail(message),
    showConfirm: () => assert.fail("empty durations must not ask for confirmation"),
    fetch: async (url, init = {}) => {
      if (url === "/api/health") return Response.json({ runtime: "cloudflare-worker" });
      if (url === "/api/cds/11/tracks") {
        return Response.json([190, 191].map((id, index) => ({
          id, title: `Track ${index + 1}`, track_number: index + 1,
          duration: savedDurations.get(id) ?? "",
        })));
      }
      const id = Number(/\/tracks\/(\d+)/.exec(url)?.[1]);
      if (init.method === "POST" && url.endsWith("/audio")) {
        if (id === 190) {
          firstStarted();
          await firstUpload;
        }
        return Response.json({ file_hash: `${id}.wav`, metadata: { duration_seconds: id - 65 } });
      }
      if (init.method === "PUT") {
        savedDurations.set(id, JSON.parse(init.body).duration);
        return Response.json("ok");
      }
      throw new Error(`Unexpected request: ${url}`);
    },
  });
  for (const source of sources) vm.runInContext(source, context);
  vm.runInContext(await readFile(new URL("../../static/js/edit/track-duration.js", import.meta.url), "utf8"), context);
  const file = new File(["audio"], "Track.wav", { type: "audio/wav" });
  const first = context.uploadTrackAudio("cd", 11, 190, { files: [file], value: "selected" });
  await started;
  try {
    await context.uploadTrackAudio("cd", 11, 191, { files: [file], value: "selected" });
  } finally {
    releaseFirst();
    await first;
  }
  assert.deepEqual([...savedDurations].sort(([a], [b]) => a - b), [[190, "02:05"], [191, "02:06"]]);
});

test("overlapping overwrite confirmations keep separate messages and decisions", async () => {
  const document = new EventTarget();
  const elements = new Map();
  for (const id of ["confirm-overlay", "confirm-message", "confirm-ok", "confirm-cancel"]) {
    const element = new EventTarget();
    element.classList = { add() {}, remove() {} };
    element.focus = () => { document.activeElement = element; };
    elements.set(id, element);
  }
  document.getElementById = (id) => elements.get(id);
  const context = vm.createContext({ document });
  vm.runInContext(await readFile(new URL("../../static/js/utils.js", import.meta.url), "utf8"), context);
  const first = context.showConfirm({ message: "First track: 02:05" });
  const second = context.showConfirm({ message: "Second track: 02:06" });
  await new Promise(setImmediate);
  assert.equal(elements.get("confirm-message").textContent, "First track: 02:05");
  let secondResolved = false;
  second.then(() => { secondResolved = true; });
  elements.get("confirm-ok").dispatchEvent(new Event("click"));
  assert.equal(await first, true);
  await new Promise(setImmediate);
  assert.equal(secondResolved, false);
  assert.equal(elements.get("confirm-message").textContent, "Second track: 02:06");
  elements.get("confirm-cancel").dispatchEvent(new Event("click"));
  assert.equal(await second, false);
});
