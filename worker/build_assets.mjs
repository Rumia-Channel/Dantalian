import { createHash } from "node:crypto";
import { cp, mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const workerDir = dirname(fileURLToPath(import.meta.url));
const projectDir = resolve(workerDir, "..");
const sourceDir = join(projectDir, "static");
const outputDir = join(workerDir, "public");

async function filesUnder(directory) {
    const entries = await readdir(directory, { withFileTypes: true });
    const files = [];
    for (const entry of entries) {
        const path = join(directory, entry.name);
        if (entry.isDirectory()) {
            files.push(...await filesUnder(path));
        } else if (entry.isFile()) {
            files.push(path);
        }
    }
    return files.sort();
}

const sourceFiles = await filesUnder(sourceDir);
const hash = createHash("sha256");
for (const sourceFile of sourceFiles) {
    hash.update(relative(sourceDir, sourceFile).replaceAll("\\", "/"));
    hash.update(await readFile(sourceFile));
}
const assetVersion = hash.digest("hex").slice(0, 16);

await mkdir(outputDir, { recursive: true });
await cp(sourceDir, outputDir, { recursive: true });
await cp(join(projectDir, "LICENSE"), join(outputDir, "LICENSE"));
await cp(join(projectDir, "NOTICE"), join(outputDir, "NOTICE"));

for (const sourceFile of sourceFiles.filter((file) => file.endsWith(".html"))) {
    const outputFile = join(outputDir, relative(sourceDir, sourceFile));
    const html = await readFile(outputFile, "utf8");
    await writeFile(outputFile, html.replaceAll("ASSET_VERSION", assetVersion));
}

console.log(`Prepared Worker assets: ${assetVersion}`);
