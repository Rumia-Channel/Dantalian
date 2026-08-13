import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";
const apiToken = process.env.WORKER_API_TOKEN ?? "dantalian-ci-test-token";
const headers = {
  authorization: `Bearer ${apiToken}`,
  "content-type": "application/json",
};

async function createJob(body) {
  const response = await fetch(`${baseUrl}/api/audio/jobs`, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  return { response, body: await response.json() };
}

test("audio jobs reject unsafe object keys before touching external storage", async () => {
  const { response, body } = await createJob({
    input_object_key: "audio/original.mp3",
    output_object_key: "audio/../encoded.opus",
    codec: "opus",
  });
  assert.equal(response.status, 400);
  assert.match(body.error, /invalid object key/i);
});

test("audio jobs enforce codec-specific output and bitrate bounds", async () => {
  const wrongExtension = await createJob({
    input_object_key: "audio/original.mp3",
    output_object_key: "audio/encoded.aac",
    codec: "opus",
  });
  assert.equal(wrongExtension.response.status, 400);
  assert.match(wrongExtension.body.error, /must end with \.opus/i);

  const invalidBitrate = await createJob({
    input_object_key: "audio/original.mp3",
    output_object_key: "audio/encoded.opus",
    codec: "opus",
    bitrate_kbps: 1,
  });
  assert.equal(invalidBitrate.response.status, 400);
  assert.match(invalidBitrate.body.error, /between 8 and 512/i);
});

test("audio job lookup rejects malformed identifiers without querying D1", async () => {
  const response = await fetch(`${baseUrl}/api/audio/jobs/not-a-job-id`, {
    headers: { authorization: `Bearer ${apiToken}` },
  });
  assert.equal(response.status, 400);
  assert.deepEqual(await response.json(), { error: "invalid audio job id" });
});
