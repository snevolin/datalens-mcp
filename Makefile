SHELL := /usr/bin/env bash

PACKAGE := datalens-mcp
VERSION := $(shell sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -n1)
TOPDIR ?= $(CURDIR)/rpmbuild
SPEC := $(PACKAGE).spec
SOURCE_ARCHIVE := $(TOPDIR)/SOURCES/$(PACKAGE)-$(VERSION).tar.gz

.PHONY: help build run check test fmt fmt-check clippy clean deb rpm rpm-source rpm-clean

help:
	@echo "Targets:"
	@echo "  build       Build release binary with cargo"
	@echo "  run         Run release binary with cargo"
	@echo "  check       Run cargo check"
	@echo "  test        Run cargo test (all targets/features)"
	@echo "  fmt         Format code"
	@echo "  fmt-check   Verify formatting"
	@echo "  clippy      Run clippy with warnings denied"
	@echo "  clean       Remove cargo build artifacts"
	@echo "  deb         Build Debian package (.deb) into target/debian"
	@echo "  rpm         Build RPM + SRPM into $(TOPDIR)"
	@echo "  rpm-source  Prepare rpmbuild tree and source archive only"
	@echo "  rpm-clean   Remove $(TOPDIR)"

build:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo build --release

run:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo run --release

check:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo check

test:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo test --all-targets --all-features

fmt:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo fmt --all

fmt-check:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo fmt --all --check

clippy:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo clippy --all-targets --all-features -- -D warnings

clean:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	cargo clean

deb:
	@command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }
	@cargo deb --help >/dev/null 2>&1 || { echo "cargo-deb is required (install cargo-deb: cargo install cargo-deb --locked)"; exit 1; }
	cargo deb
	@echo "Built DEBs:"
	@find target/debian -type f -name '*.deb' -print

rpm-source:
	@command -v git >/dev/null 2>&1 || { echo "git is required"; exit 1; }
	@mkdir -p "$(TOPDIR)"/BUILD "$(TOPDIR)"/RPMS "$(TOPDIR)"/SOURCES "$(TOPDIR)"/SPECS "$(TOPDIR)"/SRPMS
	git archive --format=tar.gz \
		--prefix="$(PACKAGE)-$(VERSION)/" \
		-o "$(SOURCE_ARCHIVE)" \
		HEAD
	cp "$(SPEC)" "$(TOPDIR)/SPECS/"

rpm: rpm-source
	@command -v rpmbuild >/dev/null 2>&1 || { echo "rpmbuild is required (install rpm-build)"; exit 1; }
	rpmbuild -ba "$(TOPDIR)/SPECS/$(SPEC)" --define "_topdir $(TOPDIR)"
	@echo "Built RPMs:"
	@find "$(TOPDIR)/RPMS" -type f -name '*.rpm' -print
	@echo "Built SRPMs:"
	@find "$(TOPDIR)/SRPMS" -type f -name '*.rpm' -print

rpm-clean:
	rm -rf "$(TOPDIR)"
