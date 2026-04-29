# Makefile (Phase 8 — coverage gate orchestration)
# Usage:
#   make coverage           — run both backend + frontend coverage; fail on threshold miss
#   make coverage-backend   — backend only (cargo-llvm-cov + lcov post-processor)
#   make coverage-frontend  — frontend only (Vitest with --coverage)
#
# The same commands are invoked by .github/workflows/ci.yml so local and CI runs
# produce the same numbers (within toolchain version tolerance).

.PHONY: coverage coverage-backend coverage-frontend

coverage: coverage-backend coverage-frontend
	@echo "All coverage gates passed."

coverage-backend:
	cd backend && cargo llvm-cov nextest --branch --all-features \
	  --ignore-filename-regex '(main\.rs|tests/common/.*)' \
	  --fail-under-lines 90 --lcov --output-path lcov.info
	bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60
	cd backend && cargo llvm-cov --branch --all-features --no-clean --html
	@echo "Backend HTML: backend/target/llvm-cov/html/index.html"

coverage-frontend:
	cd frontend && npx vitest run --coverage
	@echo "Frontend HTML: frontend/coverage/index.html"

.PHONY: e2e e2e-install e2e-build

e2e-install:
	cd frontend && npm ci && npx playwright install --with-deps chromium

e2e-build:
	cd backend && cargo build --release --bin cronometrix
	cd backend && cargo build --release --bin mock_hikvision --features mock-hikvision
	cd backend && cargo build --release --bin seed_e2e --features seed-e2e

e2e: e2e-build
	cd frontend && npx playwright test
	@echo "E2E HTML: frontend/playwright-report/index.html"
