import assert from 'node:assert/strict';
import { after, before, beforeEach, test } from 'node:test';
import { playwright, startCacheFixture } from './fixture.mjs';

let fixture, browser, context, page;
before(async () => { fixture = await startCacheFixture(); browser = await playwright.chromium.launch({ headless: true }); });
beforeEach(async () => {
    if (context) await context.close();
    context = await browser.newContext(); page = await context.newPage();
    Object.assign(fixture.state, { revision: '2026-01-01T00:00:00Z', edition: 'a', epubStatus: 200, metadataStatus: 200, sourceDownloads: 0, s3Downloads: 0, metadataReads: 0, settingsWrites: 0, delay: null });
    await page.goto(fixture.origin + '/login');
    await page.addScriptTag({ url: fixture.origin + '/js/epub-cache.js' });
});
after(async () => { await browser?.close(); await fixture?.close(); });

function readerUrl(kind = 'narou') {
    const url = new URL('/vendor/bibi/dantalian.html', fixture.origin);
    url.searchParams.set('book', kind === 'narou' ? '/api/narou/novels/1/epub' : '/epubs/immutable.epub');
    if (kind !== 'narou') url.searchParams.set('book-key', 'book:1');
    return url.href;
}
async function ready(target = page) {
    await target.waitForFunction(() => window.Bibi?.API?.getState().status === 'ready' && Bibi.API.getState().operable, { timeout: 20000 });
}
async function contents(target = page) {
    return (await Promise.all(target.frames().map(frame => frame.locator('body').innerText().catch(() => '')))).join('\n');
}
async function stats() { return page.evaluate(() => epubCache.stats()); }

test('immutable S3 EPUB reopens from device bytes without another object download', async () => {
    await page.goto(readerUrl('s3')); await ready();
    assert.match(await contents(), /Revision A: the complete original EPUB/);
    assert.equal(fixture.state.s3Downloads, 1);
    await page.reload(); await ready();
    assert.match(await contents(), /Revision A: the complete original EPUB/);
    assert.equal(fixture.state.s3Downloads, 1);
    assert.equal((await stats()).count, 1);
});

test('narou last_update reuses unchanged bytes and replaces an updated EPUB', async () => {
    await page.goto(readerUrl()); await ready();
    assert.match(await contents(), /Revision A: the complete original EPUB/);
    const originalHash = new URL(page.url()).searchParams.get('content-hash');
    await page.reload(); await ready();
    assert.match(await contents(), /Revision A: the complete original EPUB/);
    assert.equal(fixture.state.sourceDownloads, 1);
    Object.assign(fixture.state, { revision: '2026-01-02T00:00:00Z', edition: 'b' });
    await page.reload(); await ready();
    assert.match(await contents(), /Revision B: the updated EPUB replaces the original/);
    assert.notEqual(new URL(page.url()).searchParams.get('content-hash'), originalHash);
    assert.equal(fixture.state.sourceDownloads, 2);
    assert.equal((await stats()).count, 1);
    await page.reload(); await ready();
    assert.match(await contents(), /Revision B: the updated EPUB replaces the original/);
    assert.equal(fixture.state.sourceDownloads, 2);
});

test('failed update preserves the real 503 and never supplies old bytes as the new version', async () => {
    await page.goto(readerUrl()); await ready();
    Object.assign(fixture.state, { revision: '2026-01-02T00:00:00Z', edition: 'b', epubStatus: 503 });
    await page.reload();
    await page.waitForFunction(() => window.Bibi?.API?.getState().status === 'error');
    assert.match(await page.locator('body').innerText(), /503/);
    assert.doesNotMatch(await contents(), /Revision A: the complete original EPUB/);
    assert.equal(fixture.state.sourceDownloads, 2);
    assert.equal((await stats()).count, 1);
    fixture.state.epubStatus = 200;
    await page.reload(); await ready();
    assert.match(await contents(), /Revision B: the updated EPUB replaces the original/);
});

test('cached private book is not served when fresh source authentication fails', async () => {
    await page.goto(readerUrl()); await ready();
    fixture.state.metadataStatus = 401;
    await page.reload();
    await page.getByRole('alert').waitFor();
    assert.doesNotMatch(await contents(), /Revision A: the complete original EPUB/);
    assert.equal(fixture.state.sourceDownloads, 1);
});

test('byte limit, LRU, shrink, version ordering and clear generation are atomic', async () => {
    const result = await page.evaluate(async () => {
        await epubCache.setLimitBytes(10);
        let generation = (await epubCache.settings()).generation;
        const put = (key, revision = 'a', revisionTime = 1) => epubCache.put({ key, revision, revisionTime, blob: new Blob(['12345']) }, generation);
        await put('a'); await put('b'); await epubCache.read('a', 'a'); await put('c');
        const evicted = await epubCache.read('b', 'a');
        const retained = !!await epubCache.read('a', 'a');
        const full = await epubCache.stats();
        const shrink = await epubCache.setLimitBytes(5);
        generation = (await epubCache.settings()).generation;
        await put('version', 'new', 200);
        const older = await put('version', 'old', 100);
        const newSurvives = !!await epubCache.read('version', 'new');
        const oversize = await epubCache.put({ key: 'large', revision: 'a', revisionTime: null, blob: new Blob(['123456']) }, generation);
        await epubCache.clear();
        const stale = await put('late');
        const empty = await epubCache.stats();
        await epubCache.setLimitBytes(0);
        generation = (await epubCache.settings()).generation;
        const disabled = await put('disabled');
        return { evicted, retained, full, shrink, older, newSurvives, oversize, stale, empty, disabled };
    });
    assert.equal(result.evicted, null); assert.equal(result.retained, true);
    assert.deepEqual([result.full.usedBytes, result.full.count], [10, 2]);
    assert.deepEqual([result.shrink.usedBytes, result.shrink.count], [5, 1]);
    assert.equal(result.older.stored, false); assert.equal(result.newSurvives, true);
    assert.equal(result.oversize.reason, 'oversize');
    assert.equal(result.stale.stored, false); assert.equal(result.empty.count, 0);
    assert.equal(result.disabled.stored, false);
});

test('simultaneous readers share one download and clear prevents an in-flight cache resurrection', async () => {
    let release;
    fixture.state.delay = new Promise(resolve => { release = resolve; });
    const second = await context.newPage();
    await Promise.all([page.goto(readerUrl()), second.goto(readerUrl())]);
    await fixture.waitForSourceDownloads(1);
    release(); fixture.state.delay = null;
    await Promise.all([ready(), ready(second)]);
    assert.equal(fixture.state.sourceDownloads, 1);
    assert.match(await contents(second), /Revision A: the complete original EPUB/);
    await second.close();
    await page.evaluate(() => epubCache.clear());
    fixture.state.delay = new Promise(resolve => { release = resolve; });
    await page.reload();
    await fixture.waitForSourceDownloads(2);
    await page.evaluate(() => epubCache.clear());
    release(); fixture.state.delay = null;
    await ready();
    assert.match(await contents(), /Revision A: the complete original EPUB/);
    assert.equal((await stats()).count, 0);
});

test('oversize EPUB remains readable without storage and disabled cache never persists', async () => {
    await page.evaluate(() => epubCache.setLimitBytes(1));
    await page.goto(readerUrl()); await ready();
    assert.match(await contents(), /Revision A: the complete original EPUB/);
    assert.equal((await stats()).count, 0);
    await page.evaluate(() => epubCache.setLimitBytes(0));
    await page.reload(); await ready();
    assert.match(await contents(), /Revision A: the complete original EPUB/);
    assert.equal((await stats()).count, 0);
    assert.equal(fixture.state.sourceDownloads, 2);
});

test('management controls save locally, show usage and remove cached EPUBs', async () => {
    await page.goto(readerUrl()); await ready();
    await page.goto(fixture.origin + '/manage/');
    await page.getByRole('tab', { name: '設定', exact: true }).click();
    await page.locator('#epub-cache-limit-mib').waitFor();
    await page.waitForFunction(() => document.querySelector('#epub-cache-usage').textContent.includes('保存数 1 冊'));
    await page.locator('#epub-cache-limit-mib').fill('2');
    await page.locator('#epub-cache-form button[type=submit]').click();
    await page.waitForFunction(() => document.querySelector('#epub-cache-status').textContent === '保存しました。');
    assert.equal((await stats()).limitBytes, 2 * 1024 * 1024);
    assert.equal(fixture.state.settingsWrites, 0);
    await page.locator('#epub-cache-clear').click();
    await page.getByRole('button', { name: '削除する', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('#epub-cache-status').textContent.includes('削除しました'));
    assert.equal((await stats()).count, 0);
    await page.locator('#epub-cache-limit-mib').fill('0');
    await page.locator('#epub-cache-form button[type=submit]').click();
    await page.waitForFunction(() => document.querySelector('#epub-cache-status').textContent.includes('無効化'));
    assert.equal((await stats()).limitBytes, 0);
    assert.equal(fixture.state.settingsWrites, 0);
});

test('a revision changed during generation is not stored under the earlier last_update', async () => {
    let release;
    fixture.state.delay = new Promise(resolve => { release = resolve; });
    await page.goto(readerUrl());
    await fixture.waitForSourceDownloads(1);
    Object.assign(fixture.state, { revision: '2026-01-02T00:00:00Z', edition: 'b' });
    release(); fixture.state.delay = null;
    await ready();
    assert.match(await contents(), /Revision B: the updated EPUB replaces the original/);
    assert.equal((await stats()).count, 0);
    await page.reload(); await ready();
    assert.match(await contents(), /Revision B: the updated EPUB replaces the original/);
    assert.equal((await stats()).count, 1);
    assert.equal(fixture.state.sourceDownloads, 2);
});
