mod doctor;
mod hook;
mod setup;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use context_mode_core::db_schema;
use context_mode_core::local_indexer;
use context_mode_core::search;
use rusqlite::Connection;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "context-mode")]
#[command(about = "Context-mode CLI")]
struct Args {
    #[arg(short, long, default_value = "context-mode.db")]
    db: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize the context-mode environment
    Setup,
    /// Check the context-mode installation
    Doctor,
    /// Start the MCP stdio server
    Serve,
    /// Search the knowledge base
    Search {
        query: String,
        #[arg(short, long)]
        repo: Option<String>,
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Index a directory
    Index {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long, default_value = "default")]
        repo: String,
    },
    /// Run a platform hook (reads event JSON from stdin)
    Hook {
        /// Target platform (e.g. claude-code)
        platform: String,
        /// Hook type (e.g. posttooluse, pretooluse, precompact, sessionstart, userpromptsubmit)
        hook_type: String,
    },
}

fn open_db(path: &PathBuf) -> anyhow::Result<Connection> {
    let conn = Connection::open(path)?;
    db_schema::init_local_schema(&conn)?;
    Ok(conn)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();

    match args.command {
        Some(Commands::Setup) => {
            setup::run()?;
        }
        Some(Commands::Doctor) => {
            doctor::run().await?;
        }
        Some(Commands::Serve) => {
            info!("Starting Context Mode MCP stdio server");
            context_mode_server::server::run_server().await?;
        }
        Some(Commands::Search { query, repo, limit }) => {
            let conn = open_db(&args.db)?;
            let results = match repo {
                Some(r) => search::search_repo(&conn, &r, &query, limit)?,
                None => search::search(&conn, &query, limit)?,
            };
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        }
        Some(Commands::Index { path, repo }) => {
            let mut conn = open_db(&args.db)?;
            let provider = context_mode_core::embedding::OllamaEmbeddingProvider::new();
            let results = local_indexer::index_repository(&mut conn, &repo, &path, &provider)?;
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        }
        Some(Commands::Hook {
            platform,
            hook_type,
        }) => {
            hook::run(&platform, &hook_type).await;
        }
        None => {
            error!("No command provided. Use --help for usage.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_serve() {
        let args = Args::parse_from(["context-mode", "--db", "test.db", "serve"]);
        assert!(matches!(args.command, Some(Commands::Serve)));
    }

    #[test]
    fn test_cli_parse_search() {
        let args = Args::parse_from([
            "context-mode",
            "search",
            "hello",
            "--repo",
            "myrepo",
            "--limit",
            "5",
        ]);
        assert!(
            matches!(args.command, Some(Commands::Search { query, repo, limit }) if query == "hello" && repo == Some("myrepo".into()) && limit == 5)
        );
    }

    #[test]
    fn test_cli_parse_index() {
        let args = Args::parse_from([
            "context-mode",
            "index",
            "--path",
            "/tmp/src",
            "--repo",
            "r1",
        ]);
        assert!(
            matches!(args.command, Some(Commands::Index { path, repo }) if path == PathBuf::from("/tmp/src") && repo == "r1")
        );
    }

    #[test]
    fn test_cli_parse_setup() {
        let args = Args::parse_from(["context-mode", "setup"]);
        assert!(matches!(args.command, Some(Commands::Setup)));
    }

    #[test]
    fn test_cli_parse_doctor() {
        let args = Args::parse_from(["context-mode", "doctor"]);
        assert!(matches!(args.command, Some(Commands::Doctor)));
    }

    #[test]
    fn test_cli_parse_hook() {
        let args = Args::parse_from(["context-mode", "hook", "claude-code", "posttooluse"]);
        assert!(
            matches!(args.command, Some(Commands::Hook { platform, hook_type }) if platform == "claude-code" && hook_type == "posttooluse")
        );
    }
}
