import test from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";

const root = fileURLToPath(new URL("../../", import.meta.url));
const settings = {
  "audio.data_saver.enabled": "true",
  "audio.data_saver.extensions": "flac",
};

// Each test gets its own Worker, in-memory D1, queue and object transport.
// No live Wasabi credentials, shared queue claims, or decoder assertions here.
async function audioFixture() {
  const objects = new Map();
  const mf = new Miniflare(convertV4MiniflareOptions({
    modulesRoot: path.join(root, "worker/build"),
    modules: [
      { type: "ESModule", path: path.join(root, "worker/build/index.js") },
      { type: "CompiledWasm", path: path.join(root, "worker/build/index_bg.wasm") },
    ],
    compatibilityDate: "2026-08-11",
    bindings: {
      DANTALIAN_AUTH_REQUIRED: "false",
      WASABI_ACCESS_KEY_ID: "fixture",
      WASABI_SECRET_ACCESS_KEY: "fixture",
      WASABI_ENDPOINT: "https://storage.fixture.test",
      WASABI_REGION: "us-east-1",
      WASABI_BUCKET: "fixture",
      WASABI_PREFIX: "audio-contract",
    },
    d1Databases: { DB: "audio-contract" },
    queueProducers: { AUDIO_JOB_QUEUE: "audio-contract" },
    outboundService: { node: async (request, response) => {
      const url = new URL(request.url, `http://${request.headers.host}`);
      assert.equal(url.hostname, "storage.fixture.test");
      if (request.method === "PUT") {
        const chunks = [];
        for await (const chunk of request) chunks.push(chunk);
        objects.set(url.pathname, Buffer.concat(chunks));
        response.writeHead(200, { ETag: '"fixture-etag"' });
        response.end();
        return;
      }
      if (request.method === "DELETE") {
        objects.delete(url.pathname);
        response.writeHead(204);
        response.end();
        return;
      }
      const bytes = objects.get(url.pathname);
      if (!bytes) {
        response.writeHead(404);
        response.end();
        return;
      }
      const headers = { "accept-ranges": "bytes" };
      let start = 0, end = bytes.length - 1, status = 200;
      if (request.headers.range) {
        const range = /^bytes=(\d+)-(\d*)$/.exec(request.headers.range);
        assert.ok(range, `Unexpected byte range: ${request.headers.range}`);
        start = Number(range[1]);
        if (range[2]) end = Math.min(Number(range[2]), end);
        if (start > end) {
          response.writeHead(416, { "content-range": `bytes */${bytes.length}` });
          response.end();
          return;
        }
        status = 206;
        headers["content-range"] = `bytes ${start}-${end}/${bytes.length}`;
      }
      headers["content-length"] = end - start + 1;
      response.writeHead(status, headers);
      response.end(request.method === "HEAD" ? undefined : bytes.subarray(start, end + 1));
    } },
  }));
  try {
    const db = await mf.getD1Database("DB");
    for (const filename of (await readdir(path.join(root, "worker/migrations"))).filter(name => name.endsWith(".sql")).sort()) {
      const sql = await readFile(path.join(root, "worker/migrations", filename), "utf8");
      await db.exec(sql.replace(/--[^\n]*/g, "").replace(/\s+/g, " "));
    }
    async function json(method, route, body) {
      const response = await mf.dispatchFetch(`http://localhost${route}`, {
        method,
        headers: body === undefined ? {} : { "content-type": "application/json" },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
      assert.ok(response.ok, `${method} ${route}: ${response.status} ${response.ok ? "" : await response.text()}`);
      return response.json();
    }
    return {
      db, objects, json,
      close: () => mf.dispose(),
      async upload() {
        const created = await json("POST", "/api/cds", { title: "FLAC contract", tracks: [{ track_number: 1, title: "Track" }] });
        const cd = created.cd;
        const track = cd.tracks[0];
        const form = new FormData();
        form.append("audio", new Blob([new Uint8Array([102, 76, 97, 67])]), "contract.flac");
        // Serialize with the platform Request before crossing Miniflare's
        // separate Fetch implementation (different FormData constructor).
        const request = new Request(`http://localhost/api/cds/${cd.id}/tracks/${track.id}/audio`, { method: "POST", body: form });
        const response = await mf.dispatchFetch(request.url, {
          method: "POST",
          headers: Object.fromEntries(request.headers),
          body: await request.arrayBuffer(),
        });
        assert.equal(response.status, 200, response.ok ? "" : await response.text());
        const { file_hash: legacyHash } = await response.json();
        return { trackId: track.id, legacyHash, bareHash: legacyHash.replace(/\.[^.]+$/, "") };
      },
      async jobs() {
        return (await db.prepare("SELECT id, status, input_object_key, output_object_key, codec, idempotency_key FROM audio_jobs ORDER BY codec").all()).results;
      },
    };
  } catch (error) {
    await mf.dispose();
    throw error;
  }
}

function assertCanonicalJobs(jobs, bareHash) {
  assert.deepEqual(jobs.map(job => job.codec), ["aac", "opus"]);
  for (const job of jobs) {
    assert.equal(job.status, "queued");
    assert.equal(job.input_object_key, `audio-contract/audio/${bareHash}.flac`);
    assert.equal(job.output_object_key, `audio-contract/audio/encoded/${job.codec}/${bareHash}.${job.codec}`);
    assert.equal(job.idempotency_key, `data-saver:${bareHash}:${job.codec}`);
  }
}

for (const legacy of [false, true]) {
  test(`ready converted audio is discoverable with a ${legacy ? "legacy" : "bare"} file hash`, { timeout: 30000 }, async () => {
    const fixture = await audioFixture();
    try {
      const { bareHash, legacyHash } = await fixture.upload();
      for (const format of ["opus", "aac"]) {
        fixture.objects.set(`/fixture/audio-contract/audio/encoded/${format}/${bareHash}.${format}`, Buffer.from(`encoded-${format}`));
      }
      const availability = await fixture.json("GET", `/api/audio/playability/${legacy ? legacyHash : bareHash}?ext=flac`);
      assert.equal(availability.original.available, true);
      assert.equal(availability.opus.available, true);
      assert.equal(availability.aac.available, true);
      assert.equal(availability.preferred_format, "opus");
    } finally { await fixture.close(); }
  });
}

test("enabling data saver finds a legacy FLAC row and both hash forms reuse the same jobs", { timeout: 30000 }, async () => {
  const fixture = await audioFixture();
  try {
    const { bareHash, legacyHash, trackId } = await fixture.upload();
    await fixture.db.prepare("UPDATE tracks SET file_hash = ? WHERE id = ?").bind(legacyHash, trackId).run();
    assert.deepEqual(await fixture.jobs(), []);
    await fixture.json("PUT", "/api/settings", settings);
    const first = await fixture.jobs();
    assertCanonicalJobs(first, bareHash);
    for (const hash of [bareHash, legacyHash]) await fixture.json("GET", `/api/audio/playability/${hash}?ext=flac`);
    await fixture.json("PUT", "/api/settings", settings);
    assert.deepEqual(await fixture.jobs(), first, "scans and playback must not duplicate or reset queued work");
  } finally { await fixture.close(); }
});

test("an eligible direct upload schedules both compressed variants immediately", { timeout: 30000 }, async () => {
  const fixture = await audioFixture();
  try {
    await fixture.json("PUT", "/api/settings", settings);
    const { bareHash } = await fixture.upload();
    assertCanonicalJobs(await fixture.jobs(), bareHash);
  } finally { await fixture.close(); }
});
