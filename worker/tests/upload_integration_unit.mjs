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

// Two uploads started back to back used to mix up their parts and re-render the
// track list out of order. Each upload must keep its own session and never post
// another session's part number.
test("two concurrent uploads keep their own sessions and parts", async () => {
  const partSize = 8 * 1024 * 1024;
  const sessions = {
    a: "1".repeat(32),
    b: "2".repeat(32),
  };
  const calls = [];
  globalThis.fetch = async (url, init = {}) => {
    const target = String(url);
    calls.push({ url: target, method: init.method ?? "GET", body: init.body });
    if (target === "/api/health") {
      return new Response(JSON.stringify({ runtime: "cloudflare-worker" }), { status: 200 });
    }
    if (target.includes("/191/audio/multipart") && init.method === "POST" && !target.includes("/parts/") && !target.includes("/complete")) {
      return new Response(
        JSON.stringify({ id: sessions.a, part_size: partSize, object_key: "audio/a.flac" }),
        { status: 200 },
      );
    }
    if (target.includes("/192/audio/multipart") && init.method === "POST" && !target.includes("/parts/") && !target.includes("/complete")) {
      return new Response(
        JSON.stringify({ id: sessions.b, part_size: partSize, object_key: "audio/b.flac" }),
        { status: 200 },
      );
    }
    if (target.includes("/parts/") && target.includes("/sign")) {
      return new Response(
        JSON.stringify({ upload_url: `https://wasabi.test/${target.includes(sessions.a) ? "a" : "b"}`, part_number: 1 }),
        { status: 200 },
      );
    }
    if (target.startsWith("https://wasabi.test/")) {
      return new Response("", { status: 200, headers: { etag: "\"etag-1\"" } });
    }
    if (target.includes("/finalize")) {
      return new Response(JSON.stringify({ file_hash: "h" }), { status: 200 });
    }
    return new Response(JSON.stringify({ parts: [] }), { status: 200 });
  };

  const [first, second] = await Promise.all([
    uploadFileWithChunks("/api/cds/11/tracks/191/audio", "audio", fakeFile(9 * 1024 * 1024, "a.flac", "audio/flac"), "audio"),
    uploadFileWithChunks("/api/cds/11/tracks/192/audio", "audio", fakeFile(9 * 1024 * 1024, "b.flac", "audio/flac"), "audio"),
  ]);
  assert.equal(first.status, 200);
  assert.equal(second.status, 200);

  const uploads = calls.filter((call) => call.method === "POST" && call.url.includes("/parts/"));
  const perTrack = new Map();
  for (const call of uploads) {
    const track = call.url.includes("/191/") ? "191" : "192";
    const session = call.url.split("/multipart/")[1].split("/parts/")[0];
    perTrack.set(track, new Set([...(perTrack.get(track) ?? []), session]));
  }
  assert.deepEqual([...perTrack.get("191")], [sessions.a]);
  assert.deepEqual([...perTrack.get("192")], [sessions.b]);

  const finalizes = calls.filter((call) => call.url.endsWith("/audio/finalize"));
  assert.equal(finalizes.length, 2);
  const firstSession = JSON.parse(String(finalizes[0].body)).session_id;
  const secondSession = JSON.parse(String(finalizes[1].body)).session_id;
  assert.deepEqual(
    [firstSession, secondSession].sort(),
    [sessions.a, sessions.b].sort(),
  );
});
