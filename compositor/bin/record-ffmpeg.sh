# SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
#
# SPDX-License-Identifier: EUPL-1.2

#!/bin/sh

# ATTENTION: This script is used by the compositor crate!

if [ $# -ne 5 ]; then
	echo Script that uses ffmpeg to receive audio/video input froma a source and
	echo generates a MPEG4 DASH instance consisting of a MPD and some media files.
	echo
	echo Usage: record-ffmpeg.sh URL MPD BITRATE SEG_DURATION SEG_TYPE
	echo
    echo Arguments:
	echo "  URL          Audio/video source URL \(e.g. tcp://localhost:9000\)"
	echo "  MPD          Dash MPD file path \(e.g. ./test_output/dash.mpd\)"
	echo "  BITRATE      Aimed bitrate \(e.g. 1M or 192k\)"
	echo "  SEG_DURATION Set the segment length in seconds \(fractional value can be set\)"
	echo "  SEG_TYPE     auto, mp4 or webm"
	echo
	echo All media files will be placed beside the MPD file.

    exit 1
fi

URL=$1
MPD=$2
BITRATE=$3
SEG_DURATION=$4
SEG_TYPE=$5

echo Connecting to $URL

# debug output
set -x

ffmpeg 												`# More help: https://ffmpeg.org/ffmpeg-formats.html#dash-2` \
	-v warning											`# Set loglevel. ` \
	-y													`# Overwrite output files without asking` \
	-nostdin											`# Disable interaction on standard input` \
	-i "$URL"											`# Input file url` \
    -map 0												`# Map incoming file` \
	-b:0 $BITRATE										`# Set bitrate` \
	-use_timeline 1										`# Enable (1) or disable (0) use of SegmentTimeline in SegmentTemplate.` \
	-use_template 1										`# Enable (1) or disable (0) use of SegmentTemplate instead of SegmentList.` \
	-seg_duration $SEG_DURATION							`# Set the segment length in seconds (fractional value can be set).` \
	-adaptation_sets "id=0,streams=v id=1,streams=a"	`# Assign streams to AdaptationSets.` \
	-dash_segment_type $SEG_TYPE						`# segment type (e.g. auto, mp4 or webm)` \
    -f dash 									        `# Write DASH files` \
    "$MPD"										        `# MPD file path` \
