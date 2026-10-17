import test from "node:test";
import assert from "node:assert/strict";
import { createNdlFixture } from "./helpers/ndl_fixture.mjs";

function assertBook(result, title) {
  assert.equal(result.status, 201, JSON.stringify(result.body));
  assert.equal(result.body.book.title, title);
  assert.equal(result.body.book.jpno, "12345678");
  const authors = result.body.book.authors;
  assert.equal(authors.find(author => author.name === "夏目 漱石").ndl_id, "00054222");
  assert.equal(authors.find(author => author.name === "森 鴎外").ndl_id, "00046801");
  assert.equal(authors.find(author => author.name === "Fixture Other Author").ndl_id, null);
}

test("different API isolates preserve explicit ISBN alternatives and serialize batched NDL requests through full bodies", { timeout: 90000 }, async t => {
  const fixture = await createNdlFixture();
  try {
    const [tenOnly, thirteenOnly, failed] = await Promise.all([
      fixture.register(0, "9784041164693"),
      fixture.register(1, "0262033844"),
      fixture.register(0, "9780000000002"),
    ]);
    assertBook(tenOnly, "ISBN10-only record");
    assertBook(thirteenOnly, "ISBN13-only record");
    assert.equal(failed.status, 502, JSON.stringify(failed.body));
    assert.equal(fixture.maximumActive, 1, "NDL response bodies from different API isolates must not overlap");
    assert.equal(fixture.events.filter(event => event.kind === "sru").length, 5);
    assert.equal(fixture.events.filter(event => event.kind === "authority").length, 2, "Two unresolved names per book need one batch, not two requests");
    assert.equal(fixture.events.filter(event => event.kind === "open_library").length, 0, "Neither NDL hits nor NDL failures should invoke the fallback");
    const events = fixture.events.toSorted((a, b) => a.started - b.started);
    const idle = events.slice(1).map((event, index) => event.started - events[index].finished);
    assert.ok(idle.every(ms => ms >= 1900), `Idle gaps: ${JSON.stringify(idle)}`);
    t.diagnostic(`Maximum NDL concurrency ${fixture.maximumActive}; idle gaps in ms: ${idle.map(Math.round).join(", ")}`);
  } finally {
    await fixture.close();
  }
});

test("a stalled NDL response body times out without keeping the shared coordinator locked", { timeout: 45000 }, async () => {
  const fixture = await createNdlFixture();
  try {
    const stalled = await fixture.register(0, "9780000000019");
    assert.equal(stalled.status, 502, JSON.stringify(stalled.body));
    const next = await fixture.register(1, "9780000000033");
    assert.equal(next.status, 201, JSON.stringify(next.body));
    assert.equal(next.body.book.title, "After timeout");
    assert.equal(fixture.maximumActive, 1);
    assert.equal(fixture.events[0].cancelled, true, "A timed-out body must close its upstream HTTP connection");
  } finally {
    await fixture.close();
  }
});

test("Open Library fills an NDL miss using the matching ISBN edition and the result is cached", { timeout: 45000 }, async () => {
  const fixture = await createNdlFixture();
  try {
    const result = await fixture.register(0, "0140328726");
    assert.equal(result.status, 201, JSON.stringify(result.body));
    assert.equal(result.body.source, "open_library");
    assert.equal(result.body.book.title, "Fantastic Mr. Fox");
    assert.equal(result.body.book.publisher, "Puffin");
    assert.equal(result.body.book.publish_date, "1988-10-01");
    assert.equal(result.body.book.ndl_url, null);
    assert.equal(result.body.book.jpno, null);
    assert.equal(result.body.book.authors[0].name, "Roald Dahl");
    assert.equal(result.body.book.authors[0].ndl_id, null);
    assert.equal(fixture.events.filter(event => event.kind === "open_library").length, 1);
    assert.equal(fixture.events.filter(event => event.kind === "authority").length, 0);
    assert.equal(fixture.maximumActive, 1);
    const requests = fixture.events.length;
    const cached = await fixture.register(1, "0140328726");
    assert.equal(cached.status, 200);
    assert.equal(cached.body.source, "cache");
    assert.equal(cached.body.book.id, result.body.book.id);
    assert.equal(fixture.events.length, requests, "A saved book must not be fetched again");
  } finally {
    await fixture.close();
  }
});

test("Open Library distinguishes an absent book from rate limiting and malformed responses", { timeout: 45000 }, async () => {
  const fixture = await createNdlFixture();
  try {
    assert.equal((await fixture.register(0, "9798999999999")).status, 404);
    assert.equal((await fixture.register(1, "9790000000015")).status, 502);
    assert.equal((await fixture.register(0, "9790000000022")).status, 502);
    assert.equal(fixture.events.filter(event => event.kind === "open_library").length, 3, "Fallback failures must not be retried");
    assert.equal(fixture.maximumActive, 1);
  } finally {
    await fixture.close();
  }
});
