# Configuration

When the recorder gets started, it loads the configuration from the
environment. It reads the settings in this order:

- Read environment variables which have a specific name, see section
  [Environment variables](#environment-variables).
- Load from a configuration file which defaults to `config.toml` in the current
  working directory.

## Sections in the configuration file

Functionality that can be configured through the configuration file:

- [Auth](auth.md)
- [Controller](controller.md)
- [HTTP](http.md)
- [Recorder](recorder.md)

## Environment variables

Settings in the configuration file can be overwritten by environment variables,
nested fields are separated by two underscores `__`. The pattern looks like
this:

```sh
OPENTALK_REC_<field>__<nested-field>…
```

### Limitations

Some settings can not be overwritten by environment variables. This is for
example the case for entries in lists, because there is no environment variable
naming pattern that could identify the index of the entry inside the list.

### Examples

In order to set the `auth.client_id` field, this environment variable could be used:

```sh
OPENTALK_REC_AUTH__CLIENT_ID=Recorder
```

## Example configuration file

This file can be found in the source code distribution under `extra/example.toml`

<!-- begin:fromfile:toml:config/example.toml -->

```toml
# SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
#
# SPDX-License-Identifier: EUPL-1.2

[auth]
# Note:
# The Recorder client must be a service account
# with the _service account role_ "opentalk-recorder"

issuer = "http://localhost:8080/auth/realms/MyRealm"
client_id = "Recorder"
client_secret = "INSERT_KEY"


[controller]
domain = "localhost:11311"
insecure = true


[http]
# The host to bind the HTTP Server to (defaults to 0.0.0.0).
host = "0.0.0.0"
# The port to bind the HTTP Server to (defaults to 11311).
port = "5555"

# Development:
# Always to stream to an extra display sink for monitoring
#[recorder]
#[[recorder.sinks]]
#type = "display"

# Static RTMP streaming, for testing purpose
#[[recorder.sinks]]
#type = "rtmp"
#location = "rtmp://localhost:1935/live/$room live=1"
# optional RTMP fields
#audio_bitrate = 96000
#audio_rate = 48000
#video_bitrate = 6000
#video_speed_preset = fast
```

<!-- end:fromfile:toml:config/example.toml -->
