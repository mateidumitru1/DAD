#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetCountResponse, GetStakeResponse, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE, STAKES};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:staking_contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => execute::increment(deps),
        ExecuteMsg::Reset { count } => execute::reset(deps, info, count),
        ExecuteMsg::Stake { amount } => execute::stake(deps, info, amount),
        ExecuteMsg::Unstake { amount } => execute::unstake(deps, info, amount),
    }
}

pub mod execute {
    use super::*;

    pub fn increment(deps: DepsMut) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            state.count += 1;
            Ok(state)
        })?;

        Ok(Response::new().add_attribute("action", "increment"))
    }

    pub fn reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
        STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
            if info.sender != state.owner {
                return Err(ContractError::Unauthorized {});
            }
            state.count = count;
            Ok(state)
        })?;
        Ok(Response::new().add_attribute("action", "reset"))
    }

    pub fn stake(deps: DepsMut, info: MessageInfo, amount: Uint128) -> Result<Response, ContractError> {
        if info.funds.is_empty() || info.funds[0].amount < amount {
            return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
                "Insufficient funds sent for staking",
            )));
        }

        if amount.is_zero() {
            return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
                "Stake amount must be greater than zero",
            )));
        }
    
        STAKES.update(deps.storage, &info.sender, |balance| -> StdResult<_> {
            Ok(balance.unwrap_or(Uint128::zero()) + amount)
        })?;
    
        Ok(Response::new()
            .add_attribute("action", "stake")
            .add_attribute("staker", info.sender)
            .add_attribute("amount", amount.to_string()))
    }
    

    pub fn unstake(deps: DepsMut, info: MessageInfo, amount: Uint128) -> Result<Response, ContractError> {
        let sender = info.sender.clone();
    
        let current_stake = STAKES.may_load(deps.storage, &sender)?.unwrap_or(Uint128::zero());
    
        if amount > current_stake {
            return Err(ContractError::Std(cosmwasm_std::StdError::generic_err(
                "Cannot unstake more than your current balance",
            )));
        }
    
        let new_stake = current_stake - amount;
        
        if new_stake.is_zero() {
            STAKES.remove(deps.storage, &sender);
        } else {
            STAKES.save(deps.storage, &sender, &new_stake)?;
        }
    
        let bank_msg = cosmwasm_std::BankMsg::Send {
            to_address: sender.to_string(),
            amount: vec![cosmwasm_std::Coin {
                denom: "token".to_string(),
                amount,
            }],
        };
    
        Ok(Response::new()
            .add_attribute("action", "unstake")
            .add_attribute("staker", sender)
            .add_attribute("amount", amount.to_string())
            .add_message(bank_msg))
    }
    
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_json_binary(&query::count(deps)?),
        QueryMsg::GetStake { address } => to_json_binary(&query::stake(deps, address)?),
    }
}

pub mod query {
    use super::*;

    pub fn count(deps: Deps) -> StdResult<GetCountResponse> {
        let state = STATE.load(deps.storage)?;
        Ok(GetCountResponse { count: state.count })
    }

    pub fn stake(deps: Deps, address: String) -> StdResult<GetStakeResponse> {
        let addr = deps.api.addr_validate(&address)?;
        let amount = STAKES.may_load(deps.storage, &addr)?.unwrap_or(Uint128::zero());
        Ok(GetStakeResponse { amount })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_json};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_json(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_json(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_json(&res).unwrap();
        assert_eq!(5, value.count);
    }

    #[test]
    fn stake_tokens() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 0 };
        let info = mock_info("creator", &coins(1000, "token"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        
        let staker = mock_info(deps.api.addr_make("staker1").as_str(), &coins(500, "token"));
        let msg = ExecuteMsg::Stake { amount: Uint128::new(500) };
        execute(deps.as_mut(), mock_env(), staker.clone(), msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStake { address: staker.sender.to_string() }).unwrap();
        let value: GetStakeResponse = from_json(&res).unwrap();
        assert_eq!(value.amount, Uint128::new(500));
    }

    #[test]
    fn unstake_tokens() {
        let mut deps = mock_dependencies();
    
        let msg = InstantiateMsg { count: 0 };
        let info = mock_info("creator", &coins(1000, "token"));
        instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    
        let staker_addr = deps.api.addr_make("staker1");  // ← Creăm o adresă Bech32 validă
        let staker = mock_info(staker_addr.as_str(), &coins(500, "token"));
    
        let msg = ExecuteMsg::Stake { amount: Uint128::new(500) };
        execute(deps.as_mut(), mock_env(), staker.clone(), msg).unwrap();
    
        let msg = ExecuteMsg::Unstake { amount: Uint128::new(300) };
        execute(deps.as_mut(), mock_env(), staker.clone(), msg).unwrap();
    
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetStake { address: staker_addr.to_string() }).unwrap();
        let value: GetStakeResponse = from_json(&res).unwrap();
        assert_eq!(value.amount, Uint128::new(200));
    }
    
    #[test]
    fn stake_without_funds_should_fail() {
        let mut deps = mock_dependencies();
        
        let msg = InstantiateMsg { count: 0 };
        let info = mock_info("creator", &coins(1000, "token"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        
        let msg = ExecuteMsg::Stake { amount: Uint128::new(500) };
        let err = execute(deps.as_mut(), mock_env(), mock_info("staker1", &[]), msg).unwrap_err();

        assert!(format!("{:?}", err).contains("Insufficient funds sent for staking"));
    }

    #[test]
    fn unstake_more_than_staked_should_fail() {
        let mut deps = mock_dependencies();
        
        let msg = InstantiateMsg { count: 0 };
        let info = mock_info("creator", &coins(1000, "token"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        
        let staker = mock_info("staker1", &coins(500, "token"));
        let stake_msg = ExecuteMsg::Stake { amount: Uint128::new(500) };
        execute(deps.as_mut(), mock_env(), staker.clone(), stake_msg).unwrap();
        
        let unstake_msg = ExecuteMsg::Unstake { amount: Uint128::new(1000) }; // Trying to unstake more than staked
        let err = execute(deps.as_mut(), mock_env(), staker, unstake_msg).unwrap_err();
        
        assert_eq!(err, ContractError::Std(cosmwasm_std::StdError::generic_err("Cannot unstake more than your current balance")));
    }

    #[test]
    fn unstake_full_balance_should_leave_zero() {
        let mut deps = mock_dependencies();
        
        let msg = InstantiateMsg { count: 0 };
        let info = mock_info("creator", &coins(1000, "token"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        
        let staker_addr = deps.api.addr_make("staker1");
        let staker = mock_info(staker_addr.as_str(), &coins(500, "token")); 
    
        let stake_msg = ExecuteMsg::Stake { amount: Uint128::new(500) };
        execute(deps.as_mut(), mock_env(), staker.clone(), stake_msg).unwrap();
        
        let unstake_msg = ExecuteMsg::Unstake { amount: Uint128::new(500) };
        execute(deps.as_mut(), mock_env(), staker.clone(), unstake_msg).unwrap();
        
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetStake { address: staker_addr.as_str().to_string() }
        ).unwrap();
    
        let value: GetStakeResponse = from_json(&res).unwrap();
    
        assert_eq!(value.amount, Uint128::zero());
    }
    

}
