use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
    /// The config path for elara-kv
    #[structopt(short, long)]
    pub config: String,
}
