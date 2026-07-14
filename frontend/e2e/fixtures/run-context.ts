import path from "node:path";
import { tmpdir } from "node:os";

const RUN_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;
const E2E_BASENAME_PREFIX = "cronometrix-e2e-";

export function validateE2ERunId(runId: string): string {
  if (!RUN_ID_PATTERN.test(runId)) {
    throw new Error(
      "Invalid E2E run id: use 1-64 portable letters, digits, underscores, or hyphens",
    );
  }
  return runId;
}

export function resolveE2EPaths(
  runId: string,
  tempDirectory = tmpdir(),
): { root: string; dbPath: string } {
  const safeRunId = validateE2ERunId(runId);
  const tempRoot = path.resolve(tempDirectory);
  const expectedBasename = `${E2E_BASENAME_PREFIX}${safeRunId}`;
  const root = path.resolve(tempRoot, expectedBasename);
  if (path.dirname(root) !== tempRoot || path.basename(root) !== expectedBasename) {
    throw new Error("E2E temp path containment check failed");
  }
  return { root, dbPath: `${root}.db` };
}

export function assertE2ETeardownPaths(
  runId: string,
  root: string,
  dbPath: string,
): void {
  const expected = resolveE2EPaths(runId);
  if (path.resolve(root) !== expected.root || path.resolve(dbPath) !== expected.dbPath) {
    throw new Error("E2E teardown path containment check failed");
  }
}

export const E2E_RUN_ID = validateE2ERunId(
  process.env.CRONOMETRIX_E2E_RUN_ID ?? process.env.GITHUB_RUN_ID ?? "local",
);
process.env.CRONOMETRIX_E2E_RUN_ID = E2E_RUN_ID;

const E2E_PATHS = resolveE2EPaths(E2E_RUN_ID);
export const E2E_ROOT = E2E_PATHS.root;
export const E2E_DB_PATH = E2E_PATHS.dbPath;
export const E2E_DEVICE_CREDS_KEY =
  "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";
export function e2eEnv(): NodeJS.ProcessEnv {
  return {
    ...process.env,
    CRONOMETRIX_E2E: "true",
    CRONOMETRIX_LICENSE_BYPASS: "true",
    CRONOMETRIX_TEST_RESET_ENABLED: "true",
    CRONOMETRIX_DB_PATH: E2E_DB_PATH,
    TURSO_DATABASE_URL: "",
    TURSO_AUTH_TOKEN: "",
    JWT_SECRET: "e2e-test-secret-must-be-32-bytes-long-1234",
    DEVICE_CREDS_KEY: E2E_DEVICE_CREDS_KEY,
    LICENSE_JWT_PATH: path.join(E2E_ROOT, "license.jwt"),
    CRONOMETRIX_LEAVES_ROOT: path.join(E2E_ROOT, "leaves"),
    CRONOMETRIX_EVENTS_ROOT: path.join(E2E_ROOT, "events"),
    ENROLLMENTS_DIR: path.join(E2E_ROOT, "enrollments"),
    CRONOMETRIX_CAPTURES_TMP: path.join(E2E_ROOT, "captures-tmp"),
    DATA_DIR: E2E_ROOT,
    TZ: "America/Caracas",
  };
}
