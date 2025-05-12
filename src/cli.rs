// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use clap::{ArgAction, Parser};

#[derive(Parser, Debug, Clone)]
pub struct Args {
    #[clap(short('V'), long, action=ArgAction::SetTrue, help = "Print version information")]
    version: bool,

    /// Path of the configuration file.
    ///
    /// If present, exactly this config file will be used.
    ///
    /// If absent, `recorder` looks for a config file in these locations and uses the first one that is found:
    ///
    /// - `config.toml` in the current directory (deprecated, for backwards compatibility only)
    /// - `recorder.toml` in the current directory
    /// - `<XDG_CONFIG_HOME>/opentalk/recorder.toml` (where `XDG_CONFIG_HOME` is usually `~/.config`)
    /// - `/etc/opentalk/recorder.toml`
    #[clap(short, long, verbatim_doc_comment)]
    pub config: Option<String>,
}

impl Args {
    /// Returns true if we want to startup the controller after we finished the cli part
    pub fn should_start(&self) -> bool {
        !self.version
    }
}

/// Parses the CLI-Arguments into [`Args`]
///
/// Also runs (optional) cli commands if necessary
pub fn parse_args() -> Args {
    let args = Args::parse();
    if args.version {
        print_version();
    }
    args
}

opentalk_version::build_info!();

fn print_version() {
    println!("{}", build_info::BuildInfo::new());
}
