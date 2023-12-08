# Recorder

The OpenTalk Recorder is capable of streaming into different sinks. A sink can
be a `MP4 file`, `Display`, or `RTMP stream`.

## Configuration

The section in the [configuration file](README.md) is called `recorder`.

| Field                | Type     | Required | Default value | Description                                  |
| -------------------- | -------- | -------- | ------------- | -------------------------------------------- |
| `sink`               | `string` | no       | "mp4"         | The sink where the recorder should stream to |
| `rtmp_uri`           | `int`    | yes*     | -             | The location for the rtmp sink               |
| `rtmp_audio_bitrate` | `int`    | no       | 96000         | The audio bitrate for the rtmp sink          |
| `rtmp_audio_rate`    | `int`    | no       | 48000         | The audio rate for the rtmp sink             |
| `rtmp_video_bitrate` | `int`    | no       | 6000          | The video bitrate for the rtmp sink          |
| `rtmp_speed_preset`  | `string` | no       | "fast"        | The video speed preset for the rtmp sink     |

*`rtmp_uri` is only required when the sink `rtmp` is in use.

### Examples

#### Example with mp4 sink (default behaviour)

The Display sink can be used to stream from the recorder to a mp4 file.

```toml
[recorder]
sink = "mp4"
```

#### Example with display sink

The Display sink can be used to stream from the recorder to a display.

```toml
[recorder]
sink = "display"
```

#### Example with rtmp sink

The RTMP sink can be used to stream from the recorder to an external rtmp
server. `rtmp_uri` is optionally replacing the `$room` variable with the current room id.

```toml
[recorder]
sink = "rtmp"
rtmp_uri = "rtmp://localhost:1935/live/$room live=1"
# optional for the rtmp sink:
#rtmp_audio_bitrate = 96000
#rtmp_audio_rate = 48000
#rtmp_video_bitrate = 6000
#rtmp_video_speed_preset = fast
```
