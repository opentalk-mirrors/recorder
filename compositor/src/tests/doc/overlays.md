<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Overlays Test

Test talk overlay (which is placed on the output picture) and source overlays which are placed inside each source picture.

`test_overlay()` can be found in `/src/tests/overlays.rs`.

## Test Steps

- create a `Talk` which uses a `TestSink` to show output on screen or ignore it
- set a speaker
- set talk title to "test_overlay"
- re-layout talk
- wait a sec
- add three streams
- show all streams
- re-layout talk
- wait
- set all stream (source) overlays to "new text" one after the other and wait between

## Automatic Test

- usage of the built-in overlays of Talk
    - `TextOverlay`
    - `ClockOverlay`
- usage of methods in `Talk`
    - `set_title()`
    - `set_stream_title()`
- Handling of overlays within the pipeline

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_overlay
```

Then visually check results:

1. shows the talk title "test_overlay"
2. shows the current time
3. after some time all streams are displayed with initial titles 'Participant 'X (where `X` is a number from `0` to `2`)
4. then one after the other title will be replaced by "new text"
