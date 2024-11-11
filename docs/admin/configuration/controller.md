# Controller

## Configuration

The section in the [configuration file](README.md) is called `controller`.

| Field               | Type     | Required | Default value | Description                                                              |
| ----------          | -------- | -------- | ------------- | ------------------------------------------------------------------------ |
| `domain`            | `string` | yes      | -             | The host and optional port of the controller in the format `host[:port]` |
| `insecure`          | `bool`   | no       | false         | true to disable transport security to the controller                     |
| `upload_chunk_size` | `int`    | no       | 5242880       | The size for each chunk on upload. Expected the value to lie inbetween 5 MiB and 5 GiB. The value is represented in Bytes. (Default: 5 MiB) |

### Example

```toml
[controller]
domain = "localhost:11311"
insecure = true
upload_chunk_size = 10485760 # 10 MiB
```
