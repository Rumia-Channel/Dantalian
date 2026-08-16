import test from "node:test";
import assert from "node:assert/strict";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";
const authHeaders = { authorization: `Bearer ${apiToken}` };


test("audio encode endpoint delegates to the external processor", async () => {
  for (const format of ["opus", "aac"]) {
    const response = await fetch(
      `${baseUrl}/api/audio/encode/${format}`,
      {
        method: "POST",
        headers: authHeaders,
      },
    );
    assert.equal(response.status, 501);
    assert.deepEqual(await response.json(), {
      error: "audio processing requires the external processor",
      code: "audio_processing_external_required",
    });
  }
});

test("audio encode endpoint keeps invalid formats out of the external boundary", async () => {
  const response = await fetch(
    `${baseUrl}/api/audio/encode/wav`,
    {
      method: "POST",
      headers: authHeaders,
    },
  );
  assert.equal(response.status, 501);
});

async function jsonRequest(method, path, body) {
  const headers = { ...authHeaders, "content-type": "application/json" };
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers,
    body: JSON.stringify(body),
  });
  const text = await response.text();
  return {
    status: response.status,
    body: text.length === 0 ? undefined : JSON.parse(text),
  };
}

test("Worker audio metadata retains technical fields and tags", async () => {
  const created = await jsonRequest("POST", "/api/cds", {
    title: `metadata-contract-${Date.now()}`,
    tracks: [{ track_number: 1, title: "Metadata contract track" }],
  });
  assert.equal(created.status, 201);
  const cd = created.body?.cd;
  const track = cd?.tracks?.[0];
  assert.ok(cd?.id);
  assert.ok(track?.id);

  try {
    const put = await jsonRequest(
      "PUT",
      `/api/cds/${cd.id}/tracks/${track.id}/metadata`,
      {
        duration_seconds: 12.5,
        sample_rate: 44_100,
        channels: 2,
        bitrate_kbps: 320,
        tags: { title: "Metadata contract track", tracknumber: "01" },
      },
    );
    assert.equal(put.status, 204);

    const fetched = await jsonRequest(
      "GET",
      `/api/cds/${cd.id}/tracks/${track.id}/metadata`,
      undefined,
    );
    assert.equal(fetched.status, 200);
    assert.equal(fetched.body.duration_seconds, 12.5);
    assert.equal(fetched.body.sample_rate, 44_100);
    assert.equal(fetched.body.channels, 2);
    assert.equal(fetched.body.bitrate_kbps, 320);
    assert.deepEqual(fetched.body.tags, {
      title: "Metadata contract track",
      tracknumber: "01",
    });
  } finally {
    await jsonRequest("DELETE", `/api/cds/${cd.id}`, {});
  }
});

