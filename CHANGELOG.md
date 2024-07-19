# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
