pub mod config;
pub mod error;
use crate::{
    address::error::AddressError,
    payjoin::config::*, primitives::AccountId, payout_queue::PayoutQueues, app::error::ApplicationError, job,
};
use std::collections::HashMap;

use anyhow::{anyhow, Result, Context};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use payjoin::{
    receive::{PayjoinProposal, ProvisionalProposal, UncheckedProposal},
    Error,
};
use tokio::runtime::Handle;
use tracing::instrument;

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

    #[instrument(name = "payjoin_app.process_proposal", skip(self), err)]
    pub async fn process_proposal(
        self,
        account_id: AccountId, // subdirectory
        proposal: UncheckedProposal,
    ) -> Result<PayjoinProposal, Error> {
        // in a payment processor where the sender could go offline, this is where you schedule to broadcast the original_tx
        let _to_broadcast_in_failure_case = proposal.extract_tx_to_schedule_broadcast();
        // we have to look up the output address from a list of payjoin addresses that should NOT contain change addresses
        // if we hit 2x payjoin addresses, we should abort

        // The network is used for checks later
        let network = self.network;

        // Receive Check 1: Can Broadcast
        let proposal = proposal.check_broadcast_suitability(None, |tx| {
            let _raw_tx = bitcoin::consensus::encode::serialize_hex(&tx);
            // TODO test_mempool_accept e.g.:
            //
            // Fulcrum does not yet support this, so we need to devise a way to check this to the best of our ability
            Ok(true)
        })?;
        tracing::trace!("check1");

        let network = network.clone();

        // Receive Check 2: receiver can't sign for proposal inputs
        let proposal = proposal.check_inputs_not_owned(|input| {
            self.rt.block_on(async {
                let address = bitcoin::BdkAddress::from_script(&input, network)
                    .map_err(|e| Error::Server(e.into()))?;
                match self.addresses.find_by_address(account_id, address.to_string()).await {
                    Ok(_) => Ok(true),
                    Err(AddressError::AddressNotFound(_)) => Ok(false),
                    Err(e) => Err(Error::Server(e.into())),
                }
            })
        })?;

        tracing::trace!("check2");
        // Receive Check 3: receiver can't sign for proposal inputs
        let proposal = proposal.check_no_mixed_input_scripts()?;
        tracing::trace!("check3");

        // Receive Check 4: have we seen this input before? More of a check for non-interactive i.e. payment processor receivers.
        let payjoin = proposal.check_no_inputs_seen_before(|input| {
            // TODO implement input_seen_before database check
            // Ok(!self.insert_input_seen_before(*input).map_err(|e| Error::Server(e.into()))?)
            Ok(false)
        })?;
        tracing::trace!("check4");

        // Receive Check 4: receiver can't sign for proposal inputs
        let network = network.clone();

        let mut provisional_payjoin = payjoin.identify_receiver_outputs(|output_script| {
            self.rt.block_on(async {
                let address = bitcoin::BdkAddress::from_script(&output_script, network)
                    .map_err(|e| Error::Server(e.into()))?;
                match self.addresses.find_by_address(account_id, address.to_string()).await {
                    Ok(_) => Ok(true), // TODO OK && is ours
                    Err(AddressError::AddressNotFound(_)) => Ok(false),
                    Err(e) => Err(Error::Server(e.into())),
                }
            })
        })?;

        // payout queue config, batch signing job

        // Don't throw an error. Continue optimistic process even if we can't contribute inputs.
        self.try_contributing_inputs(account_id, &mut provisional_payjoin)
            .await
            .map_err(|e| tracing::warn!("Failed to contribute inputs: {}", e));

        // Output substitution could go here

        let payjoin_proposal = provisional_payjoin.finalize_proposal(
            |psbt: &bitcoin::psbt::Psbt| {
                Err(Error::Server(anyhow!("TODO sign psbt").into()))
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

    #[instrument(skip_all, err)]
    async fn handle_web_request(self, req: Request<Body>) -> Result<Response<Body>> {
        let mut response = match (req.method(), req.uri().path()) {
            (&Method::POST, _) => self
                .handle_payjoin_post(req)
                .await
                .map_err(|e| match e {
                    Error::BadRequest(e) => Response::builder()
                        .status(400)
                        .body(Body::from(e.to_string()))
                        .unwrap(),
                    e => Response::builder()
                        .status(500)
                        .body(Body::from(e.to_string()))
                        .unwrap(),
                })
                .unwrap_or_else(|err_resp| err_resp),
            _ => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not found"))
                .unwrap(),
        };
        response.headers_mut().insert(
            "Access-Control-Allow-Origin",
            hyper::header::HeaderValue::from_static("*"),
        );
        Ok(response)
    }

    #[instrument(skip_all, err)]
    async fn handle_payjoin_post(self, req: Request<Body>) -> Result<Response<Body>, Error> {
        let (parts, body) = req.into_parts();
        let headers = Headers(&parts.headers);
        let query_string = parts.uri.query().unwrap_or("");
        let body = std::io::Cursor::new(
            hyper::body::to_bytes(body)
                .await
                .map_err(|e| Error::Server(e.into()))?
                .to_vec(),
        );
        let proposal =
            payjoin::receive::UncheckedProposal::from_request(body, query_string, headers)?;

        let account_id = AccountId::new(); // TODO get from req subdir
        let payjoin_proposal = self.process_proposal(account_id, proposal).await?;
        let psbt = payjoin_proposal.psbt();
        let body = base64::encode(psbt.serialize());
        println!(
            "Responded with Payjoin proposal {}",
            psbt.clone().extract_tx().txid()
        );
        Ok(Response::new(Body::from(body)))
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

pub async fn start(pj: PayjoinReceiver) -> Result<()> {
    println!("Starting payjoin server on port {}", pj.config.listen_port);
    let bind_addr = std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
        pj.config.listen_port,
    );
    let server = Server::bind(&bind_addr);
    let make_svc = make_service_fn(|_| {
        let payjoin = pj.clone();
        async move {
            let handler = move |req| payjoin.clone().handle_web_request(req);
            Ok::<_, hyper::Error>(service_fn(handler))
        }
    });
    server.serve(make_svc).await?;
    Ok(())
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
