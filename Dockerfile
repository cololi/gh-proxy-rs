# syntax=docker/dockerfile:1

# ---- build stage ----
# Build a fully-static musl binary. In multi-arch buildx builds, BuildKit runs
# this stage under QEMU emulation for each requested platform, so the native
# rust:1-alpine image targets the correct architecture automatically.
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /src

# Cache dependency compilation independently of source changes.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src tests \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && cargo build --release \
    && rm -rf src

# Copy real sources and rebuild. Touching main.rs invalidates the cached
# fingerprint so cargo rebuilds the binary (deps stay cached).
COPY src/ ./src/
RUN touch src/main.rs src/lib.rs \
    && cargo build --release \
    && cp target/release/hub-proxy /out-hub-proxy

# ---- runtime stage ----
FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /out-hub-proxy /hub-proxy

EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/hub-proxy"]
