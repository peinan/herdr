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

# macOS 26 の Command Line Tools SDK には libSystem の arm64-macos ターゲットが無く、
# zig 0.15.2 はそのままだとリンクに失敗する(undefined symbol: _exit ...)。zig は PATH 上の
# `xcrun --show-sdk-path` で SDK を探すので、旧 SDK を返す xcrun シムを zig にだけ見せる
# ラッパ経由で呼ぶ。作り方は docs/fork/FORK_WORKFLOW.md §0。
# ラッパが無い環境では未設定のままにして、build.rs の PATH 探索(= mise の zig)に委ねる。
ZIG_WRAPPER := $(HOME)/.local/share/herdr-fork/zig
ZIG         ?= $(wildcard $(ZIG_WRAPPER))

# Fork build identity. `Cargo.toml`'s version stays upstream-managed, so the base
# version follows upstream automatically; HERDR_BUILD_ID counts fork feature
# generations on top of it. `herdr --version` then reports e.g. 0.9.0-fork.9,
# which upstream's own binary can never claim.
#
# Bump HERDR_BUILD_ID when a fork feature lands, not on an upstream sync: the
# feature set is what the number describes. The generations are catalogued in
# docs/fork/FORK_FEATURES.md.
#
# These deliberately live here and not in `justfile`: several tests assert on
# env!("CARGO_PKG_VERSION"), so exporting them for `just check` would fail the
# suite. Only the build/install path should see them.
HERDR_BUILD_CHANNEL ?= fork
HERDR_BUILD_ID      ?= 9

.DEFAULT_GOAL := help

.PHONY: help
help:
	@echo "Personal (fork-only) targets:"
	@echo "  make build    - release build via 'just build' (zig from mise)"
	@echo "  make install  - build, then install the binary into $(INSTALL_DIR)"
	@echo ""
	@echo "Build identity: $(HERDR_BUILD_CHANNEL).$(HERDR_BUILD_ID) (override with make build HERDR_BUILD_ID=N)"

# Release build. Delegates to the upstream justfile recipe so build flags stay
# in one place; only adds the zig toolchain via mise and the fork build identity.
.PHONY: build
build:
	HERDR_BUILD_CHANNEL=$(HERDR_BUILD_CHANNEL) HERDR_BUILD_ID=$(HERDR_BUILD_ID) \
		$(if $(ZIG),ZIG=$(ZIG)) mise exec zig@$(ZIG_VERSION) -- just build

# Build, then replace the installed binary. Restart herdr afterwards to apply.
.PHONY: install
install: build
	install -m 0755 target/release/$(BIN) $(INSTALL_DIR)/$(BIN)
	@echo "installed -> $(INSTALL_DIR)/$(BIN)"
	@echo "restart herdr to apply: run 'herdr server stop', then relaunch the herdr TUI"
