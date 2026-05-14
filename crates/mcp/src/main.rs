use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    context_mode_server::server::run_server().await
}
