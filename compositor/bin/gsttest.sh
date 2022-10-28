#!/bin/sh

cat $1 | xargs gst-launch-1.0 -v
