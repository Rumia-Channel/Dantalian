import test from "node:test";
import assert from "node:assert/strict";

import {
  apiToken,
  audioFixture,
  baseUrl,
  enabled,
  jsonRequest,
} from "./helpers/wasabi_audio.mjs";

/// The browser builds these URLs itself (static/js/upload.js +
/// static/js/edit/tracks.js): it appends /multipart, /parts/<n>/sign, /complete,
/// and /finalize to the track audio URL. Route drift between the client and the
/// Worker is what broke the first v0.1.8 upload, so this contract pins the exact
/// browser URL shape.
test("audio multipart uses the browser session URL shape", { skip: !enabled }, async () => {
  const created = await jsonRequest("POST", "/api/cds", {
    title: `browser-url-${Date.now()}`,
    tracks: [{ track_number: 1, title: "Browser URL shape" }],
  });
  assert.equal(created.response.status, 201);
  const cd = created.body.cd;
  const track = cd.tracks[0];
  const sessionUrl = `/api/cds/${cd.id}/tracks/${track.id}/audio/multipart`;

  try {
    const { bytes } = audioFixture();
    const initialized = await jsonRequest("POST", sessionUrl, {
      expected_size: bytes.length,
      content_type: "audio/flac",
      object_kind: "audio",
      file_name: "Browser Shape.flac",
    });
    assert.equal(initialized.response.status, 200);
    const sessionId = initialized.body.id;
    const partSize = initialized.body.part_size;

    const parts = [];
    for (let offset = 0, partNumber = 1; offset < bytes.length; offset += partSize, partNumber += 1) {
      const signed = await jsonRequest(
        "POST",
        `${sessionUrl}/${sessionId}/parts/${partNumber}/sign`,
      );
      assert.equal(signed.response.status, 200);
      const chunk = bytes.subarray(offset, Math.min(offset + partSize, bytes.length));
      const uploaded = await fetch(signed.body.upload_url, { method: "PUT", body: chunk });
      assert.equal(uploaded.status, 200);
      parts.push({ part_number: partNumber, etag: uploaded.headers.get("etag") });
    }

    const completed = await jsonRequest("POST", `${sessionUrl}/${sessionId}/complete`, { parts });
    assert.equal(completed.response.status, 200);

    const finalized = await jsonRequest("POST", `/api/cds/${cd.id}/tracks/${track.id}/audio/finalize`, {
      session_id: sessionId,
      name: "Browser Shape.flac",
    });
    assert.equal(finalized.response.status, 200);
    assert.equal(finalized.body.file_hash, initialized.body.object_key.split("/").at(-1).replace(/\.flac$/, ""));
  } finally {
    await jsonRequest("DELETE", `/api/cds/${cd.id}/tracks/${track.id}`);
    await jsonRequest("DELETE", `/api/cds/${cd.id}`, {});
  }
});
