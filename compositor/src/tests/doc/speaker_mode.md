<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Speaker Mode

Uses all variants of 'SpeakerNode' to switch speaker within a `Talk`.

`test_speaker_mode()` can be found in `/src/tests/speaker_mode.rs`.

## Test Steps

- create a `Talk` which uses a `TestSink`
- sequentially make every stream a speaker in `FirstShift` mode`
- sequentially make every stream a speaker in `FirstSwap` mode`

## Automatic Test

- usage of parameter `SpeakerMode` in `Talk::layout()`

## Manual Test

Start Test with:

```sh
USER_TEST=1 USE_DISPLAY=1 cargo test -p compositor test_speaker_mode
```

Then visually check results:

1. first "Participant 0" is displayed as speaker without other streams to be visible
2. then "Participant 1" to "Participant 9" will get speaker within a speaker layout with growing number of visibles
3. number of maximum visibles is 5
4. the speaker is switching in the way described by the `SpeakerMode``
5. `SpeakerMode::FirstShift` and  `SpeakerMode::FirstSwap` are tested
