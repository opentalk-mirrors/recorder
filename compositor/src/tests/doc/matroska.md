<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Matroska Test

Tests `MatroskaSink` which provides Matroska output at 127.0.0.1:0.

`test_matroska()` can be found in `/src/tests/matroska.rs`.

## Test Steps

- create a `Talk` which uses a `MatroskaSink` to  write into output directory
- add some streams
- set a speaker and layout
- wait `3` seconds

## Automatic Test

- usage of the `MatroskaSink` and `MatroskaParameters`
- `MatroskaSink`'s ability to run

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_matroska
```

Then visually check results:

1. Use VLC (for example) to open local stream (mkv://127.0.0.1) and see the output
