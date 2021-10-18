use clap::Parser;

#[derive(Debug, Parser)]
#[clap(version = "1.0")]
pub struct Config {
    // #[clap(short, long, default_value = "http")]
    // protocol: String,
    #[clap(short, long, default_value = "127.0.0.1")]
    pub host: String,

    #[clap(short, long, default_value = "9943")]
    pub port: u32,

    /// how many concurrent tasks to spawn. Requests are spread over these connections    
    #[clap(short, long, default_value = "2")]
    pub concurrency: usize,

    /// how many transactions / s to send
    #[clap(short, long, default_value = "1000")]
    pub throughput: u64,

    /// how long to run the benchmark for (in seconds)
    #[clap(short, long, default_value = "10")]
    pub duration: u64,
}
