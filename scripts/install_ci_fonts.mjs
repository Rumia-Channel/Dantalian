import { createHash } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { basename, join } from 'node:path';
import process from 'node:process';

// Noto CJK (SIL OFL): https://github.com/notofonts/noto-cjk
// Install only in the CI user's font directory, not in shipped reader assets.
const revision = 'f8d157532fbfaeda587e826d4cd5b21a49186f7c';
const fonts = [
    ['Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf', '68a3fc98800b2a27b371f2fb79991daf3633bd89309d4ffaa6946fd587f375b5'],
    ['Serif/OTF/Japanese/NotoSerifCJKjp-Regular.otf', 'd9854c7a8ef170b5a7932558856fd64eb8de0b007cd823fed6f9f514ad2803d3'],
];
const directory = join(process.env.XDG_DATA_HOME || join(homedir(), '.local/share'), 'fonts/dantalian-ci');
await mkdir(directory, { recursive: true });
for (const [path, expected] of fonts) {
    const response = await fetch(`https://raw.githubusercontent.com/notofonts/noto-cjk/${revision}/${path}`, {
        signal: AbortSignal.timeout(30_000),
    });
    if (!response.ok) throw new Error(`Font download failed: HTTP ${response.status} (${path})`);
    const bytes = Buffer.from(await response.arrayBuffer());
    if (createHash('sha256').update(bytes).digest('hex') !== expected) {
        throw new Error(`Font checksum mismatch: ${path}`);
    }
    await writeFile(join(directory, basename(path)), bytes);
    console.log(`Installed verified ${basename(path)}`);
}
