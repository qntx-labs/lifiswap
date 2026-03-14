//! Shared test helpers for execution module tests.

use crate::types::{
    Action, ChainId, LiFiStep, LiFiStepExtended, RouteBase, RouteExtended, StepType, Token,
};

pub fn dummy_token() -> Token {
    Token {
        address: "0x0".to_owned(),
        decimals: 18,
        symbol: "TST".to_owned(),
        chain_id: ChainId(1),
        coin_key: None,
        name: "Test".to_owned(),
        logo_uri: None,
        price_usd: None,
    }
}

pub fn dummy_step(id: &str) -> LiFiStepExtended {
    LiFiStepExtended {
        step: LiFiStep {
            id: id.to_owned(),
            step_type: StepType::Swap,
            tool: None,
            tool_details: None,
            action: Action {
                from_chain_id: ChainId(1),
                to_chain_id: ChainId(1),
                from_token: dummy_token(),
                to_token: dummy_token(),
                from_amount: None,
                from_address: None,
                to_address: None,
                slippage: None,
                destination_call_data: None,
            },
            estimate: None,
            included_steps: None,
            integrator: None,
            transaction_request: None,
            execution: None,
            typed_data: None,
            insurance: None,
        },
        execution: None,
    }
}

pub fn dummy_route(id: &str) -> RouteExtended {
    RouteExtended {
        base: RouteBase {
            id: id.to_owned(),
            from_chain_id: ChainId(1),
            to_chain_id: ChainId(137),
            from_amount: "1000".to_owned(),
            to_amount: "999".to_owned(),
            from_amount_usd: None,
            to_amount_usd: None,
            to_amount_min: None,
            from_token: dummy_token(),
            to_token: dummy_token(),
            from_address: None,
            to_address: None,
            tags: None,
            insurance: None,
            gas_cost_usd: None,
        },
        steps: vec![dummy_step("step-1")],
    }
}
