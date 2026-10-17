import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const record = await readFile(path.join(root, "worker/tests/fixtures/ndl_v3.xml"), "utf8");
const openLibrary = await readFile(path.join(root, "worker/tests/fixtures/open_library_search.json"), "utf8");
const empty = await readFile(path.join(root, "worker/tests/fixtures/ndl_no_records.xml"), "utf8");

export async function createNdlFixture() {
  const events = [];
  let active = 0;
  let maximumActive = 0;
  const outbound = async (request, response) => {
    const url = new URL(request.url, `http://${request.headers.host}`);
    assert.ok(["ndlsearch.ndl.go.jp", "id.ndl.go.jp", "openlibrary.org"].includes(url.hostname), `Unexpected external request: ${url}`);
    const event = { kind: url.hostname === "openlibrary.org" ? "open_library" : url.hostname === "id.ndl.go.jp" ? "authority" : "sru", started: performance.now() };
    events.push(event);
    maximumActive = Math.max(maximumActive, ++active);
    response.once("close", () => {
      event.finished = performance.now();
      event.cancelled = !response.writableFinished;
      --active;
    });
    let xml = empty;
    let status = 200;
    let stalled = false;
    if (event.kind === "sru") {
      // Deliberately no automatic 10/13 conversion: some NDL records need an explicit alternate ISBN.
      const terms = [...url.searchParams.get("query").matchAll(/isbn="([0-9X]+)"/g)].map(match => match[1]);
      if (terms.includes("4041164699")) {
        xml = book("ISBN10-only record");
      } else if (terms.includes("9780262033848")) {
        xml = book("ISBN13-only record");
      } else if (terms.includes("9780000000002")) {
        status = 503;
        xml = "upstream unavailable";
      } else if (terms.includes("9780000000019")) {
        xml = "<searchRetrieveResponse>";
        stalled = true;
      } else if (terms.includes("9780000000033")) {
        xml = book("After timeout", true);
      }
    } else if (event.kind === "open_library") {
      assert.equal(url.pathname, "/search.json");
      const query = url.searchParams.get("q");
      if (query.includes("9780140328721") && url.searchParams.get("fields").split(",").includes("key")) {
        xml = openLibrary;
      } else if (query.includes("9790000000015")) {
        status = 429;
        xml = "rate limited";
      } else if (query.includes("9790000000022")) {
        xml = '{"docs":[';
      } else {
        xml = '{"docs":[]}';
      }
    } else {
      request.setEncoding("utf8");
      let body = "";
      for await (const chunk of request) body += chunk;
      const form = new URLSearchParams(body);
      assert.equal(url.pathname, "/auth/ndla/sparql");
      assert.equal(form.get("format"), "json");
      xml = JSON.stringify({ results: { bindings: [
        { slot: { type: "typed-literal", value: "0" }, id: { type: "uri", value: "http://id.ndl.go.jp/auth/entity/00046801" } },
      ] } });
    }
    response.writeHead(status, { "content-type": event.kind === "sru" ? "application/xml" : "application/json" });
    const split = Math.floor(xml.length / 2);
    response.write(xml.slice(0, split));
    if (stalled) return;
    // Longer than the cooldown: releasing a permit at response headers would overlap bodies.
    await delay(events.length === 1 ? 2700 : 100);
    if (!response.destroyed) response.end(xml.slice(split));
  };
  function config(name) {
    return {
      name,
      modulesRoot: path.join(root, "worker/build"),
      modules: [
        { type: "ESModule", path: path.join(root, "worker/build/index.js") },
        { type: "CompiledWasm", path: path.join(root, "worker/build/index_bg.wasm") },
      ],
      compatibilityDate: "2026-08-11",
      bindings: { DANTALIAN_AUTH_REQUIRED: "false" },
      d1Databases: { DB: "ndl-contract" },
      durableObjects: { NDL_COORDINATOR: { className: "NdlCoordinator", scriptName: "coordinator", useSQLite: true } },
      // Raw HTTP close events let the timeout test observe upstream cancellation.
      outboundService: { node: outbound },
    };
  }
  const mf = new Miniflare(convertV4MiniflareOptions({ workers: [config("api-a"), config("api-b"), config("coordinator")] }));
  try {
    const db = await mf.getD1Database("DB", "api-a");
    for (const filename of (await readdir(path.join(root, "worker/migrations"))).filter(name => name.endsWith(".sql")).sort()) {
      const sql = await readFile(path.join(root, "worker/migrations", filename), "utf8");
      await db.exec(sql.replace(/--[^\n]*/g, "").replace(/\s+/g, " "));
    }
    const clients = await Promise.all([mf.getWorker("api-a"), mf.getWorker("api-b")]);
    return {
      events,
      get maximumActive() { return maximumActive; },
      async register(client, isbn) {
        const response = await clients[client].fetch("http://localhost/api/books", {
          method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ isbn, cover_url: "fixture.jpg" }),
        });
        return { status: response.status, body: await response.json() };
      },
      async close() { await mf.dispose(); },
    };
  } catch (error) {
    await mf.dispose();
    throw error;
  }
}

function book(title, knownOnly = false) {
  let xml = record.replaceAll("NDL v3 fixture", title);
  if (knownOnly) {
    xml = xml.replace(/<dcterms:creator>[\s\S]*?<\/dcterms:creator>/g, creator => creator.includes("00054222") ? creator : "");
  }
  return xml;
}
