import test from "node:test";
import assert from "node:assert/strict";

import {
  apiToken,
  audioFixture,
  baseUrl,
  enabled,
  jsonRequest,
  uploadAudioMultipart,
} from "./helpers/wasabi_audio.mjs";

const MULTIPART_PATH = "/api/uploads/multipart";
const partSize = 8 * 1024 * 1024;

test("Wasabi audio multipart finalize attaches the object to a CD track", { skip: !enabled }, async () => {
  const created = await jsonRequest("POST", "/api/cds", {
    title: `audio-multipart-${Date.now()}`,
    tracks: [{ track_number: 1, title: "Presigned audio" }],
  });
  assert.equal(created.response.status, 201);
  const cd = created.body.cd;
  const track = cd.tracks[0];
  let sessionId;

  try {
    const { bytes } = audioFixture();
    const upload = await uploadAudioMultipart(bytes, {
      fileName: "Presigned Track.flac",
      contentType: "audio/flac",
    });
    sessionId = upload.sessionId;
    assert.match(upload.objectKey, /\/audio\/[0-9a-f]{32}\.flac$/);
    assert.equal(upload.completed.response.status, 200);
    assert.equal(upload.completed.body.object_kind, "audio");
    assert.equal(upload.completed.body.expected_size, bytes.length);

    const finalized = await jsonRequest(
      "POST",
      `/api/cds/${cd.id}/tracks/${track.id}/audio/finalize`,
      { session_id: sessionId, name: "Presigned Track.flac" },
    );
    assert.equal(finalized.response.status, 200);
    const fileHash = upload.objectKey.split("/").at(-1).replace(/\.flac$/, "");
    assert.equal(finalized.body.file_hash, fileHash);
    assert.equal(finalized.body.file_name, `${fileHash}.flac`);
    assert.equal(finalized.body.object_key, upload.objectKey);

    // The stream route must resolve the canonical key the finalize step wrote.
    const streamed = await fetch(`${baseUrl}/audio/${fileHash}.flac`, {
      headers: { authorization: `Bearer ${apiToken}` },
    });
    assert.equal(streamed.status, 200);
    assert.deepEqual(Buffer.from(await streamed.arrayBuffer()), bytes);

    // A completed session cannot be attached twice.
    const repeated = await jsonRequest(
      "POST",
      `/api/cds/${cd.id}/tracks/${track.id}/audio/finalize`,
      { session_id: sessionId, name: "Presigned Track.flac" },
    );
    assert.equal(repeated.response.status, 409);

    // An EPUB session can never be attached to an audio track.
    const epubSession = await jsonRequest("POST", `${MULTIPART_PATH}/init`, {
      expected_size: partSize,
      content_type: "application/epub+zip",
    });
    assert.equal(epubSession.response.status, 200);
    const wrongKind = await jsonRequest(
      "POST",
      `/api/cds/${cd.id}/tracks/${track.id}/audio/finalize`,
      { session_id: epubSession.body.id },
    );
    assert.equal(wrongKind.response.status, 400);
    await jsonRequest("DELETE", `${MULTIPART_PATH}/${epubSession.body.id}`);
  } finally {
    await jsonRequest("DELETE", `/api/cds/${cd.id}/tracks/${track.id}`);
    await jsonRequest("DELETE", `/api/cds/${cd.id}`, {});
  }
});

test("audio multipart init rejects unsupported kinds, extensions, and content types", { skip: !enabled }, async () => {
  const badExtension = await jsonRequest("POST", `${MULTIPART_PATH}/init`, {
    expected_size: partSize,
    content_type: "audio/flac",
    object_kind: "audio",
    file_name: "track.exe",
  });
  assert.equal(badExtension.response.status, 400);

  const badContentType = await jsonRequest("POST", `${MULTIPART_PATH}/init`, {
    expected_size: partSize,
    content_type: "video/mp4",
    object_kind: "audio",
    file_name: "track.mp4",
  });
  assert.equal(badContentType.response.status, 400);

  const badKind = await jsonRequest("POST", `${MULTIPART_PATH}/init`, {
    expected_size: partSize,
    content_type: "image/png",
    object_kind: "cover",
    file_name: "cover.png",
  });
  assert.equal(badKind.response.status, 400);
});
