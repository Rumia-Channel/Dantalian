import test from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

// The browser loads js/upload-target.js as a classic script, so its function
// declarations become globals before js/upload.js runs. Node module scope does
// not, so bridge the exports onto globalThis to mirror the browser shape.
const uploadTarget = require("../../static/js/upload-target.js");
globalThis.uploadTargetIsAudio = uploadTarget.uploadTargetIsAudio;
globalThis.uploadTargetObjectKind = uploadTarget.uploadTargetObjectKind;
globalThis.uploadTargetAudioSessionUrl = uploadTarget.uploadTargetAudioSessionUrl;
globalThis.uploadTargetAudioFinalizeUrl = uploadTarget.uploadTargetAudioFinalizeUrl;
globalThis.uploadTargetContentType = uploadTarget.uploadTargetContentType;
globalThis.uploadTargetPickUploadKind = uploadTarget.uploadTargetPickUploadKind;
globalThis.readApiDiagnostic = async (response) => ({
  status: response.status,
  headers: {},
  body: "",
  json: null,
  cloudflareCode: "",
  rayId: "",
  serverMessage: "",
});
globalThis.describeApiDiagnostic = () => "stub";
const { uploadFileWithChunks } = require("../../static/js/upload.js");

function fakeFile(size, name, type) {
  return { size, name, type, slice: (start, end) => ({ size: end - start }) };
}

function recordedFetch(routes) {
  const calls = [];
  globalThis.fetch = async (url, init = {}) => {
    calls.push({ url: String(url), method: init.method ?? "GET", body: init.body });
    const match = routes.find((route) => String(url).startsWith(route.prefix));
    const payload = match ? match.body : { ok: true };
    return new Response(JSON.stringify(payload), { status: match?.status ?? 200 });
  };
  return calls;
}

function initRequestBody(calls, path) {
  const call = calls.find((entry) => entry.url === path);
  assert.ok(call, `no request recorded for ${path}: ${calls.map((c) => c.url).join(", ")}`);
  return JSON.parse(String(call.body));
}

test("m4a audio keeps an audio content type and the audio object kind", async () => {
  const partSize = 8 * 1024 * 1024;
  const sessionPath = "/api/cds/11/tracks/191/audio/multipart";
  const calls = recordedFetch([
    { prefix: "/api/health", body: { runtime: "cloudflare-worker" } },
    { prefix: `${sessionPath}/`, body: { upload_url: "https://wasabi.test/part", part_number: 1 } },
    { prefix: sessionPath, body: { id: "b".repeat(32), part_size: partSize, object_key: "audio/x.m4a" } },
    { prefix: "/api/cds/11/tracks/191/audio/finalize", body: { file_hash: "x" } },
  ]);
  const original = globalThis.fetch;
  globalThis.fetch = async (url, init = {}) => {
    if (String(url).startsWith("https://wasabi.test/part")) {
      calls.push({ url: String(url), method: "PUT" });
      return new Response("", { status: 200, headers: { etag: "\"etag-1\"" } });
    }
    return original(url, init);
  };

  // Windows のレジストリでは .m4a の MIME が video/mp4 になることがある。
  const response = await uploadFileWithChunks(
    "/api/cds/11/tracks/191/audio",
    "audio",
    fakeFile(9 * 1024 * 1024, "track.m4a", "video/mp4"),
    "audio",
  );
  assert.equal(response.status, 200);

  const body = initRequestBody(calls, sessionPath);
  assert.equal(body.object_kind, "audio");
  assert.equal(body.content_type, "audio/mp4");
  assert.equal(body.file_name, "track.m4a");
});

test("large audio uploads post to the browser-shaped multipart URLs", async () => {
  const partSize = 8 * 1024 * 1024;
  const calls = recordedFetch([
    { prefix: "/api/health", body: { runtime: "cloudflare-worker" } },
    { prefix: "/api/cds/11/tracks/191/audio/multipart/", body: { upload_url: "https://wasabi.test/part", part_number: 1 } },
    { prefix: "/api/cds/11/tracks/191/audio/multipart", body: { id: "a".repeat(32), part_size: partSize, object_key: "audio/x.flac" } },
    { prefix: "https://wasabi.test/part", body: {} },
    { prefix: "/api/cds/11/tracks/191/audio/finalize", body: { file_hash: "x" } },
  ]);
  // Part responses need an ETag header, which the generic recorder cannot set.
  const original = globalThis.fetch;
  globalThis.fetch = async (url, init = {}) => {
    if (String(url).startsWith("https://wasabi.test/part")) {
      calls.push({ url: String(url), method: "PUT" });
      return new Response("", { status: 200, headers: { etag: "\"etag-1\"" } });
    }
    return original(url, init);
  };

  const response = await uploadFileWithChunks(
    "/api/cds/11/tracks/191/audio",
    "audio",
    fakeFile(9 * 1024 * 1024, "track.flac", "audio/flac"),
  );
  assert.equal(response.status, 200);

  const urls = calls.map((call) => call.url);
  assert.equal(urls.includes("/api/cds/11/tracks/191/audio/multipart"), true, urls.join("\n"));
  assert.equal(
    urls.some((url) => url === "/api/cds/11/tracks/191/audio/multipart/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/parts/1/sign"),
    true,
    urls.join("\n"),
  );
  assert.equal(
    urls.some((url) => url === "/api/cds/11/tracks/191/audio/multipart/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/complete"),
    true,
    urls.join("\n"),
  );
  assert.equal(
    urls.some((url) => url === "/api/cds/11/tracks/191/audio/finalize"),
    true,
    urls.join("\n"),
  );
  // The regression that reached production: finalize nested under the session URL.
  assert.equal(urls.some((url) => url.includes("/multipart/finalize")), false, urls.join("\n"));
});

test("small audio keeps the single direct upload", async () => {
  const calls = recordedFetch([
    { prefix: "/api/health", body: { runtime: "cloudflare-worker" } },
    { prefix: "/api/cds/11/tracks/191/audio", body: { file_hash: "x" } },
  ]);
  const response = await uploadFileWithChunks(
    "/api/cds/11/tracks/191/audio",
    "audio",
    fakeFile(1024 * 1024, "track.flac", "audio/flac"),
  );
  assert.equal(response.status, 200);
  // isWorkerRuntime() memoizes /api/health for the module lifetime, so only the
  // upload POST is guaranteed to appear here.
  assert.deepEqual(calls.map((call) => call.url), ["/api/cds/11/tracks/191/audio"]);
  assert.equal(calls.every((call) => call.url.includes("/multipart") === false), true);
});
