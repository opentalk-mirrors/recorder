# SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
#
# SPDX-License-Identifier: EUPL-1.2

# Prepare a release
prepare-release VERSION:
    # Set the version number for all packages in the workspace
    cargo set-version --workspace {{ VERSION }}
    # Regenerate the lockfile
    cargo check
