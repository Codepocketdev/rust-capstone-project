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
    let amount = Amount::from_btc(20.0)?;
    let txid =
        miner_rpc.send_to_address(&trader_address, amount, None, None, None, None, None, None)?;
    println!("Transaction ID: {}", txid);

    // Fetch unconfirmed transaction from mempool
    #[derive(Deserialize, Debug)]
    struct MempoolEntry {
        fees: MempoolFees,
        vsize: u64,
    }

    #[derive(Deserialize, Debug)]
    struct MempoolFees {
        base: f64,
    }

    let mempool_entry = rpc.call::<MempoolEntry>("getmempoolentry", &[json!(txid.to_string())])?;
    println!("Mempool entry: {:?}", mempool_entry);

    // Mine 1 block to confirm the transaction
    let block_hashes = miner_rpc.generate_to_address(1, &mining_address)?;
    let confirm_block_hash = &block_hashes[0];

    // Get block info to find block height
    let block_info = rpc.get_block_info(confirm_block_hash)?;
    let block_height = block_info.height;

    // Structs for deserializing gettransaction response
    #[derive(Deserialize, Debug)]
    struct TxDetail {
        txid: String,
        fee: f64,
        details: Vec<TxDetailEntry>,
        decoded: DecodedTx,
        blockhash: String,
        blockheight: u64,
    }

    #[derive(Deserialize, Debug)]
    struct TxDetailEntry {
        address: String,
        category: String,
        amount: f64,
    }

    #[derive(Deserialize, Debug)]
    struct DecodedTx {
        vin: Vec<TxVin>,
        vout: Vec<TxVout>,
    }

    #[derive(Deserialize, Debug)]
    struct TxVin {
        txid: Option<String>,
        vout: Option<u32>,
        coinbase: Option<String>,
    }

    #[derive(Deserialize, Debug)]
    struct TxVout {
        value: f64,
        #[serde(rename = "scriptPubKey")]
        script_pub_key: ScriptPubKey,
    }

    #[derive(Deserialize, Debug)]
    struct ScriptPubKey {
        address: Option<String>,
    }

    // Struct for deserializing getrawtransaction response
    #[derive(Deserialize, Debug)]
    struct RawTx {
        vout: Vec<TxVout>,
    }

    let tx_detail = miner_rpc.call::<TxDetail>(
        "gettransaction",
        &[json!(txid.to_string()), json!(null), json!(true)],
    )?;

    // Find miner input address using getrawtransaction on the previous tx
    // getrawtransaction works for coinbase txs unlike gettransaction
    let vin = &tx_detail.decoded.vin[0];
    let prev_txid = vin.txid.clone().unwrap_or_default();
    let prev_vout = vin.vout.unwrap_or(0) as usize;

    let prev_tx = rpc.call::<RawTx>("getrawtransaction", &[json!(prev_txid), json!(true)])?;
    let miner_input_address = prev_tx.vout[prev_vout]
        .script_pub_key
        .address
        .clone()
        .unwrap_or_default();
    let miner_input_amount = prev_tx.vout[prev_vout].value;

    // Find trader output and miner change output from transaction vouts
    let trader_addr_str = trader_address.to_string();
    let mut trader_output_address = String::new();
    let mut trader_output_amount = 0.0_f64;
    let mut miner_change_address = String::new();
    let mut miner_change_amount = 0.0_f64;

    for vout in &tx_detail.decoded.vout {
        if let Some(addr) = &vout.script_pub_key.address {
            if addr == &trader_addr_str {
                trader_output_address = addr.clone();
                trader_output_amount = vout.value;
            } else {
                miner_change_address = addr.clone();
                miner_change_amount = vout.value;
            }
        }
    }

    // Write output to out.txt in the required format
    let mut file = File::create("../out.txt")?;
    writeln!(file, "{}", txid)?;
    writeln!(file, "{}", miner_input_address)?;
    writeln!(file, "{}", miner_input_amount)?;
    writeln!(file, "{}", trader_output_address)?;
    writeln!(file, "{}", trader_output_amount)?;
    writeln!(file, "{}", miner_change_address)?;
    writeln!(file, "{}", miner_change_amount)?;
    writeln!(file, "{}", tx_detail.fee)?;
    writeln!(file, "{}", block_height)?;
    writeln!(file, "{}", confirm_block_hash)?;

    println!("Output written to out.txt");

    Ok(())
}
