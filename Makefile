.PHONY: build test clean run dist docker

BINARY_NAME := hub-proxy
DIST_DIR    := bin
TARGET_AMD64 := x86_64-unknown-linux-musl
TARGET_ARM64 := aarch64-unknown-linux-musl

# Prefer `cross` for cross-compilation when available. Fall back to cargo + rustup target.
# Install cross with: cargo install cross --git https://github.com/cross-rs/cross
CROSS := $(shell command -v cross 2>/dev/null)
ifeq ($(CROSS),)
  BUILDER := cargo
else
  BUILDER := cross
endif

build:
	cargo build --release
	cp target/release/$(BINARY_NAME) ./$(BINARY_NAME)

test:
	cargo test --all-targets

clean:
	cargo clean
	rm -rf $(DIST_DIR)
	rm -f ./$(BINARY_NAME)

run: build
	./$(BINARY_NAME)

# Cross-compile static linux/amd64 and linux/arm64 binaries via musl.
# Uses `cross` if installed (recommended), otherwise plain cargo (which
# requires `rustup target add <triple>` and a working musl toolchain).
dist:
	@mkdir -p $(DIST_DIR)
	@echo "==> Building $(TARGET_AMD64) using $(BUILDER)"
	$(BUILDER) build --release --target $(TARGET_AMD64)
	cp target/$(TARGET_AMD64)/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-linux-amd64
	@echo "==> Building $(TARGET_ARM64) using $(BUILDER)"
	$(BUILDER) build --release --target $(TARGET_ARM64)
	cp target/$(TARGET_ARM64)/release/$(BINARY_NAME) $(DIST_DIR)/$(BINARY_NAME)-linux-arm64

docker:
	docker build -t $(BINARY_NAME):local .
