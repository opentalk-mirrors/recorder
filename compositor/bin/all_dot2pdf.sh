# SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
#
# SPDX-License-Identifier: EUPL-1.2

#!/bin/sh

for file in ./pipelines/*.dot; do
    echo "$file -> $(dirname "$file")/../images/$(basename "$file").png"
    dot -Tpng $file -o "$(dirname "$file")/../images/$(basename "$file").png"
done

