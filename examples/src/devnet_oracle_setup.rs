use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use doppler_program::PriceFeed;
use doppler_sdk::{bootstrap, Oracle};
use serde::Deserialize;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
};

const RPC_URL: &str = "https://api.devnet.solana.com";
const ORACLE_SPACE: u64 = 16;

#[derive(Debug, Deserialize)]
struct DeploymentConfig {
    doppler_program_id: String,
    doppler_admin_key: String,
    oracles: HashMap<String, String>,
}

#[derive(Debug)]
struct OracleSpec<'a> {
    name: &'a str,
    config_key: &'a str,
    keypair_file: &'a str,
    initial_price: u64,
}

fn main() -> Result<()> {
    let repo_root = determine_repo_root()?;
    let config = load_config(&repo_root)?;
    let doppler_program = config
        .doppler_program_id
        .parse::<Pubkey>()
        .context("invalid doppler_program_id in deployments/devnet.json")?;

    let admin_keypair = read_keypair_file(repo_root.join("deployments/devnet/master-keypair.json"))
        .map_err(|err| anyhow!("failed to read master keypair: {err}"))?;

    if admin_keypair.pubkey().to_string() != config.doppler_admin_key {
        eprintln!(
            "warning: doppler_admin_key in config ({}) does not match master keypair pubkey ({})",
            config.doppler_admin_key,
            admin_keypair.pubkey()
        );
    }

    let specs = [
        OracleSpec {
            name: "SOL/USDC",
            config_key: "sol_usdc",
            keypair_file: "deployments/devnet/oracle-sol-usdc-keypair.json",
            initial_price: 100_000_000,
        },
        OracleSpec {
            name: "SOL/USDT",
            config_key: "sol_usdt",
            keypair_file: "deployments/devnet/oracle-sol-usdt-keypair.json",
            initial_price: 99_500_000,
        },
        OracleSpec {
            name: "BONK/SOL",
            config_key: "bonk_sol",
            keypair_file: "deployments/devnet/oracle-bonk-sol-keypair.json",
            initial_price: 1_000,
        },
    ];

    println!("RPC endpoint:        {RPC_URL}");
    println!("Doppler program ID:  {doppler_program}");
    println!("Admin (funding) key: {}", admin_keypair.pubkey());

    let client =
        RpcClient::new_with_commitment(RPC_URL.to_string(), CommitmentConfig::confirmed());

    for spec in &specs {
        let oracle_pubkey = config
            .oracles
            .get(spec.config_key)
            .ok_or_else(|| anyhow!("missing {} oracle in deployments/devnet.json", spec.name))?
            .parse::<Pubkey>()
            .with_context(|| format!("invalid pubkey for {}", spec.name))?;

        let oracle_keypair = read_keypair_file(repo_root.join(spec.keypair_file)).map_err(|err| {
            anyhow!(
                "failed to read keypair for {} from {}: {err}",
                spec.name,
                spec.keypair_file
            )
        })?;

        if oracle_keypair.pubkey() != oracle_pubkey {
            return Err(anyhow!(
                "{} keypair pubkey ({}) does not match config ({})",
                spec.name,
                oracle_keypair.pubkey(),
                oracle_pubkey
            ));
        }

        println!("\n== {} ({oracle_pubkey}) ==", spec.name);
        bootstrap::ensure_oracle_account(
            &client,
            &admin_keypair,
            &oracle_keypair,
            &doppler_program,
            ORACLE_SPACE,
        )?;

        if let Ok(account) = client.get_account(&oracle_pubkey) {
            println!(
                "account owner={}, lamports={}",
                account.owner, account.lamports
            );
        }

        let sequence = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_secs();
        let oracle_payload = Oracle {
            sequence,
            payload: PriceFeed {
                price: spec.initial_price,
            },
        };

        bootstrap::seed_oracle(
            &client,
            &admin_keypair,
            oracle_pubkey,
            doppler_program,
            oracle_payload,
        )?;

        println!(
            "seeded oracle with price {} (sequence {})",
            spec.initial_price, sequence
        );
    }

    println!("\nAll devnet oracles are initialised.");
    Ok(())
}

fn determine_repo_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("failed to resolve repository root from CARGO_MANIFEST_DIR"))
}

fn load_config(root: &Path) -> Result<DeploymentConfig> {
    let config_path = root.join("deployments/devnet.json");
    let contents = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let cfg: DeploymentConfig = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", config_path.display()))?;
    Ok(cfg)
}
