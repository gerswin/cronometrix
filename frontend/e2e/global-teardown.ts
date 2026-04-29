import * as fs from 'node:fs/promises'

/**
 * Playwright globalTeardown — runs once at the end of the full test suite.
 *
 * Removes per-run ephemeral state from /tmp so disk doesn't accumulate across runs:
 * - /tmp/cronometrix-e2e-{RUN_ID}/  (filesystem roots: leaves, events, enrollments, …)
 * - /tmp/cronometrix-e2e-{RUN_ID}.db (SQLite DB file + WAL/SHM siblings)
 *
 * .auth/{role}.json files are intentionally LEFT in place between local runs —
 * Playwright treats them as a cache and the setup project regenerates them on the
 * next run anyway. CI runners always start with a fresh FS so this is moot there.
 */
export default async function globalTeardown(): Promise<void> {
  const RUN_ID = process.env.GITHUB_RUN_ID ?? `local-${process.pid}`
  const PATHS_ROOT = `/tmp/cronometrix-e2e-${RUN_ID}`
  const DB_PATH = `${PATHS_ROOT}.db`

  await Promise.all([
    // Remove the paths root directory (leaves, events, enrollments, captures-tmp, overrides)
    fs.rm(PATHS_ROOT, { recursive: true, force: true }).catch(() => undefined),
    // Remove the SQLite DB file itself
    fs.rm(DB_PATH, { force: true }).catch(() => undefined),
    // Also remove WAL and SHM sidecar files that libSQL may have created
    fs.rm(`${DB_PATH}-wal`, { force: true }).catch(() => undefined),
    fs.rm(`${DB_PATH}-shm`, { force: true }).catch(() => undefined),
  ])
}
