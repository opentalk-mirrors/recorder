<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Mixer Layout Tests

Tests the available layouts.

`test_layout_speaker()` and `test_layout_grid()` can be found in `/src/tests/mixer.rs`.

## Test Steps

- create a talk with a `TestSink`
- add some streams
- show more an more streams and re-layout each time

## Automatic Test

Tests usage of:

- `Talk::layout()`
- `layout::Speaker`
- `layout::Grid`

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_layout_speaker
```

...or...

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_layout_grid
```

Then visually check results:

1. display window must show expected layout
2. starts with one stream
3. continues until all 5 streams are visible
