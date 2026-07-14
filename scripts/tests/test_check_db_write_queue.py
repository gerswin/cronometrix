from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CHECKER_PATH = REPO_ROOT / "scripts" / "check_db_write_queue.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("check_db_write_queue", CHECKER_PATH)
    if spec is None or spec.loader is None:
        raise AssertionError(f"cannot load checker from {CHECKER_PATH}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class CheckerFixture:
    def __init__(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name) / "repo"
        (self.root / "backend" / "src").mkdir(parents=True)

    def close(self) -> None:
        self._tmp.cleanup()

    def write(self, relative: str, source: str) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(textwrap.dedent(source), encoding="utf-8")
        return path


class DbWriteQueueCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = CheckerFixture()

    def tearDown(self) -> None:
        self.fixture.close()

    def scan(self):
        checker = load_checker()
        return checker.scan_path(
            self.fixture.root / "backend" / "src", self.fixture.root
        )

    def test_exact_allowlisted_migration_and_queue_writers_pass(self) -> None:
        self.fixture.write(
            "backend/src/db/mod.rs",
            """
            async fn migrate(conn: &libsql::Connection) {
                conn.execute_batch("CREATE TABLE example(id TEXT);").await;
            }
            """,
        )
        self.fixture.write(
            "backend/src/db/write_queue.rs",
            """
            async fn writer(connection: &libsql::Connection) {
                connection.execute("INSERT INTO x VALUES (1)", ()).await;
                let _tx = connection.transaction().await;
            }
            """,
        )
        self.assertEqual(self.scan(), [])

    def test_feature_gated_e2e_allowlist_is_exact(self) -> None:
        self.fixture.write(
            "backend/src/bin/seed_e2e.rs",
            'async fn seed(c: &Connection) { c.execute("INSERT", ()).await; }',
        )
        self.fixture.write(
            "backend/src/test_reset/mod.rs",
            'async fn reset(c: &Connection) { c.execute("DELETE", ()).await; }',
        )
        self.fixture.write(
            "backend/src/not_db/mod.rs",
            'async fn bypass(c: &Connection) { c.execute("INSERT", ()).await; }',
        )

        violations = self.scan()

        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].path.as_posix(), "backend/src/not_db/mod.rs")

    def test_production_execute_reports_actionable_file_line(self) -> None:
        self.fixture.write(
            "backend/src/employees/service.rs",
            """
            async fn save(conn: &Connection) {
                conn.execute("INSERT INTO employees VALUES (1)", ()).await;
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].path.as_posix(), "backend/src/employees/service.rs")
        self.assertEqual(violations[0].line, 3)
        self.assertEqual(violations[0].method, "execute")

    def test_multiline_renamed_receiver_and_all_raw_methods_fail(self) -> None:
        self.fixture.write(
            "backend/src/domain/service.rs",
            """
            async fn write(db_writer: &Connection) {
                db_writer
                    . execute
                    ("UPDATE records SET x = 1", ())
                    .await;
                db_writer.execute_batch("DELETE FROM records;").await;
                let _tx = db_writer.transaction().await;
                Connection::execute(db_writer, "INSERT", ()).await;
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(
            [violation.method for violation in violations],
            ["execute", "execute_batch", "transaction", "execute"],
        )

    def test_turbofish_cannot_hide_raw_write_identifiers(self) -> None:
        self.fixture.write(
            "backend/src/domain/turbofish.rs",
            """
            async fn write(conn: &Connection) {
                conn.execute::<usize>("INSERT", ()).await;
                conn.execute_batch::<()>("DELETE").await;
                conn.transaction::<libsql::Deferred>().await;
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(
            [violation.method for violation in violations],
            ["execute", "execute_batch", "transaction"],
        )

    def test_ufcs_and_method_references_cannot_hide_raw_writers(self) -> None:
        self.fixture.write(
            "backend/src/domain/references.rs",
            """
            fn references() {
                let write = Connection::execute;
                let batch = <Connection as RawWriter>::execute_batch;
                let begin = libsql::Connection::transaction;
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(
            [violation.method for violation in violations],
            ["execute", "execute_batch", "transaction"],
        )

    def test_lifetimes_on_same_line_do_not_hide_raw_writer(self) -> None:
        self.fixture.write(
            "backend/src/domain/lifetimes.rs",
            """
            async fn persist<'a>(conn: &Connection) { conn.execute("INSERT", ()).await; let _: Marker<&'a ()>; }
            """,
        )

        violations = self.scan()

        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].method, "execute")

    def test_lifetimes_and_character_literals_without_writers_pass(self) -> None:
        self.fixture.write(
            "backend/src/domain/lifetime_read.rs",
            r"""
            fn read<'a, 'static_value>(value: &'a str) -> &'a str {
                let plain = 'x';
                let escaped = '\n';
                let quote = '\'';
                value
            }
            """,
        )

        self.assertEqual(self.scan(), [])

    def test_macro_argument_cannot_parameterize_forbidden_method_name(self) -> None:
        self.fixture.write(
            "backend/src/domain/macro_dispatch.rs",
            """
            macro_rules! dispatch {
                ($receiver:expr, $method:ident) => {
                    $receiver.$method("INSERT", ())
                };
            }

            async fn write(conn: &Connection) {
                dispatch!(conn, execute).await;
                dispatch!(conn, execute_batch).await;
                dispatch!(conn, transaction).await;
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(
            [violation.method for violation in violations],
            ["execute", "execute_batch", "transaction"],
        )

    def test_comments_strings_and_non_forbidden_identifiers_pass(self) -> None:
        self.fixture.write(
            "backend/src/domain/read.rs",
            r'''
            // conn.execute("DELETE", ()).await;
            /* outer comment /* nested: other.transaction() */ other.execute_batch() */
            const MESSAGE: &str = "conn.execute(\"INSERT\", ())";
            const RAW: &str = r#"renamed.transaction().await"#;
            struct Names { execute_count: usize, transaction_count: usize }
            fn read(conn: &Connection) { let _ = conn.query("SELECT 1", ()); }
            ''',
        )
        self.assertEqual(self.scan(), [])

    def test_production_macro_body_cannot_hide_raw_writer(self) -> None:
        self.fixture.write(
            "backend/src/domain/macros.rs",
            """
            macro_rules! mutate {
                ($renamed:expr) => {{
                    $renamed.execute("INSERT INTO records VALUES (1)", ())
                }};
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].method, "execute")

    def test_cfg_test_module_and_function_are_excluded_structurally(self) -> None:
        self.fixture.write(
            "backend/src/domain/service.rs",
            """
            #[cfg(
                test
            )]
            mod tests {
                async fn seed(test_conn: &Connection) {
                    test_conn.execute("INSERT", ()).await;
                    if true { test_conn.execute_batch("DELETE;").await; }
                }
            }

            #[cfg(test)]
            async fn test_helper(conn: &Connection)
            where
                Connection: Clone,
            {
                conn.transaction().await;
            }
            """,
        )
        self.assertEqual(self.scan(), [])

    def test_cfg_test_item_does_not_hide_following_production_item(self) -> None:
        self.fixture.write(
            "backend/src/domain/service.rs",
            """
            #[cfg(test)]
            async fn seed(conn: &Connection) {
                conn.execute("INSERT test", ()).await;
            }

            async fn production(conn: &Connection) {
                conn.execute("INSERT production", ()).await;
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].line, 8)

    def test_only_cfg_expressions_that_imply_test_are_excluded(self) -> None:
        self.fixture.write(
            "backend/src/domain/service.rs",
            """
            #[cfg(all(test, feature = "fixture"))]
            fn test_only(conn: &Connection) { conn.execute("INSERT test", ()); }

            #[cfg(any(test, feature = "production-extra"))]
            fn can_be_production(conn: &Connection) {
                conn.execute("INSERT production", ());
            }
            """,
        )

        violations = self.scan()

        self.assertEqual(len(violations), 1)
        self.assertEqual(violations[0].line, 7)

    def test_cli_is_cwd_independent_and_returns_nonzero_with_file_line(self) -> None:
        target = self.fixture.write(
            "backend/src/domain/service.rs",
            'async fn save(c: &Connection) { c.execute("INSERT", ()).await; }',
        )
        with tempfile.TemporaryDirectory() as unrelated_cwd:
            result = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER_PATH),
                    "--repo-root",
                    str(self.fixture.root),
                    str(self.fixture.root / "backend" / "src"),
                ],
                cwd=unrelated_cwd,
                text=True,
                capture_output=True,
                check=False,
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(f"{target.relative_to(self.fixture.root).as_posix()}:1", result.stdout)
        self.assertIn(
            "forbidden raw write identifier 'execute' bypasses state.db_write",
            result.stdout,
        )

    def test_cli_pass_output_is_deterministic(self) -> None:
        self.fixture.write(
            "backend/src/domain/read.rs",
            'async fn read(c: &Connection) { c.query("SELECT 1", ()).await; }',
        )
        result = subprocess.run(
            [
                sys.executable,
                str(CHECKER_PATH),
                "--repo-root",
                str(self.fixture.root),
                str(self.fixture.root / "backend" / "src"),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout, "DB write queue boundary: PASS (0 violations)\n")


class GateWiringTests(unittest.TestCase):
    def test_make_target_invokes_checker_on_backend_src(self) -> None:
        makefile = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
        self.assertIn("check-db-write-queue:", makefile)
        self.assertIn("python3 scripts/check_db_write_queue.py backend/src", makefile)

    def test_ci_runs_checker_before_backend_coverage(self) -> None:
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        checker = workflow.index("Run DB write queue boundary checker")
        coverage = workflow.index("Run coverage with project-wide line gate")
        self.assertLess(checker, coverage)


if __name__ == "__main__":
    unittest.main()
