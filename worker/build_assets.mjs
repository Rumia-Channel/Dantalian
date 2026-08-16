import { createHash } from "node:crypto";
import { cp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const workerDir = dirname(fileURLToPath(import.meta.url));
const projectDir = resolve(workerDir, "..");
const sourceDir = join(projectDir, "static");
const outputDir = join(workerDir, "public");
const assetVersionPath = join(workerDir, ".asset-version");
const licenseInputFiles = [
    join(projectDir, "about.hbs"),
    join(projectDir, "about.toml"),
    join(projectDir, "LICENSE"),
    join(projectDir, "NOTICE"),
    join(workerDir, "Cargo.toml"),
    join(workerDir, "Cargo.lock"),
];

function run(command, args) {
    return new Promise((resolvePromise, reject) => {
        const child = spawn(command, args, { cwd: projectDir, stdio: "inherit" });
        child.once("error", reject);
        child.once("exit", (code, signal) => {
            if (code === 0) {
                resolvePromise();
                return;
            }
            reject(new Error(`${command} exited with ${code ?? signal}`));
        });
    });
}

await run(process.execPath, [join(projectDir, "scripts", "build_audio_preprocessor.mjs")]);

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
for (const sourceFile of [...sourceFiles, ...licenseInputFiles]) {
    hash.update(relative(projectDir, sourceFile).replaceAll("\\", "/"));
    hash.update(await readFile(sourceFile));
}
const assetVersion = hash.digest("hex").slice(0, 16);
let previousAssetVersion = "";
try {
    previousAssetVersion = await readFile(assetVersionPath, "utf8");
} catch {}
if (previousAssetVersion.trim() === assetVersion && await readFile(join(outputDir, "index.html"), "utf8").then(() => true).catch(() => false)) {
    console.log(`Worker assets unchanged: ${assetVersion}`);
    process.exit(0);
}

await mkdir(outputDir, { recursive: true });
await cp(sourceDir, outputDir, { recursive: true });
await cp(join(projectDir, "LICENSE"), join(outputDir, "LICENSE"));
await cp(join(projectDir, "NOTICE"), join(outputDir, "NOTICE"));

const generatedLicensePath = join(workerDir, ".worker-license.html");
await rm(generatedLicensePath, { force: true });
await run("cargo", [
    "about",
    "generate",
    "--manifest-path",
    join(workerDir, "Cargo.toml"),
    "--config",
    join(projectDir, "about.toml"),
    "--locked",
    "--offline",
    "--output-file",
    generatedLicensePath,
    join(projectDir, "about.hbs"),
]);
const generatedLicense = await readFile(generatedLicensePath, "utf8");
const dantalianLicense = await readFile(join(projectDir, "LICENSE"), "utf8");
await writeFile(
    join(outputDir, "licenses", "index.html"),
    generatedLicense
        .replace("__DANTALIAN_LICENSE__", dantalianLicense)
        .replaceAll("ASSET_VERSION", assetVersion),
);
await rm(generatedLicensePath, { force: true });

for (const sourceFile of sourceFiles.filter((file) => file.endsWith(".html"))) {
    const outputFile = join(outputDir, relative(sourceDir, sourceFile));
    const html = await readFile(outputFile, "utf8");
    await writeFile(outputFile, html.replaceAll("ASSET_VERSION", assetVersion));
}
await writeFile(assetVersionPath, `${assetVersion}\n`);

console.log(`Prepared Worker assets: ${assetVersion}`);
