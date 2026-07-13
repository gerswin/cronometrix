import { test as setup, expect } from "@playwright/test";
import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { API_BASE, resetMutableTables } from "../fixtures/api";
import { E2E_ROOT, E2E_DB_PATH, e2eEnv } from "../fixtures/run-context";

const BACKEND_DIR = path.resolve(__dirname, "../../../backend");

setup("verify backend health", async ({ request }) => {
  const r = await request.get(`${API_BASE}/health`);
  expect(r.status()).toBe(200);
});

setup("seed e2e database (idempotent)", async () => {
  // Backend webServer already started with the same deterministic local env.
  // Run the seed binary against the same CRONOMETRIX_DB_PATH.
  const storageDir = E2E_ROOT;

  const seedEnv: NodeJS.ProcessEnv = {
    ...process.env,
    ...e2eEnv(),
    JWT_SECRET: "e2e-test-secret-must-be-32-bytes-long-1234",
    LICENSE_JWT_PATH: "/tmp/nonexistent-license.jwt",
    TZ: "America/Caracas",
    // Filesystem roots (CLAUDE.md convention — Paths::from_env reads these)
    CRONOMETRIX_LEAVES_ROOT: `${storageDir}/leaves`,
    CRONOMETRIX_EVENTS_ROOT: `${storageDir}/events`,
    ENROLLMENTS_DIR: `${storageDir}/enrollments`,
    CRONOMETRIX_CAPTURES_TMP: `${storageDir}/captures-tmp`,
    DATA_DIR: storageDir,
  };

  // Prefer a pre-built binary over `cargo run` (faster + avoids swallowing build errors).
  // In CI, the binary is pre-built by the workflow step before Playwright starts.
  // In local dev, run `make e2e-build` once after pulling the branch.
  const isCi = !!process.env.CI;
  const buildProfile = isCi ? "release" : "debug";
  const prebuiltPath = path.join(
    BACKEND_DIR,
    "target",
    buildProfile,
    "seed_e2e",
  );
  const haveBinary = fs.existsSync(prebuiltPath);

  let seedCmd: string;
  if (haveBinary) {
    seedCmd = prebuiltPath;
  } else {
    // Fallback: build + run via cargo. Normalize whitespace so releaseFlag="" in dev
    // doesn't produce a double-space ("cargo run  --bin …") that some shells reject.
    const releaseFlag = isCi ? "--release" : "";
    seedCmd =
      `cargo run ${releaseFlag} --bin seed_e2e --features seed-e2e --quiet`
        .replace(/\s+/g, " ")
        .trim();
  }

  execSync(seedCmd, {
    cwd: BACKEND_DIR,
    env: seedEnv,
    stdio: "inherit",
  });
});

setup("reset mutable tables (clean slate per run)", async ({ request }) => {
  // D-12: at the start of EVERY full test run, wipe attendance_events / leaves /
  // audit_log so seeded events from prior runs don't leak in.
  await resetMutableTables(request);
});
