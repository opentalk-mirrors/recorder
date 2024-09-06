# Recorder

The OpenTalk Recorder is capable of streaming into different sinks. A sink can
be a `WebM file`, `Display`, or `RTMP stream`.

## Configuration

The section in the [configuration file](README.md) is called `recorder`.

| Field                     | Type                   | Required | Default value | Description                                                      |
| ------------------------- | ---------------------- | -------- | ------------- | ---------------------------------------------------------------- |
| `clock_format`            | `string`               | no       | "%x %X %Z"    | The time format for the clock, see `man strftime` for details    |
| `hardware_acceleration`   | `HardwareAcceleration` | no       | `<empty>`     | Enabled Hardware Acceleration, which enables GPU Encoding        |
| `sinks`                   | `array<sink>`          | no       | `<empty>`     | The sink where the recorder should stream to                     |
| `sink.type`               | `string`               | yes      | -             | The sink type is one of `rtmp`, `webm` or `display`              |
| `sink.rtmp_uri`           | `int`                  | yes[^1]  | -             | The location for the rtmp sink                                   |
| `sink.rtmp_audio_bitrate` | `int`                  | no       | 96000         | The audio bitrate for the rtmp sink                              |
| `sink.rtmp_audio_rate`    | `int`                  | no       | 48000         | The audio rate for the rtmp sink                                 |
| `sink.rtmp_video_bitrate` | `int`                  | no       | 6000          | The video bitrate for the rtmp sink                              |
| `sink.rtmp_speed_preset`  | `string`               | no       | "fast"        | The video speed preset for the rtmp sink                         |
| `max_load`                | `int`                  | no       | 80            | The usage value per core (in %) until when new jobs are accepted |

[^1]: `rtmp_uri` is only required when the sink `rtmp` is in use.

### Hardware Acceleration Configuration

| Field          | Type           | Required | Default value | Description                                                    |
| -------------- | -------------- | -------- | ------------- | -------------------------------------------------------------- |
| `manufacturer` | `enum (intel)` | no       | -             | Enabled Hardware Acceleration, if Manufacturer is set          |
| `device`       | `string`       | no       | -             | The device where the metrics should be fetch (only intel GPUs) |

Right now only `Intel` is support for Hardware Acceleration, if `Intel` is not set, the CPU will be used for Video Encoding.

Note: Please keep in mind to passthrough the device and capabilities, if you're using docker.

```yaml
service:
  recorder:
    image: recorder
    # ...
    cap_add:
      - CAP_PERFMON
    devices:
      - /dev/dri
```

Of if you only want to passthrough one device (in case of load balancing):

```yaml
service:
  recorder:
    image: recorder
    # ...
    cap_add:
      - CAP_PERFMON
    devices:
      - /dev/dri/renderD129
```

Note for passing through Intel Cards: Please keep in mind, to set the `device`
if you have multiple GPUs. It doesn't matter if you only passthrough one GPU
to a container, if you have multiple GPUs intel will monitor the main device.
This means that if you have multiple graphics cards, it's mandatory to set the
`device` entry.

### Examples

#### Example with Hardware Acceleration

```toml
[recorder.hardware_acceleration]
manufacturer = "intel"
device = "/dev/dri/renderD129"
```

#### Example with clock_format

Set the time format for the clock in the recording.

```toml
[recorder]
clock_format = "%d.%m.%Y %H:%M:%S"
```

#### Example with webm sink (default behaviour)

The Display sink can be used to stream from the recorder to a webm file.

```toml
[recorder]

[[recorder.sinks]]
type = "webm"
```

#### Example with display sink

The Display sink can be used to stream from the recorder to a display.

```toml
[recorder]

[[recorder.sinks]]
type = "display"
```

#### Example with rtmp sink

The RTMP sink can be used to stream from the recorder to an external rtmp
server. `rtmp_uri` is optionally replacing the `$room` variable with the current room id.

```toml
[recorder]

[[recorder.sinks]]
type = "rtmp"
rtmp_uri = "rtmp://localhost:1935/live/$room live=1"
# optional for the rtmp sink:
#rtmp_audio_bitrate = 96000
#rtmp_audio_rate = 48000
#rtmp_video_bitrate = 6000
#rtmp_video_speed_preset = fast
```
