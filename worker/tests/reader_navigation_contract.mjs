import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { randomInt, randomUUID } from 'node:crypto';

const baseUrl = process.env.WORKER_BASE_URL ?? 'http://127.0.0.1:8793';
const token = process.env.WORKER_API_TOKEN ?? 'dantalian-ci-test-token';
const epub = Buffer.from((await readFile(new URL('fixtures/reader.epub.base64', import.meta.url), 'utf8')).trim(), 'base64');
const pdf = await readFile(new URL('fixtures/reader.pdf', import.meta.url));
async function request(method, path, body) {
    const response = await fetch(baseUrl + path, { method, headers: { authorization: `Bearer ${token}`, ...(body === undefined ? {} : { 'content-type': 'application/json' }) }, body: body === undefined ? undefined : JSON.stringify(body) });
    return { status: response.status, headers: response.headers, body: await response.json().catch(() => undefined) };
}
async function upload(id, name = 'fixture.epub', bytes = epub) {
    const form = new FormData();
    form.append(process.env.READER_NAVIGATION_BACKEND === 'native' ? 'epub' : 'file', new Blob([bytes]), name);
    const response = await fetch(`${baseUrl}/api/books/${id}/epub`, { method: 'POST', headers: { authorization: `Bearer ${token}` }, body: form });
    assert.equal(response.status, 200, await response.text());
    return (await request('GET', `/api/books/${id}`)).body;
}
const navigation = (book, returnTo = '/books/?search=作品#selected', hash = book.epub_file_hash) => request('GET', '/api/reader-navigation?' + new URLSearchParams({ book_key: 'book:' + book.id, content_hash: hash, return_to: returnTo }));
function check(result, status, code) {
    assert.equal(result.status, status, JSON.stringify(result.body));
    assert.equal(result.headers.get('cache-control'), 'no-store');
    if(code) assert.equal(result.body.code, code);
}

// Run this same contract against the real Native binary and the local Worker.
test('reader navigation selects immediate registered volumes, preserves origin and never writes reading state', async () => {
    const cleanup = [];
    try {
        const createdSeries = await request('POST', '/api/series', { name: 'Navigation-' + randomUUID() }); assert.equal(createdSeries.status, 201);
        const series = createdSeries.body; cleanup.push(() => request('DELETE', '/api/series/' + series.id));
        const books = [];
        for(const number of [1, 4, 4, null]) {
            const created = await request('POST', '/api/books/manual', { title: `Navigation volume ${number}`, isbn: '978' + String(randomInt(10_000_000_000)).padStart(10, '0') }); assert.equal(created.status, 201, JSON.stringify(created.body));
            const book = created.body.book ?? created.body; books.push(book); cleanup.unshift(() => request('DELETE', '/api/books/' + book.id));
            assert.equal((await request('PUT', `/api/books/${book.id}/series`, { series_id: series.id, series_number: number })).status, 204);
        }
        const first = await upload(books[0].id), current = await upload(books[1].id), last = await upload(books[3].id);
        assert.equal((await request('PUT', `/api/books/${current.id}`, { title: current.title, reading_status: 'reading' })).status, 204);
        const before = (await request('GET', `/api/books/${current.id}`)).body;
        let result = await navigation(current); check(result, 200);
        assert.equal(result.body.previous.bookKey, 'book:' + first.id);
        const previous = new URL(result.body.previous.href, baseUrl);
        assert.equal(previous.pathname, '/reader/'); assert.equal(previous.searchParams.get('book'), String(first.id)); assert.equal(previous.searchParams.get('entry'), 'end'); assert.equal(previous.searchParams.get('from'), '/books/?search=作品#selected');
        assert.equal(result.body.next.bookKey, 'book:' + books[2].id); assert.equal(result.body.next.status, 'missing-file'); assert.equal(result.body.next.href, null);
        result = await navigation(first); check(result, 200); assert.equal(result.body.previous, null); assert.equal(result.body.next.bookKey, 'book:' + current.id);
        result = await navigation(last); check(result, 200); assert.equal(result.body.previous.bookKey, 'book:' + books[2].id); assert.equal(result.body.next, null);
        check(await navigation({ ...books[2], epub_file_hash: 'unknown' }), 422, 'BOOK_UNAVAILABLE');
        await upload(books[2].id, 'unsupported.pdf', pdf); result = await navigation(current); check(result, 200); assert.equal(result.body.next.status, 'unsupported-format'); assert.equal(result.body.next.href, null);
        const changedBytes = Buffer.from(epub); changedBytes.writeUInt16LE(2, changedBytes.length - 2);
        const replacement = await upload(current.id, 'replacement.epub', Buffer.concat([changedBytes, Buffer.from('v2')]));
        check(await navigation(current), 409, 'CONTENT_CHANGED'); check(await navigation(replacement), 200);
        assert.equal((await request('DELETE', '/api/books/' + books[2].id)).status, 204);
        result = await navigation(replacement); check(result, 200); assert.equal(result.body.next.bookKey, 'book:' + last.id); assert.equal(new URL(result.body.next.href, baseUrl).searchParams.get('entry'), 'start');
        assert.equal((await request('PUT', `/api/books/${current.id}/series`, { series_id: null })).status, 204);
        result = await navigation(replacement); check(result, 200); assert.equal(result.body.series, null); assert.equal(result.body.previous, null); assert.equal(result.body.next, null);
        for(const unsafe of ['//other.example/books/', 'https://other.example/books/', '/admin/', '/books/../admin/', '/\\other.example/']) {
            result = await navigation(replacement, unsafe); check(result, 200); assert.equal(result.body.returnTo.href, '/books/');
        }
        const after = (await request('GET', `/api/books/${current.id}`)).body;
        assert.equal(after.reading_status, before.reading_status);
    } finally { for(const remove of cleanup) await remove(); }
});

test('reader navigation returns stable no-store errors for malformed or missing identity', async () => {
    for(const query of ['', 'book_key=book%3A1', 'book_key=book%3A01&content_hash=x', 'book_key=book%3A0&content_hash=x', 'book_key=book%3A9223372036854775808&content_hash=x', 'book_key=book%3A1&content_hash=x&content_hash=y']) check(await request('GET', '/api/reader-navigation?' + query), 400, 'INVALID_REQUEST');
    check(await request('GET', '/api/reader-navigation?book_key=book%3A9223372036854775807&content_hash=x'), 404, 'BOOK_NOT_FOUND');
    if(process.env.WORKER_EXPECT_AUTH_REQUIRED === 'true') {
        const response = await fetch(baseUrl + '/api/reader-navigation?book_key=book%3A1&content_hash=x');
        assert.equal(response.status, 401); assert.equal(response.headers.get('cache-control'), 'no-store');
    }
});
