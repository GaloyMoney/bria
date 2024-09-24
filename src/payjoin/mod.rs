pub mod config;
pub mod error;

use payjoin::bitcoin;
use crate::{
    address::error::AddressError, app::error::ApplicationError, job::{self, process_payout_queue::{self, ProcessPayoutQueueData}}, payjoin::config::*, payout_queue::PayoutQueues, primitives::{AccountId, ProfileId}, profile::Profile
};
use std::{any::Any, collections::HashMap, str::FromStr, time::Duration};

use anyhow::{anyhow, Result, Context};
use bdk::bitcoin::{psbt::Psbt, Address, Transaction, Txid};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use payjoin::{
    receive::v2::{ActiveSession, PayjoinProposal, ProvisionalProposal, UncheckedProposal, WantsInputs, WantsOutputs}, send::RequestContext, Error
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

/// A representation of a payjoin receiver "service"
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

    /// Initializes a payjoin session and listens for a payjoin request on a background thread.
    /// TODO save the session to the database so it can be resumed after a shutdown
    pub async fn init_payjoin_session(&self, account_id: &AccountId, address: payjoin::bitcoin::Address) -> Result<(RecvSession, payjoin::OhttpKeys), anyhow::Error> {
        let payjoin_dir =  Url::parse("https://payjo.in").expect("Invalid URL");
        let ohttp_relays: [Url; 2] = [
            Url::parse("https://pj.bobspacebkk.com").expect("Invalid URL"),
            Url::parse("https://ohttp.payjoin.org").expect("Invalid URL"),
        ];
        dbg!("fetch");
        let payjoin_dir_clone = payjoin_dir.clone();
        let ohttp_relay_clone = ohttp_relays[0].clone();
        let ohttp_keys = tokio::task::spawn_blocking(move || {
            payjoin::io::fetch_ohttp_keys(ohttp_relay_clone, payjoin_dir_clone)
        }).await?.await?;
        let http_client = reqwest::Client::builder().build()?;
        dbg!("fetched");
        fn random_ohttp_relay(ohttp_relays: [Url; 2]) -> Url {
            use rand::seq::SliceRandom;
            use rand::thread_rng;
            ohttp_relays.choose(&mut thread_rng()).unwrap().clone()
        }
        dbg!("enroll");
        let mut enroller = payjoin::receive::v2::SessionInitializer::new(
            address,
            payjoin_dir.to_owned(),
            ohttp_keys.clone(),
            ohttp_relays[0].to_owned(),
            None,
        );
        dbg!("req");
        let (req, context) = enroller.extract_req().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let ohttp_response = http_client
            .post(req.url)
            .header("Content-Type", "message/ohttp-req")
            .body(req.body)
            .send()
            .await?;
        let ohttp_response = ohttp_response.bytes().await?;
        dbg!("res");
        let session = enroller.process_res(ohttp_response.as_ref(), context).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let recv_session = RecvSession { account_id: account_id.clone(), session: session.clone(), expiry: std::time::Duration::from_secs(60 * 60 * 24), payjoin_tx: None };
        self.spawn_recv_session(recv_session.clone());
        // ^^ ABOVE DOES THIS
        // tokio::task::spawn(move || {
        //     let wants_outputs = self.sanity_check(recv_session, proposal).await?;

        //     // let rt = tokio::runtime::Runtime::new().unwrap();
        //     // rt.block_on(async {
        //     //     let proposal = poll_for_fallback_psbt(&http_client, &mut recv_session).await?;
        //     //     // TODO start listening, on a job?
        //     // })
        //     // TODO start listening, on a job?
        //     // TODO listen on thread for a payjoin request
        //     // spawn_recv_session(recv_session, pj).await?;
        // });
        // TODO save session to DB before returning
        // TODO start listening, on a job?
        dbg!("made sesh");
        Ok((
            recv_session,
            ohttp_keys,
        ))
    }

    // fn complete_payjoin(self, payjoin: WantsOutputs) -> Result<PayjoinProposal, Error> {
    
    //     // payout queue config, batch signing job
    //     println!("contribute");
    //     // Don't throw an error. Continue optimistic process even if we can't contribute inputs.
    //     self.try_contributing_inputs(account_id, payjoin)
    //         .await
    //         .map_err(|e| println!("Failed to contribute inputs: {}", e));

    //     // Output substitution could go here
    //     println!("finalize");

    //     let payjoin_proposal = payjoin.finalize_proposal(
    //         |psbt: &bitcoin::psbt::Psbt| {
    //             process_payout_queue:
    //             Ok(psbt.clone())
    //             // TODO sign proposal psbt with our inputs & subbed outputs e.g.:
    //             //
    //             // bitcoind
    //             //     .wallet_process_psbt(&base64::encode(psbt.serialize()), None, None, Some(false))
    //             //     .map(|res| bitcoin::psbt::Psbt::from_str(&res.psbt).map_err(|e| Error::Server(e.into())))
    //             //     .map_err(|e| Error::Server(e.into()))?
    //         },
    //         None, // TODO set to bitcoin::FeeRate::MIN or similar
    //     )?;
    //     let payjoin_proposal_psbt = payjoin_proposal.psbt();
    //     println!(
    //         "Responded with Payjoin proposal {}",
    //         payjoin_proposal_psbt.clone().extract_tx().txid()
    //     );
    //     Ok(payjoin_proposal)
    // }

    async fn try_contributing_inputs(self, account_id: AccountId, payjoin: WantsInputs) -> Result<()> {
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

        let candidate_inputs: HashMap<payjoin::bitcoin::Amount, payjoin::bitcoin::OutPoint> = available_inputs
            .clone()
            // Why is a utxo output value NOT saved in bitcoin::Amount? How can it be partial satoshis?
            .map(|i| {
                let txid = payjoin::bitcoin::Txid::from_str(&i.outpoint.txid.to_string()).unwrap();
                (
                    payjoin::bitcoin::Amount::from_sat(i.value.into()),
                    payjoin::bitcoin::OutPoint::new(txid, i.outpoint.vout),
                )
            })
            .collect();
        let selected_outpoint = payjoin
            .try_preserving_privacy(candidate_inputs)
            .expect("no privacy preserving utxo found");
        let selected_utxo = available_inputs
            .find(|i| {
                let txid = payjoin::bitcoin::Txid::from_str(&i.outpoint.txid.to_string()).unwrap();
                payjoin::bitcoin::OutPoint::new(txid, i.outpoint.vout) == selected_outpoint
            })
            .context("This shouldn't happen. Failed to retrieve the privacy preserving utxo from those we provided to the seclector.")?;

        let txo_to_contribute = payjoin::bitcoin::TxOut {
            value: payjoin::bitcoin::Amount::from_sat(selected_utxo.value.into()),
            script_pubkey: payjoin::bitcoin::ScriptBuf::from_bytes(selected_utxo
                .address
                .clone()
                .ok_or_else(|| anyhow!("selected_utxo missing script"))?
                .script_pubkey().to_bytes()),
        };
        payjoin.contribute_witness_inputs(vec![(selected_outpoint, txo_to_contribute)]);
        Ok(())
    }

    pub async fn spawn_recv_session(&self, mut session: RecvSession) -> Result<()> {
        let payout_queues = self.payout_queues.clone();
        let pool = self.pool.clone();
        let addresses = self.addresses.clone();
        let network = self.network.clone();
        tokio::spawn(async move {
            let qs = payout_queues.clone().list_by_account_id(session.account_id).await.unwrap();
            let payout_queue_id = &qs.first().unwrap().id;
            let http_client = reqwest::Client::builder().build().unwrap();
            let proposal = poll_for_fallback_psbt(session.clone(), &http_client).await.unwrap();
            let wants_outputs = sanity_check(session.clone(), proposal, network, addresses.clone()).await.unwrap();
            job::spawn_process_payout_queue(&pool.clone(), (session.account_id, *payout_queue_id, wants_outputs)).await.unwrap();
            // let _ = self.resume_recv_session(session).await.unwrap();d
        });
        Ok(())
    }

//     async fn resume_recv_session(self, mut session: RecvSession) -> Result<Txid> {
//         dbg!("RESUME RECEIVE SESSION");
//         let http_client = reqwest::Client::builder()
//             .build()?;
//         let proposal: UncheckedProposal = poll_for_fallback_psbt(
//             session,
//             &http_client,
//         )
//         .await?;
//         dbg!("POLLED RECEIVE SESSION");
//         let _original_tx = proposal.extract_tx_to_schedule_broadcast();
//         let mut payjoin_proposal = match sanity_check(session, proposal, self.network, self.addresses)
//             .await
//             .map_err(|e| anyhow::anyhow!(e.to_string()))
//         {
//             Ok(p) => p,
//             Err(e) => {
//                 // TODO pj.wallet.broadcast_transaction(original_tx).await?;
//                 return Err(e.into());
//             }
//         };

//         let (req, ohttp_ctx) = payjoin_proposal
//             .extract_v2_req().map_err(|e| anyhow::anyhow!(e.to_string()))?;
//         let res = http_client
//             .post(req.url)
//             .header("Content-Type", "message/ohttp-req")
//             .body(req.body)
//             .send()
//             .await?;

//         let res = res.bytes().await?;
//         // enroll must succeed
//         let _res = payjoin_proposal
//             .deserialize_res(res.to_vec(), ohttp_ctx).map_err(|e| anyhow::anyhow!(e.to_string()))?;
//         let payjoin_tx = payjoin_proposal.psbt().clone().extract_tx();
//         let payjoin_txid = payjoin_tx.txid();
//         // TODO 
//         // wallet
//         //     .insert_tx(
//         //         payjoin_tx.clone(),
//         //         ConfirmationTime::unconfirmed(utils::now().as_secs()),
//         //         None,
//         //     )
//         //     .await?;
//         // session.payjoin_tx = Some(payjoin_tx);
//         // storage.update_recv_session(session)?;
//         Ok(payjoin_txid)
//     }
}

pub async fn poll_for_fallback_psbt(
    session: RecvSession,
    client: &reqwest::Client,
) -> Result<payjoin::receive::v2::UncheckedProposal> {
    let mut session = session.session;
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
        let (req, context) = session.extract_req().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let ohttp_response = client
            .post(req.url)
            .header("Content-Type", "message/ohttp-req")
            .body(req.body)
            .send()
            .await?;
        let ohttp_response = ohttp_response.bytes().await?;
        let proposal = session
            .process_res(ohttp_response.as_ref(), context).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match proposal {
            Some(proposal) => return Ok(proposal),
            None => tokio::time::sleep(tokio::time::Duration::from_secs(5)).await,
        }
    }
}

pub async fn sanity_check(
    session: RecvSession,
    proposal: UncheckedProposal,
    network: Network,
    addresses: Addresses,
) -> Result<WantsOutputs, Box<dyn std::error::Error>> {
    // in a payment processor where the sender could go offline, this is where you schedule to broadcast the original_tx
    let _to_broadcast_in_failure_case = proposal.extract_tx_to_schedule_broadcast();
    // we have to look up the output address from a list of payjoin addresses that should NOT contain change addresses
    // if we hit 2x payjoin addresses, we should abort
    let account_id = session.account_id;

    // Receive Check 1: Can Broadcast
    let proposal = proposal.check_broadcast_suitability(None, |_tx| {
        // TODO test_mempool_accept e.g.:
        //
        // Fulcrum does not yet support this, so we need to devise a way to check this to the best of our ability
        // Probably by using bitcoind directly and deprecating Fulcrum
        Ok(true)
    }).expect("check1 failed");
    dbg!("check2");
    let network = network.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    // Receive Check 2: receiver can't sign for proposal inputs
    let proposal = proposal.check_inputs_not_owned(|input| {
        // Spawn a new thread for each input check
        let tx = tx.clone();
        let addresses = addresses.clone();
        let input = input.to_string();
        let network = network.clone();
        tokio::spawn(async move {
            let result = match bitcoin::BdkAddress::from_str(&input) {
                Ok(address) => {
                    match addresses.find_by_address(account_id, address.require_network(network).unwrap().to_string()).await {
                        Ok(_) => Ok(true),
                        Err(AddressError::AddressNotFound(_)) => Ok(false),
                        Err(e) => {
                            eprintln!("ERROR: {}", e);
                            Err(e.to_string())
                        },
                    }
                },
                Err(e) => Err(e.to_string()),
            };
            tx.send(result).unwrap();
        });

        // This will block until the async operation is complete
        rx.recv().unwrap().map_err(|e| payjoin::Error::Server(e.into()))
    }).expect("check2 failed");
    dbg!("check3");

    // Receive Check 3: receiver can't sign for proposal inputs
    let proposal = proposal.check_no_mixed_input_scripts()?;

    // Receive Check 4: have we seen this input before? More of a check for non-interactive i.e. payment processor receivers.
    let payjoin = proposal.check_no_inputs_seen_before(|input| {
        // TODO implement input_seen_before database check
        // Ok(!self.insert_input_seen_before(*input).map_err(|e| Error::Server(e.into()))?)
        Ok(false)
    }).expect("check4 failed");

    // Receive Check 4: receiver can't sign for proposal inputs
    let network = network.clone();
    let (tx2, rx2) = std::sync::mpsc::channel();
    let mut payjoin = payjoin.identify_receiver_outputs(|output_script| {
        // Clone transmitter for each output_script
        let tx2 = tx2.clone();
        let addresses = addresses.clone();
        let output_script = output_script.to_string();
        // Spawn a new thread for each output_script check
        std::thread::spawn(move || {
            dbg!("check4");
            let rt = tokio::runtime::Runtime::new().unwrap(); // Create a new runtime for the thread
            rt.block_on(async {
                let result = match bitcoin::BdkAddress::from_str(&output_script) {
                    Ok(address) => {
                        match addresses.find_by_address(account_id, address.assume_checked().to_string()).await {
                            Ok(_) => Ok(true), // TODO: Confirm ownership logic if needed
                            Err(AddressError::AddressNotFound(_)) => Ok(false),
                            Err(e) => {
                                dbg!("ERROR!");
                                Err(e.to_string())
                            },
                        }
                    },
                    Err(e) => Err(e.to_string()),
                };
                dbg!("check4");
                tx2.send(result).unwrap(); // Send the result back to the main thread
            });
        });

        // Block until the async operation is complete
        rx2.recv().unwrap().map_err(|e| payjoin::Error::Server(e.into()))
    }).expect("check5 failed");
    Ok(payjoin)
}

use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;

pub(crate) enum ProcessPsbtControl {
    Pause,
    Resume,
    Stop,
}

/// sign and finalize the proposal psbt
pub fn wallet_process_psbt(psbt: bitcoin::Psbt) -> Result<bitcoin::Psbt, payjoin::Error> {
    Ok(psbt)
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecvSession {
    pub account_id: AccountId,
    pub session: ActiveSession,
    pub expiry: Duration,
    pub payjoin_tx: Option<Transaction>,
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
