use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(author, about)]
pub struct CliOpts {
    /// Sets a custom config file
    #[structopt(short, long, name = "FILE")]
    pub config: PathBuf,
}

impl CliOpts {
    pub fn init() -> Self {
        CliOpts::from_args()
    }
}
