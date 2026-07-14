# Phase 12 Plan 12-03 — Serialized persistence and audit proof

Verdict: PASS — scoped persistence/audit checkpoint

Release gate: FAIL — deferred to 12-05

The bounded database writer, atomic evidence-backed mutations, immutable audit
history, queue-only production boundary, and the required Linux/arm64 load
profiles are proven for this checkpoint. This is not an unqualified v1.0
release verdict: the unchanged project-wide backend coverage thresholds remain
below 90% lines / 85% branches and the final release candidate still belongs
to Plan 12-05.

## Identity

- Plan: `12-03`
- Plan base SHA: `c3fe7935a8ccccc0b15826bcb82d413d22e83188`
- Tested implementation SHA: `e5b45ae1ce050b1bb3b8eaad24e4d023d9c7f5fc`
- Implementation commits: `70c952b` (`test(db): prove serialized writes under concurrent load`) and `e5b45ae` (`fix(db): wait out transient writer locks`)
- Official load run: `20260714T193007Z`
- Official load platform: Linux/arm64 in an isolated Docker container
- Local verification platform: macOS arm64

The external marker `/tmp/cronometrix-12-03-base-sha` was absent at Task 7
close and was not recreated. `git cat-file -e` verifies the literal plan base
above is a commit, and the ownership checker independently required that same
literal SHA.

## Delivered contracts

### Bounded and drained writer

- One bounded `tokio::mpsc` queue owns production database mutations.
- Default capacity is 1024; foreground admission has one five-second deadline.
- Background operations retry only `Busy`, after 100/250/500 ms; accepted jobs
  are never replayed.
- `flush` is FIFO. `close_and_flush` closes admission, enqueues the shutdown
  barrier after already admitted work, drains the worker, and rejects later
  producers.
- Worker-owned transactions alone commit or roll back. Post-commit callbacks
  execute after commit and before the reply, flush, or shutdown can complete;
  a panicking callback is isolated from the worker.
- The persistent writer independently enables foreign keys,
  `synchronous=NORMAL`, and a five-second busy timeout. These SQLite PRAGMAs
  are connection-local and cannot be inherited from the migration connection.
- Stable unavailable responses remain `DB_WRITE_QUEUE_BUSY` and
  `DB_WRITE_QUEUE_UNAVAILABLE` with HTTP 503.

The queue statistics contract is:

| Counter | Meaning |
|---|---|
| `depth` | Accepted jobs not yet received by the worker |
| `accepted` | Jobs successfully admitted; excludes control barriers |
| `completed` | Admitted jobs ending successfully |
| `failed` | Admitted jobs ending with a job/transaction error |
| `busy_rejections` | Admission attempts exhausting their deadline/retries |
| `closed_rejections` | Work or non-closing control rejected after close |

### Atomic mutation domains

The following multi-resource or multi-statement domains now use one explicit
queue/transaction ownership boundary:

- leave overlap check, evidence publication, insert, cancellation, actor and
  justification retention, and inclusive-range recompute;
- daily-record override evidence, insert, response metadata, and recompute;
- attendance-event insert/dedup, optional photo ownership, and local-date
  recompute;
- daily-record upsert plus anomaly replacement;
- report computation/delivery ordering with committed export audit;
- enrollment, backfill, purge, capture cleanup, and device-operation recovery
  checkpoints, including ambiguous external-device side effects;
- authentication refresh rotation, CRUD mutations, supervisor state writes,
  E2E reset writes, and the remaining background mutations migrated by this
  plan.

Filesystem roots are injected through `state.paths`. Evidence publication is
same-directory, no-clobber, durable, and compensating. Cleanup verifies inode
ownership and refuses symlinks. Linux uses `renameat2(RENAME_NOREPLACE)` and
the platform-correct `O_NOFOLLOW` value; the Task 7 Linux/arm64 validation
found and corrected the previous hard-coded `0x20000` value, which is
`0x8000` on Linux aarch64. The existing symlink safety test passes on both
macOS and the Linux/arm64 deployment target.

### Queue-only and immutable audit boundaries

- `make check-db-write-queue` reports `PASS (0 violations)`.
- Raw mutation APIs remain confined to the exact infrastructure allowlist;
  production services cannot bypass the serialized writer.
- Migration 020 installs `BEFORE UPDATE` and `BEFORE DELETE` guards on
  `audit_log`. Both operations abort and preserve the original bytes.
- E2E reset intentionally preserves `audit_log`.
- Leave cancellation and daily-record override audit JSON retain the
  authenticated actor and legal justification.
- The audit immutability integration suite passes all eight tests.

## Official concurrent-load evidence

Command shape:

```bash
BASE_URL=http://127.0.0.1:4001 \
SERVER_LOG=/tmp/cronometrix-write-queue-remediated.log \
DURATION_SECONDS=60 \
OUT_DIR=.planning/phases/12-v1-0-release-stabilization/evidence/03-write-queue-load \
bash backend/scripts/run_write_queue_load_profiles.sh
```

The runner builds the existing production API Dockerfile, seeds an isolated
database, executes exactly four profiles, sends SIGTERM, requires backend exit
0, and then reconciles accepted HTTP writes against SQLite. Test-only license
flags exist only inside the temporary container. The trap removes its
container, image tag, and database.

| Profile | Concurrency | Mix | 2xx | Accepted writes | Persisted | 500 | 503/BUSY | p50 / p95 / p99 ms |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `c1-w100` | 1 | 100% write | 12,219 | 12,219 | 12,219 | 0 | 0 | 4.36 / 6.72 / 13.95 |
| `c32-r100` | 32 | 100% read | 16,532 | 0 | 0 | 0 | 0 | 113.81 / 169.26 / 201.99 |
| `c32-w100` | 32 | 100% write | 10,274 | 10,274 | 10,274 | 0 | 0 | 187.72 / 214.11 / 234.52 |
| `c32-w70` | 32 | 70% write | 11,244 | 7,854 | 7,854 | 0 | 0 | 191.64 / 253.13 / 289.63 |

Combined result: 50,269 successful requests, 30,347 accepted writes, 30,347
persisted rows, zero HTTP 500, zero HTTP 503, zero queue-busy responses, zero
other failures, zero `database is locked` log occurrences, and clean SIGTERM
exit 0. Evidence is 56 KiB and contains no password, token, authorization
header, or secret value.

Evidence directory:
`.planning/phases/12-v1-0-release-stabilization/evidence/03-write-queue-load`.

The load script records deterministic JSON/CSV names, separates read/write
latencies, limits samples to 20, and refuses non-60-second official runs unless
`ALLOW_SHORT_PROFILES=true` is explicitly set for diagnostics.

## Coverage ownership and release ledger

Backend coverage instrumentation executed 909 tests, all passing, with the
pre-existing 22 skipped cases unchanged. The raw command intentionally retains
the project-wide hard gate and exited 2:

| Scope | Result | Threshold | Status |
|---|---:|---:|---|
| Backend lines | 11,021 / 12,797 = 86.12% | 90% | FAIL — inherited release debt |
| Backend branches | 801 / 1,086 = 73.76% | 85% | FAIL — inherited release debt |
| 12-03 owned files | 31 backend / 0 frontend | 70% lines / 60% branches per file | PASS |

Exact scoped checker output:

```text
PASS owned-coverage plan=12-03 backend=31 frontend=0
```

The ownership manifest contains every backend production file changed from the
plan base and present in LCOV. No threshold, exclusion, workflow permission,
or source denominator was weakened. The new behavior tests closed scoped gaps
in the write queue, device service, enrollment dispatcher, watchdog, E2E reset,
user service, database-writer wrapper, and shutdown signal handling.

The global result remains executable debt assigned to 12-05; it is not
reclassified as a pass by the scoped checker.

## Verification ledger

| Verification | Exit/result |
|---|---|
| Initial load-script smoke before implementation | 1, expected RED |
| Transient external-lock regression before writer PRAGMAs | 101, expected RED |
| Transient external-lock regression after writer PRAGMAs | 0 |
| Repeated Linux/arm64 one-second diagnostics | 0, 3/3 passed with zero locks |
| `make check-db-write-queue` | 0, zero violations |
| `cargo fmt --all -- --check` | 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 |
| First final local full-suite attempt | 101, unrelated Darwin libSQL misuse flake retained in diagnostics |
| Isolated department regression repetitions | 0, 10/10 passed |
| Full local suite retry | 0, all test binaries passed |
| Write-queue integration suite | 0, 19/19 passed |
| Linux/arm64 `cargo test -j1 --all-targets --all-features` | 0, all 59 result blocks passed |
| `make coverage-backend` | 2, 909/909 passed; global floor failed |
| 12-03 owned coverage checker | 0, 31/31 backend files passed |
| Official Linux/arm64 60-second load profiles | 0, four profiles passed |
| Official post-shutdown SQLite reconciliation | 0, every accepted write matched |
| Audit immutability | 0, update/delete rejected and evidence retained |
| `git diff --check` / staged diff check after LF normalization | 0 |

The tested implementation SHA was created only after the remediated official
evidence and gates above. The initial implementation commit also changed CSV
writers from CRLF to LF. The final fix commit contains the writer-connection
remediation, its regression, and evidence produced from that exact runtime
source.

## Diagnostics and remaining risks

### macOS libSQL FFI

The first local macOS 60-second profile run was not accepted as evidence. A
mixed profile produced HTTP 500 and the backend later crashed with
`EXC_BAD_ACCESS` in `sqlite3Close` (`btreeEnterAll -> sqlite3Close ->
Drop<libsql local Connection>`). A separate full Rust run also once crashed in
the same libSQL close path, while its complete retry passed. Pure reads on a
fresh database passed. The authoritative deployment-target Linux/arm64 full
suite and official load run both completed normally. This remains a local
Darwin/libSQL FFI risk to track; it is not evidence of a Linux release pass.

The final local full-suite attempt first hit the same native instability as
`bad parameter or other API misuse` in an unrelated department read. That
exact test passed 10/10 isolated runs, coverage had already passed all 909
tests, and the complete full-suite retry exited 0. No department code was
changed without causal evidence.

### Startup lock remediation

After the first official run, a deliberately non-authoritative one-second
diagnostic detected one `database is locked` from
`supervisor.connection-state` immediately after startup and correctly exited
1. It was not dismissed because all HTTP writes happened to reconcile.

Systematic reproduction held a real external `BEGIN IMMEDIATE` lock while the
queue writer attempted an insert. Before the fix, the write ended immediately
and the regression failed with exit 101. Root cause: SQLite `busy_timeout` and
`foreign_keys` are connection-local; `init_db` configured the migration
connection, but the queue opened a different persistent connection. The writer
now configures its own connection before receiving commands. The regression
proves it remains pending during a short external lock, resumes after commit,
and persists the row.

After remediation, three consecutive one-second Linux/arm64 four-profile
smokes passed (3/3), each with zero locks and clean shutdown. The complete
60-second official matrix was then rerun and also recorded zero locks. This
finding is closed for 12-03 rather than deferred or normalized away.

### Release-close items

- Plan 12-05 must lift unchanged backend project coverage to 90% lines / 85%
  branches and run the final authoritative Linux candidate.
- The deferred live CI validation still requires positive Actions/artifact
  verification, a deliberate negative coverage PR, and required branch
  protection checks.
- Hardware-backed Hikvision and licensing evidence remains a release-level
  environment validation, not something inferred from synthetic load.

## Historical plan reconciliation

`.planning/db-write-queue-migration-plan.md` is retained as history and marked
`superseded/completed by Phase 12 Plan 12-03`. Its six sprints map to Tasks
12-03-1 through 12-03-7, and the corrected executable paths are
`backend/scripts/load_test.sh` and
`backend/scripts/run_write_queue_load_profiles.sh`.
