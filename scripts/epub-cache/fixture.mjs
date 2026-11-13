import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

export const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
export const bibiSource = path.resolve(process.argv.includes('--bibi-source') ? process.argv[process.argv.indexOf('--bibi-source') + 1] : path.join(project, '.tmp/bibi-source'));
const requireBibi = createRequire(path.join(bibiSource, 'package.json'));
export const playwright = requireBibi('playwright');
const JSZip = requireBibi('jszip');

async function epub(marker) {
    const zip = new JSZip();
    zip.file('mimetype', 'application/epub+zip', { compression: 'STORE' });
    zip.file('META-INF/container.xml', '<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="book/package.opf" media-type="application/oebps-package+xml"/></rootfiles></container>');
    zip.file('book/package.opf', `<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" unique-identifier="id" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">cache-fixture</dc:identifier><dc:title>Device EPUB cache</dc:title><dc:language>en</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata><manifest><item id="chapter" href="chapter.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="chapter"/></spine></package>`);
    zip.file('book/chapter.xhtml', `<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Cache fixture</title></head><body><h1>Device EPUB cache</h1><p id="cache-marker">${marker}</p></body></html>`);
    return zip.generateAsync({ type: 'nodebuffer', compression: 'DEFLATE' });
}

export async function startCacheFixture(port = 0) {
    const books = { a: await epub('Revision A: the complete original EPUB.'), b: await epub('Revision B: the updated EPUB replaces the original.') };
    const state = { revision: '2026-01-01T00:00:00Z', edition: 'a', epubStatus: 200, metadataStatus: 200, sourceDownloads: 0, s3Downloads: 0, metadataReads: 0, settingsWrites: 0, delay: null };
    let hostOrigin, sourceOrigin;
    const json = (response, value, status = 200, headers = {}) => { response.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store', ...headers }); response.end(JSON.stringify(value)); };
    const source = http.createServer(async (request, response) => {
        const headers = { 'Access-Control-Allow-Origin': request.headers.origin || hostOrigin, 'Access-Control-Allow-Credentials': 'true', 'Cache-Control': 'no-store' };
        const url = new URL(request.url, 'http://localhost');
        if (request.method === 'OPTIONS') { response.writeHead(204, { ...headers, 'Access-Control-Allow-Headers': 'Range', 'Access-Control-Allow-Methods': 'GET, OPTIONS' }); response.end(); return; }
        if (url.pathname === '/signed.epub') {
            if (url.searchParams.get('signature') !== 'fixture') { json(response, { error: 'forbidden' }, 403, headers); return; }
            state.s3Downloads++;
            response.writeHead(200, { ...headers, 'Content-Type': 'application/epub+zip' }); response.end(books.a); return;
        }
        if (!request.headers.cookie?.includes('cache-auth=granted')) { json(response, { error: 'unauthorized' }, 401, headers); return; }
        if (url.pathname === '/api/read/v1/novels/1') {
            state.metadataReads++;
            if (state.metadataStatus !== 200) { json(response, { error: 'unavailable' }, state.metadataStatus, headers); return; }
            json(response, { id: 1, title: 'Cache fixture', author: 'Fixture author', last_update: state.revision, general_all_no: 1, length: 50 }, 200, headers); return;
        }
        if (url.pathname === '/api/read/v1/novels/1/epub') {
            state.sourceDownloads++;
            if (state.delay) await state.delay;
            if (state.epubStatus !== 200) { json(response, { error: 'memory_capacity', retryable: true }, state.epubStatus, headers); return; }
            response.writeHead(200, { ...headers, 'Content-Type': 'application/epub+zip' });
            const bytes = books[state.edition];
            response.write(bytes.subarray(0, 512)); response.end(bytes.subarray(512)); return;
        }
        json(response, { error: 'not_found' }, 404, headers);
    });
    const host = http.createServer(async (request, response) => {
        const url = new URL(request.url, 'http://localhost');
        if (url.pathname === '/login') { response.writeHead(200, { 'Content-Type': 'text/html', 'Set-Cookie': 'cache-auth=granted; HttpOnly; SameSite=Lax; Path=/' }); response.end('<!doctype html>Authenticated cache fixture'); return; }
        if (url.pathname === '/__fixture/state') {
            if (request.method === 'POST') {
                const chunks = []; for await (const chunk of request) chunks.push(chunk);
                const changes = JSON.parse(Buffer.concat(chunks).toString());
                for (const key of ['revision', 'edition', 'epubStatus', 'metadataStatus']) if (Object.hasOwn(changes, key)) state[key] = changes[key];
            }
            json(response, { ...state, delay: !!state.delay }); return;
        }
        if (url.pathname.startsWith('/api/')) {
            if (!request.headers.cookie?.includes('cache-auth=granted')) { json(response, { error: 'unauthorized' }, 401); return; }
            if (url.pathname === '/api/health') { json(response, { ok: true, authentication_required: false, runtime: 'cloudflare-worker' }); return; }
            if (url.pathname === '/api/narou/connection') { json(response, { configured: true, source: sourceOrigin, api_base: sourceOrigin + '/api/read/v1', tags: [], use_favorites: false }); return; }
            if (url.pathname === '/api/narou/novels/1/epub') { response.writeHead(302, { Location: sourceOrigin + '/api/read/v1/novels/1/epub', 'Cache-Control': 'no-store' }); response.end(); return; }
            if (url.pathname === '/api/books/1') { json(response, { id: 1, title: 'Immutable S3 EPUB', files: [{ id: 1, file_hash: 'immutable.epub', file_name: 'fixture.epub' }] }); return; }
            if (url.pathname === '/api/books') { json(response, { items: [], next_cursor: null }); return; }
            if (url.pathname === '/api/settings') {
                if (request.method !== 'GET') state.settingsWrites++;
                json(response, { 'narou.url': sourceOrigin, 'narou.tags': '[]', 'narou.use_favorites': 'false' }); return;
            }
            json(response, []); return;
        }
        if (url.pathname === '/epubs/immutable.epub') { response.writeHead(302, { Location: sourceOrigin + '/signed.epub?signature=fixture', 'Cache-Control': 'no-store' }); response.end(); return; }
        const relative = decodeURIComponent(url.pathname).replace(/^\//, '') + (url.pathname.endsWith('/') ? 'index.html' : '');
        const file = path.resolve(project, 'static', relative);
        if (!file.startsWith(path.join(project, 'static') + path.sep)) { response.writeHead(403); response.end(); return; }
        try {
            let bytes = await fs.readFile(file);
            if (file.endsWith('.html')) bytes = Buffer.from(bytes.toString().replaceAll('ASSET_VERSION', 'cache-fixture'));
            const type = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml', '.webmanifest': 'application/manifest+json' }[path.extname(file)] || 'application/octet-stream';
            response.writeHead(200, { 'Content-Type': type, 'Cache-Control': 'no-store' }); response.end(bytes);
        } catch { response.writeHead(404); response.end(); }
    });
    const listen = (server, requested = 0) => new Promise(resolve => server.listen(requested, '127.0.0.1', () => resolve('http://127.0.0.1:' + server.address().port)));
    hostOrigin = await listen(host, port); sourceOrigin = await listen(source);
    return {
        origin: hostOrigin, sourceOrigin, state,
        async close() { await Promise.all([host, source].map(server => new Promise(resolve => { server.closeAllConnections(); server.close(resolve); }))); },
    };
}

if (process.argv.includes('--serve')) {
    const fixture = await startCacheFixture(8836);
    console.log(`EPUB cache fixture ready: ${fixture.origin}`);
    for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, async () => { await fixture.close(); process.exit(); });
}
