<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Stream Status

Activates and deactivates video and audio of every stream within a `Talk`.

`test_stream_status()` can be found in `/src/tests/stream_status.rs`.

## Test Steps

- create a `Talk` which uses a `TestSink`
- add some streams
- repeat for every stream:
    - turn video on and audio off
    - wait
    - turn video off and audio on
    - wait
    - turn video and audio off
    - wait
    - turn video off and audio on
    - wait

## Automatic Test

- usage of method `Talk::set_status()` and structure `StreamStatus`.

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_stream_status
```

Then visually check results:

1. first "Participant 0" is visible but muted
2. then "Participant 0" is invisible but hearable
3. then "Participant 0" is invisible and muted
4. then "Participant 0" is visible and hearable
5. repeats with all other streams
