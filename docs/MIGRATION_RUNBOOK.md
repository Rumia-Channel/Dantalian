# Native to Worker migration runbook

## Scope

The migration keeps Native SQLite as the source of truth during the cutover. D1 receives relational rows, and Wasabi receives media objects. The Worker is not used as a media proxy or as a multipart assembly service.

The migration binary is `dantalian_migrate`. It is intentionally native-only because it reads SQLite and the local media filesystem.

## Preconditions

1. Stop Native writes, or use a database backup taken during a write freeze.
2. Apply the current Worker migrations to the destination D1 database.
3. Use a dedicated Wasabi bucket or prefix. Never point the first run at production objects.
4. Set `WASABI_PREFIX` to a test prefix such as `migration/<run-id>` for rehearsal.
5. Confirm the source database and media root are from the same backup.

Required apply variables:

- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_D1_DATABASE_ID`
- `CLOUDFLARE_API_TOKEN`
- `WASABI_ACCESS_KEY_ID`
- `WASABI_SECRET_ACCESS_KEY`
- `WASABI_ENDPOINT`
- `WASABI_REGION`
- `WASABI_BUCKET`
- optional `WASABI_PREFIX`

Do not print these variables, bearer tokens, or generated request URLs.

## Dry run

The dry run reads all Native relational tables and all referenced local media. It performs no D1 request and no Wasabi request, so the D1 and Wasabi credentials are not required.

```text
cargo run --features native --bin dantalian_migrate -- \
  --sqlite /path/to/dantalian.db \
  --media-root /path/to/data \
  --state /path/to/migration-state.json \
  --report /path/to/migration-report.json
```

Stop on any missing referenced media, invalid object path, or invalid state/report write. Review the report before applying. The report includes source row counts, object keys, byte sizes, and SHA3-256 values.

## Apply

Run the apply command only after the dry-run report is approved:

```text
cargo run --release --features native --bin dantalian_migrate -- \
  --sqlite /path/to/dantalian.db \
  --media-root /path/to/data \
  --apply \
  --state /path/to/migration-state.json \
  --report /path/to/migration-report.json
```

The tool applies relational rows in batches of 50 with `INSERT OR IGNORE`, uploads objects directly to Wasabi, records completed row/object keys in the state file, and then writes a reconciliation report. Worker-facing media filenames are normalized to `<hash>.<extension>` for EPUB and audio rows.

State writes are atomic (`.tmp` then rename). If the process stops, rerun with the same state and report paths. Completed rows and objects are skipped. Do not edit the state file by hand while an apply is running.

## Recovery

- **D1 batch failure:** fix the reported destination/configuration issue and rerun. The current batch is not marked complete until D1 accepts it.
- **Wasabi upload failure:** verify bucket, endpoint, prefix, and permissions; rerun. The object is not marked uploaded until the request succeeds.
- **Process termination:** rerun with the same state path. The operation is resume-safe at row/object boundaries.
- **Unexpected object bytes:** compare the report SHA3-256 and size with the source file. Delete only the test prefix after investigation; never bulk-delete a production prefix.
- **Reconciliation mismatch:** keep Native writes frozen, inspect the report, and rerun. Do not manually “fix” D1 rows without recording the reason.

The migration does not delete Native rows or Wasabi objects. Rollback is therefore a read cutover back to Native after stopping Worker traffic; newly uploaded test-prefix objects can be cleaned up separately.

## Audio job operations

The authenticated API currently exposes `GET /api/audio/jobs/:id` for status and `POST /api/audio/jobs/:id/retry` for failed-job retry. Processor claims, lease renewal, completion, failure, and expired-lease recovery are bounded by the job state machine.

There is deliberately no public list or cancel endpoint in this cutover. A list endpoint needs pagination and owner-scoped operational authorization; cancellation also needs an explicit policy for an in-flight processor and any output object. Until that contract exists, operators use the job id from structured processor events and the authenticated status/retry routes. Do not edit D1 status rows manually.

The queue-backed processor is deployed separately with `wrangler.audio.toml`. Its queue payload contains only the 32-character audio job id. The controller starts a named container instance (`audio-<job-id>`) and acknowledges the queue message after startup; the processor then claims the job through the controller's authenticated `/internal-api` service-binding proxy. D1 remains the job state authority. If the container fails before startup, Queue retries the message; if processing fails after a lease, the processor reports the failure and the job retry policy applies.

The processor downloads and uploads Wasabi objects directly through its dedicated Wasabi credential. It stages each object in the container's ephemeral filesystem, removes the workspace on success or failure, and does not route object bytes through the Worker. Configure `PROCESSOR_API_BASE_URL` to the controller's `/internal-api` URL and never store that URL or any credential in a queue message.

## Post-migration checks

1. Compare every source and destination table count in the reconciliation report.
2. Verify a book with a cover, EPUB, and audio track through Worker authenticated routes.
3. Verify direct Wasabi GET bytes and `Content-Type` for representative objects.
4. Verify Worker audio processing remains an external job boundary.
5. Keep the state and report files with the migration change record; redact secrets before sharing.
