import { createHash } from 'node:crypto';
import { lstat, readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';

export const manifestName = 'build-manifest.json';
export const importManifestName = 'import-manifest.json';
export const requiredAssets = [
    'index.html', 'and/jo.js', 'presets/default.js',
    'resources/scripts/bibi.js', 'resources/scripts/bibi.wand.js', 'resources/styles/bibi.css',
    'wardrobe/everyday/bibi.dress.css', 'extensions/sanitizer.js',
    'extensions/extractor/at-once.js', 'extensions/extractor/on-the-fly.js',
    'extensions/extractor/on-the-fly.worker.js',
    'extensions/extractor/on-the-fly.bibi-zip-loader.worker.js',
];
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

export function renderHostEntry(indexBytes, presetBytes) {
    const html = indexBytes.toString('utf8');
    const presetAttribute = 'data-bibi-preset=""';
    const scriptVersion = /src="resources\/scripts\/bibi\.js\?v=([^"]+)"/g;
    if (html.split(presetAttribute).length !== 2 || [...html.matchAll(scriptVersion)].length !== 1) {
        throw new Error('Bibi entry must have one empty document preset and one versioned reader bundle.');
    }
    return Buffer.from(html.replace(presetAttribute, 'data-bibi-preset="dantalian"')
        .replace(scriptVersion, (_, version) => `src="resources/scripts/bibi.js?v=${version}.${sha256(presetBytes)}"`));
}

export async function filesUnder(root) {
    const stat = await lstat(root);
    if (!stat.isDirectory() || stat.isSymbolicLink()) throw new Error(`Not a regular distribution directory: ${root}`);
    const files = [];
    async function visit(directory, prefix = '') {
        for (const entry of await readdir(directory, { withFileTypes: true })) {
            const name = prefix + entry.name;
            if (entry.isDirectory()) await visit(join(directory, entry.name), name + '/');
            else if (entry.isFile()) files.push(name);
            else throw new Error(`Unsupported distribution entry: ${name}`);
        }
    }
    await visit(root);
    return files.sort();
}

function validateInventory(files) {
    if (!Array.isArray(files) || !files.length) throw new Error('Missing distribution file inventory.');
    const names = files.map(file => file.path);
    for (const file of files) {
        if (typeof file.path !== 'string' || file.path.includes('\\') || file.path.includes(':') ||
            file.path.includes('\0') || file.path.split('/').some(part => !part || part === '.' || part === '..') ||
            !Number.isSafeInteger(file.bytes) || file.bytes <= 0 || !/^[a-f0-9]{64}$/.test(file.sha256)) {
            throw new Error(`Invalid distribution file entry: ${file.path}`);
        }
    }
    if (names.some((name, index) => index > 0 && names[index - 1] >= name)) {
        throw new Error('Distribution inventory must be sorted and unique.');
    }
    return names;
}

export async function verifyUpstream(root, pin, additions = []) {
    const manifestBytes = await readFile(join(root, manifestName));
    const manifest = JSON.parse(manifestBytes);
    if (manifest.schemaVersion !== 1 || manifest.version !== pin.version ||
        manifest.source?.commit !== pin.commit || manifest.source?.dirty !== false ||
        manifest.source?.lockSha256 !== pin.lockSha256 || manifest.source?.sourceDateEpoch !== pin.sourceDateEpoch ||
        manifest.toolchain?.node !== pin.toolchain.node || manifest.toolchain?.bun !== pin.toolchain.bun) {
        throw new Error('Bibi manifest does not match the pinned clean source and toolchain.');
    }
    const names = validateInventory(manifest.files);
    if (names.includes(manifestName) || names.includes(importManifestName) || additions.some(name => names.includes(name))) {
        throw new Error('Bibi inventory collides with host import metadata or additions.');
    }
    for (const name of requiredAssets) if (!names.includes(name)) throw new Error(`Missing reader asset: ${name}`);
    const expected = [...names, manifestName, ...additions].sort();
    const actual = await filesUnder(root);
    if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('Bibi distribution contains missing or unexpected files.');
    for (const file of manifest.files) {
        const bytes = await readFile(join(root, file.path));
        if (bytes.length !== file.bytes || sha256(bytes) !== file.sha256) throw new Error(`Bibi digest mismatch: ${file.path}`);
    }
    return { manifest, manifestBytes };
}

export async function verifyImported(root, pin, presetBytes) {
    const additions = ['LICENSE', 'dantalian.html', 'presets/dantalian.js'];
    const upstream = await verifyUpstream(root, pin, [...additions, importManifestName]);
    const hostEntry = renderHostEntry(await readFile(join(root, 'index.html')), presetBytes);
    const metadata = JSON.parse(await readFile(join(root, importManifestName)));
    if (metadata.schemaVersion !== 1 || metadata.repository !== pin.repository ||
        metadata.upstreamManifestSha256 !== sha256(upstream.manifestBytes)) {
        throw new Error('Invalid Bibi host import manifest.');
    }
    const names = validateInventory(metadata.files);
    if (JSON.stringify(names) !== JSON.stringify(additions)) throw new Error('Invalid Bibi host additions.');
    for (const file of metadata.files) {
        const bytes = await readFile(join(root, file.path));
        const expectedHash = file.path === 'LICENSE' ? pin.licenseSha256 : sha256(file.path === 'dantalian.html' ? hostEntry : presetBytes);
        if (bytes.length !== file.bytes || sha256(bytes) !== file.sha256 || file.sha256 !== expectedHash) {
            throw new Error(`Bibi host addition mismatch: ${file.path}`);
        }
    }
    return upstream.manifest;
}
