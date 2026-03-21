use clap::Parser;
use xgent_gateway::config;

#[derive(Parser, Debug)]
#[command(name = "xgent-gateway", about = "Pull-model task gateway")]
struct Cli {
    /// Path to configuration TOML file
    #[arg(long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let _config = config::load_config(cli.config.as_deref())?;

    tracing::info!("xgent-gateway starting");

    Ok(())
}
