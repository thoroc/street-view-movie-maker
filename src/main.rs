mod cli;
mod compositing;
mod directions;
mod geo;
mod itinerary;
mod lineup;
mod maps;
mod net;
mod pipeline;
mod pricing;
mod prompt;
mod streetview;
mod video;

#[tokio::main]
async fn main() {
    if let Err(err) = pipeline::run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
