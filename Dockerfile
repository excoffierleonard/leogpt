##############################
# Stage 1: Prepare the Recipe
##############################
FROM rust:alpine AS chef
RUN cargo install cargo-chef
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

##############################
# Stage 2: Cache Dependencies
##############################
FROM rust:alpine AS builder

# Opus version for static linking (required by songbird)
ARG OPUS_VERSION=1.5.2

# Opus static linking (paths without /lib - build.rs appends it)
ENV OPUS_STATIC=1 \
    OPUS_NO_PKG=1 \
    OPUS_LIB_DIR=/opt/opus \
    LIBOPUS_LIB_DIR=/opt/opus

# Musl build flags
ENV CC_x86_64_unknown_linux_musl=/usr/bin/gcc \
    CFLAGS_x86_64_unknown_linux_musl="-fno-stack-protector" \
    CMAKE_C_COMPILER=/usr/bin/gcc \
    CMAKE_C_FLAGS="-fno-stack-protector" \
    RUSTFLAGS="-C link-arg=-lm"

RUN apk add --no-cache build-base musl-dev cmake curl

# Build static opus
RUN mkdir -p /tmp/opus && \
    cd /tmp/opus && \
    curl -L -o opus.tar.gz https://downloads.xiph.org/releases/opus/opus-${OPUS_VERSION}.tar.gz && \
    tar -xzf opus.tar.gz && \
    cmake -S opus-${OPUS_VERSION} -B build \
    -DOPUS_BUILD_SHARED_LIBRARY=OFF \
    -DOPUS_STACK_PROTECTOR=OFF \
    -DOPUS_HARDENING=OFF \
    -DOPUS_FORTIFY_SOURCE=OFF \
    -DCMAKE_INSTALL_PREFIX=/opt/opus && \
    cmake --build build --target install

RUN cargo install cargo-chef
WORKDIR /app
COPY --from=chef /app/recipe.json .
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

##############################
# Stage 3: Final Image
##############################
FROM scratch
WORKDIR /app
COPY --from=builder /app/target/release/leogpt .
ENTRYPOINT ["./leogpt"]
