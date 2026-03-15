//! Solana chain provider implementation.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lifiswap::error::{LiFiError, LiFiErrorCode, Result};
use lifiswap::provider::{Provider, StepExecutor};
use lifiswap::types::{ChainType, StepExecutorOptions, Token, TokenAmount};
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

use crate::executor::SvmStepExecutor;
use crate::rpc::RpcPool;
use crate::signer::SvmSigner;

/// Solana system program address (used as native SOL token address).
const SOL_NATIVE_MINT: Pubkey = pubkey!("11111111111111111111111111111111");

/// SPL Token program ID.
const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// SPL Token-2022 program ID.
const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

/// Associated Token Account program ID.
const ATA_PROGRAM_ID: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// Solana chain provider using [`solana-sdk`] for on-chain interactions.
///
/// Handles address validation, balance queries (SOL + SPL tokens), SNS name
/// resolution, and creates [`SvmStepExecutor`] instances for step execution.
///
/// The signing backend is abstracted via [`SvmSigner`], allowing different
/// backends (local keypair, hardware wallet, etc.).
///
/// # Example
///
/// ```ignore
/// use lifiswap_svm::{SvmProvider, KeypairSigner};
/// use solana_sdk::signature::Keypair;
///
/// let keypair = Keypair::new();
/// let signer = KeypairSigner::new(keypair);
/// let rpc_url: url::Url = "https://api.mainnet-beta.solana.com".parse().unwrap();
/// let provider = SvmProvider::new(signer, &rpc_url);
/// ```
#[derive(Clone)]
pub struct SvmProvider {
    signer: Arc<dyn SvmSigner>,
    rpc_pool: RpcPool,
    skip_simulation: bool,
}

impl std::fmt::Debug for SvmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvmProvider")
            .field("pubkey", &self.signer.pubkey())
            .field("rpc_count", &self.rpc_pool.len())
            .field("skip_simulation", &self.skip_simulation)
            .finish_non_exhaustive()
    }
}

impl SvmProvider {
    /// Create a new Solana provider with the given signer and RPC URL.
    #[must_use]
    pub fn new(signer: impl SvmSigner, rpc_url: &url::Url) -> Self {
        Self {
            signer: Arc::new(signer),
            rpc_pool: RpcPool::from_single(rpc_url),
            skip_simulation: false,
        }
    }

    /// Create a provider with multiple RPC endpoints for redundancy.
    ///
    /// # Errors
    ///
    /// Returns an error if no RPC URLs are provided.
    pub fn with_rpc_urls(signer: impl SvmSigner, rpc_urls: &[url::Url]) -> Result<Self> {
        Ok(Self {
            signer: Arc::new(signer),
            rpc_pool: RpcPool::new(rpc_urls)?,
            skip_simulation: false,
        })
    }

    /// Skip transaction simulation before sending.
    ///
    /// By default, transactions are simulated before being broadcast.
    /// Disabling simulation can speed up execution but removes an
    /// early error detection step.
    #[must_use]
    pub const fn with_skip_simulation(mut self) -> Self {
        self.skip_simulation = true;
        self
    }

    /// Returns the wallet public key derived from the signer.
    #[must_use]
    pub fn pubkey(&self) -> Pubkey {
        self.signer.pubkey()
    }
}

impl Provider for SvmProvider {
    fn chain_type(&self) -> ChainType {
        ChainType::SVM
    }

    fn is_address(&self, address: &str) -> bool {
        address.parse::<Pubkey>().is_ok()
    }

    fn resolve_address<'a>(
        &'a self,
        name: &'a str,
        _chain_id: Option<u64>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + 'a>> {
        Box::pin(async move { resolve_sns_address(name).await })
    }

    fn get_balance<'a>(
        &'a self,
        wallet_address: &'a str,
        tokens: &'a [Token],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<TokenAmount>>> + Send + 'a>> {
        Box::pin(async move {
            if tokens.is_empty() {
                return Ok(vec![]);
            }

            let owner: Pubkey = wallet_address.parse().map_err(|_| {
                LiFiError::Validation(format!("Invalid Solana address: {wallet_address}"))
            })?;

            let sol_balance = self
                .rpc_pool
                .call_with_retry(move |rpc| async move {
                    rpc.get_balance(&owner)
                        .await
                        .map_err(|e| LiFiError::Provider {
                            code: LiFiErrorCode::ProviderUnavailable,
                            message: format!("Failed to fetch SOL balance: {e}"),
                        })
                })
                .await?;

            let block_number = self
                .rpc_pool
                .call_with_retry(|rpc| async move {
                    rpc.get_slot().await.map_err(|e| LiFiError::Provider {
                        code: LiFiErrorCode::ProviderUnavailable,
                        message: format!("Failed to fetch slot: {e}"),
                    })
                })
                .await
                .ok();

            let mut results = Vec::with_capacity(tokens.len());
            for token in tokens {
                let mint: Option<Pubkey> = token.address.parse().ok();
                let is_native = mint.as_ref() == Some(&SOL_NATIVE_MINT);

                if is_native {
                    results.push(TokenAmount {
                        token: token.clone(),
                        amount: Some(sol_balance.to_string()),
                        block_number,
                    });
                    continue;
                }

                let amount = if let Some(mint) = mint {
                    get_spl_token_balance(&self.rpc_pool, &owner, &mint).await
                } else {
                    None
                };
                results.push(TokenAmount {
                    token: token.clone(),
                    amount,
                    block_number,
                });
            }

            Ok(results)
        })
    }

    fn create_step_executor<'a>(
        &'a self,
        options: StepExecutorOptions,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn StepExecutor>>> + Send + 'a>> {
        Box::pin(async move {
            let executor: Box<dyn StepExecutor> = Box::new(SvmStepExecutor::new(
                Arc::clone(&self.signer),
                self.rpc_pool.clone(),
                options,
                self.skip_simulation,
            ));
            Ok(executor)
        })
    }
}

/// Derive the Associated Token Account (ATA) address for an owner + mint.
///
/// Layout: `[owner, token_program, mint]` seeded against the ATA program.
fn derive_ata(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    let (ata, _) = Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ATA_PROGRAM_ID,
    );
    ata
}

/// SPL Token Account data layout offset for the `amount` field.
/// Layout: mint (32) + owner (32) + amount (8) = offset 64.
const TOKEN_AMOUNT_OFFSET: usize = 64;
const TOKEN_AMOUNT_END: usize = TOKEN_AMOUNT_OFFSET + 8;

/// Get SPL token balance for a specific mint by checking the ATA.
///
/// Tries standard Token program first, then Token-2022. Returns `None`
/// if the account doesn't exist or parsing fails.
async fn get_spl_token_balance(
    rpc_pool: &RpcPool,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Option<String> {
    // Try standard Token program ATA, then Token-2022
    for program in &[TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID] {
        let ata = derive_ata(owner, mint, program);
        let account = rpc_pool
            .call_with_retry(move |rpc| async move {
                rpc.get_account(&ata)
                    .await
                    .map_err(|e| LiFiError::Provider {
                        code: LiFiErrorCode::ProviderUnavailable,
                        message: format!("Failed to fetch token account: {e}"),
                    })
            })
            .await
            .ok()?;

        if account.data.len() >= TOKEN_AMOUNT_END {
            let amount = u64::from_le_bytes(
                account.data[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_END]
                    .try_into()
                    .ok()?,
            );
            return Some(amount.to_string());
        }
    }

    None
}

#[derive(serde::Deserialize)]
struct SnsResult {
    #[serde(rename = "s")]
    status: String,
    result: String,
}

/// Resolve a `.sol` domain name via the Bonfida SNS SDK proxy.
///
/// Returns `Ok(None)` if the name doesn't end with `.sol` or cannot be resolved.
async fn resolve_sns_address(name: &str) -> Result<Option<String>> {
    if !name.to_ascii_lowercase().ends_with(".sol") {
        return Ok(None);
    }

    let url = format!("https://sns-sdk-proxy.bonfida.workers.dev/resolve/{name}");

    let response = reqwest::get(&url).await.map_err(|e| LiFiError::Provider {
        code: LiFiErrorCode::ProviderUnavailable,
        message: format!("SNS resolution request failed: {e}"),
    })?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let data: SnsResult = match response.json().await {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    if data.status != "ok" {
        return Ok(None);
    }

    // Validate the result is a valid Solana address
    if data.result.parse::<Pubkey>().is_err() {
        return Ok(None);
    }

    Ok(Some(data.result))
}
