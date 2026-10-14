import { execFileSync } from 'node:child_process';
import { cp, lstat, mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { importManifestName, renderHostEntry, sha256, verifyImported, verifyUpstream } from './bibi/distribution.mjs';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pin = JSON.parse(await readFile(join(project, 'bibi.lock.json'), 'utf8'));
const preset = await readFile(join(project, 'scripts/bibi/dantalian.js'));
const args = process.argv.slice(2);
const verifyOnly = args[0] === '--verify';
if (!args.length || (verifyOnly ? args.length !== 1 && args.length !== 3 : args.length !== 1 && args.length !== 3) ||
    args.length === 3 && args[1] !== '--destination') {
    throw new Error('Usage: node scripts/import_bibi.mjs SOURCE_ROOT|--verify [--destination DIRECTORY]');
}
const destination = args.length === 3 ? resolve(args[2]) : join(project, 'static/vendor/bibi');
if (verifyOnly) {
    const manifest = await verifyImported(destination, pin, preset);
    console.log(`Verified Bibi ${manifest.version} (${manifest.source.commit}): ${manifest.files.length} reader assets.`);
} else {
    const source = resolve(args[0]);
    if (source === destination || source.startsWith(destination + sep) || destination.startsWith(source + sep)) {
        throw new Error('Source checkout and import destination must be separate directories.');
    }
    const git = argv => execFileSync('git', argv, { cwd: source, encoding: 'utf8' }).trim();
    if (git(['rev-parse', 'HEAD']) !== pin.commit || git(['status', '--porcelain', '--untracked-files=normal'])) {
        throw new Error('Import requires the pinned clean Bibi checkout.');
    }
    if (sha256(await readFile(join(source, 'bun.lock'))) !== pin.lockSha256) throw new Error('Bibi source lockfile mismatch.');
    const license = await readFile(join(source, 'LICENSE'));
    if (sha256(license) !== pin.licenseSha256) throw new Error('Bibi license notice mismatch.');
    const reader = join(source, '__dist/bibi');
    const upstream = await verifyUpstream(reader, pin);
    const parent = dirname(destination);
    await mkdir(parent, { recursive: true });
    const stage = await mkdtemp(join(parent, '.bibi-import-'));
    const prepared = join(stage, 'reader');
    const backup = join(stage, 'previous');
    let movedPrevious = false;
    try {
        await cp(reader, prepared, { recursive: true, preserveTimestamps: true });
        await writeFile(join(prepared, 'LICENSE'), license);
        await writeFile(join(prepared, 'presets/dantalian.js'), preset);
        const hostEntry = renderHostEntry(await readFile(join(reader, 'index.html')), preset);
        await writeFile(join(prepared, 'dantalian.html'), hostEntry);
        const files = [
            { path: 'LICENSE', bytes: license.length, sha256: sha256(license) },
            { path: 'dantalian.html', bytes: hostEntry.length, sha256: sha256(hostEntry) },
            { path: 'presets/dantalian.js', bytes: preset.length, sha256: sha256(preset) },
        ];
        await writeFile(join(prepared, importManifestName), JSON.stringify({
            schemaVersion: 1, repository: pin.repository,
            upstreamManifestSha256: sha256(upstream.manifestBytes), files,
        }, null, 2) + '\n');
        await verifyImported(prepared, pin, preset);
        const existing = await lstat(destination).catch(error => {
            if (error.code === 'ENOENT') return null;
            throw error;
        });
        if (existing) {
            if (!existing.isDirectory() || existing.isSymbolicLink()) throw new Error('Refusing to replace a non-directory import destination.');
            const ownership = JSON.parse(await readFile(join(destination, importManifestName)));
            if (ownership.schemaVersion !== 1 || ownership.repository !== pin.repository) {
                throw new Error('Refusing to replace a directory without Dantalian Bibi import metadata.');
            }
            await rename(destination, backup);
            movedPrevious = true;
        }
        try { await rename(prepared, destination); }
        catch (error) {
            if (movedPrevious) {
                await rename(backup, destination);
                movedPrevious = false;
            }
            throw error;
        }
        movedPrevious = false;
        console.log(`Imported Bibi ${pin.version} (${pin.commit}) to ${destination}.`);
    } finally {
        if (!movedPrevious) await rm(stage, { recursive: true, force: true });
    }
}
