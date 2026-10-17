//! AGN CLI — command-line entry point with OOBE (out-of-box experience).

use agn_core::{AgnConfig, ServerSettings, ensure_data_dir};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "agn", version, about = "AGN Studios - A multi-platform AI agent tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Serve {
        #[arg(long, default_value = "false")]
        remote: Option<String>,
        #[arg(long)]
        no_webui: bool,
        #[arg(long, default_value = "11828")]
        port: u16,
        #[arg(long, default_value = "11829")]
        webui_port: u16,
    },
    /// Run OOBE setup wizard
    Setup,
    /// Show version info
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { remote, no_webui, port, webui_port }) => {
            let mut config = load_config();

            if let Some(remote_addr) = remote {
                config.server.start_local = false;
                config.server.remote_address = remote_addr;
            } else {
                config.server.start_local = true;
            }

            if no_webui {
                config.server.start_webui = false;
            }

            config.server.api_port = port;
            config.server.webui_port = webui_port;

            agn_server::run_server(config).await?;
        }
        Some(Commands::Setup) => {
            oobe().await?;
        }
        Some(Commands::Version) => {
            println!("AGN Studios v{}", env!("CARGO_PKG_VERSION"));
        }
        None => {
            // No command — run OOBE if first time, else serve
            let config_path = AgnConfig::config_path();
            if !config_path.exists() {
                oobe().await?;
            } else {
                let config = load_config();
                agn_server::run_server(config).await?;
            }
        }
    }

    Ok(())
}

/// Load config from disk (or create default).
fn load_config() -> AgnConfig {
    let path = AgnConfig::config_path();
    match AgnConfig::load(&path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Warning: Failed to load config: {}, using defaults", e);
            AgnConfig::default()
        }
    }
}

/// Out-of-box experience (OOBE) setup wizard.
async fn oobe() -> anyhow::Result<()> {
    use std::io::{self, Write, BufRead};

    println!();
    println!("╔══════════════════════════════════════╗");
    println!("║     Welcome to AGN Studios! 🚀       ║");
    println!("╚══════════════════════════════════════╝");
    println!();
    println!("This quick setup will get you started.");
    println!();

    // Step 1: Server mode
    println!("Step 1: Server Configuration");
    println!("  [1] Start local server (default)");
    println!("  [2] Connect to an existing server");
    print!("Choose (1/2) [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    let stdin = io::stdin();
    stdin.lock().read_line(&mut input)?;
    let server_choice = input.trim();
    let start_local = server_choice != "2";

    let mut remote_address = String::new();
    if !start_local {
        print!("Enter server address (e.g. http://192.168.1.100:10828): ");
        io::stdout().flush()?;
        stdin.lock().read_line(&mut remote_address)?;
    }

    // Step 2: WebUI
    println!();
    println!("Step 2: Web UI");
    print!("Start Web UI? [Y/n]: ");
    io::stdout().flush()?;
    input.clear();
    stdin.lock().read_line(&mut input)?;
    let start_webui = !input.trim().eq_ignore_ascii_case("n");

    // Step 3: Model (skippable)
    println!();
    println!("Step 3: Model Configuration (optional)");
    print!("Enter a model name or skip [skip]: ");
    io::stdout().flush()?;
    input.clear();
    stdin.lock().read_line(&mut input)?;
    let model = input.trim().to_string();

    // Step 4: Gateway provider (optional)
    println!();
    println!("Step 4: Gateway Provider (optional)");
    print!("Enter provider base URL (e.g. https://api.openai.com/v1) or skip: ");
    io::stdout().flush()?;
    input.clear();
    stdin.lock().read_line(&mut input)?;
    let provider_url = input.trim().to_string();

    let mut provider_name = String::new();
    let mut api_keys = String::new();
    if !provider_url.is_empty() {
        print!("Enter provider name [OpenAI]: ");
        io::stdout().flush()?;
        stdin.lock().read_line(&mut provider_name)?;
        let provider_name = if provider_name.trim().is_empty() { "OpenAI".to_string() } else { provider_name.trim().to_string() };

        println!("Paste your API keys (one per line, empty line to finish):");
        loop {
            input.clear();
            stdin.lock().read_line(&mut input)?;
            let key = input.trim();
            if key.is_empty() {
                break;
            }
            if !api_keys.is_empty() {
                api_keys.push('\n');
            }
            api_keys.push_str(key);
        }

        // Create provider
        let mut config = AgnConfig::default();
        let mut provider = agn_core::Provider {
            id: agn_core::new_id(),
            name: provider_name,
            base_url: provider_url.to_string(),
            keys: Vec::new(),
            models: Vec::new(),
            model_allowlist_only: false,
            protocol: agn_core::Protocol::Openai,
            aliases: Default::default(),
        };
        for line in api_keys.lines() {
            provider.keys.push(agn_core::ApiKey {
                id: agn_core::new_id(),
                key: line.trim().to_string(),
                label: String::new(),
                cooldown_secs: None,
                learned_cooldown: None,
            });
        }
        config.gateway.providers.push(provider);
        config.server.start_local = start_local;
        config.server.start_webui = start_webui;
        if !remote_address.is_empty() {
            config.server.remote_address = remote_address.trim().to_string();
        }
        if !model.is_empty() {
            config.agent.selected_model = model.to_string();
        }

        let path = AgnConfig::config_path();
        config.save(&path)?;
        println!("\n✓ Configuration saved to {}", path.display());
    } else {
        // Save minimal config
        let mut config = AgnConfig::default();
        config.server.start_local = start_local;
        config.server.start_webui = start_webui;
        if !remote_address.is_empty() {
            config.server.remote_address = remote_address.trim().to_string();
        }
        if !model.is_empty() {
            config.agent.selected_model = model.to_string();
        }

        let path = AgnConfig::config_path();
        config.save(&path)?;
        println!("\n✓ Configuration saved to {}", path.display());
    }

    // Done
    println!();
    println!("╔══════════════════════════════════════╗");
    println!("║  Setup complete! 🎉                  ║");
    println!("╚══════════════════════════════════════╝");
    println!();
    println!("Run 'agn serve' to start the server.");
    println!();

    Ok(())
}