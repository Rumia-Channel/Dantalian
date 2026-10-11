import test from "node:test";
import assert from "node:assert/strict";

import { audioFixture, enabled, jsonRequest } from "./helpers/wasabi_audio.mjs";

/// The playlist API joins `cds` for the cover and every track row, so it is the
/// most schema-sensitive endpoint in the Worker. A `playlists.cover_url` column
/// that only exists in the native schema made `GET /api/playlists` answer 500.
test("playlist API serves the list, the cover, and the joined tracks", { skip: !enabled }, async () => {
  const created = await jsonRequest("POST", "/api/cds", {
    title: `playlist-contract-${Date.now()}`,
    tracks: [{ track_number: 1, title: "Playlist contract track" }],
  });
  assert.equal(created.response.status, 201);
  const cd = created.body.cd;
  const track = cd.tracks[0];
  let playlistId;

  try {
    const { bytes } = audioFixture();
    const sessionUrl = `/api/cds/${cd.id}/tracks/${track.id}/audio/multipart`;
    const initialized = await jsonRequest("POST", sessionUrl, {
      expected_size: bytes.length,
      content_type: "audio/flac",
      object_kind: "audio",
      file_name: "Playlist Track.flac",
    });
    assert.equal(initialized.response.status, 200);
    const sessionId = initialized.body.id;
    const partSize = initialized.body.part_size;
    const parts = [];
    for (let offset = 0, partNumber = 1; offset < bytes.length; offset += partSize, partNumber += 1) {
      const signed = await jsonRequest("POST", `${sessionUrl}/${sessionId}/parts/${partNumber}/sign`);
      assert.equal(signed.response.status, 200);
      const uploaded = await fetch(signed.body.upload_url, {
        method: "PUT",
        body: bytes.subarray(offset, Math.min(offset + partSize, bytes.length)),
      });
      assert.equal(uploaded.status, 200);
      parts.push({ part_number: partNumber, etag: uploaded.headers.get("etag") });
    }
    const completed = await jsonRequest("POST", `${sessionUrl}/${sessionId}/complete`, { parts });
    assert.equal(completed.response.status, 200);
    const finalized = await jsonRequest(
      "POST",
      `/api/cds/${cd.id}/tracks/${track.id}/audio/finalize`,
      { session_id: sessionId, name: "Playlist Track.flac" },
    );
    assert.equal(finalized.response.status, 200);
    const fileHash = finalized.body.file_hash;

    // The empty list, the schema-sensitive read that used to answer 500.
    const emptyList = await jsonRequest("GET", "/api/playlists");
    assert.equal(emptyList.response.status, 200);
    assert.ok(Array.isArray(emptyList.body));

    const playlist = await jsonRequest("POST", "/api/playlists", {
      name: `Contract playlist ${Date.now()}`,
      description: "playlist contract",
      cover_cd_id: cd.id,
    });
    assert.equal(playlist.response.status, 201);
    playlistId = playlist.body.id;
    // The cover comes from the joined CD, not from the playlist row.
    assert.equal(playlist.body.cover_cd_id, cd.id);

    const withTrack = await jsonRequest("PUT", `/api/playlists/${playlistId}`, {
      track_ids: [track.id],
    });
    assert.equal(withTrack.response.status, 200);

    const fetched = await jsonRequest("GET", `/api/playlists/${playlistId}`);
    assert.equal(fetched.response.status, 200);
    assert.equal(fetched.body.tracks.length, 1);
    assert.equal(fetched.body.tracks[0].file_hash, fileHash);

    const listed = await jsonRequest("GET", "/api/playlists");
    assert.equal(listed.response.status, 200);
    const listedEntry = listed.body.find((entry) => entry.id === playlistId);
    assert.ok(listedEntry, "created playlist must appear in the list");
    assert.equal(listedEntry.tracks.length, 1);
    assert.equal(listedEntry.tracks[0].file_hash, fileHash);
  } finally {
    if (playlistId !== undefined) {
      await jsonRequest("DELETE", `/api/playlists/${playlistId}`);
    }
    await jsonRequest("DELETE", `/api/cds/${cd.id}/tracks/${track.id}`);
    await jsonRequest("DELETE", `/api/cds/${cd.id}`, {});
  }
});
