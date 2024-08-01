---
title: Recorder
---

# Administration guide for the OpenTalk Recorder

## General information about the service

- [Configuration](./configuration/README.md)

### Environment

- `TZ: Europe/Berlin` to specify the timezone, see Alpine Linux documentation.

#### Debugging and Development

- `RUST_BACKTRACE: full` to get detailed stack traces
- `RUST_LOG: info,opentalk_recorder=debug,rustls=info,opentalk_recorder::signaling=debug` for more verbose logging
- `GST_DEBUG: 2` to increase GStreamer logging maximum is 6 but 3 is already very noisy
- `GST_DEBUG_FILE: /var/recording_logs/recorder_debug.log` dedicated log file. Necessary for `GST_DEBUG > 3` in combination with a dedicated docker volume
- `GST_DEBUG_DUMP_DOT_DIR: /var/recording_pipelines` dedicated directory to store pipeline snapshots.  Active when `GST_DEBUG > 2` -- we recommend to a dedicated docker volume.

## Interaction between OpenTalk Recorder and other services

### Services required by OpenTalk Recorder

- [Auth](./configuration/auth.md)
- [Controller](./configuration/controller.md)
- [RabbitMQ](./configuration/rabbitmq.md)
