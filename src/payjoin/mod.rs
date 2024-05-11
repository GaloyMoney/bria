pub mod config;
pub mod error;
use crate::{
    address::error::AddressError,
    payjoin::config::*, primitives::AccountId, payout_queue::PayoutQueues, app::error::ApplicationError, job,
};
use std::{collections::HashMap, time::Duration};

use anyhow::{anyhow, Result, Context};
use bdk::bitcoin::{psbt::Psbt, Transaction, Txid};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use payjoin::{
    receive::v2::{Enrolled, PayjoinProposal, ProvisionalProposal, UncheckedProposal}, send::RequestContext, Error
};
use tokio::runtime::Handle;
use tracing::instrument;
use url::Url;

type ProtoClient =
    crate::api::proto::bria_service_client::BriaServiceClient<tonic::transport::Channel>;
use crate::{
    address::Addresses,
    primitives::bitcoin::{self, Network},
    utxo::Utxos,
    wallet::Wallets,
};

#[derive(Clone)]
pub struct PayjoinReceiver {
    rt: Handle,
    pool: sqlx::PgPool,
    payout_queues: PayoutQueues,
    config: PayjoinConfig,
    addresses: Addresses,
    utxos: Utxos,
    wallets: Wallets,
    network: Network,
}

impl PayjoinReceiver {
    pub fn new(
        pool: sqlx::PgPool,
        payout_queues: PayoutQueues,
        config: PayjoinConfig,
        addresses: Addresses,
        utxos: Utxos,
        wallets: Wallets,
        network: Network,
    ) -> Self {
        Self {
            rt: Handle::current(),
            pool,
            payout_queues,
            config,
            addresses,
            utxos,
            wallets,
            network,
        }
    }

    pub async fn process_proposal(
        self,
        session: RecvSession,
        proposal: UncheckedProposal,
    ) -> Result<PayjoinProposal, Error> {
        // in a payment processor where the sender could go offline, this is where you schedule to broadcast the original_tx
        let _to_broadcast_in_failure_case = proposal.extract_tx_to_schedule_broadcast();
        // we have to look up the output address from a list of payjoin addresses that should NOT contain change addresses
        // if we hit 2x payjoin addresses, we should abort

        // The network is used for checks later
        let network = self.network;
        let account_id = session.account_id;

        // Receive Check 1: Can Broadcast
        let proposal = proposal.check_broadcast_suitability(None, |tx| {
            let _raw_tx = bitcoin::consensus::encode::serialize_hex(&tx);
            // TODO test_mempool_accept e.g.:
            //
            // Fulcrum does not yet support this, so we need to devise a way to check this to the best of our ability
            Ok(true)
        })?;
        println!("check2");
        let network = network.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        // Receive Check 2: receiver can't sign for proposal inputs
        let proposal = proposal.check_inputs_not_owned(|input| {
            let network = network.clone(); // Make sure to clone the network or ensure it's moved properly
            let address_result = bitcoin::BdkAddress::from_script(&input, network);
    
            // Spawn a new thread for each input check
            let tx = tx.clone();
            let addresses = self.addresses.clone();
            println!("check2");
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    println!("check2");
                    let result = match address_result {
                        Ok(address) => {
                            match addresses.find_by_address(account_id, address.to_string()).await {
                                Ok(_) => Ok(true),
                                Err(AddressError::AddressNotFound(_)) => Ok(false),
                                Err(e) => {
                                    println!("ERROR! {:?}", e.to_string());
                                    Err(Error::Server(e.into()))
                                },
                            }
                        },
                        Err(e) => Err(Error::Server(e.into())),
                    };
                    println!("check2");
                    tx.send(result).unwrap();
                });
            });
    
            // This will block until the async operation is complete
            rx.recv().unwrap()
        })?;
        println!("check3");

        // Receive Check 3: receiver can't sign for proposal inputs
        let proposal = proposal.check_no_mixed_input_scripts()?;

        // Receive Check 4: have we seen this input before? More of a check for non-interactive i.e. payment processor receivers.
        let payjoin = proposal.check_no_inputs_seen_before(|input| {
            // TODO implement input_seen_before database check
            // Ok(!self.insert_input_seen_before(*input).map_err(|e| Error::Server(e.into()))?)
            Ok(false)
        })?;

        // Receive Check 4: receiver can't sign for proposal inputs
        let network = network.clone();
        let (tx2, rx2) = std::sync::mpsc::channel();
        let mut provisional_payjoin = payjoin.identify_receiver_outputs(|output_script| {
            let network = network.clone(); // Clone network to safely move it into the thread
            let address_result = bitcoin::BdkAddress::from_script(&output_script, network);
            
            // Clone transmitter for each output_script
            let tx2 = tx2.clone();
            let addresses = self.addresses.clone(); // Assuming addresses can be cloned or it's wrapped in an Arc
    
            // Spawn a new thread for each output_script check
            std::thread::spawn(move || {
                println!("check4");
                let rt = tokio::runtime::Runtime::new().unwrap(); // Create a new runtime for the thread
                rt.block_on(async {
                    let result = match address_result {
                        Ok(address) => {
                            match addresses.find_by_address(account_id, address.to_string()).await {
                                Ok(_) => Ok(true), // TODO: Confirm ownership logic if needed
                                Err(AddressError::AddressNotFound(_)) => Ok(false),
                                Err(e) => {
                                    println!("ERROR!");
                                    Err(Error::Server(e.into()))
                                },
                            }
                        },
                        Err(e) => Err(Error::Server(e.into())),
                    };
                    println!("check4");
                    tx2.send(result).unwrap(); // Send the result back to the main thread
                });
            });
    
            // Block until the async operation is complete
            rx2.recv().unwrap()
        })?;

        // payout queue config, batch signing job
        println!("contribute");
        // Don't throw an error. Continue optimistic process even if we can't contribute inputs.
        self.try_contributing_inputs(account_id, &mut provisional_payjoin)
            .await
            .map_err(|e| println!("Failed to contribute inputs: {}", e));

        // Output substitution could go here
        println!("finalize");

        let payjoin_proposal = provisional_payjoin.finalize_proposal(
            |psbt: &bitcoin::psbt::Psbt| {
                Ok(psbt.clone())
                // TODO sign proposal psbt with our inputs & subbed outputs e.g.:
                //
                // bitcoind
                //     .wallet_process_psbt(&base64::encode(psbt.serialize()), None, None, Some(false))
                //     .map(|res| bitcoin::psbt::Psbt::from_str(&res.psbt).map_err(|e| Error::Server(e.into())))
                //     .map_err(|e| Error::Server(e.into()))?
            },
            None, // TODO set to bitcoin::FeeRate::MIN or similar
        )?;
        let payjoin_proposal_psbt = payjoin_proposal.psbt();
        println!(
            "Responded with Payjoin proposal {}",
            payjoin_proposal_psbt.clone().extract_tx().txid()
        );
        Ok(payjoin_proposal)
    }

    async fn try_contributing_inputs(self, account_id: AccountId, payjoin: &mut ProvisionalProposal) -> Result<()> {
        use bitcoin::OutPoint;

        let available_wallets = self
            .wallets
            .list_by_account_id(account_id)
            .await
            .context("Failed to list wallets")?;
        let keychain_ids = available_wallets
            .iter()
            .flat_map(|wallet| wallet.keychain_ids());
        let mut keychain_utxos = self.utxos.find_keychain_utxos(keychain_ids).await.context("failed to find keychain utxos")?;
        let keychain_utxos = keychain_utxos
            .drain()
            .map(|(_, keychain_utxos)| keychain_utxos)
            .collect::<Vec<_>>();
        
        let mut available_inputs = keychain_utxos
            .iter()
            .flat_map(|keychain_utxos| keychain_utxos.utxos.iter());

        let candidate_inputs: HashMap<bitcoin::Amount, OutPoint> = available_inputs
            .clone()
            // Why is a utxo output value NOT saved in bitcoin::Amount? How can it be partial satoshis?
            .map(|i| {
                (
                    bitcoin::Amount::from_sat(i.value.into()),
                    i.outpoint.clone(),
                )
            })
            .collect();

        let selected_outpoint = payjoin
            .try_preserving_privacy(candidate_inputs)
            .expect("no privacy preserving utxo found");
        let selected_utxo = available_inputs
            .find(|i| i.outpoint == selected_outpoint)
            .context("This shouldn't happen. Failed to retrieve the privacy preserving utxo from those we provided to the seclector.")?;

        let txo_to_contribute = bitcoin::TxOut {
            value: selected_utxo.value.into(),
            script_pubkey: selected_utxo
                .address
                .clone()
                .ok_or_else(|| anyhow!("selected_utxo missing script"))?
                .script_pubkey(),
        };
        payjoin.contribute_witness_input(txo_to_contribute, selected_outpoint);
        Ok(())
    }

    async fn trigger_payout_queue(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<(), ApplicationError> {
        let payout_queue = self
            .payout_queues
            .find_by_name(account_id, name)
            .await?;
        job::spawn_payjoin_payout_queue(&self.pool, (payout_queue.account_id, payout_queue.id))
            .await?;
        Ok(())
    }
}

pub async fn init_payjoin_session(pj: PayjoinReceiver, account_id: AccountId) -> Result<(Enrolled, payjoin::OhttpKeys), anyhow::Error> {
    let payjoin_dir =  Url::parse("https://payjo.in").expect("Invalid URL");
    let ohttp_relays: [Url; 2] = [
        Url::parse("https://pj.bobspacebkk.com").expect("Invalid URL"),
        Url::parse("https://ohttp-relay.obscuravpn.io").expect("Invalid URL"),
    ];
    println!("fetch");
    let payjoin_dir_clone = payjoin_dir.clone();
    let ohttp_relay_clone = ohttp_relays[0].clone();
    let ohttp_keys = tokio::task::spawn_blocking(move || {
        payjoin_defaults::fetch_ohttp_keys(ohttp_relay_clone, payjoin_dir_clone)
    }).await??;
    let http_client = reqwest::Client::builder().build()?;
    println!("fetched");
    fn random_ohttp_relay(ohttp_relays: [Url; 2]) -> Url {
        use rand::seq::SliceRandom;
        use rand::thread_rng;
        ohttp_relays.choose(&mut thread_rng()).unwrap().clone()
    }
    println!("enroll");
    let mut enroller = payjoin::receive::v2::Enroller::from_directory_config(
        payjoin_dir.to_owned(),
        ohttp_keys.clone(),
        ohttp_relays[0].to_owned(),
    );
    println!("req");
    let (req, context) = enroller.extract_req().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let ohttp_response = http_client
        .post(req.url)
        .header("Content-Type", "message/ohttp-req")
        .body(req.body)
        .send()
        .await?;
    let ohttp_response = ohttp_response.bytes().await?;
    println!("res");
    let enrolled = enroller.process_res(ohttp_response.as_ref(), context).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let recv_session = RecvSession { enrolled: enrolled.clone(), expiry: std::time::Duration::from_secs(60 * 60 * 24), payjoin_tx: None, account_id };
    // TODO listen on thread for a payjoin request
    println!("made sesh");
    spawn_recv_session(recv_session, pj).await?;
    Ok((
        enrolled,
        ohttp_keys,
    ))
}

pub async fn spawn_recv_session(session: RecvSession, pj: PayjoinReceiver) -> Result<()> {
    tokio::spawn(async move {
        let _ = resume_recv_session(session, pj).await;
    });
    Ok(())
}

async fn resume_recv_session(mut session: RecvSession, pj: PayjoinReceiver) -> Result<Txid> {
    println!("RESUME RECEIVE SESSION");
    let http_client = reqwest::Client::builder()
        .build()?;
    let proposal: UncheckedProposal = poll_for_fallback_psbt(
        &http_client,
        &mut session,
    )
    .await?;
    println!("POLLED RECEIVE SESSION");
    let _original_tx = proposal.extract_tx_to_schedule_broadcast();
    let mut payjoin_proposal = match pj
        .process_proposal(session, proposal)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    {
        Ok(p) => p,
        Err(e) => {
            // TODO pj.wallet.broadcast_transaction(original_tx).await?;
            return Err(e.into());
        }
    };

    let (req, ohttp_ctx) = payjoin_proposal
        .extract_v2_req().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let res = http_client
        .post(req.url)
        .header("Content-Type", "message/ohttp-req")
        .body(req.body)
        .send()
        .await?;

    let res = res.bytes().await?;
    // enroll must succeed
    let _res = payjoin_proposal
        .deserialize_res(res.to_vec(), ohttp_ctx).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let payjoin_tx = payjoin_proposal.psbt().clone().extract_tx();
    let payjoin_txid = payjoin_tx.txid();
    // TODO 
    // wallet
    //     .insert_tx(
    //         payjoin_tx.clone(),
    //         ConfirmationTime::unconfirmed(utils::now().as_secs()),
    //         None,
    //     )
    //     .await?;
    // session.payjoin_tx = Some(payjoin_tx);
    // storage.update_recv_session(session)?;
    Ok(payjoin_txid)
}

async fn poll_for_fallback_psbt(
    client: &reqwest::Client,
    session: &mut crate::payjoin::RecvSession,
) -> Result<payjoin::receive::v2::UncheckedProposal> {
    loop {
        // if stop.load(Ordering::Relaxed) {
        //     return Err(crate::payjoin::Error::Shutdown);
        // }

        // if session.expiry < utils::now() {
        //     if let Some(payjoin_tx) = &session.payjoin_tx {
        //         wallet
        //             .cancel_tx(payjoin_tx)
        //             .map_err(|_| crate::payjoin::Error::CancelPayjoinTx)?;
        //     }
        //     let _ = storage.delete_recv_session(&session.enrolled.pubkey());
        //     return Err(crate::payjoin::Error::SessionExpired);
        // }
        println!("POLLING RECEIVE SESSION");
        let (req, context) = session.enrolled.extract_req().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let ohttp_response = client
            .post(req.url)
            .header("Content-Type", "message/ohttp-req")
            .body(req.body)
            .send()
            .await?;
        let ohttp_response = ohttp_response.bytes().await?;
        let proposal = session
            .enrolled
            .process_res(ohttp_response.as_ref(), context).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match proposal {
            Some(proposal) => return Ok(proposal),
            None => tokio::time::sleep(tokio::time::Duration::from_secs(5)).await,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecvSession {
    pub enrolled: Enrolled,
    pub expiry: Duration,
    pub payjoin_tx: Option<Transaction>,
    pub account_id: AccountId,
}

// impl RecvSession {
//     pub fn pubkey(&self) -> [u8; 33] {
//         self.enrolled.pubkey()
//     }
// }

#[derive(Clone, PartialEq)]
pub struct SendSession {
    pub original_psbt: Psbt,
    pub req_ctx: RequestContext,
    pub labels: Vec<String>,
    pub expiry: Duration,
}

struct Headers<'a>(&'a hyper::HeaderMap);

impl payjoin::receive::Headers for Headers<'_> {
    fn get_header(&self, key: &str) -> Option<&str> {
        self.0
            .get(key)
            .map(|v| v.to_str())
            .transpose()
            .ok()
            .flatten()
    }
}
