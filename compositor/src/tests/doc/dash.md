<!--
SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>

SPDX-License-Identifier: EUPL-1.2
-->

# Dash Test

Tests the functionality of `DashSink` and `DashParameters`.

`test_dash()` can be found in `/src/tests/dash.rs`.

## Test Steps

- create a `Talk` which uses a `DashSink` to  write into output directory
- add some streams
- set a speaker and layout
- wait `4` seconds

## Automatic Test

- usage of the `DashSink` and `DashParameters`
- `DashSink`'s ability to run

## Manual Test

Start Test with:

```sh
cargo test -p compositor test_dash
```

Then visually check results:

1. A file called `/test_output/dash.mpd` which shall look similar to this:

    ```mpd
    <?xml version="1.0" encoding="utf-8"?>
    <MPD xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
        xmlns="urn:mpeg:dash:schema:mpd:2011"
        xmlns:xlink="http://www.w3.org/1999/xlink"
        xsi:schemaLocation="urn:mpeg:DASH:schema:MPD:2011 http://standards.iso.org/ittf/PubliclyAvailableStandards/MPEG-DASH_schema_files/DASH-MPD.xsd"
        profiles="urn:mpeg:dash:profile:isoff-live:2011"
        type="static"
        mediaPresentationDuration="PT4.3S"
        maxSegmentDuration="PT1.0S"
        minBufferTime="PT8.7S">
        <ProgramInformation>
        </ProgramInformation>
        <ServiceDescription id="0">
        </ServiceDescription>
        <Period id="0" start="PT0.0S">
            <AdaptationSet id="0" contentType="video" startWithSAP="1" segmentAlignment="true" bitstreamSwitching="true" frameRate="25/1" maxWidth="1280" maxHeight="720" par="16:9" lang="eng">
                <Representation id="0" mimeType="video/mp4" codecs="avc1.64001f" bandwidth="1048000" width="1280" height="720" sar="1:1">
                    <SegmentTemplate timescale="12800" initialization="init-stream$RepresentationID$.m4s" media="chunk-stream$RepresentationID$-$Number%05d$.m4s" startNumber="1">
                        <SegmentTimeline>
                            <S t="0" d="55808" />
                        </SegmentTimeline>
                    </SegmentTemplate>
                </Representation>
            </AdaptationSet>
            <AdaptationSet id="1" contentType="audio" startWithSAP="1" segmentAlignment="true" bitstreamSwitching="true" lang="eng">
                <Representation id="1" mimeType="audio/mp4" codecs="mp4a.40.2" bandwidth="128000" audioSamplingRate="48000">
                    <AudioChannelConfiguration schemeIdUri="urn:mpeg:dash:23003:3:audio_channel_configuration:2011" value="2" />
                    <SegmentTemplate timescale="48000" initialization="init-stream$RepresentationID$.m4s" media="chunk-stream$RepresentationID$-$Number%05d$.m4s" startNumber="1">
                        <SegmentTimeline>
                            <S t="0" d="47104" />
                            <S d="48128" r="2" />
                            <S d="17792" />
                        </SegmentTimeline>
                    </SegmentTemplate>
                </Representation>
            </AdaptationSet>
        </Period>
    </MPD>
    ```

2. One file called `chunk-stream0-00001.m4s`
3. Several files called `chunk-stream1-0000x.m4s` (`x` = `1`...n)
4. `dash.mpd` must be playable with VLC player
