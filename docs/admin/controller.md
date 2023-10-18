<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
SPDX-License-Identifier: EUPL-1.2
-->

# RabbitMQ

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
