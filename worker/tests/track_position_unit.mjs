import test from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { addTrackToDisc } = require("../../static/js/edit/tracks.js");

async function appendTrack(numbers) {
  const keys = ["document", "prompt", "alert", "fetch"];
  const previous = new Map(keys.map((key) => [key, globalThis[key]]));
  const created = [];
  const alerts = [];
  globalThis.document = {
    querySelectorAll: () => numbers.map((number) => ({
      querySelector: () => ({ value: String(number) }),
    })),
    getElementById: () => null,
  };
  globalThis.prompt = () => "Additional track";
  globalThis.alert = (message) => alerts.push(message);
  globalThis.fetch = async (_url, init) => {
    created.push(JSON.parse(init.body));
    return new Response(null, { status: 204 });
  };
  try {
    await addTrackToDisc(1, 2, "cd");
    return { created, alerts };
  } finally {
    for (const [key, value] of previous) {
      if (value === undefined) delete globalThis[key];
      else globalThis[key] = value;
    }
  }
}

test("adding a track after manual renumbering does not reuse an occupied number", async () => {
  const { created, alerts } = await appendTrack([1, 3]);
  assert.equal(created.length, 1);
  assert.equal(created[0].track_number, 4);
  assert.equal(alerts.length, 0);
});

test("adding beyond the supported track number range does not create an overflowed track", async () => {
  const { created, alerts } = await appendTrack([2147483647]);
  assert.equal(created.length, 0);
  assert.equal(alerts.length, 1);
});
