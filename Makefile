# Makefile (Phase 8 — coverage gate orchestration)
# Usage:
#   make coverage           — run both backend + frontend coverage; fail on threshold miss
#   make coverage-backend   — backend only (cargo-llvm-cov + lcov post-processor)
#   make coverage-frontend  — frontend only (Vitest with --coverage)
#
# The same commands are invoked by .github/workflows/ci.yml so local and CI runs
# produce the same numbers (within toolchain version tolerance).

.PHONY: test-ci-config check-db-write-queue write-queue-load-profiles coverage coverage-backend coverage-frontend

test-ci-config:
	bash scripts/tests/test-ci-node-version-files.sh
	bash scripts/tests/test-ci-node-version-files-portability.sh
	bash scripts/test-e2e-harness-config.sh

coverage: test-ci-config coverage-backend coverage-frontend
	@echo "All coverage gates passed."

check-db-write-queue:
	python3 scripts/check_db_write_queue.py backend/src

write-queue-load-profiles:
	BASE_URL=http://127.0.0.1:4001 DURATION_SECONDS=60 \
	  bash backend/scripts/run_write_queue_load_profiles.sh

coverage-backend: check-db-write-queue
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

NEXT_PUBLIC_API_URL ?= http://localhost:4001

e2e-install:
	cd frontend && npm ci && npx playwright install --with-deps chromium

e2e-build: test-ci-config
	cd backend && cargo build --release --bin cronometrix
	cd backend && cargo build --release --bin mock_hikvision --features mock-hikvision
	cd backend && cargo build --release --bin seed_e2e --features seed-e2e
	cd frontend && NEXT_PUBLIC_API_URL="$(NEXT_PUBLIC_API_URL)" npm run build

e2e: e2e-build
	cd frontend && CRONOMETRIX_E2E_RELEASE=true CRONOMETRIX_E2E_RUN_ID=$${CRONOMETRIX_E2E_RUN_ID:-local-$$(date +%s)} npx playwright test
	@echo "E2E HTML: frontend/playwright-report/index.html"
