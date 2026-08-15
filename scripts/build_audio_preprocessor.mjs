import { mkdir, rm } from "node:fs/promises";
import { spawn } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectDir = resolve(scriptDir, "..");
const manifestPath = join(projectDir, "Cargo.toml");
const wasmPath = join(
    projectDir,
    "target",
    "wasm32-unknown-unknown",
    "release",
    "preprocessor.wasm",
);
const outputDir = join(projectDir, "static", "wasm");

function run(command, args) {
    return new Promise((resolvePromise, reject) => {
        const child = spawn(command, args, {
            cwd: projectDir,
            stdio: "inherit",
        });
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

await run("cargo", [
    "build",
    "--release",
    "--no-default-features",
    "--features",
    "preprocessor-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--bin",
    "preprocessor",
    "--manifest-path",
    manifestPath,
]);

await rm(outputDir, { recursive: true, force: true });
await mkdir(outputDir, { recursive: true });
await run("wasm-bindgen", [
    wasmPath,
    "--target",
    "web",
    "--out-dir",
    outputDir,
    "--out-name",
    "audio_preprocessor",
    "--no-typescript",
]);
