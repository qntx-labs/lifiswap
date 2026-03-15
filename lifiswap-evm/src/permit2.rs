//! Permit2 support for gasless token approvals.
//!
//! Implements the Uniswap Permit2 signature-based approval flow used by the
//! LI.FI `Permit2Proxy` contract. Supports both:
//! - **Permit2** (`PermitTransferFrom` EIP-712 signing)
//! - **Native EIP-2612** (`permit` signature with v/r/s)

use std::time::{SystemTime, UNIX_EPOCH};

use alloy::primitives::{Address, Bytes, U256};
use alloy::sol;
use alloy::sol_types::SolCall as _;

sol! {
    struct TokenPermissions {
        address token;
        uint256 amount;
    }

    struct PermitTransferFromData {
        TokenPermissions permitted;
        uint256 nonce;
        uint256 deadline;
    }

    #[sol(rpc)]
    contract IPermit2Proxy {
        function callDiamondWithPermit2(
            bytes calldata diamondCalldata,
            PermitTransferFromData permit,
            bytes calldata signature
        ) external payable returns (bytes);

        function callDiamondWithEIP2612Signature(
            address token,
            uint256 amount,
            uint256 deadline,
            uint8 v,
            bytes32 r,
            bytes32 s,
            bytes calldata diamondCalldata
        ) external payable returns (bytes);

        function callDiamondWithPermit2Witness(
            bytes calldata diamondCalldata,
            address owner,
            PermitTransferFromData permit,
            bytes calldata signature
        ) external payable returns (bytes);

        function nextNonce(address owner) external view returns (uint256);
    }
}

/// Parameters for a Permit2 `PermitTransferFrom` message.
#[derive(Debug, Clone, Copy)]
pub struct PermitTransferFrom {
    /// ERC-20 token address being permitted.
    pub token: Address,
    /// Amount of tokens permitted.
    pub amount: U256,
    /// Spender address (typically the `Permit2Proxy` contract).
    pub spender: Address,
    /// Unique nonce from the `Permit2Proxy` contract.
    pub nonce: U256,
    /// Unix timestamp deadline for the permit.
    pub deadline: U256,
}

/// Build EIP-712 typed data JSON for a `PermitTransferFrom` message.
///
/// The returned [`lifiswap::types::TypedData`] can be signed via
/// [`EvmSigner::sign_typed_data`](crate::signer::EvmSigner::sign_typed_data).
#[must_use]
pub fn build_permit2_typed_data(
    permit: &PermitTransferFrom,
    permit2_address: Address,
    chain_id: u64,
) -> lifiswap::types::TypedData {
    let domain = serde_json::json!({
        "name": "Permit2",
        "chainId": chain_id,
        "verifyingContract": format!("{permit2_address:#x}"),
    });

    let types = serde_json::json!({
        "EIP712Domain": [
            { "name": "name", "type": "string" },
            { "name": "chainId", "type": "uint256" },
            { "name": "verifyingContract", "type": "address" }
        ],
        "TokenPermissions": [
            { "name": "token", "type": "address" },
            { "name": "amount", "type": "uint256" }
        ],
        "PermitTransferFrom": [
            { "name": "permitted", "type": "TokenPermissions" },
            { "name": "spender", "type": "address" },
            { "name": "nonce", "type": "uint256" },
            { "name": "deadline", "type": "uint256" }
        ]
    });

    let message = serde_json::json!({
        "permitted": {
            "token": format!("{:#x}", permit.token),
            "amount": permit.amount.to_string(),
        },
        "spender": format!("{:#x}", permit.spender),
        "nonce": permit.nonce.to_string(),
        "deadline": permit.deadline.to_string(),
    });

    lifiswap::types::TypedData {
        domain: Some(domain),
        types: Some(types),
        primary_type: Some("PermitTransferFrom".to_owned()),
        message: Some(message),
    }
}

/// Encode the `callDiamondWithPermit2` calldata.
///
/// Wraps the original `diamond_calldata` with a Permit2 signature so the
/// `Permit2Proxy` can transfer tokens on behalf of the user.
#[must_use]
pub fn encode_permit2_calldata(
    diamond_calldata: &[u8],
    permit: &PermitTransferFrom,
    signature: &[u8],
) -> Bytes {
    let permit_data = PermitTransferFromData {
        permitted: TokenPermissions {
            token: permit.token,
            amount: permit.amount,
        },
        nonce: permit.nonce,
        deadline: permit.deadline,
    };
    let call = IPermit2Proxy::callDiamondWithPermit2Call {
        diamondCalldata: Bytes::from(diamond_calldata.to_vec()),
        permit: permit_data,
        signature: Bytes::from(signature.to_vec()),
    };
    Bytes::from(call.abi_encode())
}

/// Encode the `callDiamondWithEIP2612Signature` calldata.
///
/// Wraps the original `diamond_calldata` with a native EIP-2612 permit signature.
#[must_use]
pub fn encode_native_permit_calldata(
    token: Address,
    amount: U256,
    deadline: U256,
    v: u8,
    r: [u8; 32],
    s: [u8; 32],
    diamond_calldata: &[u8],
) -> Bytes {
    let call = IPermit2Proxy::callDiamondWithEIP2612SignatureCall {
        token,
        amount,
        deadline,
        v,
        r: r.into(),
        s: s.into(),
        diamondCalldata: Bytes::from(diamond_calldata.to_vec()),
    };
    Bytes::from(call.abi_encode())
}

/// Get the next nonce for a Permit2 transfer from the `Permit2Proxy` contract.
///
/// # Errors
///
/// Returns [`LiFiError::Provider`](lifiswap::error::LiFiError::Provider) if the
/// on-chain call fails.
pub async fn fetch_next_nonce(
    rpc_url: &url::Url,
    permit2_proxy: Address,
    owner: Address,
) -> lifiswap::error::Result<U256> {
    let provider = alloy::providers::ProviderBuilder::new().connect_http(rpc_url.clone());
    let contract = IPermit2Proxy::new(permit2_proxy, &provider);

    contract
        .nextNonce(owner)
        .call()
        .await
        .map_err(|e| lifiswap::error::LiFiError::Provider {
            code: lifiswap::error::LiFiErrorCode::ProviderUnavailable,
            message: format!("Failed to fetch Permit2 nonce: {e}"),
        })
}

/// Compute a 30-minute deadline from now (unix timestamp).
#[must_use]
pub fn default_deadline() -> U256 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    U256::from(now + 30 * 60)
}
