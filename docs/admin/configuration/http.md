# HTTP Server

The OpenTalk Recorder provides its functionality to clients through a build-in HTTP server.

## Configuration

The section in the [configuration file](README.md) is called `http`.

| Field     | Type     | Required | Default value | Description                           |
| --------- | -------- | -------- | ------------- | ------------------------------------- |
| `port`    | `int`    | no       | 11511         | The port for the http server.         |
| `addr`    | `string` | no       | 0.0.0.0       | The address used for the http server. |

### Example

```toml
[monitoring]
port = 8080
addr = "0.0.0.0"
```
