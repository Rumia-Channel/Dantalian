import test from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const {
  uploadTargetIsAudio,
  uploadTargetObjectKind,
  uploadTargetAudioSessionUrl,
  uploadTargetAudioFinalizeUrl,
  uploadTargetIsEpubUpload,
  uploadTargetPresignedSessionUrl,
  uploadTargetPresignedInitUrl,
  uploadTargetPresignedFinalizeUrl,
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
  ];
  // `/api/audio/stream/...` is an audio-sounding route but never an upload target,
  // so it must not reach the audio session builder by accident.
  for (const url of nonAudioUrls) {
    assert.equal(uploadTargetIsAudio(url), false, url);
    assert.equal(uploadTargetObjectKind(url), "epub", url);
  }
  assert.equal(uploadTargetIsAudio(undefined), false);
});

// v0.1.10 shipped a regression where finalize was derived from the session URL
// and the browser posted to `…/audio/multipart/finalize` (404). The finalize
// route lives at `…/audio/finalize`, so pin both derived URLs together.
test("session and finalize URLs are derived from the track audio URL", () => {
  const trackUrl = "/api/cds/11/tracks/191/audio";
  assert.equal(uploadTargetAudioSessionUrl(trackUrl), "/api/cds/11/tracks/191/audio/multipart");
  assert.equal(uploadTargetAudioFinalizeUrl(trackUrl), "/api/cds/11/tracks/191/audio/finalize");

  // Finalize must never be nested under the session URL.
  assert.equal(uploadTargetAudioFinalizeUrl(trackUrl).includes("/multipart/"), false);
  // Passing an already-suffixed session URL must not double the suffix.
  const sessionUrl = "/api/cds/11/tracks/191/audio/multipart";
  assert.equal(uploadTargetAudioSessionUrl(sessionUrl), sessionUrl);
  assert.equal(uploadTargetAudioFinalizeUrl(sessionUrl), "/api/cds/11/tracks/191/audio/finalize");
});

test("the generic EPUB multipart route is left untouched", () => {
  assert.equal(
    uploadTargetAudioSessionUrl("/api/uploads/multipart"),
    "/api/uploads/multipart",
  );
  // The generic session URL is the base for parts/complete; init lives at
  // /api/uploads/multipart/init (posting to the base returned 404 in 0.1.25/0.1.26).
  assert.equal(
    uploadTargetPresignedInitUrl("/api/books/7/epub"),
    "/api/uploads/multipart/init",
  );
  assert.equal(
    uploadTargetPresignedSessionUrl("/api/books/7/epub"),
    "/api/uploads/multipart",
  );
  assert.equal(
    uploadTargetPresignedInitUrl("/api/cds/11/tracks/191/audio"),
    "/api/cds/11/tracks/191/audio/multipart",
  );
});

// 大容量 EPUB は音声と同じ presigned multipart を使うが、セッションはトラック
// 配下ではなく汎用の `/api/uploads/multipart`、finalize は書籍の EPUB URL 直下。
test("large EPUB uploads use the generic session and the book finalize URL", () => {
  assert.equal(uploadTargetIsEpubUpload("/api/books/7/epub"), true);
  assert.equal(uploadTargetIsEpubUpload("https://dantalian.example/api/books/7/epub"), true);
  assert.equal(uploadTargetIsEpubUpload("/api/books/7/epub?x=1"), true);
  assert.equal(uploadTargetIsEpubUpload("/api/books/7/cover"), false);
  assert.equal(uploadTargetIsEpubUpload("/api/cds/11/tracks/191/audio"), false);
  assert.equal(uploadTargetIsEpubUpload(undefined), false);

  assert.equal(
    uploadTargetPresignedSessionUrl("/api/books/7/epub"),
    "/api/uploads/multipart",
  );
  assert.equal(
    uploadTargetPresignedFinalizeUrl("/api/books/7/epub"),
    "/api/books/7/epub/finalize",
  );

  // 音声の派生 URL は従来のまま。
  assert.equal(
    uploadTargetPresignedSessionUrl("/api/books/2/tracks/9/audio"),
    "/api/books/2/tracks/9/audio/multipart",
  );
  assert.equal(
    uploadTargetPresignedFinalizeUrl("/api/books/2/tracks/9/audio"),
    "/api/books/2/tracks/9/audio/finalize",
  );
});
