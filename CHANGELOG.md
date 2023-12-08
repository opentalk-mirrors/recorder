# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
