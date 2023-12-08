# RabbitMQ

## Configuration

The section in the [configuration file](README.md) is called `rabbitmq`.

| Field   | Type     | Required | Default value | Description                                     |
| ------- | -------- | -------- | ------------- | ----------------------------------------------- |
| `uri`   | `string` | yes      | -             | The RabbitMQ broker URL                         |
| `queue` | `string` | yes      | -             | The name of the RabbitMQ queue for the recorder |

### Example

```toml
[rabbitmq]
uri = "amqp://username:password@localhost/%2F"
recording_task_queue = "opentalk_recorder"
```
