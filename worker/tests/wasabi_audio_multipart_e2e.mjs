import test from "node:test";
import assert from "node:assert/strict";

import {
  apiToken,
  audioFixture,
  baseUrl,
  enabled,
  jsonRequest,
  testPrefix,
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
    // The key is `[<prefix>/]audio/<session id>.flac`; the prefix is optional.
    assert.match(upload.objectKey, /(^|\/)audio\/[0-9a-f]{32}\.flac$/);
    if (testPrefix) {
      assert.ok(
        upload.objectKey.startsWith(`${testPrefix}/`),
        `object key must carry the deployment prefix: ${upload.objectKey}`,
      );
    }
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
    // The stream route resolves the canonical key the finalize step wrote. Read
    // the redirect target explicitly: fetching it with the API token would make
    // Wasabi reject the request for carrying two authentication mechanisms.
    const streamed = await fetch(`${baseUrl}/audio/${fileHash}`, {
      headers: { authorization: `Bearer ${apiToken}` },
      redirect: "manual",
    });
    assert.equal(streamed.status, 302);
    const location = streamed.headers.get("location");
    assert.ok(location, "stream route must redirect to a presigned URL");
    assert.ok(
      location.includes(`/audio/${fileHash}.flac?`),
      `presigned URL must point at the canonical audio object: ${location.split("?")[0]}`,
    );
    const object = await fetch(location);
    assert.equal(object.status, 206);
    assert.equal(object.headers.get("content-type"), "audio/flac");
    assert.equal(
      object.headers.get("content-range"),
      `bytes 0-0/${bytes.length}`,
      "stored object size must match the uploaded audio",
    );

    // A completed session cannot be attached twice.
    const repeated = await jsonRequest(
      "POST",
      `/api/cds/${cd.id}/tracks/${track.id}/audio/finalize`,
      { session_id: sessionId, name: "Presigned Track.flac" },
    );
    assert.equal(repeated.response.status, 409);

    // An EPUB session can never be attached to an audio track. The session must
    // be completed first, otherwise the status guard rejects it as a conflict.
    const epubSession = await jsonRequest("POST", `${MULTIPART_PATH}/init`, {
      expected_size: partSize,
      content_type: "application/epub+zip",
      file_name: "book.epub",
    });
    assert.equal(epubSession.response.status, 200);
    const epubPart = await jsonRequest(
      "POST",
      `${MULTIPART_PATH}/${epubSession.body.id}/parts/1/sign`,
    );
    assert.equal(epubPart.response.status, 200);
    const epubUpload = await fetch(epubPart.body.upload_url, {
      method: "PUT",
      body: Buffer.alloc(partSize, 9),
    });
    assert.equal(epubUpload.status, 200);
    const epubComplete = await jsonRequest(
      "POST",
      `${MULTIPART_PATH}/${epubSession.body.id}/complete`,
      { parts: [{ part_number: 1, etag: epubUpload.headers.get("etag") }] },
    );
    assert.equal(epubComplete.response.status, 200);
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
