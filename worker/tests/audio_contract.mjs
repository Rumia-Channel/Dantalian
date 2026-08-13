import test from "node:test";
import assert from "node:assert/strict";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";


test("audio encode endpoint delegates to the external processor", async () => {
  for (const format of ["opus", "aac"]) {
    const response = await fetch(
      `${baseUrl}/api/audio/encode/${format}`,
      { method: "POST" },
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
    { method: "POST" },
  );
  assert.equal(response.status, 501);
});

