# Personal convenience targets for this fork. NOT part of upstream.
#
# Upstream's canonical recipes live in `justfile`; keep shared build/test/release
# logic there. This Makefile only holds personal helpers that upstream does not
# have (e.g. building/installing with the locally required zig toolchain).
#
# Building the vendored libghostty-vt needs zig, which here is provided via mise
# (not on PATH), so build targets wrap `just` with `mise exec`. Keep ZIG_VERSION
# at or above the `minimum_zig_version` in vendor/libghostty-vt/build.zig.zon.

ZIG_VERSION ?= 0.15.2
INSTALL_DIR ?= $(HOME)/.local/bin
BIN         := herdr

.DEFAULT_GOAL := help

.PHONY: help
help:
	@echo "Personal (fork-only) targets:"
	@echo "  make build    - release build via 'just build' (zig from mise)"
	@echo "  make install  - build, then install the binary into $(INSTALL_DIR)"

# Release build. Delegates to the upstream justfile recipe so build flags stay
# in one place; only adds the zig toolchain via mise.
.PHONY: build
build:
	mise exec zig@$(ZIG_VERSION) -- just build

# Build, then replace the installed binary. Restart herdr afterwards to apply.
.PHONY: install
install: build
	install -m 0755 target/release/$(BIN) $(INSTALL_DIR)/$(BIN)
	@echo "installed -> $(INSTALL_DIR)/$(BIN)"
	@echo "restart herdr to apply: run 'herdr server stop', then relaunch the herdr TUI"
