# RabbitMQ

## Configuration

The section in the [configuration file](configuration.md) is called `rabbit_mq`.

| Field   | Type     | Required | Default value | Description                                 |
| ------- | -------- | -------- | ------------- | ------------------------------------------- |
| `url`   | `string` | yes      | -             | The RabbitMQ broker URL connection          |
| `queue` | `string` | yes      | -             | Name of the RabbitMQ queue for the recorder |

### Example

```toml
[rabbit_mq]
url = "amqp://username:password@host/%2F"
recording_task_queue = "opentalk_recorder"
```
