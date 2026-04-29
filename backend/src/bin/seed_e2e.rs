//! Phase 9 — E2E DB seed binary. Gated by Cargo feature "seed-e2e".
//! Refuses to run unless CRONOMETRIX_E2E=true at runtime.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("CRONOMETRIX_E2E").as_deref() != Ok("true") {
        eprintln!("seed_e2e refuses to run without CRONOMETRIX_E2E=true");
        std::process::exit(2);
    }
    anyhow::bail!("seed_e2e: not implemented yet (Plan 03 Task 2)");
}
