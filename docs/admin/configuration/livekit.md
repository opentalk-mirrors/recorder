# LiveKit

The OpenTalk Recorder is capable of using `LiveKit` for receiving the audio and video streams.

## Configuration

The section in the [configuration file](README.md) is called `livekit`.

| Field        | Type     | Required | Default value | Description                               |
| -------------| -------- | -------- | ------------- | ----------------------------------------- |
| `url`        | `string` | yes      | -             | The `url` of the livekit server.          |
| `api_key`    | `string` | yes      | -             | The `api key` from the livekit server.    |
| `api_secret` | `string` | yes      | -             | The `api secret` from the livekit server. |

### Examples

```toml
[livekit]
url = "localhost:7880"
api_key = "devkey"
api_secret = "secret"
```
