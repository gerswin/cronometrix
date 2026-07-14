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
  const isRelease = process.env.CRONOMETRIX_E2E_RELEASE === "true";
  const buildProfile = isRelease ? "release" : "debug";
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
    const releaseFlag = isRelease ? "--release" : "";
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

setup("verify backend sees seeded database", async ({ request }) => {
  expect(e2eEnv().CRONOMETRIX_DB_PATH).toBe(E2E_DB_PATH);
  expect(e2eEnv().LICENSE_JWT_PATH).toBe(path.join(E2E_ROOT, "license.jwt"));

  const login = await request.post(`${API_BASE}/auth/login`, {
    data: { username: "e2e_admin", password: "e2e-admin-pass" },
  });
  expect(login.status()).toBe(200);
  const loginBody = await login.json();
  expect(loginBody.access_token).toBeTruthy();

  const employees = await request.get(`${API_BASE}/employees?limit=100`, {
    headers: { Authorization: `Bearer ${loginBody.access_token}` },
  });
  expect(employees.status()).toBe(200);
  const employeesBody = await employees.json();
  const ids = (employeesBody.data ?? employeesBody.items ?? employeesBody).map(
    (employee: { id: string }) => employee.id,
  );

  expect(ids).toEqual(
    expect.arrayContaining([
      "emp-ana",
      "emp-luis",
      "emp-maria",
      "emp-pedro",
      "emp-carmen",
      "emp-jose",
    ]),
  );
});

setup("reset mutable tables (clean slate per run)", async ({ request }) => {
  // D-12: wipe mutable attendance state at the start of every full run.
  // audit_log is append-only legal evidence and is deliberately preserved;
  // specs isolate current-run evidence with record IDs and time windows.
  await resetMutableTables(request);
});
