#![allow(unused)]
use bitcoin::hex::DisplayHex;
use bitcoincore_rpc::bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;

// Node access params
const RPC_URL: &str = "http://127.0.0.1:18443";
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";

fn send(rpc: &Client, addr: &str) -> bitcoincore_rpc::Result<String> {
    let args = [
        json!([{addr : 100 }]),
        json!(null),
        json!(null),
        json!(null),
        json!(null),
    ];

    #[derive(Deserialize)]
    struct SendResult {
        complete: bool,
        txid: String,
    }
    let send_result = rpc.call::<SendResult>("send", &args)?;
    assert!(send_result.complete);
    Ok(send_result.txid)
}

fn main() -> bitcoincore_rpc::Result<()> {
    // Connect to Bitcoin Core RPC
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Create or load Miner wallet
    let miner_rpc = match rpc.create_wallet("Miner", None, None, None, None) {
        Ok(_) => Client::new(
            &format!("{}/wallet/Miner", RPC_URL),
            Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
        )?,
        Err(_) => {
            let _ = rpc.load_wallet("Miner");
            Client::new(
                &format!("{}/wallet/Miner", RPC_URL),
                Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
            )?
        }
    };

    // Create or load Trader wallet
    let trader_rpc = match rpc.create_wallet("Trader", None, None, None, None) {
        Ok(_) => Client::new(
            &format!("{}/wallet/Trader", RPC_URL),
            Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
        )?,
        Err(_) => {
            let _ = rpc.load_wallet("Trader");
            Client::new(
                &format!("{}/wallet/Trader", RPC_URL),
                Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
            )?
        }
    };

    // Generate a mining address from Miner wallet with label "Mining Reward"
    let mining_address = miner_rpc.get_new_address(Some("Mining Reward"), None)?;
    let mining_address = mining_address
        .require_network(bitcoincore_rpc::bitcoin::Network::Regtest)
        .unwrap();

    // Mine 101 blocks to get spendable balance.
    // Coinbase rewards require 100 confirmations before they can be spent.
    // So the first block reward only becomes spendable after 100 more blocks are mined on top of it.
    miner_rpc.generate_to_address(101, &mining_address)?;

    // Print Miner wallet balance
    let miner_balance = miner_rpc.get_balance(None, None)?;
    println!("Miner balance: {} BTC", miner_balance);

    // Generate a receiving address from Trader wallet with label "Received"
    let trader_address = trader_rpc.get_new_address(Some("Received"), None)?;
    let trader_address = trader_address
        .require_network(bitcoincore_rpc::bitcoin::Network::Regtest)
        .unwrap();

    // Send 20 BTC from Miner to Trader

    // Check transaction in mempool

    // Mine 1 block to confirm the transaction

    // Extract all required transaction details

    // Write the data to ../out.txt in the specified format given in readme.md

    Ok(())
}
