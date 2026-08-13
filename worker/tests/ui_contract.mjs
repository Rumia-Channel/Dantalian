import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.WORKER_BASE_URL ?? "http://127.0.0.1:8793";

async function request(method, path, body) {
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers: body === undefined ? undefined : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let parsed;
  try {
    parsed = text.length === 0 ? undefined : JSON.parse(text);
  } catch {
    parsed = text;
  }
  return { status: response.status, body: parsed };
}

test("Worker responses preserve the UI book and CD contracts", async () => {
  const suffix = Date.now().toString();
  const ids = { series: undefined, grandSeries: undefined, book: undefined, cd: undefined };

  try {
    const series = await request("POST", "/api/series", { name: `ui-contract-series-${suffix}` });
    assert.equal(series.status, 201);
    ids.series = series.body.id;

    const book = await request("POST", "/api/books", {
      isbn: `978${suffix.slice(-10)}`,
      title: `ui-contract-book-${suffix}`,
    });
    assert.equal(book.status, 201);
    ids.book = book.body.book.id;

    const initialBook = await request("GET", `/api/books/${ids.book}`);
    assert.equal(initialBook.status, 200);
    assert.deepEqual(initialBook.body.authors, []);
    assert.equal(initialBook.body.copies_count, 0);
    assert.equal(initialBook.body.lent_count, 0);

    const copy = await request("POST", `/api/books/${ids.book}/copies`, { copy_type: "physical" });
    assert.equal(copy.status, 201);
    const bookWithCopy = await request("GET", `/api/books/${ids.book}`);
    assert.equal(bookWithCopy.body.copies_count, 1);

    assert.equal(
      (await request("PUT", `/api/books/${ids.book}/series`, {
        series_id: ids.series,
        series_number: 7,
      })).status,
      204,
    );
    assert.equal(
      (await request("PUT", `/api/books/${ids.book}/series`, { series_number: 8 })).status,
      204,
    );
    const updatedBook = await request("GET", `/api/books/${ids.book}`);
    assert.equal(updatedBook.body.series_id, ids.series);
    assert.equal(updatedBook.body.series_number, 8);

    const grandSeries = await request("POST", "/api/grand-series", {
      name: `ui-contract-grand-series-${suffix}`,
    });
    assert.equal(grandSeries.status, 201);
    ids.grandSeries = grandSeries.body.id;
    assert.equal(
      (await request("PUT", `/api/books/${ids.book}`, {
        title: updatedBook.body.title,
        grand_series_id: ids.grandSeries,
      })).status,
      204,
    );

    const cd = await request("POST", "/api/cds", {
      jan: `49${suffix.slice(-11)}`,
      title: `ui-contract-cd-${suffix}`,
      tracks: [{
        disc_number: 1,
        track_number: 1,
        title: "Contract track",
        duration: "03:00",
        artist: "Track artist",
        album: "Track album",
        album_artist: "Album artist",
        year: 2024,
        genre: "J-pop",
      }],
      grand_series_id: ids.grandSeries,
    });
    assert.equal(cd.status, 201);
    ids.cd = cd.body.cd.id;
    assert.equal(cd.body.cd.track_artist, "Track artist");
    assert.equal(cd.body.cd.album_artist, "Album artist");
    assert.equal(cd.body.cd.tracks.length, 1);

    const grandSeriesList = await request("GET", "/api/grand-series");
    const grandSeriesRow = grandSeriesList.body.find((value) => value.id === ids.grandSeries);
    assert.ok(grandSeriesRow.items.some((item) => item.item_type === "book" && item.item_id === ids.book));
    assert.ok(grandSeriesRow.items.some((item) => item.item_type === "cd" && item.item_id === ids.cd));
  } finally {
    if (ids.cd !== undefined) await request("DELETE", `/api/cds/${ids.cd}`);
    if (ids.book !== undefined) await request("DELETE", `/api/books/${ids.book}`);
    if (ids.grandSeries !== undefined) await request("DELETE", `/api/grand-series/${ids.grandSeries}`);
    if (ids.series !== undefined) await request("DELETE", `/api/series/${ids.series}`);
  }
});
