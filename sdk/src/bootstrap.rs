#![cfg(feature = "bootstrap")]

use anyhow::{anyhow, Context, Result};
use solana_client::rpc_client::RpcClient;
#[allow(deprecated)]
use solana_sdk::system_instruction;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::Oracle;

/// Creates or reallocates an oracle account owned by the provided Doppler program.
///
/// The helper handles three scenarios:
/// - Missing account: issues a `system_instruction::create_account` to allocate the data and assign ownership.
/// - Existing but already owned by Doppler with the correct size: does nothing.
/// - Existing but owned by another program or with incorrect size: tops up rent if necessary,
///   then performs `allocate` + `assign` so the account becomes a canonical Doppler oracle.
#[allow(deprecated)]
pub fn ensure_oracle_account(
    client: &RpcClient,
    payer: &Keypair,
    oracle_keypair: &Keypair,
    program_id: &Pubkey,
    oracle_data_space: u64,
) -> Result<()> {
    if let Ok(account) = client.get_account(&oracle_keypair.pubkey()) {
        if account.owner == *program_id && account.data.len() as u64 == oracle_data_space {
            return Ok(());
        }

        let rent = client
            .get_minimum_balance_for_rent_exemption(oracle_data_space as usize)
            .context("failed to fetch rent exemption threshold")?;

        let mut instructions = Vec::new();
        if account.lamports < rent {
            instructions.push(system_instruction::transfer(
                &payer.pubkey(),
                &oracle_keypair.pubkey(),
                rent - account.lamports,
            ));
        }
        instructions.push(system_instruction::allocate(
            &oracle_keypair.pubkey(),
            oracle_data_space,
        ));
        instructions.push(system_instruction::assign(
            &oracle_keypair.pubkey(),
            program_id,
        ));

        let blockhash = client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer.pubkey()),
            &[payer, oracle_keypair],
            blockhash,
        );
        client
            .send_and_confirm_transaction(&tx)
            .context("failed to reassign oracle account")?;
        return Ok(());
    }

    let rent = client
        .get_minimum_balance_for_rent_exemption(oracle_data_space as usize)
        .context("failed to fetch rent exemption threshold")?;
    let ix = system_instruction::create_account(
        &payer.pubkey(),
        &oracle_keypair.pubkey(),
        rent,
        oracle_data_space,
        program_id,
    );
    let blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, oracle_keypair],
        blockhash,
    );
    client
        .send_and_confirm_transaction(&tx)
        .context("failed to create oracle account")?;
    Ok(())
}

/// Writes the initial oracle payload using the provided admin signer.
pub fn seed_oracle<T: Sized + Copy>(
    client: &RpcClient,
    admin: &Keypair,
    oracle_pubkey: Pubkey,
    program_id: Pubkey,
    payload: Oracle<T>,
) -> Result<()> {
    let data = payload.to_bytes();
    if data.len() < 8 {
        return Err(anyhow!("oracle payload must contain at least the sequence prefix"));
    }

    let ix = Instruction {
        program_id,
        accounts: vec![
            solana_sdk::instruction::AccountMeta::new_readonly(admin.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(oracle_pubkey, false),
        ],
        data,
    };

    let blockhash = client.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[admin],
        blockhash,
    );
    client
        .send_and_confirm_transaction(&tx)
        .context("failed to seed oracle account")?;
    Ok(())
}
