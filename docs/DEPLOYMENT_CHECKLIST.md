# Production deployment checklist

## Before deployment

- [ ] `develop` is the reviewed source branch; release changes are not made directly on `main` or `master`.
- [ ] Native database backup completed and restore tested.
- [ ] Native writes are frozen for the migration snapshot.
- [ ] Current Worker migrations are applied to the target D1 database.
- [ ] Wasabi bucket, region, endpoint, and a unique migration prefix are confirmed.
- [ ] D1 and Wasabi credentials are stored only in the deployment secret store.
- [ ] No credential, Authorization header, cookie token, signing key, or full presigned URL appears in logs or artifacts.
- [ ] Worker authentication remains fail-closed when the API token is not configured.
- [ ] Presigned URL routes perform authentication and authorization before signing.
- [ ] Worker audio routes retain the external-processing boundary; no Worker-side full decode, FFmpeg, or large PCM allocation is enabled.

## Verification before traffic shift

- [ ] Native checks pass: `cargo check --no-default-features`.
- [ ] Native codec WASM check passes when applicable.
- [ ] Native tests pass with `cargo test --features native --no-fail-fast`.
- [ ] Worker checks pass: `cargo check --target wasm32-unknown-unknown` and `cargo test --lib`.
- [ ] `worker-build --release` succeeds.
- [ ] Wrangler local contracts pass: health, authenticated routes, static assets, scheduled handler, audio 501 boundary, and books pagination.
- [ ] Wasabi basic lifecycle passes: PUT, HEAD, full GET, range GET byte equality, and DELETE.
- [ ] Wasabi multipart lifecycle passes: init, direct part upload, complete, full GET byte equality, abort, and invalid-session cases.
- [ ] Migration dry-run report contains no missing media or invalid object keys.
- [ ] Migration apply uses a unique state file and produces a reconciliation report with no missing rows.

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
