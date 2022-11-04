#!/bin/sh
ffmpeg -v warning					`# Set loglevel. ` \
	-y								`# Overwrite output files without asking` \
	-nostdin						`# Disable interaction on standard input` \
	-i tcp://localhost:9000			`# Input file url`  \
	-f mpegts output.ts				`# Force input or output file format. `\
	