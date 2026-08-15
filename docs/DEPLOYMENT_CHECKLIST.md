# Production deployment checklist

## Before deployment

- [ ] `develop` is the reviewed source branch; merges to `master` are reviewed release candidates, `master` pushes deploy staging, and only `v*` tags deploy production.
- [ ] Native database backup completed and restore tested.
- [ ] Native writes are frozen for the migration snapshot.
- [ ] Current Worker migrations are applied to the target D1 database.
- [ ] Wasabi bucket, region, endpoint, and a unique migration prefix are confirmed.
- [ ] D1 and Wasabi credentials are stored only in the deployment secret store.
- [x] No credential, Authorization header, cookie token, signing key, or full presigned URL appears in logs or artifacts.
- [ ] Production custom domain is protected by a Cloudflare Access application with an explicit allow policy.
- [x] Production Worker uses Cloudflare Access as its user-facing authentication boundary; local/CI can require the app token fail-closed.
- [ ] Presigned URL routes are only reachable after the Access-protected application boundary.
- [x] Worker audio routes retain the external-processing boundary; no Worker-side full decode, FFmpeg, or large PCM allocation is enabled.

## MyDeploy production inputs

- [ ] GitHub Environment `production` contains the Cloudflare API credentials and five Wasabi `*_SECRET_NAME` references used by both target jobs.
- [ ] GitHub Environment `production` contains a random `DANTALIAN_STAGING_API_TOKEN`; it is used only for staging API tests and is not reused by production.
- [ ] GitHub Environment `production` contains the base `DANTALIAN_D1_DATABASE_NAME=dantalian` and `DANTALIAN_AUDIO_JOB_QUEUE=dantalian-audio-jobs` values; CI appends `-staging` or `-production` and resolves/creates the resources.
- [ ] Staging uses `dantalian/staging` as its automatic Wasabi prefix and production uses `dantalian/production`; the bucket remains shared.
- [ ] Production `SERVICE_DOMAIN` is present only in the production Environment and is attached as the custom domain.
- [ ] The production Wrangler template contains the non-secret Secrets Store resource ID; it is not copied into GitHub secrets.

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
- [x] Cloudflare remote-preview verification passes cover completion, multipart completion, and media-sync object existence checks.
- [ ] Migration dry-run report contains no missing media or invalid object keys.
- [ ] Migration apply uses a unique state file and produces a reconciliation report with no missing rows.

## Verification record

The checked items above record repository, local Wrangler, local live Wasabi, and Cloudflare remote-preview verification completed on 2026-08-13. The remote preview initially failed because the Worker-side authenticated Wasabi `HEAD` request returned HTTP 403 from the endpoint's Cloudflare edge. Wasabi metadata validation now uses a bounded `GET` with `Range: bytes=0-0`, reads the original size from `Content-Range`, and never proxies the object body to the client. Local and remote-preview basic, multipart, and audio object-existence checks pass. A temporary deployed Worker rehearsal was deleted after the public `workers.dev` URL returned HTTP 404 (`error code: 1042`), so the deployed Worker item remains unchecked. Credentials and signed URLs were not printed in verification logs.

## External audio processor deployment

- [ ] Create `dantalian-audio-jobs` and `dantalian-audio-jobs-dlq`; do not put audio bytes in the queue payload.
- [ ] Configure the controller `PROCESSOR_API_BASE_URL` to its externally reachable `/internal-api` route.
- [ ] Store `DANTALIAN_PROCESSOR_TOKEN` only as a controller secret and configure the same value as the Worker processor credential.
- [ ] Store a dedicated least-privilege Wasabi access key for the processor; never use the Wasabi account root key.
- [ ] Configure the controller service binding to the deployed `dantalian-worker`.
- [ ] Deploy `wrangler.audio.toml` with `max_instances = 4`; verify queue messages are acknowledged after container start, not after audio completion.
- [ ] Verify processor logs contain only job id, attempt, status, and redacted error class/summary.
- [ ] Run a short Opus and AAC job through queued, running, completed, and failed/retry transitions before traffic shift.

## Credential incident and cleanup

- [ ] Immediately disable and rotate the exposed Wasabi root key before any further Wasabi access.
- [x] Using the replacement credential, delete only `e2e/20260813/a1/` and `e2e/20260813/a2/` from `test.dantalian.dev`.
- [x] Confirm both test prefixes are empty; unrelated objects were not targeted.
- [ ] Store the replacement credential only in the secret store or ignored local configuration; the Wasabi IAM check still identifies the replacement access key as a root key, so least-privilege status is unconfirmed.
## Traffic shift

- [ ] Deploy Worker with the intended D1 binding, Static Assets binding, Cron, and Wasabi variables.
- [ ] Confirm the production custom domain is protected by Cloudflare Access and `workers.dev` is disabled.
- [ ] Test one representative book, cover, EPUB, CD, and audio track through the Access-protected domain.
- [ ] Confirm browser uploads go directly to Wasabi; large objects do not pass through Worker.
- [ ] Monitor structured migration/job events by `job_id`, `object_key`, status, attempt, processor, and error class. Do not log secrets or full signed URLs.
- [ ] Keep Native available as rollback until post-deployment reconciliation is complete.

## Rollback

- [ ] Stop Worker writes and route reads/writes back to Native.
- [ ] Preserve the migration state/report and relevant redacted logs.
- [ ] Do not delete shared Wasabi objects during rollback.
- [ ] Investigate D1/object reconciliation before retrying.
- [ ] Resume only with a new reviewed migration run if the source snapshot changed.
