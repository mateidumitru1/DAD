use cosmwasm_schema::{cw_serde, QueryResponses};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use cosmwasm_std::{Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    pub count: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ExecuteMsg {
    Increment {},
    Reset { count: i32 },
    Stake { amount: Uint128 },
    Unstake { amount: Uint128 }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(GetCountResponse)]
    GetCount {},

    #[returns(GetStakeResponse)]
    GetStake { address: String },
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetCountResponse {
    pub count: i32,
}

#[cw_serde]
pub struct GetStakeResponse {
    pub amount: Uint128,
}
