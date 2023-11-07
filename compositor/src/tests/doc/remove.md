<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Mixer Remove

Periodically adds some streams to a `Talk` and removes them bunch by bunch.

`test_remove()` can be found in `/src/tests/mixer.rs`.

## Test Steps

- create a `Talk` which uses a `TestSink`
- repeat many times (ID0-ID7 will count like: `0`-`7`, `8`-`15`, ...):
    - add eight streams (ID0-ID7)
    - set a speaker and layout
    - set talk title to `remove` ID0 `(left` ID1 `-` ID7 `)`
    - remove first stream (ID0)
    - wait
    - set talk title to `remove` ID1 `-` ID2 `(left` ID3 `-` ID7 `)`
    - remove streams ID1 and ID2
    - wait
    - set talk title to `remove` ID3 `-` ID6 `(left` ID7 `)`
    - remove streams ID3 - ID6
    - wait
    - set talk title to `remove` ID7 `(none left)`
    - remove streams ID7
    - wait

## Automatic Test

- usage of method `remove_stream()` in `Talk`

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_remove
```

Then visually check results:

1. eight streams are visible in a grid and one by one is disappearing.
2. repeat several times
