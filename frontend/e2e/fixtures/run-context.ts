import path from "node:path";

export const E2E_RUN_ID =
  process.env.CRONOMETRIX_E2E_RUN_ID ?? process.env.GITHUB_RUN_ID ?? "local";
export const E2E_ROOT = `/tmp/cronometrix-e2e-${E2E_RUN_ID}`;
export const E2E_DB_PATH = path.join(E2E_ROOT, "cronometrix.db");
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
