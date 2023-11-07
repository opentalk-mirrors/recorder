# SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
#
# SPDX-License-Identifier: EUPL-1.2

#!/bin/sh

cat $1 | xargs gst-launch-1.0 -v
