#!/bin/sh

# Usage:
# 
#   record-ffmpeg.sh URL MPD BITRATE
#
# URL = Source URL (e.g. `tcp://localhost:9000`)
# PD = Dash MPD file path (e.g. `./output/dash.mpd`)
# BITRATE = Aimed bitrate (e.g. '1M', '192k' - see FFmpeg option)
#
# All a/v files will be placed beside the MPD file.

URL=$1
MPD_FILENAME=$2
BITRATE=$3

echo Connecting to $ADDRESS

ffmpeg -v warning										`# Set loglevel. ` \
	-y													`# Overwrite output files without asking` \
	-nostdin											`# Disable interaction on standard input` \
	-i "$URL"											`# Input file url`  \
    -map 0												`# Map incoming file` \
	-b:0 $BITRATE										`# Set bitrate` \
	-use_timeline 1										`# dsf` \
	-use_template 1										`# ` \
	-window_size 5										`# ` \
	-adaptation_sets "id=0,streams=v id=1,streams=a"	`# ` \
    -f dash "$MPD"										`# Write DASH files` \
			