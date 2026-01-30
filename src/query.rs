use cosmwasm_std::{Addr, Deps, Env, Order, StdResult, Uint128, Timestamp};
use cw_storage_plus::Bound;

use crate::msg::{
    BalanceResponse, CircleResponse, CirclesResponse, CycleResponse, DepositsResponse,
    EventsResponse, MemberStatsResponse, MembersResponse, PayoutsResponse, PenaltiesResponse,
    RefundsResponse, StatusResponse, CircleStatsResponse, MemberLockedAmountResponse,
    BlockedMembersResponse, MemberPseudonymResponse, PrivateMembersResponse,
    DistributionCalendarResponse, ArchivedDateResponse, CalendarRound,
};
use crate::state::{
    Circle, CircleStatus, CIRCLES, DEPOSITS, EVENTS, EVENT_COUNTER, PAYOUTS, PENALTIES, REFUNDS,
    MEMBER_LOCKED_AMOUNTS, BLOCKED_MEMBERS, MEMBER_PSEUDONYMS, PRIVATE_MEMBER_LIST,
};

pub fn query_circle(deps: Deps, _env: Env, circle_id: u64) -> StdResult<CircleResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    Ok(CircleResponse { circle })
}

pub fn query_circles(
    deps: Deps,
    _env: Env,
    start_after: Option<u64>,
    limit: Option<u32>,
    status: Option<CircleStatus>,
    creator: Option<Addr>,
) -> StdResult<CirclesResponse> {
    let limit = limit.unwrap_or(30).min(100) as usize;
    let start = start_after.map(Bound::exclusive);

    let circles: StdResult<Vec<Circle>> = CIRCLES
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, circle) = item?;
            Ok(circle)
        })
        .collect();

    let mut circles = circles?;

    // Filter by status if provided
    if let Some(ref filter_status) = status {
        circles.retain(|c| &c.circle_status == filter_status);
    }

    // Filter by creator if provided
    if let Some(ref filter_creator) = creator {
        circles.retain(|c| &c.creator_address == filter_creator);
    }

    Ok(CirclesResponse { circles })
}

pub fn query_circle_members(deps: Deps, _env: Env, circle_id: u64) -> StdResult<MembersResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    Ok(MembersResponse {
        members: circle.members_list,
        pending_members: circle.pending_members,
    })
}

pub fn query_circle_status(deps: Deps, _env: Env, circle_id: u64) -> StdResult<StatusResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    Ok(StatusResponse {
        status: circle.circle_status,
    })
}

pub fn query_current_cycle(deps: Deps, _env: Env, circle_id: u64) -> StdResult<CycleResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    Ok(CycleResponse {
        current_cycle: circle.current_cycle_index,
        total_cycles: circle.total_cycles,
        next_payout_date: circle.next_payout_date,
        members_paid: circle.members_paid_this_cycle,
        members_late: circle.members_late_this_cycle,
    })
}

pub fn query_cycle_deposits(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    cycle: u32,
) -> StdResult<DepositsResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    let mut deposits = vec![];

    for member in &circle.members_list {
        if let Ok(Some(deposit)) =
            DEPOSITS.may_load(deps.storage, (circle_id, member.clone(), cycle))
        {
            deposits.push(deposit);
        }
    }

    Ok(DepositsResponse { deposits })
}

pub fn query_member_deposits(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    member: Addr,
) -> StdResult<DepositsResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    let mut deposits = vec![];

    for cycle in 1..=circle.total_cycles {
        if let Ok(Some(deposit)) =
            DEPOSITS.may_load(deps.storage, (circle_id, member.clone(), cycle))
        {
            deposits.push(deposit);
        }
    }

    Ok(DepositsResponse { deposits })
}

pub fn query_payouts(deps: Deps, _env: Env, circle_id: u64) -> StdResult<PayoutsResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    let mut payouts = vec![];

    for cycle in 1..=circle.cycles_completed {
        if let Ok(Some(payout)) = PAYOUTS.may_load(deps.storage, (circle_id, cycle)) {
            payouts.push(payout);
        }
    }

    Ok(PayoutsResponse { payouts })
}

pub fn query_payout_history(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    cycle: Option<u32>,
) -> StdResult<PayoutsResponse> {
    if let Some(cycle_num) = cycle {
        let payout = PAYOUTS.may_load(deps.storage, (circle_id, cycle_num))?;
        Ok(PayoutsResponse {
            payouts: payout.into_iter().collect(),
        })
    } else {
        query_payouts(deps, _env, circle_id)
    }
}

pub fn query_circle_balance(deps: Deps, env: Env, circle_id: u64) -> StdResult<BalanceResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    
    // Get actual balance from contract
    let balance = deps
        .querier
        .query_balance(env.contract.address, &circle.denomination)?;
    
    Ok(BalanceResponse {
        balance: balance.amount,
    })
}

pub fn query_member_balance(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    member: Addr,
) -> StdResult<BalanceResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    let mut total_contributed = Uint128::zero();
    let mut total_received = Uint128::zero();

    // Calculate total contributed
    for cycle in 1..=circle.total_cycles {
        if let Ok(Some(deposit)) =
            DEPOSITS.may_load(deps.storage, (circle_id, member.clone(), cycle))
        {
            total_contributed = total_contributed
                .checked_add(deposit.amount)
                .unwrap_or(total_contributed);
        }
    }

    // Calculate total received from payouts
    for cycle in 1..=circle.cycles_completed {
        if let Ok(Some(payout)) = PAYOUTS.may_load(deps.storage, (circle_id, cycle)) {
            if payout.recipient == member {
                total_received = total_received
                    .checked_add(payout.amount)
                    .unwrap_or(total_received);
            }
        }
    }

    // Balance = contributed - received
    let balance = total_contributed
        .checked_sub(total_received)
        .unwrap_or(Uint128::zero());

    Ok(BalanceResponse { balance })
}

pub fn query_penalties(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    member: Option<Addr>,
) -> StdResult<PenaltiesResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    let mut penalties = vec![];

    let members_to_check = if let Some(ref mem) = member {
        vec![mem.clone()]
    } else {
        circle.members_list
    };

    for member_addr in members_to_check {
        for cycle in 1..=circle.total_cycles {
            if let Ok(Some(penalty)) =
                PENALTIES.may_load(deps.storage, (circle_id, member_addr.clone(), cycle))
            {
                penalties.push(penalty);
            }
        }
    }

    Ok(PenaltiesResponse { penalties })
}

pub fn query_refunds(deps: Deps, _env: Env, circle_id: u64) -> StdResult<RefundsResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    let mut refunds = vec![];

    for member in &circle.members_list {
        if let Ok(Some(refund)) = REFUNDS.may_load(deps.storage, (circle_id, member.clone())) {
            refunds.push(refund);
        }
    }

    Ok(RefundsResponse { refunds })
}

pub fn query_events(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    limit: Option<u32>,
) -> StdResult<EventsResponse> {
    let limit = limit.unwrap_or(100).min(1000) as usize;
    let event_count = EVENT_COUNTER
        .may_load(deps.storage, circle_id)?
        .unwrap_or(0);

    let mut events = vec![];
    let start = if event_count > limit as u64 {
        event_count - limit as u64 + 1
    } else {
        1
    };

    for event_id in start..=event_count {
        if let Ok(Some(event)) = EVENTS.may_load(deps.storage, (circle_id, event_id)) {
            events.push(event);
        }
    }

    Ok(EventsResponse { events })
}

pub fn query_circle_stats(deps: Deps, _env: Env, circle_id: u64) -> StdResult<CircleStatsResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    
    let mut total_payouts = Uint128::zero();
    for cycle in 1..=circle.cycles_completed {
        if let Ok(Some(payout)) = PAYOUTS.may_load(deps.storage, (circle_id, cycle)) {
            total_payouts = total_payouts
                .checked_add(payout.amount)
                .unwrap_or(total_payouts);
        }
    }

    Ok(CircleStatsResponse {
        circle_id,
        total_members: circle.members_list.len() as u32,
        total_cycles: circle.total_cycles,
        cycles_completed: circle.cycles_completed,
        total_amount_locked: circle.total_amount_locked,
        total_payouts,
        total_penalties: circle.total_penalties_collected,
        total_platform_fees: circle.total_platform_fees_collected,
    })
}

pub fn query_member_stats(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    member: Addr,
) -> StdResult<MemberStatsResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    
    let mut total_contributed = Uint128::zero();
    let mut total_received = Uint128::zero();
    let mut total_penalties = Uint128::zero();
    let mut missed_payments = 0u32;

    // Calculate stats
    for cycle in 1..=circle.total_cycles {
        // Check deposits
        if let Ok(Some(deposit)) =
            DEPOSITS.may_load(deps.storage, (circle_id, member.clone(), cycle))
        {
            total_contributed = total_contributed
                .checked_add(deposit.amount)
                .unwrap_or(total_contributed);
        } else {
            missed_payments += 1;
        }

        // Check penalties
        if let Ok(Some(penalty)) =
            PENALTIES.may_load(deps.storage, (circle_id, member.clone(), cycle))
        {
            total_penalties = total_penalties
                .checked_add(penalty.amount)
                .unwrap_or(total_penalties);
        }
    }

    // Calculate received
    for cycle in 1..=circle.cycles_completed {
        if let Ok(Some(payout)) = PAYOUTS.may_load(deps.storage, (circle_id, cycle)) {
            if payout.recipient == member {
                total_received = total_received
                    .checked_add(payout.amount)
                    .unwrap_or(total_received);
            }
        }
    }

    Ok(MemberStatsResponse {
        member,
        circles_joined: 1, // This would need to be calculated across all circles
        total_contributed,
        total_received,
        total_penalties,
        missed_payments,
    })
}

pub fn query_member_locked_amount(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    member: Addr,
) -> StdResult<MemberLockedAmountResponse> {
    let locked_amount = MEMBER_LOCKED_AMOUNTS
        .may_load(deps.storage, (circle_id, member))?
        .unwrap_or(Uint128::zero());
    
    Ok(MemberLockedAmountResponse {
        amount: locked_amount,
    })
}

pub fn query_blocked_members(
    deps: Deps,
    _env: Env,
    circle_id: u64,
) -> StdResult<BlockedMembersResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    let mut blocked_members = vec![];
    
    for member in &circle.members_list {
        if let Ok(Some(blocked_from_cycle)) = BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, member.clone())) {
            blocked_members.push((member.clone(), blocked_from_cycle));
        }
    }
    
    Ok(BlockedMembersResponse {
        blocked_members,
    })
}

pub fn query_member_pseudonym(
    deps: Deps,
    _env: Env,
    circle_id: u64,
    member: Addr,
) -> StdResult<MemberPseudonymResponse> {
    let pseudonym = MEMBER_PSEUDONYMS
        .may_load(deps.storage, (circle_id, member))?
        .map(Some)
        .unwrap_or(None);
    
    Ok(MemberPseudonymResponse {
        pseudonym,
    })
}

pub fn query_private_members(
    deps: Deps,
    _env: Env,
    circle_id: u64,
) -> StdResult<PrivateMembersResponse> {
    let members = PRIVATE_MEMBER_LIST
        .may_load(deps.storage, circle_id)?
        .unwrap_or_default();
    
    Ok(PrivateMembersResponse {
        members,
    })
}

pub fn query_distribution_calendar(
    deps: Deps,
    _env: Env,
    circle_id: u64,
) -> StdResult<DistributionCalendarResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    
    let start_timestamp = circle.start_date.ok_or_else(|| {
        cosmwasm_std::StdError::generic_err("Circle has not started yet")
    })?;
    
    let mut rounds = vec![];
    
    if let Some(payout_order) = &circle.payout_order_list {
        let mut round_number = 1u32;
        for cycle in 1..=circle.total_cycles {
            for recipient in payout_order.iter() {
                // Calculate dates for this round
                let round_offset_seconds = ((round_number - 1) * circle.cycle_duration_days as u32) * 86400;
                let deposit_deadline = Timestamp::from_seconds(
                    start_timestamp.seconds() + round_offset_seconds as u64
                );
                let distribution_date = Timestamp::from_seconds(
                    start_timestamp.seconds() + round_offset_seconds as u64 + (circle.cycle_duration_days as u64 * 86400)
                );
                
                rounds.push(CalendarRound {
                    round_number,
                    cycle_number: cycle,
                    deposit_deadline,
                    distribution_date,
                    recipient: Some(recipient.clone()),
                });
                
                round_number += 1;
            }
        }
    }
    
    Ok(DistributionCalendarResponse {
        rounds,
    })
}

pub fn query_archived_date(
    deps: Deps,
    _env: Env,
    circle_id: u64,
) -> StdResult<ArchivedDateResponse> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;
    
    let archived_date = if let (Some(end_date), grace_period_hours) = (circle.end_date, circle.grace_period_hours) {
        Some(Timestamp::from_seconds(
            end_date.seconds() + (grace_period_hours as u64 * 3600)
        ))
    } else {
        None
    };
    
    Ok(ArchivedDateResponse {
        archived_date,
    })
}

