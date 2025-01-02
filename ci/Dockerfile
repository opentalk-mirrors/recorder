# SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
#
# SPDX-License-Identifier: EUPL-1.2

FROM git.opentalk.dev:5050/opentalk/backend/containers/gstreamer:1.24-intel AS builder

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates rustup build-essential pkg-config libglib2.0-dev \
    # This is needed for the livekit crate in the compositor
    libxext-dev libx11-dev libgl-dev && \
    rustup set auto-self-update disable && \
    rustup install stable --profile minimal

WORKDIR /recorder

COPY ./build.rs ./build.rs
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY src src

RUN cargo build --release

FROM git.opentalk.dev:5050/opentalk/backend/containers/gstreamer:1.24-intel

RUN apt-get update && \
    apt-get install -y --no-install-recommends libssl3 libcrypto++ \
    intel-media-va-driver intel-gpu-tools \
    libopenh264-7 ca-certificates \
    tzdata \
    fontconfig fonts-terminus ttf-bitstream-vera fonts-liberation fonts-noto \
    libcap2 && \
    setcap cap_perfmon=+ep $(which intel_gpu_top); \
    fc-cache -fv

WORKDIR /recorder

COPY --from=builder /recorder/target/release/opentalk-recorder .

ENTRYPOINT [ "./opentalk-recorder" ]
