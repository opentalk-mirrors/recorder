# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.0] - 2024-12-12

[0.13.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.12.0...v0.13.0

### 📦 Dependencies

- (deps) Update git.opentalk.dev:5050/opentalk/backend/containers/rust docker tag to v1.83.0 ([!343](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/343))
- (deps) Update rust crate thiserror to v2.0.4 ([!345](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/345))
- (deps) Update rust crate tokio to v1.42.0 ([!344](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/344))
- (deps) Update rust crate anyhow to v1.0.94 ([!346](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/346))
- (deps) Update rust crate sysinfo to 0.33 ([!347](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/347))
- (deps) Update alpine docker tag to v3.21 ([!348](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/348))
- (deps) Lock file maintenance ([!350](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/350))
- (deps) Update rust crate serde to v1.0.216 ([!351](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/351))
- (deps) Update rust crate service-probe to 0.2 ([!352](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/352))
- (deps) Update controller crates to 0.28.0 ([!354](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/354))

## [0.12.0] - 2024-12-02

[0.12.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.11.0...v0.12.0

### 🐛 Bug fixes

- Chunk size should be at least 5 MiB and not 5 MB ([!339](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/339), [#187](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/187))

### 📚 Documentation

- (admin) Missing kernel settings entry in documentation ([!329](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/329), [#183](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/183))
- (monitoring) Explain optional behaviour ([!332](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/332))

### 🔨 Refactor

- (monitoring) Use service_probe crate ([!330](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/330))

### 📦 Dependencies

- (deps) Lock file maintenance ([!340](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/340), [!333](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/333))
- (deps) Update rust crate gst to v0.23.3 ([!303](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/303))
- (deps) Update rust crate hyper to v1.5.1 ([!324](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/324))
- (deps) Update rust crate serde_json to v1.0.133 ([!317](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/317))
- (deps) Update rust crate service-probe to v0.1.2 ([!336](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/336))

## [0.11.0] - 2024-11-21

[0.11.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.10.0...v0.11.0

### 🚀 New features

- Add liveness probe ([!291](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/291), [#153](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/153))
- Add configurable chunk size ([!314](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/314))

### 🐛 Bug fixes

- Close the livekit session after recording has finished ([!307](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/307))
- Remove obsolete usage of `media` signaling module ([!316](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/316))

### 📦 Dependencies

- (deps) Update rust crate thiserror to v2 ([!310](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/310))
- (deps) Update rust crate tokio to v1.41.1 ([!311](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/311))
- (deps) Update rust crate hyper-util to v0.1.10 ([!312](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/312))
- (deps) Update rust crate anyhow to v1.0.93 ([!302](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/302))
- (deps) Update rust crate serde to v1.0.215 ([!315](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/315))
- (deps) Update rust crate thiserror to v2.0.3 ([!313](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/313))
- (deps) Update opentalk-controller to 0.27 ([!326](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/326))

### Ci

- (renovate) Group opentalk-types crates ([!325](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/325))

## [0.10.1]

[0.10.1]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.10.0...v0.10.1

### 🐛 Bug fixes

- Close the livekit session after recording has finished ([!307](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/307), [#179](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/179))

## 0.10.0

[0.10.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.7.0...v0.10.0

### 🚀 New features

- Use new compositor ([!246](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/246), [#139](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/139), [#103](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/103), [#146](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/146), [#152](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/152), [#132](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/132))
- Receive livekit configuration from controller ([!297](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/297))

### 🐛 Bug fixes

- Handle None websockets as EOS ([!264](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/264))
- Disable container build for release branches ([!261](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/261), [#163](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/163))
- Update base image to add back missing RTMP GStreamer elements, which are required for streaming ([!265](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/265))
- Upload can fail if the chunks are too small ([!281](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/281), [#169](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/169))
- Fix wrong and obsolte documentation (sinks and display) ([!279](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/279), [#168](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/168))
- Break out of infinite loop ([!293](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/293))
- Remove old livekit section from docs ([!305](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/305))

### 📦 Dependencies

- (deps) Lock file maintenance ([!260](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/260), [!272](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/272), [!290](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/290))
- (deps) Update git.opentalk.dev:5050/opentalk/backend/containers/rust docker tag to v1.82.0 ([!268](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/268))
- (deps) Update rust crate anyhow to v1.0.91 ([!283](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/283))
- (deps) Update rust crate bytes to v1.8.0 ([!276](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/276))
- (deps) Update rust crate config to v0.14.1 ([!286](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/286))
- (deps) Update rust crate pin-project-lite to v0.2.15 ([!287](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/287))
- (deps) Update rust crate serde to v1.0.214 ([!277](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/277), [!282](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/282), [!292](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/292))
- (deps) Update rust crate serde_json to v1.0.129 ([!266](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/266))
- (deps) Update rust crate thiserror to v1.0.65 ([!284](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/284))
- (deps) Update rust crate tokio to v1.41.0 ([!278](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/278))

### ⚙ Miscellaneous

- Add documentation for hardware acceleration ([!263](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/263))
- (update) Compositor to v0.9.0 ([!289](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/289), [#155](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/155), [#164](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/164))

### Ci

- Introduce changelog bot ([!275](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/275))
- (fix) Don't lint commits on main ([!285](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/285))

## [0.7.2]

[0.7.2]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.7.1...v0.7.2

### 🐛 Bug fixes

- Upload can fail under some circumstances ([#169](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/169))

## [0.7.1]

[0.7.1]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.7.0...v0.7.1

### 🐛 Bug fixes

- Update base image ([!265](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/265))
- handle `None` websockets as `EOS` ([!273](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/273))

## [0.7.0]

[0.7.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.6.0...v0.7.0

### 🚀 New features

- Publish the compositor on crates.io ([#114](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/114), [!239](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/239))

### 🐛 Bug fixes

- (release) Exclude images directory when publishing to crates.io ([!241](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/241))
- Add configurations for descriptor ([#113](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/113), [!190](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/190))

### 🔨 Refactor

- All the events in the main recorder ([#113](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/113), [!190](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/190))
- Signaling connection receive into separate func ([#113](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/113), [!190](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/190))
- Move participants list from signaling to recording session ([#113](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/113), [!190](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/190))
- Use self-built ubuntu based gstreamer image ([#129](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/129), [#159](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/159), [!255](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/255))

### 📦 Dependencies

- (deps) Lock file maintenance ([!238](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/238))
- (deps) Lock file maintenance ([!245](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/245))
- (deps) Update rust crate types to 0.20.0 ([!236](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/236))
- (deps) Lock file maintenance ([!250](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/250))
- (deps) Update rust crate sysinfo to 0.32 ([!249](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/249))
- (deps) Update opentalk-types crate to 0.21 ([!256](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/256))

### ⚙ Miscellaneous

- (release) Add a `justfile` with a `create-release` target for release automation ([!251](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/251))

## [0.6.1]

[0.6.1]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.6.0...v0.6.1

### 🚀 New features

- feat: publish the compositor on crates.io ([#114](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/114), [!239](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/239))

## [0.6.0]

[0.6.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.5.0...v0.6.0

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

## [0.5.1]

[0.5.1]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.5.0...v0.5.1

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

## [0.5.0]

[0.5.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.4.0...v0.5.0

### 🚀 New features

- Auto Configure Quality of Subscribed Video Streams ([#119](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/119))
- Watch GStreamer bus for better error handling ([#117](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/117))

### 🐛 Bug fixes

- Check for gstreamer srtp elements on start ([!198](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/198))
- Update TLS dependencies to support self signed certs ([!192](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/192))
- Update docs ([#147](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/147))
- Add timezone package to container ([#147](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/147))
- End all streams when recorder is done ([#140](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/140))

### ⚙ Miscellaneous

- Build executable with `cargo auditable`

### 📚 Documentation

- Add clock pattern to example config and ENV ([#147](https://git.opentalk.dev/opentalk/backend/services/controller/-/issues/147))

### 📦 Dependencies

- Update gstreamer-rs
- Update rust crate bytes to v1.6.1
- Update rust crate env_logger to v0.11.5
- Update rust crate lapin to v2.5.0
- Update rust crate serde_json to v1.0.121
- Update rust crate thiserror to v1.0.63
- Update rust crate tokio to v1.39.2

## [0.4.0]

[0.4.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.3.0...v0.4.0

### 🚀 New features

- Implement simple load balancing logic [#125](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/125))
- configure recording to use vp8 encoder in realtime mode and webm container format [#136](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/136))

### 🐛 Bug fixes

- Remove hysteresis and use absolute value instead of avg ([!132](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/132))
- End all streams when recorder is done [#140](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/140))
- *(deps)* Update rust crate bytes to v1.6.1

### ⚙ Miscellaneous

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

## [0.3.0]

[0.3.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.2.0...v0.3.0

### Added

- Add the capability to set the format of the clock in the `config.toml` ([#108](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/108))

## [0.2.0]

[0.2.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/v0.1.0...v0.2.0

### Added

- Add dynamic RTMP streaming configuration ([#100](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/100))

### Fixed

- Clean shutdown of Matroska and MP4 Sink([#106](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/106))
- Recording should start from 0 and not from system time ([#115](https://git.opentalk.dev/opentalk/backend/services/recorder/-/issues/115))
- Mixer::set_stream_to_position invalid check and possible panic ([!125](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/125))
- Disable participants swap for two participants ([!128](https://git.opentalk.dev/opentalk/backend/services/recorder/-/merge_requests/128))

## [0.1.0]

[0.1.0]: https://git.opentalk.dev/opentalk/backend/services/recorder/-/compare/7018ca4d...v0.1.0

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
