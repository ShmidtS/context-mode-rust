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
    /// Start the HTTP server
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
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
}

fn open_db(path: &PathBuf) -> anyhow::Result<Connection> {
    let mut conn = Connection::open(path)?;
    db_schema::init_local_schema(&mut conn)?;
    Ok(conn)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    match args.command {
        Some(Commands::Serve { port }) => {
            info!("Starting server on port {}", port);
            info!("Run: context-mode-server --port {}", port);
        }
        Some(Commands::Search {
            query,
            repo,
            limit,
        }) => {
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
            let results = local_indexer::index_repository(&mut conn, &repo, &path)?;
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
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
        let args = Args::parse_from(["context-mode", "--db", "test.db", "serve", "--port", "8080"]);
        assert!(matches!(args.command, Some(Commands::Serve { port }) if port == 8080));
    }

    #[test]
    fn test_cli_parse_search() {
        let args = Args::parse_from(["context-mode", "search", "hello", "--repo", "myrepo", "--limit", "5"]);
        assert!(matches!(args.command, Some(Commands::Search { query, repo, limit }) if query == "hello" && repo == Some("myrepo".into()) && limit == 5));
    }

    #[test]
    fn test_cli_parse_index() {
        let args = Args::parse_from(["context-mode", "index", "--path", "/tmp/src", "--repo", "r1"]);
        assert!(matches!(args.command, Some(Commands::Index { path, repo }) if path == PathBuf::from("/tmp/src") && repo == "r1"));
    }
}
