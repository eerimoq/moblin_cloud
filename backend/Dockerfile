FROM --platform=$BUILDPLATFORM rust:1.98-slim-trixie AS builder
ARG BUILDARCH
ARG TARGETARCH
WORKDIR /app
RUN case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-gnu; packages="gcc-x86-64-linux-gnu libc6-dev-amd64-cross"; linker=x86_64-linux-gnu-gcc ;; \
      arm64) target=aarch64-unknown-linux-gnu; packages="gcc-aarch64-linux-gnu libc6-dev-arm64-cross"; linker=aarch64-linux-gnu-gcc ;; \
      *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && rustup target add "$target" \
    && if [ "$BUILDARCH" != "$TARGETARCH" ]; then \
         apt-get update \
         && apt-get install -y --no-install-recommends $packages \
         && rm -rf /var/lib/apt/lists/* \
         && mkdir .cargo \
         && printf '[target.%s]\nlinker = "%s"\n' "$target" "$linker" > .cargo/config.toml; \
       fi \
    && echo "$target" > /rust-target
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked --target "$(cat /rust-target)" \
    && rm -rf src "target/$(cat /rust-target)/release/moblin-website-backend"* \
       "target/$(cat /rust-target)/release/deps/moblin_website_backend"*
COPY src ./src
ARG VERSION=dev
ENV MOBLIN_WEBSITE_VERSION=$VERSION
RUN cargo build --release --locked --target "$(cat /rust-target)" \
    && cp "target/$(cat /rust-target)/release/moblin-website-backend" /moblin-website-backend

FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=builder /moblin-website-backend /usr/local/bin/moblin-website-backend
ENV LISTEN=0.0.0.0:8080
EXPOSE 8080
ENTRYPOINT ["moblin-website-backend"]
