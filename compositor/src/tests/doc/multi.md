<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Multi Output Test

Test splitting output into multiple outputs with `MultiSink`.

`test_multi()` can be found in `/src/tests/multi.rs`.

## Test Steps

- create a `Talk` which uses a `MultiSink` to  write into output directory
- add some streams
- set a speaker and layout
- wait `4` seconds

## Automatic Test

- usage of the `MultiSink` and `MultiParameters`
- `MultiSink`'s ability to run and supply two different output sinks

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_multi
```

Then visually check results:

1. Two windows must open showing the same output)
