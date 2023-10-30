<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# MP4 Test

Tests `MP4Sink` which provides MP4 file output.

`test_mp4()` can be found in `/src/tests/mp4.rs`.

## Test Steps

- create a `Talk` which uses a `Mp4Sink` to  write into output directory
- add some streams
- set a speaker and layout
- wait 5 seconds

## Automatic Test

- usage of the `Mp4Sink` and `Mp4Parameters`
- `Mp4Sink`'s ability to run

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_mp4
```

Then visually check results:

1. A file called `/test_output/mp4sink.mp4`
2. File must be playable with VLC player
