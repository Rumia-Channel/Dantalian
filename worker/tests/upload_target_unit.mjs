import test from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const {
  uploadTargetIsAudio,
  uploadTargetObjectKind,
  uploadTargetAudioSessionUrl,
} = require("../../static/js/upload-target.js");

// v0.1.9 shipped a regression where the session URL `…/audio/multipart` was
// classified as EPUB because the check required `/audio` to end the URL. The
// Worker then validated a .flac file as an epub object and answered 400.
test("audio session URLs keep the audio object kind", () => {
  const audioUrls = [
    "/api/cds/11/tracks/191/audio",
    "/api/cds/11/tracks/191/audio/multipart",
    "/api/books/2/tracks/9/audio",
    "/api/books/2/tracks/9/audio/multipart",
    "https://dantalian.rumia-ch.uk/api/cds/11/tracks/191/audio/multipart",
    "/api/cds/11/tracks/191/audio/multipart?x=1",
  ];
  for (const url of audioUrls) {
    assert.equal(uploadTargetIsAudio(url), true, url);
    assert.equal(uploadTargetObjectKind(url), "audio", url);
  }
});

test("epub, cover, and lookalike URLs stay epub", () => {
  const nonAudioUrls = [
    "/api/books/7/epub",
    "/api/cds/11/cover",
    "/api/books/7/audiobook",
    "/api/audio/stream/abc",
    "",
  ];
  // `/api/audio/stream/...` is an audio-sounding route but never an upload target,
  // so it must not reach the audio session builder by accident.
  for (const url of nonAudioUrls.slice(0, 4)) {
    assert.equal(uploadTargetIsAudio(url), false, url);
    assert.equal(uploadTargetObjectKind(url), "epub", url);
  }
  assert.equal(uploadTargetIsAudio(undefined), false);
});

test("session URL building is idempotent", () => {
  assert.equal(
    uploadTargetAudioSessionUrl("/api/cds/11/tracks/191/audio"),
    "/api/cds/11/tracks/191/audio/multipart",
  );
  assert.equal(
    uploadTargetAudioSessionUrl("/api/cds/11/tracks/191/audio/multipart"),
    "/api/cds/11/tracks/191/audio/multipart",
  );
});
