<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# WebM Test

Tests `WebMSink` which provides WebM output at 127.0.0.1:0.

`test_webm()` can be found in `/src/tests/webm.rs`.

## Test Steps

- create a `Talk` which uses a `WebMSink` to  write into output directory
- add some streams
- set a speaker and layout
- wait `3` seconds

## Automatic Test

- usage of the `WebMSink` and `WebMParameters`
- `WebMSink`'s ability to run

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_webm
```

Then visually check results:

1. Use VLC (for example) to open local stream (webm://127.0.0.1) and see the output
