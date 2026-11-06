import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import test from "node:test";
import vm from "node:vm";

const authSource = await readFile(new URL("../../static/js/auth.js", import.meta.url), "utf8");
const utilsSource = await readFile(new URL("../../static/js/utils.js", import.meta.url), "utf8");
const revisionKey = "dantalian_data_revision";

async function fixture(t) {
  const books = [{ id: 1, title: "Original" }];
  const tabs = [];
  const stored = new Map();
  let reads = 0;
  let gate = null;
  const server = createServer(async (req, res) => {
    res.setHeader("content-type", "application/json");
    if (req.url === "/api/health") return res.end('{"authentication_required":false}');
    if (req.method === "GET") {
      reads += 1;
      const page = JSON.stringify({ items: books.slice(), next_cursor: null });
      if (gate) {
        const current = gate;
        gate = null;
        current.started();
        await current.wait;
      }
      return res.end(page);
    }
    let text = "";
    for await (const chunk of req) text += chunk;
    const body = JSON.parse(text);
    if (!body.title) { res.statusCode = 400; return res.end('{"error":"title required"}'); }
    const book = { id: books.length + 1, title: body.title };
    books.push(book);
    res.statusCode = 201;
    res.end(JSON.stringify(book));
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(() => new Promise(resolve => { server.closeAllConnections(); server.close(resolve); }));
  const origin = `http://127.0.0.1:${server.address().port}`;

  function tab() {
    const events = new EventTarget();
    const document = Object.assign(new EventTarget(), {
      visibilityState: "visible", getElementById: () => null, cookie: "",
    });
    const session = new Map();
    const state = { titles: [], pending: Promise.resolve() };
    const context = vm.createContext({
      URL, URLSearchParams, Request, Response, Headers, console, setTimeout,
      location: new URL(origin), document,
      addEventListener: events.addEventListener.bind(events),
      dispatchEvent: events.dispatchEvent.bind(events),
      fetch: (input, init) => fetch(new URL(input, origin), init),
      sessionStorage: {
        getItem: key => session.get(key) ?? null,
        setItem: (key, value) => session.set(key, value),
        removeItem: key => session.delete(key),
      },
      localStorage: {
        getItem: key => stored.get(key) ?? null,
        setItem(key, value) {
          stored.set(key, value);
          for (const other of tabs) {
            if (other !== events) queueMicrotask(() => other.dispatchEvent(Object.assign(new Event("storage"), { key })));
          }
        },
      },
    });
    context.window = context;
    vm.runInContext(authSource + "\n" + utilsSource, context);
    tabs.push(events);
    const refresh = vm.runInContext("observeDataChanges", context)(() => {
      state.pending = context.loadBooks().then(ok => {
        if (ok) state.titles = JSON.parse(vm.runInContext("JSON.stringify(allBooks.map(book => book.title))", context));
        return ok;
      });
      return state.pending;
    });
    return {
      state, refresh,
      async create(title) {
        return context.fetch("/api/books/manual", { method: "POST", body: JSON.stringify({ title }) });
      },
      visible(value) {
        document.visibilityState = value ? "visible" : "hidden";
        document.dispatchEvent(new Event("visibilitychange"));
      },
      restore() { events.dispatchEvent(Object.assign(new Event("pageshow"), { persisted: true })); },
      leave() {
        document.visibilityState = "hidden";
        document.dispatchEvent(new Event("visibilitychange"));
        events.dispatchEvent(Object.assign(new Event("pagehide"), { persisted: true }));
      },
      returnTo() {
        document.visibilityState = "visible";
        document.dispatchEvent(new Event("visibilitychange"));
        events.dispatchEvent(Object.assign(new Event("pageshow"), { persisted: true }));
      },
    };
  }
  return {
    tab, books, reads: () => reads,
    async settle(page, title) {
      // The observer may need one more read when a write arrives during a read.
      for (let attempt = 0; attempt < 20; attempt += 1) {
        await page.state.pending;
        if (page.state.titles.includes(title)) return;
        await new Promise(resolve => setImmediate(resolve));
      }
      assert.fail(`Catalogue never displayed ${title}`);
    },
    holdRead() {
      let release, started;
      const wait = new Promise(resolve => { release = resolve; });
      const ready = new Promise(resolve => { started = resolve; });
      gate = { wait, started };
      return { ready, release };
    },
    revision: () => stored.get(revisionKey),
  };
}

test("another tab's successful registration updates the catalogue without a reload; failures do not reread D1", async t => {
  const f = await fixture(t);
  const catalogue = f.tab();
  const registration = f.tab();
  await catalogue.refresh();
  assert.equal((await registration.create("New work")).status, 201);
  await f.settle(catalogue, "New work");
  assert.deepEqual(catalogue.state.titles, ["Original", "New work"]);
  assert.equal(f.reads(), 2);
  const revision = f.revision();
  assert.equal((await registration.create("")).status, 400);
  assert.equal(f.revision(), revision);
  assert.equal(f.reads(), 2);
});

test("hidden catalogue tabs coalesce registrations into one read when shown", async t => {
  const f = await fixture(t);
  const catalogue = f.tab();
  const registration = f.tab();
  await catalogue.refresh();
  catalogue.visible(false);
  for (const title of ["Second", "Third", "Fourth"]) await registration.create(title);
  assert.deepEqual(catalogue.state.titles, ["Original"]);
  assert.equal(f.reads(), 1);
  catalogue.visible(true);
  await f.settle(catalogue, "Fourth");
  assert.deepEqual(catalogue.state.titles, ["Original", "Second", "Third", "Fourth"]);
  assert.equal(f.reads(), 2);
});

test("back-forward restoration rereads even without a local revision, including writes racing a read", async t => {
  const f = await fixture(t);
  const catalogue = f.tab();
  const registration = f.tab();
  await catalogue.refresh();
  f.books.push({ id: 2, title: "Other device" });
  catalogue.restore();
  await f.settle(catalogue, "Other device");
  const held = f.holdRead();
  catalogue.restore();
  await held.ready;
  await registration.create("During read");
  await registration.create("Latest");
  held.release();
  await f.settle(catalogue, "Latest");
  assert.deepEqual(catalogue.state.titles, ["Original", "Other device", "During read", "Latest"]);
  assert.equal(f.reads(), 4);
});

test("a tab's own mutation does not duplicate its explicit UI refresh with a whole-catalogue read", async t => {
  const f = await fixture(t);
  const catalogue = f.tab();
  await catalogue.refresh();
  await catalogue.create("Own edit");
  assert.ok(f.revision());
  assert.equal(f.reads(), 1);
});

test("restoration visibility, storage and pageshow events cause only one fresh read", async t => {
  const f = await fixture(t);
  const catalogue = f.tab();
  const registration = f.tab();
  await catalogue.refresh();
  catalogue.leave();
  await registration.create("Registered while away");
  catalogue.returnTo();
  await f.settle(catalogue, "Registered while away");
  await catalogue.state.pending;
  assert.deepEqual(catalogue.state.titles, ["Original", "Registered while away"]);
  assert.equal(f.reads(), 2);
});

test("a read begun before navigation cannot replace restoration revalidation", async t => {
  const f = await fixture(t);
  const catalogue = f.tab();
  await catalogue.refresh();
  const held = f.holdRead();
  const inFlight = catalogue.refresh();
  await held.ready;
  catalogue.leave();
  f.books.push({ id: 2, title: "Changed while away" });
  catalogue.returnTo();
  held.release();
  await inFlight;
  assert.deepEqual(catalogue.state.titles, ["Original", "Changed while away"]);
  assert.equal(f.reads(), 3);
});
