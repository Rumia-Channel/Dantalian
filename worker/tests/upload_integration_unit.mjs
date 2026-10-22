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
globalThis.uploadTargetIsEpubUpload = uploadTarget.uploadTargetIsEpubUpload;
globalThis.uploadTargetPresignedSessionUrl = uploadTarget.uploadTargetPresignedSessionUrl;
globalThis.uploadTargetPresignedInitUrl = uploadTarget.uploadTargetPresignedInitUrl;
globalThis.uploadTargetPresignedFinalizeUrl = uploadTarget.uploadTargetPresignedFinalizeUrl;
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

// 100MB を超える EPUB は Worker の直アップロード上限を超えるため、音声と同じ
// presigned multipart で Wasabi へ直接送り、書籍の finalize で紐付ける。
test("large EPUB uploads parts directly to Wasabi and finalizes on the book", async () => {
  const partSize = 8 * 1024 * 1024;
  const sessionId = "c".repeat(32);
  const calls = [];
  globalThis.fetch = async (url, init = {}) => {
    const target = String(url);
    calls.push({ url: target, method: init.method ?? "GET", body: init.body });
    if (target === "/api/health") {
      return new Response(JSON.stringify({ runtime: "cloudflare-worker" }), { status: 200 });
    }
    if (target === "/api/books/7/epub/finalize") {
      return new Response(JSON.stringify({ file_hash: "c.epub" }), { status: 200 });
    }
    if (target.startsWith("https://wasabi.test/")) {
      return new Response("", { status: 200, headers: { etag: "\"etag-1\"" } });
    }
    if (target.endsWith("/sign")) {
      return new Response(
        JSON.stringify({ upload_url: "https://wasabi.test/part", part_number: 1 }),
        { status: 200 },
      );
    }
    if (target.endsWith("/complete")) {
      return new Response(JSON.stringify({ status: "complete" }), { status: 200 });
    }
    if (target === "/api/uploads/multipart/init") {
      return new Response(
        JSON.stringify({ id: sessionId, part_size: partSize, object_key: "epubs/c.epub" }),
        { status: 200 },
      );
    }
    return new Response(JSON.stringify({ ok: true }), { status: 200 });
  };

  const response = await uploadFileWithChunks(
    "/api/books/7/epub",
    "file",
    fakeFile(120 * 1024 * 1024, "big-book.epub", "application/epub+zip"),
    "epub",
  );
  assert.equal(response.status, 200);

  const urls = calls.map((call) => call.url);
  // Regression: 0.1.25/0.1.26 posted init to the session base (404) and then
  // fell back to the Worker direct path, which rejected the file with 413.
  const initCall = calls.find((call) => call.url === "/api/uploads/multipart/init");
  assert.ok(initCall, urls.join("\n"));
  const initBody = JSON.parse(String(initCall.body));
  assert.equal(initBody.object_kind, "epub");
  assert.equal(initBody.content_type, "application/epub+zip");
  assert.equal(initBody.file_name, "big-book.epub");
  assert.equal(initBody.expected_size, 120 * 1024 * 1024);

  // Worker の直アップロード (/api/books/7/epub) には本文を送らない。
  assert.equal(urls.includes("/api/books/7/epub"), false, urls.join("\n"));
  assert.equal(urls.includes(`/api/uploads/multipart/${sessionId}/parts/1/sign`), true, urls.join("\n"));
  assert.equal(urls.includes(`/api/uploads/multipart/${sessionId}/complete`), true, urls.join("\n"));

  const finalize = calls.find((call) => call.url === "/api/books/7/epub/finalize");
  assert.ok(finalize, urls.join("\n"));
  const finalizeBody = JSON.parse(String(finalize.body));
  assert.equal(finalizeBody.session_id, sessionId);
  assert.equal(finalizeBody.name, "big-book.epub");
});

test("an EPUB under the direct limit keeps the single Worker upload", async () => {
  const calls = recordedFetch([
    { prefix: "/api/health", body: { runtime: "cloudflare-worker" } },
    { prefix: "/api/books/7/epub", body: { file_hash: "x" } },
  ]);
  const response = await uploadFileWithChunks(
    "/api/books/7/epub",
    "file",
    fakeFile(20 * 1024 * 1024, "book.epub", "application/epub+zip"),
    "epub",
  );
  assert.equal(response.status, 200);
  assert.deepEqual(
    calls.filter((call) => call.method === "POST").map((call) => call.url),
    ["/api/books/7/epub"],
  );
  assert.equal(calls.some((call) => call.url.includes("/multipart")), false);
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

// 新しいクライアントが presigned ルートを持たない旧 Worker に当たった場合でも、
// 直アップロード上限を超える EPUB は本文を送らずに 413 で止める。このテストは
// 30 秒のクールダウンを立てるので、必ず最後に実行する。
test("an EPUB over the direct limit fails closed when the multipart route is missing", async () => {
  const calls = [];
  globalThis.fetch = async (url, init = {}) => {
    const target = String(url);
    calls.push({ url: target, method: init.method ?? "GET" });
    if (target === "/api/health") {
      return new Response(JSON.stringify({ runtime: "cloudflare-worker" }), { status: 200 });
    }
    if (target === "/api/uploads/multipart/init") {
      return new Response("not found", { status: 404 });
    }
    return new Response(JSON.stringify({ ok: true }), { status: 200 });
  };

  const response = await uploadFileWithChunks(
    "/api/books/7/epub",
    "file",
    fakeFile(120 * 1024 * 1024, "big-book.epub", "application/epub+zip"),
    "epub",
  );
  assert.equal(response.status, 413);
  assert.deepEqual(
    calls.filter((call) => call.method === "POST").map((call) => call.url),
    ["/api/uploads/multipart/init"],
  );
});
