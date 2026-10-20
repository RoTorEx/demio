.PHONY: vibe-kernel-path vibe-kernel-set vibe-pull

vibe-kernel-path:
	@test -f .vibe/KERNEL_SOURCE || { echo "Missing .vibe/KERNEL_SOURCE. Run: make vibe-kernel-set" >&2; exit 1; }
	@sed -n '1p' .vibe/KERNEL_SOURCE

vibe-kernel-set:
	@mkdir -p .vibe; \
	if [ -n "$(KERNEL)" ]; then kernel_root="$(KERNEL)"; else printf "Kernel path: "; read -r kernel_root; fi; \
	case "$$kernel_root" in /*) ;; *) echo "ERROR: kernel path must be absolute." >&2; exit 1;; esac; \
	test -f "$$kernel_root/tools/vibe-pull" || { echo "ERROR: invalid kernel path: $$kernel_root" >&2; exit 1; }; \
	printf "%s\n" "$$kernel_root" > .vibe/KERNEL_SOURCE

vibe-pull:
	@test -f .vibe/KERNEL_SOURCE || { echo "Missing .vibe/KERNEL_SOURCE. Run: make vibe-kernel-set" >&2; exit 1; }
	@kernel_root="$$(sed -n '1p' .vibe/KERNEL_SOURCE)"; \
	python3 "$$kernel_root/tools/vibe-pull" .

# VIBE:KERNEL_MAKE_START

.PHONY: vibe-propose

vibe-propose:
	@test -f .vibe/KERNEL_SOURCE || { echo "Missing .vibe/KERNEL_SOURCE. Run: make vibe-kernel-set" >&2; exit 1; }
	@kernel_root="$$(sed -n '1p' .vibe/KERNEL_SOURCE)"; \
	test -f "$$kernel_root/tools/vibe-propose" || { echo "Missing $$kernel_root/tools/vibe-propose. Update the kernel source first." >&2; exit 1; }; \
	python3 "$$kernel_root/tools/vibe-propose" .

# VIBE:KERNEL_MAKE_END

.DEFAULT_GOAL := build
CARGO_TARGET_DIR ?= $(HOME)/construction_side/demio/target
export CARGO_TARGET_DIR

.PHONY: install build test lint fmt check run package smoke-install install-local release release-push
install:
	cargo fetch --locked
build:
	cargo build --release --locked
	xcrun swiftc native/Overlay.swift -o "$(CARGO_TARGET_DIR)/release/demio-overlay" -framework AppKit -framework Carbon -target arm64-apple-macosx14.0
lint:
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
fmt:
	cargo fmt --all
test:
	cargo test --locked
	python3 tests/cli.py "$(CARGO_TARGET_DIR)/release/demio"
	python3 tests/selector.py "$(CARGO_TARGET_DIR)/release/demio"
check: lint build test
	sh -n scripts/install.sh scripts/release-push.sh
	python3 -c 'import ast,pathlib; [ast.parse(p.read_text()) for p in list(pathlib.Path("scripts").glob("*.py"))+list(pathlib.Path("examples").rglob("*.py"))+list(pathlib.Path("tests").glob("*.py"))]'
run: build
	"$(CARGO_TARGET_DIR)/release/demio"
package: build
	python3 scripts/package.py
smoke-install: package
	python3 tests/install.py "$(CARGO_TARGET_DIR)/../dist"
install-local: package
	sh scripts/install.sh --archive "$(CARGO_TARGET_DIR)/../dist/demio-v$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)-aarch64-apple-darwin.tar.gz"
release:
	python3 scripts/release.py
release-push:
	sh scripts/release-push.sh

.PHONY: smoke-ui
smoke-ui: build
	python3 tests/native.py "$(CARGO_TARGET_DIR)/release/demio-overlay"
