#[tokio::main]
async fn main() -> anyhow::Result<()> {
    jev_obscura_browser::demo::run_cli().await
}
