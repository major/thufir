PATCH_COVERAGE_BASE ?= main
PATCH_COVERAGE_FAIL_UNDER ?= 95
PROJECT_COVERAGE_FAIL_UNDER ?= 90
DIFF_COVER ?= diff-cover

.PHONY: audit check clean clippy container-build container-push container-run container-run-config coverage doc fmt fmt-fix integration machete patch-coverage test

check: fmt clippy test doc coverage patch-coverage

fmt:
	cargo fmt --all --check

fmt-fix:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --workspace --all-features

integration:
	cargo test --workspace --all-features -- --ignored

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items

coverage:
	cargo llvm-cov --workspace --all-features --fail-under-lines $(PROJECT_COVERAGE_FAIL_UNDER) --lcov --output-path lcov.info

patch-coverage: coverage
	$(DIFF_COVER) lcov.info --compare-branch=$(PATCH_COVERAGE_BASE) --fail-under=$(PATCH_COVERAGE_FAIL_UNDER)

audit:
	cargo audit

machete:
	cargo machete

container-build:
	podman build -t thufir:dev -f Containerfile .

container-run: container-build
	podman run --rm --env-file .env thufir:dev

container-run-config: container-build
	podman run --rm --env-file .env -v ./thufir.toml:/etc/thufir/thufir.toml:ro thufir:dev --config /etc/thufir/thufir.toml

container-push:
	podman push thufir:dev ghcr.io/major/thufir:dev

clean:
	cargo clean
	rm -f lcov.info
