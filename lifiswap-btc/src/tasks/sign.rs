//! Sign task — parses PSBT from the API response, signs it, and broadcasts.
//!
//! Mirrors the TS SDK's `BitcoinSignAndExecuteTask`: decodes the PSBT hex
//! from `step.transaction_request.data`, prepares Taproot/P2SH inputs,
//! signs via [`BtcSigner`], finalizes, extracts the raw transaction, and
//! broadcasts it via the blockchain API.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bitcoin::consensus::encode;
use bitcoin::hex::FromHex as _;
use bitcoin::psbt::Psbt;
use bitcoin::script::PushBytes;
use bitcoin::{Address, AddressType, Network, Witness};
use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::execution::status::ActionUpdateParams;
use lifiswap::execution::task::{ExecutionContext, ExecutionTask};
use lifiswap::types::{ExecutionActionStatus, ExecutionActionType, TaskStatus};

use super::{BtcTxInputs, get_tx_link, now_ms};
use crate::api::BlockchainApi;
use crate::signer::BtcSigner;

/// Signs a Bitcoin PSBT from the step's transaction data, finalizes it,
/// and broadcasts the resulting raw transaction.
///
/// The API returns the PSBT as a hex string in `transaction_request.data`.
/// This task handles all address types: P2PKH, P2SH (nested `SegWit`),
/// P2WPKH, P2WSH, and P2TR (Taproot).
///
/// After broadcast, stores the first input outpoint in shared
/// [`BtcTxInputs`] for RBF detection by [`BtcConfirmTask`](super::BtcConfirmTask).
pub struct BtcSignTask {
    signer: Arc<dyn BtcSigner>,
    api: BlockchainApi,
    tx_inputs: Arc<BtcTxInputs>,
}

impl std::fmt::Debug for BtcSignTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtcSignTask")
            .field("address", &self.signer.address())
            .finish_non_exhaustive()
    }
}

impl BtcSignTask {
    pub(crate) fn new(
        signer: Arc<dyn BtcSigner>,
        api: BlockchainApi,
        tx_inputs: Arc<BtcTxInputs>,
    ) -> Self {
        Self {
            signer,
            api,
            tx_inputs,
        }
    }
}

/// Sign timeout: 10 minutes (matching TS SDK).
const SIGN_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(10);

impl ExecutionTask for BtcSignTask {
    fn should_run<'a>(
        &'a self,
        ctx: &'a ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        Box::pin(async move { !ctx.has_committed_transaction() })
    }

    fn run<'a>(
        &'a self,
        ctx: &'a mut ExecutionContext<'_>,
    ) -> Pin<Box<dyn Future<Output = Result<TaskStatus>> + Send + 'a>> {
        Box::pin(async move {
            let action_type = if ctx.is_bridge_execution {
                ExecutionActionType::CrossChain
            } else {
                ExecutionActionType::Swap
            };

            let _action = ctx
                .status_manager
                .find_action(ctx.step, action_type)
                .ok_or(LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionUnprepared,
                    message: "Unable to prepare transaction. Action not found.".to_owned(),
                })?;

            let psbt_hex = ctx
                .step
                .transaction_request
                .as_ref()
                .and_then(|tr| tr.data.as_ref())
                .ok_or_else(|| LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionUnprepared,
                    message: "No PSBT data in transaction request.".to_owned(),
                })?;

            let psbt_bytes = Vec::<u8>::from_hex(psbt_hex).map_err(|e| LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: format!("Invalid PSBT hex: {e}"),
            })?;

            let mut psbt = Psbt::deserialize(&psbt_bytes).map_err(|e| LiFiError::Transaction {
                code: LiFiErrorCode::InternalError,
                message: format!("Failed to deserialize PSBT: {e}"),
            })?;

            prepare_psbt_inputs(&mut psbt, &*self.signer);

            tokio::time::timeout(SIGN_TIMEOUT, self.signer.sign_psbt(&mut psbt))
                .await
                .map_err(|_| LiFiError::Transaction {
                    code: LiFiErrorCode::TransactionExpired,
                    message: "Transaction signing timed out after 10 minutes.".to_owned(),
                })??;

            finalize_psbt(&mut psbt);

            let tx = psbt.extract_tx().map_err(|e| LiFiError::Transaction {
                code: LiFiErrorCode::TransactionFailed,
                message: format!("Failed to extract transaction from PSBT: {e}"),
            })?;

            // Store first input outpoint for RBF detection by BtcConfirmTask
            if let Some(first_input) = tx.input.first() {
                let outpoint = &first_input.previous_output;
                let mut guard = self.tx_inputs.first_input.lock().expect("tx_inputs lock");
                *guard = Some((outpoint.txid.to_string(), outpoint.vout));
            }

            let tx_hex = encode::serialize_hex(&tx);
            let tx_hash = self.api.broadcast_tx(&tx_hex).await?;

            let tx_link = get_tx_link(ctx.from_chain, &tx_hash);

            ctx.status_manager.update_action(
                ctx.step,
                action_type,
                ExecutionActionStatus::Pending,
                Some(ActionUpdateParams {
                    tx_hash: Some(tx_hash),
                    tx_link,
                    signed_at: Some(now_ms()),
                    ..Default::default()
                }),
            )?;

            Ok(TaskStatus::Completed)
        })
    }
}

/// Prepare PSBT inputs for signing based on address type.
///
/// For **Taproot (P2TR)** inputs:
/// - Sets `tap_internal_key` from the signer's x-only public key if missing
/// - Sets `sighash_type` to `SIGHASH_ALL` if unset
///
/// For **P2SH** inputs:
/// - Sets `redeem_script` to a P2WPKH script derived from the signer's public key
fn prepare_psbt_inputs(psbt: &mut Psbt, signer: &dyn BtcSigner) {
    let pubkey = signer.public_key();

    for input in &mut psbt.inputs {
        let address_type = input.witness_utxo.as_ref().and_then(|utxo| {
            Address::from_script(&utxo.script_pubkey, Network::Bitcoin)
                .ok()
                .and_then(|addr| addr.address_type())
        });

        match address_type {
            Some(AddressType::P2tr) => {
                if input.tap_internal_key.is_none() {
                    let (x_only, _parity) = pubkey.0.x_only_public_key();
                    input.tap_internal_key = Some(x_only);
                }
                if input.sighash_type.is_none() {
                    input.sighash_type = Some(bitcoin::psbt::PsbtSighashType::from(
                        bitcoin::sighash::TapSighashType::All,
                    ));
                }
            }
            Some(AddressType::P2sh) if input.redeem_script.is_none() => {
                let wpkh = Address::p2wpkh(&pubkey, Network::Bitcoin);
                input.redeem_script = Some(wpkh.script_pubkey());
            }
            _ => {}
        }
    }
}

/// Finalize all PSBT inputs by setting `final_script_witness` / `final_script_sig`
/// from the partial signatures or taproot key signatures.
///
/// Bitcoin 0.32 does not provide a built-in `finalize_input` method, so we
/// handle the common address types manually (P2WPKH, P2TR key-path, P2SH-P2WPKH).
fn finalize_psbt(psbt: &mut Psbt) {
    for input in &mut psbt.inputs {
        // Already finalized
        if input.final_script_witness.is_some() || input.final_script_sig.is_some() {
            continue;
        }

        // Taproot key-path spend
        if let Some(tap_key_sig) = input.tap_key_sig.take() {
            input.final_script_witness = Some(Witness::from_slice(&[tap_key_sig.serialize()]));
            input.partial_sigs.clear();
            continue;
        }

        // SegWit (P2WPKH) or P2SH-P2WPKH
        if let Some((&pubkey, sig)) = input.partial_sigs.iter().next() {
            let sig_bytes = sig.serialize();
            let pubkey_bytes = pubkey.to_bytes();
            let sig_slice: &[u8] = sig_bytes.as_ref();
            let pub_slice: &[u8] = &pubkey_bytes;
            let witness = Witness::from_slice(&[sig_slice, pub_slice]);

            if let Some(ref redeem_script) = input.redeem_script {
                // P2SH-P2WPKH: push the redeem script as scriptSig
                let mut script_sig = bitcoin::script::Builder::new();
                if let Ok(push) = <&PushBytes>::try_from(redeem_script.as_bytes()) {
                    script_sig = script_sig.push_slice(push);
                }
                input.final_script_sig = Some(script_sig.into_script());
            }

            input.final_script_witness = Some(witness);
            input.partial_sigs.clear();
            input.redeem_script = None;
        }
    }
}
