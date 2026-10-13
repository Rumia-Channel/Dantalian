import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { importManifestName, manifestName, requiredAssets, sha256, verifyImported, verifyUpstream } from './distribution.mjs';

const project = new URL('../../', import.meta.url);
const sourcePin = JSON.parse(await readFile(new URL('bibi.lock.json', project)));
const preset = await readFile(new URL('scripts/bibi/dantalian.js', project));

async function fixture(t, imported = false) {
    const temp = await mkdtemp(join(tmpdir(), 'dantalian-bibi-integrity-'));
    t.after(() => rm(temp, { recursive: true, force: true }));
    const root = join(temp, 'reader');
    const license = Buffer.from('Fixture license notice\n');
    const pin = { ...sourcePin, licenseSha256: sha256(license) };
    const files = [];
    for (const path of [...requiredAssets].sort()) {
        const bytes = Buffer.from(`Distribution fixture: ${path}\n`);
        await mkdir(dirname(join(root, path)), { recursive: true });
        await writeFile(join(root, path), bytes);
        files.push({ path, bytes: bytes.length, sha256: sha256(bytes) });
    }
    const manifest = {
        schemaVersion: 1, version: pin.version,
        source: { commit: pin.commit, dirty: false, lockSha256: pin.lockSha256, sourceDateEpoch: pin.sourceDateEpoch },
        toolchain: pin.toolchain, files,
    };
    const manifestBytes = Buffer.from(JSON.stringify(manifest));
    await writeFile(join(root, manifestName), manifestBytes);
    let metadata;
    if (imported) {
        await writeFile(join(root, 'LICENSE'), license);
        await writeFile(join(root, 'presets/dantalian.js'), preset);
        metadata = {
            schemaVersion: 1, repository: pin.repository, upstreamManifestSha256: sha256(manifestBytes),
            files: [
                { path: 'LICENSE', bytes: license.length, sha256: sha256(license) },
                { path: 'presets/dantalian.js', bytes: preset.length, sha256: sha256(preset) },
            ],
        };
        await writeFile(join(root, importManifestName), JSON.stringify(metadata));
        await verifyImported(root, pin, preset);
    } else {
        await verifyUpstream(root, pin);
    }
    return { root, pin, manifest, metadata };
}

async function saveManifest(fixture) {
    await writeFile(join(fixture.root, manifestName), JSON.stringify(fixture.manifest));
}

test('changed reader bytes invalidate a previously valid distribution', async t => {
    const f = await fixture(t);
    await writeFile(join(f.root, 'and/jo.js'), 'corrupted reader');
    await assert.rejects(verifyUpstream(f.root, f.pin));
});

test('a different revision, dirty build, lockfile or toolchain cannot replace the pin', async t => {
    for (const change of [
        manifest => { manifest.source.commit = '0'.repeat(40); },
        manifest => { manifest.source.dirty = true; },
        manifest => { manifest.source.lockSha256 = '0'.repeat(64); },
        manifest => { manifest.toolchain = { ...manifest.toolchain, node: '0.0.0' }; },
    ]) {
        const f = await fixture(t);
        change(f.manifest);
        await saveManifest(f);
        await assert.rejects(verifyUpstream(f.root, f.pin));
    }
});

test('missing Jo and unrecorded assets are rejected', async t => {
    const missing = await fixture(t);
    await rm(join(missing.root, 'and/jo.js'));
    await assert.rejects(verifyUpstream(missing.root, missing.pin));
    const extra = await fixture(t);
    await writeFile(join(extra.root, 'unexpected.js'), 'extra script');
    await assert.rejects(verifyUpstream(extra.root, extra.pin));
});

test('inventory traversal and duplicate paths are rejected', async t => {
    const traversal = await fixture(t);
    traversal.manifest.files[0].path = '../outside.js';
    await saveManifest(traversal);
    await assert.rejects(verifyUpstream(traversal.root, traversal.pin));
    const duplicate = await fixture(t);
    duplicate.manifest.files.splice(1, 0, { ...duplicate.manifest.files[0] });
    await saveManifest(duplicate);
    await assert.rejects(verifyUpstream(duplicate.root, duplicate.pin));
});

test('re-signing modified host preset or license with its own digest does not authorize it', async t => {
    for (const path of ['LICENSE', 'presets/dantalian.js']) {
        const f = await fixture(t, true);
        const changed = Buffer.from('Unauthorized host addition\n');
        await writeFile(join(f.root, path), changed);
        Object.assign(f.metadata.files.find(file => file.path === path), { bytes: changed.length, sha256: sha256(changed) });
        await writeFile(join(f.root, importManifestName), JSON.stringify(f.metadata));
        await assert.rejects(verifyImported(f.root, f.pin, preset));
    }
});

test('host metadata cannot claim a different upstream manifest', async t => {
    const f = await fixture(t, true);
    f.metadata.upstreamManifestSha256 = '0'.repeat(64);
    await writeFile(join(f.root, importManifestName), JSON.stringify(f.metadata));
    await assert.rejects(verifyImported(f.root, f.pin, preset));
});
