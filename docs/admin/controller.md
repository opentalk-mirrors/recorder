# Controller

## Configuration

The section in the [configuration file](configuration.md) is called `controller`.

| Field                  | Type     | Required | Default value | Description                        |
| ---------------------- | -------- | -------- | ------------- | ---------------------------------- |
| `domain`               | `string` | yes      | -             | The RabbitMQ broker URL connection |
| `insecure`             | `bool`   | no       | false         | The RabbitMQ broker URL connection |

### Example

```toml
[controller]
url = "localhost:11311"
insecure = true
```
