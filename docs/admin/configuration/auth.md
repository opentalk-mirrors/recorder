# Auth

The OpenTalk Controller uses [keycloak](https://www.keycloak.org/), an OpenID Connect compatible
identity and access management software for single sign-on.

Note: The controller expects the `Recorder` client to be setup as service account with the _service account role_ `opentalk-recorder`.

## Configuration

The section in the [configuration file](README.md) is called `auth`.

| Field           | Type     | Required | Default value | Description                                         |
| --------------- | -------- | -------- | ------------- | --------------------------------------------------- |
| `issuer`        | `string` | yes      | -             | The issuer url from keycloak                        |
| `client_id`     | `string` | yes      | -             | The unique identifier for the OpenTalk client       |
| `client_secret` | `string` | yes      | -             | The secret corresponding to the specified client ID |

### Example

```toml
[auth]
issuer = "http://localhost:8080/auth/realms/MyRealm"
client_id = "Recorder"
client_secret = "INSERT_KEY"
```
