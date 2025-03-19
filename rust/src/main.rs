#![allow(unused)]
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use bitcoincore_rpc::bitcoin::{Address, Network, Sequence, Witness};
use bitcoincore_rpc::bitcoin::address::{NetworkUnchecked, NetworkChecked};
use bitcoincore_rpc::json::LoadWalletResult;
use anyhow::Result;
use bitcoincore_rpc::bitcoin::key::{rand, PrivateKey, PublicKey};
use bitcoincore_rpc::bitcoin::secp256k1::{Secp256k1, SecretKey, Message};
use rand::Rng;
use bitcoincore_rpc::bitcoin::{TxIn,OutPoint,ScriptBuf,TxOut,Amount,Transaction,EcdsaSighashType};
use bitcoincore_rpc::bitcoin::CompressedPublicKey;
use bitcoincore_rpc::json::EstimateMode;
use bitcoincore_rpc::bitcoin::transaction::Version;
use bitcoincore_rpc::bitcoin::absolute::LockTime;
use bitcoincore_rpc::bitcoin::sighash::SighashCache;
use bitcoincore_rpc::bitcoin::consensus::encode::serialize_hex;



// Node access params
// Bitcoin Core RPC connection parameters for regtest network (local test environment)
const RPC_URL: &str = "http://127.0.0.1:18443"; // Default regtest RPC port
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";

// You can use calls not provided in RPC lib API using the generic `call` function.
// An example of using the `send` RPC call, which doesn't have exposed API.
// You can also use serde_json `Deserialize` derivation to capture the returned json result.
// This function creates a transaction with both a payment output and an OP_RETURN data output
fn send(rpc: &Client, addr: &str) -> bitcoincore_rpc::Result<String> {
    let recipient_address = addr;
    let amount_btc = 100.0;
    let message = "We are all Satoshi!!";
    let op_return_hex = hex::encode(message.as_bytes());
    let args = [
        json!([
            { recipient_address: amount_btc }, // BTC payment output
            { "data": op_return_hex } // OP_RETURN output - stores arbitrary data on blockchain
        ]),
        json!(null), // conf target
        json!(null), // estimate mode
        json!(21),   // Explicit fee rate: 21 sat/vB
        json!({}),   // Empty options object
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

// Lists all wallets in the Bitcoin Core data directory
// Returns a vector of wallet names found in the Bitcoin Core wallet directory
fn list_wallet_dir(client: &Client) -> bitcoincore_rpc::Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Name {
        name: String,
    }
    #[derive(Deserialize)]
    struct CallResult {
        wallets: Vec<Name>,
    }

    let result: CallResult = client.call("listwalletdir", &[])?;
    Ok(result.wallets.into_iter().map(|n| n.name).collect())
}

pub fn create_or_load_wallet(client: &Client)-> Result<LoadWalletResult>  {
    println!("Getting Wallet Info");
    // Check existing wallets and create/load as needed
    let current_wallets = list_wallet_dir(&client).unwrap();
    if let 0 = current_wallets.into_iter().len(){
        println!("No wallet exists creating one");
        Ok(client.create_wallet("test",None,None,None,None).unwrap())
    }else{
        println!("Loading wallet");
        // Unload existing wallet before loading to prevent conflicts
        let _ = client.unload_wallet(Some("testwallet"));
        Ok(client.load_wallet("testwallet").unwrap())
    }
}

fn get_address_balance_scan(rpc: &Client, address: &Address) -> Result<f64> {
    // Convert the address to string
    let address_str = address.to_string();

    // Use scantxoutset to find all UTXOs for this address
    #[derive(Deserialize)]
    struct ScanResult {
        total_amount: f64,
        unspents: Vec<Unspent>,
        // other fields...
    }

    #[derive(Deserialize)]
    struct Unspent {
        txid: String,
        vout: u32,
        amount: f64,
        // other fields...
    }

    // Create the descriptor for the address
    // For a P2WPKH address, the descriptor would be "addr(address)"
    let descriptor = format!("addr({})", address_str);

    // The call method expects individual JSON values, not a single array
    let scan_result: ScanResult = rpc.call("scantxoutset", &[
        json!("start"),
        json!([descriptor])
    ])?;

    // Convert BTC to satoshis
    let balance_btc = scan_result.total_amount;

    println!("Address {} has balance: {} BTC", address_str, balance_btc);

    Ok(balance_btc)
}

fn get_address_utxos(rpc: &Client, address: &Address) -> Result<Vec<(OutPoint, u64)>> {
    // Convert the address to string
    let address_str = address.to_string();

    // Use scantxoutset to find all UTXOs for this address
    #[derive(Deserialize)]
    struct ScanResult {
        total_amount: f64,
        unspents: Vec<Unspent>,
    }

    #[derive(Deserialize)]
    struct Unspent {
        txid: String,
        vout: u32,
        amount: f64,
        height: u32,
    }

    let descriptor = format!("addr({})", address_str);

    let scan_result: ScanResult = rpc.call("scantxoutset", &[
        json!("start"),
        json!([descriptor])
    ])?;

    // Convert to list of OutPoint and amount in satoshis
    let utxos = scan_result.unspents.iter().map(|u| {
        let txid = u.txid.parse().unwrap();
        let outpoint = OutPoint { txid, vout: u.vout };
        let amount_sats = (u.amount * 100_000_000.0) as u64;
        (outpoint, amount_sats)
    }).collect();

    Ok(utxos)
}


fn main() -> bitcoincore_rpc::Result<()> {
    // Initialize RPC connection to the Bitcoin Core node
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Check Connection
    let info = rpc.get_blockchain_info()?;
    println!("{:?}", info);
    // Create or load the wallet
    // create_or_load_wallet(&rpc).unwrap();

    let secp = Secp256k1::new();
    let sk = SecretKey::new(&mut rand::thread_rng());
    let private_key_minner = PrivateKey::new(sk, Network::Regtest);
    let compressed_public_key_minner = CompressedPublicKey::from_private_key(&secp, &private_key_minner).unwrap();
    let minner_address = Address::p2wpkh(&compressed_public_key_minner, Network::Regtest);
    println!("The minnner address is {}", minner_address);


    // Mine 101 blocks to the new address to activate the wallet with mined coins
    // Check current balance and mine if needed
    // In regtest, mining 104 blocks makes coinbase rewards spendable (mature)
    let block_hashes = rpc.generate_to_address(101, &minner_address)?;
    println!("Mined 104 blocks. Last block: {}", block_hashes.last().unwrap());

    // minner spending this coin to another address
    let sk = SecretKey::new(&mut rand::thread_rng());
    let private_key_recipient = PrivateKey::new(sk, Network::Regtest);
    let compressed_public_key_recipient = CompressedPublicKey::from_private_key(&secp, &private_key_recipient).unwrap();
    let recipient1_address = Address::p2wpkh(&compressed_public_key_recipient, Network::Regtest);
    println!("The recipient1 address is {}", recipient1_address);

    // Fetch a matured coinbase UTXO (from block 1 for simplicity)
    let block_hash = rpc.get_block_hash(1)?;
    let block = rpc.get_block(&block_hash)?;
    println!("Block 1: {:?}", block_hash);
    let coinbase_tx = &block.txdata[0];
    let utxo_txid = coinbase_tx.txid();
    let utxo_vout = 0;
    let utxo_amount = coinbase_tx.output[0].value; // Amount in satoshis
    println!("Coinbase UTXO txid: {}, amount: {} satoshis", utxo_txid, utxo_amount);

    //spending
    let txin = TxIn {
        previous_output: OutPoint { txid: utxo_txid, vout: 0 as u32},
        script_sig: ScriptBuf::new() ,
        sequence: Sequence::MAX,
        witness: Witness::default(),
    };

    // fetching the fee rate but it will be zero since we didnt fill previous blocks
    let feerate = match rpc.estimate_smart_fee(100,Some(EstimateMode::Economical)){
        Ok(fee) => match fee.fee_rate {
            Some(fee_rate) => fee_rate,
            None => Amount::from_sat(1000),
        },
        Err(e) => Amount::from_sat(1000),
    };

    let txout = TxOut {
        value: utxo_amount - feerate,
        script_pubkey: recipient1_address.script_pubkey(),
    };

    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![txin],
        output: vec![txout],
    };

    // Sign the transaction
    // For P2WPKH, we need to create a signature using the proper sighash
    let witness_script = minner_address.script_pubkey();
    let sighash_type = EcdsaSighashType::All;

    // Create a sighash cache for efficient signature hash computation
    let mut sighash_cache = SighashCache::new(&tx);

    // Compute the sighash for the first input
    let sighash = sighash_cache.p2wpkh_signature_hash(
        0, // Input index
        &witness_script, // The script being spent
        utxo_amount, // Value of the output being spent
        sighash_type
    ).unwrap();


    // Sign the sighash with the private key
    let msg = Message::from_digest_slice(&sighash[..])?;
    let signature = secp.sign_ecdsa(&msg, &private_key_minner.inner);

    // Serialize the signature with the sighash type appended
    let mut signature_serialized = signature.serialize_der().to_vec();
    signature_serialized.push(sighash_type as u8);

    let mut witness = Witness::new();
    witness.push(signature_serialized);
    witness.push(compressed_public_key_minner.to_bytes().to_vec());

    // Set the witness data for our transaction
    tx.input[0].witness = witness;

    // Print the transaction hex for inspection
    let tx_hex = serialize_hex(&tx);
    println!("Signed transaction hex: {}", tx_hex);

    // Broadcast the transaction to the network
    let txid = rpc.send_raw_transaction(&tx)?;
    println!("Transaction successfully broadcast! TXID: {}", txid);
    let block_hashes = rpc.generate_to_address(1, &minner_address)?;
    let balance_recenpt_1 = get_address_balance_scan(&rpc, &recipient1_address).unwrap();


    // Create a new address for the second recipient
    let sk = SecretKey::new(&mut rand::thread_rng());
    let private_key_recipient_2 = PrivateKey::new(sk, Network::Regtest);
    let compressed_public_key_recipient_2 = CompressedPublicKey::from_private_key(&secp, &private_key_recipient_2).unwrap();
    let recipient2_address = Address::p2wpkh(&compressed_public_key_recipient_2, Network::Regtest);
    println!("The recipient1 address is {}", recipient2_address);

    // Get UTXOs for recipient1
    let recipient1_utxos = get_address_utxos(&rpc, &recipient1_address).unwrap();
    if recipient1_utxos.is_empty() {
        println!("No UTXOs found for recipient1 address");
        return Ok(());
    }
    // We know there is one UTXO so we can use the first one
    let (utxo_outpoint, utxo_amount_sats) = &recipient1_utxos[0];
    println!("Using UTXO: {} with amount: {} satoshis", utxo_outpoint, utxo_amount_sats);
    // Create the input for the transaction 
    let txin = TxIn {
        previous_output: *utxo_outpoint,
        script_sig: ScriptBuf::new() ,
        sequence: Sequence::MAX,
        witness: Witness::default(),
    };
    // Create the output for the transaction (send all minus fee) 
    let txout = TxOut {
        value: Amount::from_sat(*utxo_amount_sats) - feerate,
        script_pubkey: recipient2_address.script_pubkey(),
    };
    // Create the transaction
    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![txin],
        output: vec![txout],
    };

    // Sign the transaction
    // For P2WPKH, we need to create a signature using the proper sighash
    let witness_script = recipient1_address.script_pubkey();
    let sighash_type = EcdsaSighashType::All;

    // Create a sighash cache for efficient signature hash computation
    let mut sighash_cache = SighashCache::new(&tx);

    // Compute the sighash for the first input
    let sighash = sighash_cache.p2wpkh_signature_hash(
        0, // Input index
        &witness_script, // The script being spent
        Amount::from_sat(*utxo_amount_sats), // Value of the output being spent
        sighash_type
    ).unwrap();

    // Sign the sighash with the private key
    let msg = Message::from_digest_slice(&sighash[..])?;
    let signature = secp.sign_ecdsa(&msg, &private_key_recipient.inner);

    // Serialize the signature with the sighash type appended
    let mut signature_serialized = signature.serialize_der().to_vec();
    signature_serialized.push(sighash_type as u8);

    // Create the witness
    let mut witness = Witness::new();
    witness.push(signature_serialized);
    witness.push(compressed_public_key_recipient.to_bytes().to_vec());

    // Set the witness data for our transaction
    tx.input[0].witness = witness;

    // Print the transaction hex for inspection
    let tx_hex = serialize_hex(&tx);
    println!("Signed transaction hex (recipient1 -> recipient2): {}", tx_hex);

    // Broadcast the transaction to the network
    let txid = rpc.send_raw_transaction(&tx)?;
    println!("Transaction successfully broadcast! TXID: {}", txid);

    // Mine a block to confirm the transaction
    let block_hashes = rpc.generate_to_address(1, &minner_address)?;
    println!("Mined a block to confirm the second transaction: {}", block_hashes.last().unwrap());

    // Check balances of both recipients
    let balance_recipient1 = get_address_balance_scan(&rpc, &recipient1_address).unwrap();
    let balance_recipient2 = get_address_balance_scan(&rpc, &recipient2_address).unwrap();
    println!("Final balance of recipient1: {} BTC", balance_recipient1);
    println!("Final balance of recipient2: {} BTC", balance_recipient2);
    Ok(())
}