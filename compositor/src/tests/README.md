<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# OpenTalk Compositor Unit Tests

## Generate Example Pipeline

Shall generate the file `example_pipeline.png` which shows the current pipeline architecture.

This picture is used within documentation.

## Unit Tests

The following tests are testing several units of the compositor library:

- Stream blinding: [blinder](doc/blinder.md)
- DASH output: [dash](doc/dash.md)
- Matroska output: [matroska](doc/matroska.md)
- Mixer tests
    - Several layouts: [layout](doc/layout.md)
    - Removing streams: [remove](doc/remove.md)
- MP4 output: [mp4](doc/mp4.md)
- Multi sink output:  [multi](doc/multi.md)
- Source and talk overlays: [overlays](doc/overlays.md)
- Speaker update mode: [speaker_mode](doc/speaker_mode.md)
- Stream status updates: [stream_status](doc/stream_status.md)

## Test options

### Testing sequentially

These unit test are not made to run in parallel.
The unit tests are stressing themselves and do not need to stress each other to get better logging to debug.

So using the following option of cargo test will run tests one after the other only:

```txt
cargo test -- --test-threads=1
```

### TestSink options

TestSink selects between FakeSink and DisplaySink

All tests using `TestSink` for output can be influenced in where they are putting there output. Prefix your test call with one or more of:

- `USER_TEST=1` to slow down test speed
- `USE_DISPLAY=1` to show output in window(s)
- `USE_VIDEO=1` to use video in pipeline

### Dump Pipeline Graphs

Use `GST_DEBUG_DUMP_DOT_DIR` to activate the generation of pipeline graphs on several positions in code:

```txt
GST_DEBUG_DUMP_DOT_DIR=pipelines  cargo test
```

This will write several DOT files numerated by occurrence into the directory `pipelines`.

Hint: Some DOT will only be generated if log level is `debug` or `trace` (see `debug::dot()` vs. `debug::debug_dot()`).
