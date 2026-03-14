//! Human-readable messages for execution actions and substatuses.
//!
//! Mirrors the TypeScript SDK's `actionMessages.ts`.

use crate::types::{ExecutionActionStatus, ExecutionActionType};

/// Get a human-readable message for an action type + status combination.
#[must_use]
pub fn get_action_message(
    action_type: ExecutionActionType,
    status: ExecutionActionStatus,
) -> Option<&'static str> {
    use ExecutionActionStatus as S;
    use ExecutionActionType as T;

    match (action_type, status) {
        (T::CheckAllowance, S::Started) => Some("Checking token allowance"),
        (T::CheckAllowance, S::Pending) => Some("Waiting for token allowance check"),
        (T::CheckAllowance, S::Done) => Some("Token allowance checked"),

        (T::ResetAllowance, S::Started) => Some("Resetting token allowance"),
        (T::ResetAllowance, S::ResetRequired) => Some("Resetting token allowance"),
        (T::ResetAllowance, S::Pending) => Some("Waiting for token allowance reset"),
        (T::ResetAllowance, S::Done) => Some("Token allowance reset"),

        (T::SetAllowance, S::Started) => Some("Setting token allowance"),
        (T::SetAllowance, S::ActionRequired) => Some("Set token allowance"),
        (T::SetAllowance, S::Pending) => Some("Waiting for token allowance"),
        (T::SetAllowance, S::Done) => Some("Token allowance set"),

        (T::Swap, S::Started) => Some("Preparing swap transaction"),
        (T::Swap, S::ActionRequired) => Some("Sign swap transaction"),
        (T::Swap, S::MessageRequired) => Some("Sign swap message"),
        (T::Swap, S::Pending) => Some("Waiting for swap transaction"),
        (T::Swap, S::Done) => Some("Swap completed"),

        (T::CrossChain, S::Started) => Some("Preparing bridge transaction"),
        (T::CrossChain, S::ActionRequired) => Some("Sign bridge transaction"),
        (T::CrossChain, S::MessageRequired) => Some("Sign bridge message"),
        (T::CrossChain, S::Pending) => Some("Waiting for bridge transaction"),
        (T::CrossChain, S::Done) => Some("Bridge transaction confirmed"),

        (T::ReceivingChain, S::Started) => Some("Waiting for destination chain"),
        (T::ReceivingChain, S::Pending) => Some("Waiting for destination chain"),
        (T::ReceivingChain, S::Done) => Some("Bridge completed"),

        (T::Permit | T::NativePermit, S::Started) => Some("Preparing transaction"),
        (T::Permit | T::NativePermit, S::ActionRequired) => Some("Sign permit message"),
        (T::Permit | T::NativePermit, S::Pending) => Some("Waiting for permit message"),
        (T::Permit | T::NativePermit, S::Done) => Some("Permit message signed"),

        _ => None,
    }
}

/// Get a human-readable message for a substatus within a status response.
#[must_use]
pub fn get_substatus_message(status: &str, substatus: Option<&str>) -> Option<&'static str> {
    let sub = substatus?;
    match (status, sub) {
        ("PENDING", "BRIDGE_NOT_AVAILABLE") => {
            Some("Bridge communication is temporarily unavailable.")
        }
        ("PENDING", "CHAIN_NOT_AVAILABLE") => Some("RPC communication is temporarily unavailable."),
        ("PENDING", "UNKNOWN_ERROR") => Some(
            "An unexpected error occurred. Please seek assistance in the LI.FI discord server.",
        ),
        ("PENDING", "WAIT_SOURCE_CONFIRMATIONS") => Some(
            "The bridge deposit has been received. The bridge is waiting for more confirmations to start the off-chain logic.",
        ),
        ("PENDING", "WAIT_DESTINATION_TRANSACTION") => Some(
            "The bridge off-chain logic is being executed. Wait for the transaction to appear on the destination chain.",
        ),
        ("DONE", "PARTIAL") => {
            Some("Some of the received tokens are not the requested destination tokens.")
        }
        ("DONE", "REFUNDED") => Some("The tokens were refunded to the sender address."),
        ("DONE", "COMPLETED") => Some("The transfer is complete."),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_message_lookup() {
        assert_eq!(
            get_action_message(ExecutionActionType::Swap, ExecutionActionStatus::Started),
            Some("Preparing swap transaction")
        );
        assert_eq!(
            get_action_message(ExecutionActionType::CrossChain, ExecutionActionStatus::Done),
            Some("Bridge transaction confirmed")
        );
        assert_eq!(
            get_action_message(
                ExecutionActionType::Permit,
                ExecutionActionStatus::ActionRequired
            ),
            Some("Sign permit message")
        );
        assert_eq!(
            get_action_message(ExecutionActionType::Swap, ExecutionActionStatus::Cancelled),
            None
        );
    }

    #[test]
    fn substatus_message_lookup() {
        assert_eq!(
            get_substatus_message("PENDING", Some("WAIT_DESTINATION_TRANSACTION")),
            Some(
                "The bridge off-chain logic is being executed. Wait for the transaction to appear on the destination chain."
            )
        );
        assert_eq!(
            get_substatus_message("DONE", Some("COMPLETED")),
            Some("The transfer is complete.")
        );
        assert_eq!(get_substatus_message("DONE", None), None);
        assert_eq!(get_substatus_message("FAILED", Some("UNKNOWN")), None);
    }
}
