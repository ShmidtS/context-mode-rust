use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--help" | "-h" => {
                println!("Context Mode MCP Server");
                println!("Usage: context-mode-server [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -h, --help      Print help");
                println!("  -V, --version   Print version");
                println!();
                println!("Reads JSON-RPC requests from stdin and writes responses to stdout.");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }
    context_mode_server::server::run_server().await
}
