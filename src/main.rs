use clap::Parser;

use crate::server::Server;

mod http;
mod server;

#[derive(Parser)]
#[command(version , about , long_about = None)]
struct Args {
    #[arg(short , long , default_value_t = String::from("./"))]
    directory: String,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = Args::parse();

    let server = Server::new(args.directory);
    server.run().await
}
