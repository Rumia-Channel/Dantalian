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
- [x] Worker upload routes reject the native chunk query protocol (`upload_id`/`part`/`total_parts`) with HTTP 400 instead of storing a partial chunk as a complete media object.

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
- [x] Wasabi audio multipart finalize passes: presigned parts attach to a CD track, the stream route redirects to the canonical `audio/<hash>.<ext>` object, a repeated finalize is a conflict, and a completed EPUB session is rejected as the wrong kind (`worker/tests/wasabi_audio_multipart_e2e.mjs`, `WASABI_E2E=1`).
- [x] Deployed develop Worker to Wasabi completion and multipart E2E pass (run 38038598391, "Test develop Wasabi basic and multipart lifecycle").
- [x] Client upload URL contract is pinned without a live bucket: `worker/tests/upload_target_unit.mjs` classifies the target (audio vs epub) and derives both child URLs, and `worker/tests/upload_integration_unit.mjs` drives the real `static/js/upload.js` over a mocked fetch to assert the request sequence (`…/audio/multipart` → parts → complete → `…/audio/finalize`, never `…/multipart/finalize`). Both run in the worker job.
- [x] Production deploy smoke check passes: `…/api/cds/:id/tracks/:track_id/audio/multipart` answers with a non-404 status after the Worker deploy.
- [x] Cloudflare remote-preview verification passes cover completion, multipart completion, and media-sync object existence checks.
- [ ] Migration dry-run report contains no missing media or invalid object keys.
- [ ] Migration apply uses a unique state file and produces a reconciliation report with no missing rows.

## Verification record

The checked items above record repository, local Wrangler, local live Wasabi, and Cloudflare remote-preview verification completed on 2026-08-13, plus the audio presigned-multipart verification completed on 2026-10-10. The remote preview initially failed because the Worker-side authenticated Wasabi `HEAD` request returned HTTP 403 from the endpoint's Cloudflare edge. Wasabi metadata validation now uses a bounded `GET` with `Range: bytes=0-0`, reads the original size from `Content-Range`, and never proxies the object body to the client. Local and remote-preview basic, multipart, and audio object-existence checks pass.

Audio uploads now leave the Worker out of the media path: the browser requests a
presigned multipart session, PUTs the parts straight to Wasabi, and the Worker
only binds the completed object to the track. The develop deployment verified
that contract against the real bucket (run 38038598391) after the same suite
passed locally against a stub endpoint. The deployed E2E step writes a TAP
report that is uploaded as the `<target>-wasabi-e2e` artifact, so a failure can
be diagnosed without another live run. Credentials and signed URLs were not
printed in verification logs.

## External audio processor deployment

The tag-triggered production pipeline now provisions the queues, renders and
deploys `dantalian-audio-controller` (service binding `dantalian-worker`,
`PROCESSOR_API_BASE_URL` pointing at the controller's own `/internal-api` on
workers.dev), and stores `DANTALIAN_PROCESSOR_TOKEN` on both the Worker and the
controller. Do not deploy `wrangler.audio.toml` manually: it consumes the
unsuffixed `dantalian-audio-jobs` queue, not the production queue.

- [ ] Store a dedicated least-privilege Wasabi access key for the processor; never use the Wasabi account root key.
- [ ] Verify queue messages are acknowledged after container start, not after audio completion.
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
- [ ] Apply the Wasabi bucket CORS rules below before testing browser uploads.
- [ ] Test one representative book, cover, EPUB, CD, and audio track through the Access-protected domain.
- [ ] Confirm browser uploads go directly to Wasabi; large objects do not pass through Worker.
      Audio above `PRESIGNED_MULTIPART_MIN_BYTES` (8 MiB) uses
      `POST /api/uploads/multipart/init` → direct part `PUT` → `complete` →
      `POST /api/{cds|books}/:id/tracks/:track_id/audio/finalize`, so the Worker
      never buffers a full audio object. Cover uploads still use the Worker
      route because they are capped at 10 MiB.
- [ ] Known gap: cover and EPUB uploads have no `finalize` binding yet, so the
      browser only presigns audio. Both therefore stay on the Worker direct
      route and are capped at `WORKER_DIRECT_UPLOAD_MAX_BYTES` (95 MiB) even when
      `upload.file_max_mb` is configured higher; the client now rejects those
      files before sending instead of failing mid-upload. Extending the presigned
      path to covers and EPUBs requires a completion binding like the audio one.
- [ ] Monitor structured migration/job events by `job_id`, `object_key`, status, attempt, processor, and error class. Do not log secrets or full signed URLs.
- [ ] Keep Native available as rollback until post-deployment reconciliation is complete.

## Wasabi bucket CORS for direct browser uploads

The browser PUTs presigned part URLs and receives the presigned GET redirect, so
the bucket must allow the site origin. Without these rules the upload fails in
the browser with `net::ERR_FAILED` while the object parts are already stored, and
the UI reports a CORS hint instead of an HTTP status.

- [ ] Allow origins: `https://<SERVICE_DOMAIN>` only (no wildcard).
- [ ] Allow methods: `PUT`, `GET`, `HEAD`.
- [ ] Allow headers: none required. The client sends no custom headers, so the
      presigned `PUT` stays a CORS simple request and no preflight is needed.
- [ ] **Expose headers: `ETag`.** The multipart `complete` call needs every part
      ETag, and browsers hide response headers unless they are exposed. A missing
      `ETag` surfaces as `presignedKind === "etag"` in the client.
- [ ] Verify with one audio track larger than 8 MiB: the Network panel must show
      `PUT https://<bucket endpoint>/...?partNumber=1&uploadId=...` returning 200
      with an `ETag` response header, followed by the `finalize` call returning 200.


## Rollback

- [ ] Stop Worker writes and route reads/writes back to Native.
- [ ] Preserve the migration state/report and relevant redacted logs.
- [ ] Do not delete shared Wasabi objects during rollback.
- [ ] Investigate D1/object reconciliation before retrying.
- [ ] Resume only with a new reviewed migration run if the source snapshot changed.
