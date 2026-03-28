use std::env;

use rmcp::service::{RoleServer, RunningService};
use rmcp::ServiceExt;

mod models;
mod myfund;
mod server;

use myfund::MyfundClient;
use server::MyfundServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing MUST go to stderr — stdout is the MCP stdio channel.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_env("RUST_LOG")
                .add_directive("myfund_mcp=info".parse()?),
        )
        .init();

    let api_key = match env::var("MYFUND_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!(
                "Error: MYFUND_API_KEY environment variable is not set.\n\
                 Get your API key from myfund.pl: menu → Account → Account Settings → API Key"
            );
            return Err(anyhow::anyhow!("MYFUND_API_KEY environment variable is not set"));
        }
    };

    let portfolios: Vec<String> = env::var("MYFUND_PORTFOLIOS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let client = MyfundClient::new(api_key)?;
    let server = MyfundServer::new(client, portfolios);

    tracing::info!(
        "myfund-mcp server starting v{} (stdio transport)",
        env!("CARGO_PKG_VERSION")
    );

    let transport = rmcp::transport::io::stdio();
    let running: RunningService<RoleServer, _> = server.serve(transport).await?;
    running.waiting().await?;

    Ok(())
}
