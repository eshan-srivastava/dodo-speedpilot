#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dodo_be_test::app::run().await
}
