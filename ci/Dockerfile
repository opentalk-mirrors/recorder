# SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
#
# SPDX-License-Identifier: EUPL-1.2

FROM git.opentalk.dev:5050/opentalk/backend/containers/rust:alpine3.19 AS Builder

RUN apk add --no-cache glib-dev openssl-dev gstreamer-dev gst-plugins-base-dev gst-plugins-bad-dev

WORKDIR /recorder

COPY ./Cargo.toml ./Cargo.toml
COPY ./compositor/Cargo.toml ./compositor/Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

# cache library build
# We need a proper main.rs to make the first cargo build run
# As we haven't copied any source files yet, this will only compile the dependencies
RUN mkdir src; \
    echo 'fn main() {println!("Hello World!");}' > src/main.rs \
    && mkdir compositor/src && touch compositor/src/lib.rs;\
    RUSTFLAGS=-Ctarget-feature=-crt-static cargo build --release

# Now copy the real source over
COPY src src
COPY compositor/src compositor/src

# Cargo detects changes only by mtime
RUN touch src/main.rs; \
    RUSTFLAGS=-Ctarget-feature=-crt-static cargo build --release

FROM alpine:3.18

RUN apk add --no-cache glib libssl1.1 libcrypto1.1 ffmpeg \
    gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly libnice-gstreamer gst-libav \
    terminus-font font-bitstream-type1 font-bitstream-75dpi font-bitstream-100dpi font-liberation font-notoi; \
    fc-cache -fv

WORKDIR /recorder

COPY --from=Builder /recorder/target/release/opentalk-recorder .

ENTRYPOINT [ "./opentalk-recorder" ]
