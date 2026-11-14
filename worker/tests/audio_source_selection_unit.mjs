import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

const sources = await Promise.all([
  "player/audio-source.js", "player/audio-cache.js",
].map((name) => readFile(new URL(`../../static/js/${name}`, import.meta.url), "utf8")));

const OPUS_MIME = 'audio/ogg; codecs="opus"';
const AAC_MIME = "audio/aac";
const MP4_AAC_MIME = 'audio/mp4; codecs="mp4a.40.2"';

const flacTrack = { id: 7, file_hash: "abc123.flac", file_name: "Song.flac", title: "Song" };
const originalUrl = "/audio/abc123.flac";
const opusUrl = "/api/audio/stream/abc123.flac?ext=flac&format=opus";
const aacUrl = "/api/audio/stream/abc123.flac?ext=flac&format=aac";

const enabledSettings = {
  "audio.data_saver.enabled": "true",
  "audio.data_saver.extensions": "wav,flac",
};
const disabledSettings = {
  "audio.data_saver.enabled": "false",
  "audio.data_saver.extensions": "wav,flac",
};
const readyPlayability = {
  original: { available: true, size_bytes: 5000 },
  opus: { available: true, size_bytes: 640 },
  aac: { available: true, size_bytes: 800 },
  preferred_format: "opus",
};

// Minimal IndexedDB double: the cache keeps the audio body in IndexedDB and only
// bookkeeping in localStorage, so both stores have to behave like the browser.
function createIndexedDb() {
  const records = new Map();
  const run = (action) => {
    const request = { result: undefined, error: null, onsuccess: null, onerror: null };
    queueMicrotask(() => {
      try {
        action(request);
        if (request.onsuccess) request.onsuccess();
      } catch (error) {
        request.error = error;
        if (request.onerror) request.onerror();
      }
    });
    return request;
  };
  const db = {
    objectStoreNames: { contains: () => true },
    createObjectStore: () => ({}),
    close: () => {},
    transaction() {
      const transaction = { oncomplete: null, onerror: null, onabort: null };
      transaction.objectStore = () => ({
        get: (key) => run((request) => { request.result = records.get(key); }),
        put: (record) => run((request) => {
          records.set(record.fileHash, record);
          request.result = record.fileHash;
        }),
        delete: (key) => run(() => { records.delete(key); }),
      });
      // The real transaction settles after the caller registered its handlers.
      queueMicrotask(() => queueMicrotask(() => {
        if (transaction.oncomplete) transaction.oncomplete();
      }));
      return transaction;
    },
  };
  return {
    open() {
      const request = { result: null, error: null, onupgradeneeded: null, onsuccess: null, onerror: null, onblocked: null };
      queueMicrotask(() => {
        request.result = db;
        if (request.onupgradeneeded) request.onupgradeneeded();
        if (request.onsuccess) request.onsuccess();
      });
      return request;
    },
    records,
  };
}

function playerPage({
  canPlayType = {},
  settings = enabledSettings,
  playability = readyPlayability,
  encoded = {},
} = {}) {
  const requests = [];
  const manifest = new Map();
  let objectUrlCounter = 0;

  class TestURL extends URL {
    static createObjectURL() {
      objectUrlCounter += 1;
      return `blob:library.test/${objectUrlCounter}`;
    }
    static revokeObjectURL() {}
  }

  const fetchStub = async (url) => {
    requests.push({ url });
    if (url === "/api/settings") {
      return { ok: true, status: 200, json: async () => settings };
    }
    if (url.startsWith("/api/audio/playability/")) {
      if (!playability) return { ok: false, status: 503, json: async () => ({}) };
      return { ok: true, status: 200, json: async () => playability };
    }
    if (url.includes("/api/audio/stream/")) {
      const format = new URL(url, "https://library.test").searchParams.get("format");
      const target = encoded[format];
      if (!target) return { ok: false, status: 404, url, blob: async () => ({ size: 0 }) };
      return {
        ok: true,
        status: 200,
        url: target.url,
        blob: async () => ({ size: target.size, type: target.type }),
      };
    }
    if (url.startsWith("/audio/")) {
      return {
        ok: true,
        status: 200,
        url: `https://library.test${url}`,
        blob: async () => ({ size: 5000, type: "audio/flac" }),
      };
    }
    throw new Error(`Unexpected request: ${url}`);
  };

  const indexedDb = createIndexedDb();
  const sandbox = vm.createContext({
    console, setTimeout, clearTimeout,
    URL: TestURL, URLSearchParams,
    document: {
      createElement: () => ({ canPlayType: (mime) => canPlayType[mime] || "" }),
    },
    fetch: fetchStub,
    indexedDB: indexedDb,
    localStorage: {
      getItem: (key) => (manifest.has(key) ? manifest.get(key) : null),
      setItem: (key, value) => manifest.set(key, String(value)),
      removeItem: (key) => manifest.delete(key),
    },
    window: { location: { href: "https://library.test/music/" } },
  });
  for (const source of sources) vm.runInContext(source, sandbox);

  const call = (expression) => vm.runInContext(expression, sandbox);
  return {
    indexedDb,
    loadPolicy: () => call("loadAudioDataSaverPolicy()"),
    selection: (track) => call(`audioSourceSelection(${JSON.stringify(track)})`),
    cacheTrack: (track) => call(`cacheAudioTrack(${JSON.stringify(track)})`),
    cachedSource: (track) => call(`getCachedAudioSource(${JSON.stringify(track)})`),
    cacheStatus: (tracks) => call(`getAudioCacheStatus(${JSON.stringify(tracks)})`),
    writeRecord: (record) => call(`writeAudioCacheRecord(${JSON.stringify(record)})`),
  };
}

function cachedRecord(format, size = 640) {
  return {
    fileHash: flacTrack.file_hash,
    blob: { size, type: format === "opus" ? "audio/ogg" : format === "aac" ? "audio/aac" : "audio/flac" },
    format,
    mime: format === "opus" ? "audio/ogg" : format === "aac" ? "audio/aac" : "audio/flac",
    size,
    cachedAt: "2026-01-01T00:00:00.000Z",
    trackId: flacTrack.id,
    cdId: null,
    title: flacTrack.title,
  };
}

test("data saver enabled: a ready Opus source is chosen before the original", async () => {
  const page = playerPage({
    canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" },
  });
  await page.loadPolicy();
  const selection = await page.selection(flacTrack);
  assert.deepEqual(Array.from(selection.urls), [opusUrl, aacUrl, originalUrl]);
  assert.deepEqual(Array.from(selection.sizes), [640, 800, 5000]);
});

test("codec support decides the compressed source even when the API prefers another format", async () => {
  // Ogg Opus is unsupported here (iPadOS Safari), raw AAC is supported, and the
  // API still reports opus as its preferred format.
  const page = playerPage({ canPlayType: { [AAC_MIME]: "maybe" } });
  await page.loadPolicy();
  const selection = await page.selection(flacTrack);
  assert.deepEqual(Array.from(selection.urls), [aacUrl, originalUrl]);
});

test("the data saver disabled keeps the original as the only source", async () => {
  const page = playerPage({
    canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" },
    settings: disabledSettings,
  });
  await page.loadPolicy();
  const selection = await page.selection(flacTrack);
  assert.deepEqual(Array.from(selection.urls), [originalUrl]);
  assert.deepEqual(Array.from(selection.sizes), [5000]);
});

test("an extension outside the data saver policy keeps the original", async () => {
  const page = playerPage({ canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" } });
  await page.loadPolicy();
  const selection = await page.selection({ ...flacTrack, file_hash: "abc123.mp3", file_name: "Song.mp3" });
  assert.deepEqual(Array.from(selection.urls), ["/audio/abc123.mp3"]);
});

test("a conversion that is still pending plays the original immediately", async () => {
  const page = playerPage({
    canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" },
    playability: {
      original: { available: true, size_bytes: 5000 },
      opus: { available: false, size_bytes: null },
      aac: { available: false, size_bytes: null },
      preferred_format: "original",
    },
  });
  await page.loadPolicy();
  const selection = await page.selection(flacTrack);
  assert.deepEqual(Array.from(selection.urls), [originalUrl]);
});

test("a track without a file name extension falls back to the hash extension", async () => {
  const page = playerPage({ canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" } });
  await page.loadPolicy();
  const selection = await page.selection({ id: 8, file_hash: "def456.flac", file_name: "Song", title: "Song" });
  assert.deepEqual(Array.from(selection.urls), [
    "/api/audio/stream/def456.flac?ext=flac&format=opus",
    "/api/audio/stream/def456.flac?ext=flac&format=aac",
    "/audio/def456.flac",
  ]);
});

test("missing sources do not produce an unavailable original fallback", async () => {
  const page = playerPage({
    canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" },
    playability: {
      original: { available: false, size_bytes: null },
      opus: { available: false, size_bytes: null },
      aac: { available: false, size_bytes: null },
      preferred_format: null,
    },
  });
  await page.loadPolicy();
  const selection = await page.selection(flacTrack);
  assert.deepEqual(Array.from(selection.urls), []);
});

test("a cached blob in a format this browser cannot decode is not prioritized", async () => {
  const page = playerPage({ canPlayType: { [AAC_MIME]: "maybe" } });
  await page.loadPolicy();
  await page.writeRecord(cachedRecord("opus"));
  assert.equal(await page.cachedSource(flacTrack), null);
  const status = await page.cacheStatus([flacTrack]);
  assert.equal(status.cached, 0);
  assert.equal(status.allCached, false);
});

test("MP4 AAC support does not make the raw AAC artifact playable", async () => {
  const page = playerPage({
    canPlayType: { [MP4_AAC_MIME]: "probably" },
    encoded: {
      aac: { url: "https://library.test/audio/encoded/aac/abc123.aac", size: 700, type: "audio/aac" },
    },
  });
  await page.loadPolicy();
  const result = await page.cacheTrack(flacTrack);
  assert.equal(result.ok, false);
  assert.equal(result.error, "このブラウザは省データ再生の圧縮音声を再生できません");
  assert.equal(page.indexedDb.records.size, 0, "an unplayable artifact must not be cached");
  assert.equal(await page.cachedSource(flacTrack), null);
});

test("the offline cache stores a supported compressed artifact and reuses it", async () => {
  const page = playerPage({
    canPlayType: { [OPUS_MIME]: "probably" },
    encoded: {
      opus: { url: "https://library.test/audio/encoded/opus/abc123.opus", size: 640, type: "audio/ogg" },
    },
  });
  await page.loadPolicy();
  const result = await page.cacheTrack(flacTrack);
  assert.equal(result.ok, true);
  assert.equal(result.format, "opus");
  assert.equal(result.reused, false);
  const cached = await page.cachedSource(flacTrack);
  assert.ok(cached, "the cached blob becomes the first playback source");
  assert.equal(cached.format, "opus");
  const reused = await page.cacheTrack(flacTrack);
  assert.equal(reused.ok, true);
  assert.equal(reused.reused, true);
});

test("a cached original is not prioritized while the data saver is enabled", async () => {
  const page = playerPage({ canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" } });
  await page.loadPolicy();
  await page.writeRecord(cachedRecord("original", 5000));
  assert.equal(await page.cachedSource(flacTrack), null);
  const status = await page.cacheStatus([flacTrack]);
  assert.equal(status.cached, 0);
});

test("with the data saver disabled only a cached original is reused", async () => {
  const page = playerPage({
    canPlayType: { [OPUS_MIME]: "probably", [AAC_MIME]: "maybe" },
    settings: disabledSettings,
  });
  await page.loadPolicy();
  await page.writeRecord(cachedRecord("opus"));
  assert.equal(await page.cachedSource(flacTrack), null);
  await page.writeRecord(cachedRecord("original", 5000));
  const cached = await page.cachedSource(flacTrack);
  assert.ok(cached);
  assert.equal(cached.format, "original");
});
