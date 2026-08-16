import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const configPath = new URL("../../wrangler.audio.ci.toml", import.meta.url);

function requiredConfigValue(config, key, value) {
  assert.match(config, new RegExp(`${key} = \\"${value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\"`));
}

test("audio controller config attaches the target queue and Worker service", async () => {
  const target = process.env.DANTALIAN_DEPLOY_TARGET ?? "develop";
  const queue = process.env.DANTALIAN_AUDIO_JOB_QUEUE ?? `dantalian-audio-${target}`;
  const deadLetterQueue =
    process.env.DANTALIAN_AUDIO_JOB_DLQ ?? `${queue}-dlq`;
  const config = await readFile(configPath, "utf8");
  assert.match(config, /binding = "DANTALIAN_API"/);
  requiredConfigValue(config, "service", `dantalian-worker-${target}`);
  requiredConfigValue(config, "queue", queue);
  requiredConfigValue(config, "dead_letter_queue", deadLetterQueue);
  assert.match(config, /binding = "WASABI_ENDPOINT_STORE"/);
  assert.match(config, /max_concurrency = 2/);
});
