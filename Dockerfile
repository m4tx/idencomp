FROM ubuntu:22.04

RUN apt-get -y update && \
    apt-get upgrade -y && \
    apt-get -y install make clang-14 lld-14 curl
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
ENV PATH="/root/.cargo/bin:$PATH"

WORKDIR /usr/src/app
COPY . .
ENV CC=clang-14 \
    CFLAGS="-flto=thin -fuse-ld=lld-14" \
    CARGO_PROFILE_RELEASE_LTO=true \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang-14 \
    RUSTFLAGS="-Clinker-plugin-lto -Clink-arg=-flto -Clink-arg=-fuse-ld=lld-14 -Ctarget-feature=+sse4.2"
