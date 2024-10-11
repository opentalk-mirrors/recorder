# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.6.1

### 🚀 New features

- feat: publish the compositor on crates.io ([#114](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/114), [!239](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/239))

## 0.6.0

### 🚀 New features

- Add Chunk-Upload capability ([#92](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/92))
- Add Hardware Acceleration for Intel GPUs ([#150](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/150))

### 🐛 Bug fixes

- Add gst plugin checks ([!218](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/218))

### ⚙ Miscellaneous

- Ignore RUSTSEC-2024-0370 ([!224](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/224))
- Sync changelog for release 0.5.0 and 0.5.1 ([!220](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/220))
- Update git.opentalk.dev:5050/opentalk/backend/containers/rust docker tag to v1.81.0 ([!225](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/225))

### 📦 Dependencies

- Lock file maintenance ([!233](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/233))
- Update rust crate bytes to v1.7.2 ([!234](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/234))
- Update rust crate cocoa to 0.26 ([!212](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/212))
- Update rust crate serde_json to v1.0.128 ([!229](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/229))

## 0.5.1

### 🚀 New features

- Auto Configure Quality of Subscribed Video Streams ([!177](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/177))

### 🐛 Bug fixes

- Check for gstreamer srtp elements on start ([!198](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/198))
- Update TLS dependencies to support self signed certs ([!192](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/192))
- Update docs ([!192](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/192))
- Add timezone package ([!201](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/201))
- Replace appsink/src with intersink/src ([!206](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/206))

### 📚 Documentation

- Add clock pattern to example config and ENV ([!201](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/201))

### 📦 Dependencies

- Lock file maintenance ([!210](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/210))
- Update rust crate bytes to v1.7.1 ([!199](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/199))
- Update rust crate env_logger to v0.11.5 ([!191](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/191))
- Update rust crate lapin to v2.5.0 ([!194](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/194))
- Update rust crate serde to v1.0.205 ([!209](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/209))
- Update rust crate serde_json to v1.0.125 ([!215](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/215))
- Update rust crate sysinfo to v0.31.2 ([!205](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/205))
- Update rust crate tempfile to v3.12.0 ([!207](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/207))
- Update rust crate tokio to v1.39.2 ([!193](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/193))
- Update rust crate types to 0.19.0 ([!184](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/184))

## 0.5.0

### <!-- 0 -->🚀 New features

- Auto Configure Quality of Subscribed Video Streams ([#119](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/119))
- Watch GStreamer bus for better error handling ([#117](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/117))

### <!-- 1 -->🐛 Bug fixes

- Check for gstreamer srtp elements on start ([!198](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/198))
- Update TLS dependencies to support self signed certs ([!192](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/192))
- Update docs ([#147](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/147))
- Add timezone package to container ([#147](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/147))
- End all streams when recorder is done ([#140](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/140))

### <!-- 3 -->:gear: Miscellaneous

- Build executable with `cargo auditable`

### <!-- 4 -->📚 Documentation

- Add clock pattern to example config and ENV ([#147](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/147))

### <!-- 5 --> Dependencies

- Update gstreamer-rs
- Update rust crate bytes to v1.6.1
- Update rust crate env_logger to v0.11.5
- Update rust crate lapin to v2.5.0
- Update rust crate serde_json to v1.0.121
- Update rust crate thiserror to v1.0.63
- Update rust crate tokio to v1.39.2

## 0.4.0

### <!-- 0 -->:rocket: New features

- Implement simple load balancing logic [#125](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/125))
- configure recording to use vp8 encoder in realtime mode and webm container format [#136](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/136))

### <!-- 1 -->:bug: Bug fixes

- Remove hysteresis and use absolute value instead of avg ([!132](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/132))
- End all streams when recorder is done [#140](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/140))
- *(deps)* Update rust crate bytes to v1.6.1

### <!-- 3 -->:gear: Miscellaneous

- Update alpine docker tag to v3.20
- Update rust crate config to 0.14
- Update rust crate log to v0.4.22
- Update rust crate serde to v1.0.204
- Update rust crate serde_json to v1.0.120
- Update rust crate uuid to v1.9.1
- Update rust crates gstreamer to v0.22

### Ci

- Call `cargo-deny` with `--deny unmatched-skip` ([#130](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/130))
- Use image with fixed rust version
- Update ci image to alpine3.20

## 0.3.0

### Added

- Add the capability to set the format of the clock in the `config.toml` ([#108](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/108))

## 0.2.0

### Added

- Add dynamic RTMP streaming configuration ([#100](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/100))

### Fixed

- Clean shutdown of Matroska and MP4 Sink([#106](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/106))
- Recording should start from 0 and not from system time ([#115](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/115))
- Mixer::set_stream_to_position invalid check and possible panic ([!125](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/125))
- Disable participants swap for two participants ([!128](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/128))

## 0.1.0

### Added

- Adding multi sinks support to stream concurrently to multiple outputs ([#62](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/62))
- Prioritize screen capture over the camera feed. If someone is screen sharing, it will take higher priority over the camera feed for speaker detection ([#33](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/33))
- Add streaming sink to recorder to prepare for upcoming streaming
- Added a check for whether all gstreamer Plugins are available as well as check for presence of libnice and ffmpeg ([#89](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/89))

### Changes

- Changed way visible streams and speaker is managed
- Make the video sink/source optional in the compositor ([#88](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/88))
- Remove pipeline initialization within compositor
- Moved Multisink support to the controller, instead of the Sink. This was necessary for the upcoming streaming capability.

### Removed

- Removed SpeakerSwitchMode to make code more readable
- Removed having no max visibles and use 100 as default in tests

### Fixed

- Fix video feed is not disappearing if the latest person is sharing their screen ([#75](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/75))
- Fix recording when the user is already sharing their screen ([#77](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/77))
- Fix audio is only going to be captured after first person is starting their camera feed ([#78](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/78))
- Fix the functions `set_stream_title`, `show_clock` and `show_title`, which would cause a panic in the `compositor` ([#90](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/90))
