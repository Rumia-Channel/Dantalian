import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";

test("Worker license page includes the shared shell and dependency licenses", async () => {
  const response = await fetch(`${baseUrl}/licenses/`);
  const html = await response.text();

  assert.equal(response.status, 200);
  assert.match(html, /js\/header\.js\?v=/);
  assert.match(html, /class="licenses-page"/);
  assert.match(html, /class="license-entry"/);
  assert.match(html, /依存クレート/);
  assert.match(html, /Cargo\.lock|Cargo/);
  assert.doesNotMatch(html, /依存ライセンス一覧はビルド時に生成されます/);
});
