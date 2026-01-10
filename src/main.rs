#[tokio::main]
async fn main() {
    if let Err(err) = gg::app::run().await {
        eprintln!("gg: {err:#}");
        std::process::exit(2);
    }
}
