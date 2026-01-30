use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::query::{
    query_circle, query_circle_balance, query_circle_members, query_circle_stats,
    query_circle_status, query_current_cycle, query_cycle_deposits, query_events,
    query_member_balance, query_member_deposits, query_member_stats, query_payout_history,
    query_payouts, query_penalties, query_refunds, query_circles, query_member_locked_amount,
    query_blocked_members, query_member_pseudonym, query_private_members,
    query_distribution_calendar, query_archived_date,
};
use crate::state::PlatformConfig;

const CONTRACT_NAME: &str = "crates.io:safrimba-contract";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate platform address
    let platform_address = deps.api.addr_validate(msg.platform_address.as_str())?;

    // Store platform configuration
    let platform_config = PlatformConfig {
        platform_fee_percent: msg.platform_fee_percent,
        platform_address: platform_address.clone(),
    };
    crate::state::PLATFORM_CONFIG.save(deps.storage, &platform_config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("platform_address", platform_address.to_string())
        .add_attribute("platform_fee_percent", msg.platform_fee_percent.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    crate::execute::execute(deps, env, info, msg)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCircle { circle_id } => {
            cosmwasm_std::to_json_binary(&query_circle(deps, env, circle_id)?)
        }
        QueryMsg::GetCircles {
            start_after,
            limit,
            status,
            creator,
        } => cosmwasm_std::to_json_binary(&query_circles(
            deps, env, start_after, limit, status, creator,
        )?),
        QueryMsg::GetCircleMembers { circle_id } => {
            cosmwasm_std::to_json_binary(&query_circle_members(deps, env, circle_id)?)
        }
        QueryMsg::GetCircleStatus { circle_id } => {
            cosmwasm_std::to_json_binary(&query_circle_status(deps, env, circle_id)?)
        }
        QueryMsg::GetCurrentCycle { circle_id } => {
            cosmwasm_std::to_json_binary(&query_current_cycle(deps, env, circle_id)?)
        }
        QueryMsg::GetCycleDeposits { circle_id, cycle } => {
            cosmwasm_std::to_json_binary(&query_cycle_deposits(deps, env, circle_id, cycle)?)
        }
        QueryMsg::GetMemberDeposits { circle_id, member } => {
            cosmwasm_std::to_json_binary(&query_member_deposits(deps, env, circle_id, member)?)
        }
        QueryMsg::GetPayouts { circle_id } => {
            cosmwasm_std::to_json_binary(&query_payouts(deps, env, circle_id)?)
        }
        QueryMsg::GetPayoutHistory { circle_id, cycle } => {
            cosmwasm_std::to_json_binary(&query_payout_history(deps, env, circle_id, cycle)?)
        }
        QueryMsg::GetCircleBalance { circle_id } => {
            cosmwasm_std::to_json_binary(&query_circle_balance(deps, env, circle_id)?)
        }
        QueryMsg::GetMemberBalance { circle_id, member } => {
            cosmwasm_std::to_json_binary(&query_member_balance(deps, env, circle_id, member)?)
        }
        QueryMsg::GetPenalties { circle_id, member } => {
            cosmwasm_std::to_json_binary(&query_penalties(deps, env, circle_id, member)?)
        }
        QueryMsg::GetRefunds { circle_id } => {
            cosmwasm_std::to_json_binary(&query_refunds(deps, env, circle_id)?)
        }
        QueryMsg::GetEvents { circle_id, limit } => {
            cosmwasm_std::to_json_binary(&query_events(deps, env, circle_id, limit)?)
        }
        QueryMsg::GetCircleStats { circle_id } => {
            cosmwasm_std::to_json_binary(&query_circle_stats(deps, env, circle_id)?)
        }
        QueryMsg::GetMemberStats { circle_id, member } => {
            cosmwasm_std::to_json_binary(&query_member_stats(deps, env, circle_id, member)?)
        }
        QueryMsg::GetMemberLockedAmount { circle_id, member } => {
            cosmwasm_std::to_json_binary(&query_member_locked_amount(deps, env, circle_id, member)?)
        }
        QueryMsg::GetBlockedMembers { circle_id } => {
            cosmwasm_std::to_json_binary(&query_blocked_members(deps, env, circle_id)?)
        }
        QueryMsg::GetMemberPseudonym { circle_id, member } => {
            cosmwasm_std::to_json_binary(&query_member_pseudonym(deps, env, circle_id, member)?)
        }
        QueryMsg::GetPrivateMembers { circle_id } => {
            cosmwasm_std::to_json_binary(&query_private_members(deps, env, circle_id)?)
        }
        QueryMsg::GetDistributionCalendar { circle_id } => {
            cosmwasm_std::to_json_binary(&query_distribution_calendar(deps, env, circle_id)?)
        }
        QueryMsg::GetArchivedDate { circle_id } => {
            cosmwasm_std::to_json_binary(&query_archived_date(deps, env, circle_id)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Addr};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &coins(1000, "saf"));

        let msg = InstantiateMsg {
            platform_fee_percent: 100, // 1%
            platform_address: Addr::unchecked("platform"),
        };

        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }
}

