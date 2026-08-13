# Production deployment checklist

## Before deployment

- [ ] `develop` is the reviewed source branch; release changes are not made directly on `main` or `master`.
- [ ] Native database backup completed and restore tested.
- [ ] Native writes are frozen for the migration snapshot.
- [ ] Current Worker migrations are applied to the target D1 database.
- [ ] Wasabi bucket, region, endpoint, and a unique migration prefix are confirmed.
- [ ] D1 and Wasabi credentials are stored only in the deployment secret store.
- [x] No credential, Authorization header, cookie token, signing key, or full presigned URL appears in logs or artifacts.
- [x] Worker authentication remains fail-closed when the API token is not configured.
- [x] Presigned URL routes perform authentication and authorization before signing.
- [x] Worker audio routes retain the external-processing boundary; no Worker-side full decode, FFmpeg, or large PCM allocation is enabled.

## Verification before traffic shift

- [x] Native checks pass: `cargo check --no-default-features`.
- [x] Native codec WASM check passes when applicable.
- [x] Native tests pass with `cargo test --features native --no-fail-fast`.
- [x] Worker checks pass: `cargo check --target wasm32-unknown-unknown` and `cargo test --lib`.
- [x] `worker-build --release` succeeds.
- [x] Wrangler local contracts pass: health, authenticated routes, static assets, scheduled handler, audio 501 boundary, and books pagination.
- [x] Wasabi basic lifecycle passes: PUT, HEAD, full GET, range GET byte equality, and DELETE.
- [x] Wasabi multipart lifecycle passes: init, direct part upload, complete, full GET byte equality, abort, and invalid-session cases.
- [ ] Deployed Cloudflare Worker to Wasabi completion and multipart E2E pass.
- [ ] Migration dry-run report contains no missing media or invalid object keys.
- [ ] Migration apply uses a unique state file and produces a reconciliation report with no missing rows.

## Verification record

The checked items above record repository, local Wrangler, and local live Wasabi verification completed on 2026-08-13. A remote Cloudflare Worker rehearsal was created and isolated, but cover completion failed because the Worker-side Wasabi `HEAD` request returned HTTP 403; the remote verification item remains unchecked. Credentials and signed URLs were not printed in the verification logs. The rehearsal uploaded objects successfully before completion, so orphaned objects remain under the Wasabi test prefix `test.dantalian.dev/e2e/20260813/a1/`; the remote D1 and Worker were deleted, but the Wasabi objects were not deleted. Delete only that exact prefix from the Wasabi console, or with a newly rotated least-privilege test key. Do not use the exposed root key.

## Credential incident and cleanup

- [ ] Immediately disable and rotate the exposed Wasabi root key before any further Wasabi access.
- [ ] Using the replacement key or the Wasabi console, list and delete all objects under `e2e/20260813/a1/` in `test.dantalian.dev`.
- [ ] Confirm the prefix is empty after cleanup; do not delete unrelated objects in the test bucket.
- [ ] Store the replacement credential only in the secret store or ignored local configuration; never commit or print it.

## Traffic shift

- [ ] Deploy Worker with the intended D1 binding, Static Assets binding, Cron, and Wasabi variables.
- [ ] Confirm `/api/health` is public and protected endpoints require authentication.
- [ ] Test one representative book, cover, EPUB, CD, and audio track with an authenticated client.
- [ ] Confirm browser uploads go directly to Wasabi after Worker authorization; large objects do not pass through Worker.
- [ ] Monitor structured migration/job events by `job_id`, `object_key`, status, attempt, processor, and error class. Do not log secrets or full signed URLs.
- [ ] Keep Native available as rollback until post-deployment reconciliation is complete.

## Rollback

- [ ] Stop Worker writes and route reads/writes back to Native.
- [ ] Preserve the migration state/report and relevant redacted logs.
- [ ] Do not delete shared Wasabi objects during rollback.
- [ ] Investigate D1/object reconciliation before retrying.
- [ ] Resume only with a new reviewed migration run if the source snapshot changed.
