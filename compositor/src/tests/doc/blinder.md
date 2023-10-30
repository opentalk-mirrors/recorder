<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Blinder Test

Tests the functionality of `TestBlinder` and it's trait `Blinder`.

This test can be found in `/src/tests/blinder.rs`.

`TestBlinder` is a very simple blinder which just outputs a black picture if blinded.

## Test Steps

- create a `TestBlinder`
- create a `Talk` using that sink
- add some streams
- set a speaker and layout
- blind and un-blind the stream several times
- signal blind status in text overlay

## Automatic Test

- usage of the trait `Blinder`
- `TestBlinder`'s ability to run

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_blinder
```

Then visually check results:

1. While displaying "not blinded" output must be visible in both output windows
2. While displaying "blinded" the output must be blinded (to black) in one of the output windows only
