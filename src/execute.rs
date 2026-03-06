use cosmwasm_std::{
    Addr, BankMsg, Coin, CosmosMsg, DepsMut, DistributionMsg, Env, MessageInfo, Order, Response,
    StdResult, StakingMsg, Storage, Timestamp, Uint128,
};
use cw_utils::must_pay;

use crate::error::ContractError;
use crate::msg::ExecuteMsg;
use crate::state::{
    Circle, CircleStakingConfig, CircleStatus, DepositRecord, DistributionThreshold, EventLog,
    MemberMissedPayments, PayoutOrderType, PayoutRecord, PendingRefundRecord, PendingUndelegation,
    PenaltyRecord, RefundMode, Visibility, BLOCKED_MEMBERS, CIRCLE_COUNTER, CIRCLES, CIRCLE_STAKING,
    DEPOSITS, EVENTS, EVENT_COUNTER, MEMBER_ACCUMULATED_LATE_FEES, MEMBER_LOCKED_AMOUNTS,
    MEMBER_MISSED_PAYMENTS, MEMBER_PSEUDONYMS, PAYOUTS, PENALTIES, PENDING_PAYOUTS, PENDING_REFUNDS,
    PLATFORM_CONFIG, PRIVATE_MEMBER_LIST,
};

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateCircle {
            circle_name,
            circle_description,
            circle_image,
            max_members,
            min_members_required,
            invite_only,
            contribution_amount,
            exit_penalty_percent,
            late_fee_percent,
            total_cycles,
            cycle_duration_days,
            start_date,
            grace_period_hours,
            auto_start_when_full,
            auto_start_type,
            auto_start_date,
            payout_order_type,
            payout_order_list,
            auto_payout_enabled,
            manual_trigger_enabled,
            emergency_stop_enabled,
            auto_refund_if_min_not_met,
            strict_mode,
            visibility,
            show_member_identities,
            distribution_threshold,
        } => execute_create_circle(
            deps,
            env,
            info,
            circle_name,
            circle_description,
            circle_image,
            max_members,
            min_members_required,
            invite_only,
            contribution_amount,
            exit_penalty_percent,
            late_fee_percent,
            total_cycles,
            cycle_duration_days,
            start_date,
            grace_period_hours,
            auto_start_when_full,
            auto_start_type,
            auto_start_date,
            payout_order_type,
            payout_order_list,
            auto_payout_enabled,
            manual_trigger_enabled,
            emergency_stop_enabled,
            auto_refund_if_min_not_met,
            strict_mode,
            visibility,
            show_member_identities,
            distribution_threshold,
        ),
        ExecuteMsg::JoinCircle { circle_id } => execute_join_circle(deps, env, info, circle_id),
        ExecuteMsg::AcceptInvite { circle_id } => execute_join_circle(deps, env, info, circle_id),
        ExecuteMsg::InviteMember {
            circle_id,
            member_address,
        } => execute_invite_member(deps, env, info, circle_id, member_address),
        ExecuteMsg::ExitCircle { circle_id } => execute_exit_circle(deps, env, info, circle_id),
        ExecuteMsg::StartCircle { circle_id } => execute_start_circle(deps, env, info, circle_id),
        ExecuteMsg::DepositContribution { circle_id } => {
            execute_deposit_contribution(deps, env, info, circle_id)
        }
        ExecuteMsg::ProcessPayout { circle_id } => {
            execute_process_payout(deps, env, info, circle_id)
        }
        ExecuteMsg::Withdraw { circle_id } => execute_withdraw(deps, env, info, circle_id),
        ExecuteMsg::CheckAndEject { circle_id } => {
            execute_check_and_eject(deps, env, info, circle_id)
        }
        ExecuteMsg::PauseCircle { circle_id } => execute_pause_circle(deps, env, info, circle_id),
        ExecuteMsg::UnpauseCircle { circle_id } => {
            execute_unpause_circle(deps, env, info, circle_id)
        }
        ExecuteMsg::EmergencyStop { circle_id } => {
            execute_emergency_stop(deps, env, info, circle_id)
        }
        ExecuteMsg::CancelCircle { circle_id } => {
            execute_cancel_circle(deps, env, info, circle_id)
        }
        ExecuteMsg::UpdateCircle {
            circle_id,
            circle_name,
            circle_description,
            circle_image,
        } => execute_update_circle(
            deps,
            env,
            info,
            circle_id,
            circle_name,
            circle_description,
            circle_image,
        ),
        ExecuteMsg::WithdrawPlatformFees { circle_id } => {
            execute_withdraw_platform_fees(deps, env, info, circle_id)
        }
        ExecuteMsg::AddPrivateMember {
            circle_id,
            member_address,
            pseudonym,
        } => execute_add_private_member(deps, env, info, circle_id, member_address, pseudonym),
        ExecuteMsg::UpdateMemberPseudonym {
            circle_id,
            member_address,
            pseudonym,
        } => execute_update_member_pseudonym(deps, env, info, circle_id, member_address, pseudonym),
        ExecuteMsg::BlockMember {
            circle_id,
            member_address,
        } => execute_block_member(deps, env, info, circle_id, member_address),
        ExecuteMsg::DistributeBlockedFunds { circle_id, cycle } => {
            execute_distribute_blocked_funds(deps, env, info, circle_id, cycle)
        }
        ExecuteMsg::EnableStaking {
            circle_id,
            validator_address,
        } => execute_enable_staking(deps, env, info, circle_id, validator_address),
        ExecuteMsg::DisableStaking { circle_id } => {
            execute_disable_staking(deps, env, info, circle_id)
        }
        ExecuteMsg::ClaimPendingRefund { circle_id } => {
            execute_claim_pending_refund(deps, env, info, circle_id)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute required creator lock: contribution * (1 + max_members * 10%) = contribution * (10000 + max_members * 1000) / 10000
fn compute_creator_lock(contribution: Uint128, max_members: u32) -> Result<Uint128, ContractError> {
    let numerator = contribution
        .checked_mul(Uint128::from(10000u64 + max_members as u64 * 1000u64))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Creator lock overflow".to_string(),
        })?;
    Ok(numerator.multiply_ratio(1u128, 10000u128))
}

/// Compute late fee per round: contribution * late_fee_percent / 10000
fn compute_late_fee_per_round(contribution: Uint128, late_fee_percent: u64) -> Uint128 {
    contribution.multiply_ratio(late_fee_percent, 10000u64)
}

/// Compute exit penalty for a given locked amount: locked * exit_penalty_percent / 10000
fn compute_exit_penalty(locked: Uint128, exit_penalty_percent: u64) -> Uint128 {
    locked.multiply_ratio(exit_penalty_percent, 10000u64)
}

/// Base max_missed: floor((10000 - exit_penalty_percent) / late_fee_percent) from % penalty and late fee.
/// Returns u32::MAX if late_fee_percent is 0 (no late fees configured).
fn compute_max_missed_base(exit_penalty_percent: u64, late_fee_percent: u64) -> u32 {
    if late_fee_percent == 0 {
        return u32::MAX;
    }
    let numerator = 10000u64.saturating_sub(exit_penalty_percent);
    (numerator / late_fee_percent) as u32
}

/// Compute max_missed scaled by active members. When members_at_start is None, returns base.
fn compute_max_missed_scaled(
    exit_penalty_percent: u64,
    late_fee_percent: u64,
    members_at_start: Option<u32>,
    current_active_members: u32,
) -> u32 {
    let base = compute_max_missed_base(exit_penalty_percent, late_fee_percent);
    let Some(start_count) = members_at_start else {
        return base;
    };
    if start_count == 0 {
        return base;
    }
    // Scale: max_missed decreases (stricter) when members exit or are ejected
    let scaled = (base as u64 * current_active_members as u64 / start_count as u64) as u32;
    scaled.max(1)
}

/// Cap max_missed so ejection happens before the last round. max_missed <= total_rounds - 1.
fn cap_max_missed_by_rounds(max_missed: u32, total_rounds: u32) -> u32 {
    let cap = total_rounds.saturating_sub(1).max(1);
    max_missed.min(cap)
}

/// Legacy alias for compute_max_missed_base (used at CreateCircle for preview).
fn compute_max_missed(exit_penalty_percent: u64, late_fee_percent: u64) -> u32 {
    compute_max_missed_base(exit_penalty_percent, late_fee_percent)
}

/// Check if member meets ejection condition: accumulated_late_fees + exit_penalty >= locked_amount.
fn should_eject_member(
    storage: &dyn Storage,
    circle_id: u64,
    member: &Addr,
    locked_amount: Uint128,
    exit_penalty_percent: u64,
) -> bool {
    if locked_amount.is_zero() {
        return false;
    }
    let accumulated = MEMBER_ACCUMULATED_LATE_FEES
        .may_load(storage, (circle_id, member.clone()))
        .unwrap_or(None)
        .unwrap_or(Uint128::zero());
    let exit_penalty = compute_exit_penalty(locked_amount, exit_penalty_percent);
    accumulated + exit_penalty >= locked_amount
}

/// Eject a member from a running circle: remove from members_list, record in BLOCKED_MEMBERS, keep locked funds in pool, emit event.
fn eject_member_from_circle(
    deps: &mut DepsMut,
    env: &Env,
    circle: &mut Circle,
    member: &Addr,
) -> Result<(), ContractError> {
    circle.members_list.retain(|m| m != member);
    BLOCKED_MEMBERS.save(
        deps.storage,
        (circle.circle_id, member.clone()),
        &circle.current_cycle_index,
    )?;

    // Accumulate the exit penalty and remaining late fees as penalties collected
    let locked = MEMBER_LOCKED_AMOUNTS
        .may_load(deps.storage, (circle.circle_id, member.clone()))?
        .unwrap_or(Uint128::zero());
    let accumulated_fees = MEMBER_ACCUMULATED_LATE_FEES
        .may_load(deps.storage, (circle.circle_id, member.clone()))?
        .unwrap_or(Uint128::zero());
    let exit_penalty = compute_exit_penalty(locked, circle.exit_penalty_percent);
    let penalty_total = accumulated_fees + exit_penalty;
    let actual_penalty = if penalty_total > locked {
        locked
    } else {
        penalty_total
    };

    circle.total_penalties_collected = circle
        .total_penalties_collected
        .checked_add(actual_penalty)
        .unwrap_or(circle.total_penalties_collected);

    // Clear per-member fee tracking
    MEMBER_ACCUMULATED_LATE_FEES.remove(deps.storage, (circle.circle_id, member.clone()));

    // Record missed payments for stats
    let mut missed = MEMBER_MISSED_PAYMENTS
        .may_load(deps.storage, (circle.circle_id, member.clone()))?
        .unwrap_or(MemberMissedPayments {
            member: member.clone(),
            missed_count: 0,
            last_missed_cycle: None,
        });
    missed.last_missed_cycle = Some(circle.current_cycle_index);
    MEMBER_MISSED_PAYMENTS.save(deps.storage, (circle.circle_id, member.clone()), &missed)?;

    // Recompute max_missed_payments_allowed (dynamic from % penalty and late fee, scaled by active members)
    let total_rounds = circle.max_members * circle.total_cycles;
    circle.max_missed_payments_allowed = cap_max_missed_by_rounds(
        compute_max_missed_scaled(
            circle.exit_penalty_percent,
            circle.late_fee_percent,
            circle.members_at_start,
            circle.members_list.len() as u32,
        ),
        total_rounds,
    );

    log_event(
        deps,
        env,
        circle.circle_id,
        "member_ejected",
        &format!(
            "Member {} ejected at cycle {} (locked: {}, fees: {}, penalty: {})",
            member, circle.current_cycle_index, locked, accumulated_fees, exit_penalty
        ),
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Create Circle
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn execute_create_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_name: String,
    circle_description: String,
    circle_image: Option<String>,
    max_members: u32,
    min_members_required: u32,
    invite_only: bool,
    contribution_amount: Uint128,
    exit_penalty_percent: u64,
    late_fee_percent: u64,
    total_cycles: u32,
    cycle_duration_days: u32,
    start_date: Option<Timestamp>,
    grace_period_hours: u32,
    auto_start_when_full: bool,
    auto_start_type: Option<String>,
    auto_start_date: Option<Timestamp>,
    payout_order_type: PayoutOrderType,
    payout_order_list: Option<Vec<Addr>>,
    auto_payout_enabled: bool,
    manual_trigger_enabled: bool,
    emergency_stop_enabled: bool,
    auto_refund_if_min_not_met: bool,
    strict_mode: bool,
    visibility: Visibility,
    show_member_identities: bool,
    distribution_threshold: Option<DistributionThreshold>,
) -> Result<Response, ContractError> {
    if max_members == 0 || min_members_required == 0 {
        return Err(ContractError::InvalidParameters {
            msg: "max_members and min_members_required must be greater than 0".to_string(),
        });
    }
    if min_members_required > max_members {
        return Err(ContractError::InvalidParameters {
            msg: "min_members_required cannot exceed max_members".to_string(),
        });
    }
    if total_cycles == 0 {
        return Err(ContractError::InvalidParameters {
            msg: "total_cycles must be greater than 0".to_string(),
        });
    }
    if contribution_amount.is_zero() {
        return Err(ContractError::InvalidParameters {
            msg: "contribution_amount must be greater than 0".to_string(),
        });
    }
    if exit_penalty_percent > 9000 {
        return Err(ContractError::InvalidParameters {
            msg: "exit_penalty_percent cannot exceed 90% (9000 basis points)".to_string(),
        });
    }
    if late_fee_percent == 0 || late_fee_percent > 5000 {
        return Err(ContractError::InvalidParameters {
            msg: "late_fee_percent must be between 1 and 5000 basis points".to_string(),
        });
    }
    if exit_penalty_percent + late_fee_percent > 10000 {
        return Err(ContractError::InvalidParameters {
            msg: "exit_penalty_percent + late_fee_percent cannot exceed 100%".to_string(),
        });
    }

    // Force distribution_threshold = Total for Public circles
    let effective_threshold = match visibility {
        Visibility::Public => Some(DistributionThreshold::Total),
        Visibility::Private => {
            if let Some(DistributionThreshold::MinMembers { count }) = distribution_threshold {
                if count == 0 {
                    return Err(ContractError::InvalidParameters {
                        msg: "distribution_threshold MinMembers count must be > 0".to_string(),
                    });
                }
                if count > min_members_required {
                    return Err(ContractError::InvalidParameters {
                        msg: format!(
                            "distribution_threshold MinMembers count ({}) cannot exceed min_members_required ({})",
                            count, min_members_required
                        ),
                    });
                }
                Some(DistributionThreshold::MinMembers { count })
            } else {
                distribution_threshold
            }
        }
    };

    // Validate payout_order_list
    if let Some(ref order_list) = payout_order_list {
        if order_list.len() as u32 != max_members {
            return Err(ContractError::InvalidParameters {
                msg: "payout_order_list length must match max_members".to_string(),
            });
        }
    }

    // Auto-calculate creator lock amount: contribution * (1 + max_members * 10%)
    let required_creator_lock = compute_creator_lock(contribution_amount, max_members)?;

    // Validate payment: creator must send exactly required_creator_lock
    let payment = must_pay(&info, "usaf").map_err(|_| ContractError::InsufficientFunds {
        required: required_creator_lock.to_string(),
        sent: "0".to_string(),
    })?;

    if payment < required_creator_lock {
        return Err(ContractError::InsufficientFunds {
            required: required_creator_lock.to_string(),
            sent: payment.to_string(),
        });
    }

    // Auto-calculate max_missed_payments_allowed; cap at total_rounds-1 so ejection happens before last round
    let total_rounds = max_members * total_cycles;
    let max_missed = cap_max_missed_by_rounds(
        compute_max_missed(exit_penalty_percent, late_fee_percent),
        total_rounds,
    );

    let circle_id = CIRCLE_COUNTER
        .may_load(deps.storage)?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| ContractError::InvalidParameters {
            msg: "Circle ID overflow".to_string(),
        })?;

    let payout_amount = contribution_amount
        .checked_mul(Uint128::from(max_members))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount overflow".to_string(),
        })?;

    let end_date = start_date.map(|start| {
        Timestamp::from_seconds(
            start.seconds()
                + (cycle_duration_days as u64 * max_members as u64 * total_cycles as u64 * 86400),
        )
    });

    let final_payout_order = match payout_order_type {
        PayoutOrderType::RandomOrder => None,
        PayoutOrderType::PredefinedOrder => payout_order_list,
    };

    // manual_trigger_enabled only meaningful for Private circles
    let effective_manual_trigger = match visibility {
        Visibility::Public => false,
        Visibility::Private => manual_trigger_enabled,
    };

    let circle = Circle {
        circle_id,
        circle_name,
        circle_description,
        circle_image,
        creator_address: info.sender.clone(),
        created_at: env.block.time,
        updated_at: env.block.time,
        max_members,
        min_members_required,
        invite_only,
        members_list: vec![info.sender.clone()],
        pending_members: vec![],
        contribution_amount,
        denomination: "usaf".to_string(),
        payout_amount,
        exit_penalty_percent,
        late_fee_percent,
        platform_fee_percent: PLATFORM_CONFIG.load(deps.storage)?.platform_fee_percent,
        max_missed_payments_allowed: max_missed,
        total_cycles,
        cycle_duration_days,
        start_date,
        first_cycle_date: start_date,
        next_payout_date: start_date,
        end_date,
        grace_period_hours,
        auto_start_when_full,
        auto_start_type,
        auto_start_date,
        payout_order_type,
        payout_order_list: final_payout_order,
        auto_payout_enabled,
        manual_trigger_enabled: effective_manual_trigger,
        emergency_stop_enabled,
        emergency_stop_triggered: false,
        auto_refund_if_min_not_met,
        strict_mode,
        escrow_address: Some(env.contract.address.clone()),
        total_amount_locked: required_creator_lock, // creator lock already in contract
        total_penalties_collected: Uint128::zero(),
        total_platform_fees_collected: Uint128::zero(),
        total_pending_payouts: Uint128::zero(),
        withdrawal_lock: false,
        refund_mode: RefundMode::FullRefund,
        circle_status: CircleStatus::Draft,
        current_cycle_index: 0,
        cycles_completed: 0,
        members_paid_this_cycle: vec![],
        members_late_this_cycle: vec![],
        visibility,
        show_member_identities,
        creator_lock_amount: required_creator_lock,
        distribution_threshold: effective_threshold,
        members_at_start: None, // Set at StartCircle when member count is known
    };

    CIRCLES.save(deps.storage, circle_id, &circle)?;
    CIRCLE_COUNTER.save(deps.storage, &circle_id)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "circle_created",
        &format!(
            "Circle {} created by {} (creator_lock: {}, max_missed: {})",
            circle_id, info.sender, required_creator_lock, max_missed
        ),
    )?;

    Ok(Response::new()
        .add_attribute("action", "create_circle")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("creator", info.sender)
        .add_attribute("creator_lock_amount", required_creator_lock.to_string())
        .add_attribute("max_missed_payments_allowed", max_missed.to_string()))
}

// ---------------------------------------------------------------------------
// Join Circle (merges former LockJoinDeposit)
// ---------------------------------------------------------------------------

fn execute_join_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if !matches!(circle.circle_status, CircleStatus::Draft | CircleStatus::Open) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Draft or Open".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    if circle.members_list.len() as u32 >= circle.max_members {
        return Err(ContractError::CircleFull {
            max: circle.max_members,
        });
    }

    if circle.members_list.contains(&info.sender) {
        return Err(ContractError::AlreadyMember {
            address: info.sender.to_string(),
        });
    }

    // Invite/private check
    if circle.invite_only || matches!(circle.visibility, Visibility::Private) {
        if !circle.pending_members.contains(&info.sender) {
            return Err(ContractError::InviteOnly { circle_id });
        }
        circle.pending_members.retain(|m| m != &info.sender);
    }

    // Require member to send contribution_amount as join deposit (locked security)
    let payment = must_pay(&info, &circle.denomination).map_err(|_| {
        ContractError::InsufficientFunds {
            required: circle.contribution_amount.to_string(),
            sent: "0".to_string(),
        }
    })?;

    if payment < circle.contribution_amount {
        return Err(ContractError::InsufficientFunds {
            required: circle.contribution_amount.to_string(),
            sent: payment.to_string(),
        });
    }

    // Lock the join deposit
    if MEMBER_LOCKED_AMOUNTS
        .may_load(deps.storage, (circle_id, info.sender.clone()))?
        .is_some()
    {
        return Err(ContractError::InvalidParameters {
            msg: "Join deposit already locked".to_string(),
        });
    }

    MEMBER_LOCKED_AMOUNTS.save(
        deps.storage,
        (circle_id, info.sender.clone()),
        &circle.contribution_amount,
    )?;

    circle.total_amount_locked = circle
        .total_amount_locked
        .checked_add(circle.contribution_amount)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Total amount overflow".to_string(),
        })?;

    // Add member
    circle.members_list.push(info.sender.clone());
    circle.updated_at = env.block.time;

    if circle.members_list.len() as u32 >= circle.max_members {
        circle.circle_status = CircleStatus::Full;

        if circle.auto_start_when_full {
            if let Some(ref auto_type) = circle.auto_start_type.clone() {
                if auto_type == "by_members"
                    && (circle.members_list.len() as u32) >= circle.min_members_required
                {
                    generate_payout_order(&mut circle, &env);
                    if circle.start_date.is_none() {
                        circle.start_date = Some(env.block.time);
                        circle.first_cycle_date = Some(env.block.time);
                        circle.next_payout_date = Some(env.block.time);
                    }
                    circle.circle_status = CircleStatus::Running;
                    circle.current_cycle_index = 1;
                }
            }
        }
    } else if circle.circle_status == CircleStatus::Draft {
        circle.circle_status = CircleStatus::Open;
    }

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    let event_type = if matches!(circle.circle_status, CircleStatus::Running)
        && circle.current_cycle_index == 1
    {
        "circle_auto_started"
    } else {
        "member_joined"
    };

    log_event(
        &mut deps,
        &env,
        circle_id,
        event_type,
        &format!("Member {} joined circle {} (locked: {})", info.sender, circle_id, circle.contribution_amount),
    )?;

    let staking_msgs = rebalance_staking(deps.branch(), &env, circle_id)?;
    let mut resp = Response::new()
        .add_attribute("action", "join_circle")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("locked_amount", circle.contribution_amount.to_string());
    for msg in staking_msgs {
        resp = resp.add_message(msg);
    }
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Invite Member
// ---------------------------------------------------------------------------

fn execute_invite_member(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    member_address: Addr,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can invite members".to_string(),
        });
    }

    if !circle.invite_only {
        return Err(ContractError::InvalidParameters {
            msg: "Circle is not invite-only".to_string(),
        });
    }

    let validated_addr = deps.api.addr_validate(member_address.as_str())?;

    if circle.members_list.contains(&validated_addr) {
        return Err(ContractError::AlreadyMember {
            address: validated_addr.to_string(),
        });
    }

    if circle.pending_members.contains(&validated_addr) {
        return Err(ContractError::InvalidParameters {
            msg: "Member already invited".to_string(),
        });
    }

    circle.pending_members.push(validated_addr.clone());
    circle.updated_at = env.block.time;

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "member_invited",
        &format!("Member {} invited to circle {}", validated_addr, circle_id),
    )?;

    Ok(Response::new()
        .add_attribute("action", "invite_member")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", validated_addr))
}

// ---------------------------------------------------------------------------
// Exit Circle
// ---------------------------------------------------------------------------

fn execute_exit_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if !circle.members_list.contains(&info.sender) {
        return Err(ContractError::Unauthorized {
            msg: "Not a member of this circle".to_string(),
        });
    }

    let started = matches!(
        circle.circle_status,
        CircleStatus::Running | CircleStatus::Paused
    );

    let mut messages: Vec<CosmosMsg> = vec![];
    let mut refund_amount = Uint128::zero();

    if !started {
        // Before start: full refund of locked amount, no penalty
        if let Ok(Some(locked)) =
            MEMBER_LOCKED_AMOUNTS.may_load(deps.storage, (circle_id, info.sender.clone()))
        {
            refund_amount = locked;
            MEMBER_LOCKED_AMOUNTS.remove(deps.storage, (circle_id, info.sender.clone()));
            circle.total_amount_locked = circle
                .total_amount_locked
                .checked_sub(locked)
                .unwrap_or(circle.total_amount_locked);

            if !refund_amount.is_zero() {
                let (refund_msgs, _) = safe_refund_or_queue(
                    deps.branch(),
                    &env,
                    circle_id,
                    &info.sender,
                    refund_amount,
                    &circle.denomination,
                )?;
                messages.extend(refund_msgs);
            }
        }

        // Update status
        circle.members_list.retain(|m| m != &info.sender);
        circle.updated_at = env.block.time;

        if (circle.members_list.len() as u32) < circle.max_members
            && circle.circle_status == CircleStatus::Full
        {
            circle.circle_status = CircleStatus::Open;
        }

        if (circle.members_list.len() as u32) < circle.min_members_required
            && circle.auto_refund_if_min_not_met
        {
            circle.circle_status = CircleStatus::Cancelled;
            let locked_entries: Vec<(Addr, Uint128)> = MEMBER_LOCKED_AMOUNTS
                .prefix(circle_id)
                .range(deps.storage, None, None, Order::Ascending)
                .filter_map(|res| res.ok().map(|(m, a)| (m, a)))
                .collect();
            let mut extra_refund_total = Uint128::zero();
            for (member, amount) in locked_entries {
                if !amount.is_zero() {
                    let (refund_msgs, _) = safe_refund_or_queue(
                        deps.branch(),
                        &env,
                        circle_id,
                        &member,
                        amount,
                        &circle.denomination,
                    )?;
                    messages.extend(refund_msgs);
                    extra_refund_total += amount;
                }
                MEMBER_LOCKED_AMOUNTS.remove(deps.storage, (circle_id, member));
            }
            if !extra_refund_total.is_zero() {
                circle.total_amount_locked = circle
                    .total_amount_locked
                    .checked_sub(extra_refund_total)
                    .unwrap_or(Uint128::zero());
            }
            if !circle.creator_lock_amount.is_zero() {
                let creator_amount = circle.creator_lock_amount;
                let (refund_msgs, _) = safe_refund_or_queue(
                    deps.branch(),
                    &env,
                    circle_id,
                    &circle.creator_address,
                    creator_amount,
                    &circle.denomination,
                )?;
                messages.extend(refund_msgs);
            }
        }

        if circle.members_list.len() == 1 && circle.circle_status == CircleStatus::Open {
            circle.circle_status = CircleStatus::Draft;
        }
    } else {
        // After start: only allowed if strict_mode = false
        if circle.strict_mode {
            return Err(ContractError::StrictModeNoExit { circle_id });
        }

        let locked = MEMBER_LOCKED_AMOUNTS
            .may_load(deps.storage, (circle_id, info.sender.clone()))?
            .unwrap_or(Uint128::zero());

        let accumulated_fees = MEMBER_ACCUMULATED_LATE_FEES
            .may_load(deps.storage, (circle_id, info.sender.clone()))?
            .unwrap_or(Uint128::zero());

        let exit_penalty = compute_exit_penalty(locked, circle.exit_penalty_percent);
        let total_deduction = accumulated_fees + exit_penalty;
        let refund = if total_deduction >= locked {
            Uint128::zero()
        } else {
            locked - total_deduction
        };

        // The accumulated fees + exit penalty stay in the pool
        let penalty_kept = if total_deduction > locked {
            locked
        } else {
            total_deduction
        };
        circle.total_penalties_collected = circle
            .total_penalties_collected
            .checked_add(penalty_kept)
            .unwrap_or(circle.total_penalties_collected);

        if !refund.is_zero() {
            circle.total_amount_locked = circle
                .total_amount_locked
                .checked_sub(refund)
                .unwrap_or(circle.total_amount_locked);
            let (refund_msgs, _) = safe_refund_or_queue(
                deps.branch(),
                &env,
                circle_id,
                &info.sender,
                refund,
                &circle.denomination,
            )?;
            messages.extend(refund_msgs);
        }

        refund_amount = refund;

        // Clean up
        MEMBER_LOCKED_AMOUNTS.remove(deps.storage, (circle_id, info.sender.clone()));
        MEMBER_ACCUMULATED_LATE_FEES.remove(deps.storage, (circle_id, info.sender.clone()));

        // Recalculate payout order without this member
        circle.members_list.retain(|m| m != &info.sender);
        circle.updated_at = env.block.time;

        // Recompute max_missed_payments_allowed (dynamic from % penalty and late fee, scaled by active members)
        let total_rounds = circle.max_members * circle.total_cycles;
        circle.max_missed_payments_allowed = cap_max_missed_by_rounds(
            compute_max_missed_scaled(
                circle.exit_penalty_percent,
                circle.late_fee_percent,
                circle.members_at_start,
                circle.members_list.len() as u32,
            ),
            total_rounds,
        );

        // Remove from payout order for future rounds
        if let Some(ref mut order) = circle.payout_order_list {
            order.retain(|m| m != &info.sender);
        }

        // Recalculate payout_amount
        circle.payout_amount = circle
            .contribution_amount
            .checked_mul(Uint128::from(circle.members_list.len() as u128))
            .unwrap_or(circle.payout_amount);

        // Creator exit: forfeit creator_lock_amount to remaining active members
        if info.sender == circle.creator_address && !circle.creator_lock_amount.is_zero() {
            let active: Vec<Addr> = circle
                .members_list
                .iter()
                .filter(|m| *m != &circle.creator_address)
                .filter(|m| {
                    BLOCKED_MEMBERS
                        .may_load(deps.storage, (circle_id, (*m).clone()))
                        .unwrap_or(None)
                        .map(|bc| bc > circle.current_cycle_index)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            if !active.is_empty() {
                let count = Uint128::from(active.len() as u128);
                let per_member = circle.creator_lock_amount.multiply_ratio(1u128, count.u128());
                let remainder = circle
                    .creator_lock_amount
                    .checked_sub(per_member * count)
                    .unwrap_or(Uint128::zero());

                for (idx, member) in active.iter().enumerate() {
                    let mut share = per_member;
                    if idx == 0 {
                        share = share.checked_add(remainder).unwrap_or(share);
                    }
                    if !share.is_zero() {
                        let ep = PENDING_PAYOUTS
                            .may_load(deps.storage, (circle_id, member.clone()))?
                            .unwrap_or(Uint128::zero());
                        PENDING_PAYOUTS.save(
                            deps.storage,
                            (circle_id, member.clone()),
                            &(ep + share),
                        )?;
                    }
                }
                circle.total_pending_payouts = circle
                    .total_pending_payouts
                    .checked_add(circle.creator_lock_amount)
                    .unwrap_or(circle.total_pending_payouts);
            }
            circle.creator_lock_amount = Uint128::zero();
        }
    }

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    let staking_msgs = rebalance_staking(deps.branch(), &env, circle_id)?;
    for msg in &staking_msgs {
        messages.push(msg.clone());
    }

    log_event(
        &mut deps,
        &env,
        circle_id,
        "member_exited",
        &format!(
            "Member {} exited circle {} (started: {}, refund: {})",
            info.sender, circle_id, started, refund_amount
        ),
    )?;

    let resp = Response::new()
        .add_messages(messages)
        .add_attribute("action", "exit_circle")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("refund_amount", refund_amount.to_string());
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Start Circle
// ---------------------------------------------------------------------------

fn execute_start_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can start circle".to_string(),
        });
    }

    if !matches!(
        circle.circle_status,
        CircleStatus::Open | CircleStatus::Full
    ) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Open or Full".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    if (circle.members_list.len() as u32) < circle.min_members_required {
        return Err(ContractError::MinMembersNotMet {
            required: circle.min_members_required,
            current: circle.members_list.len() as u32,
        });
    }

    generate_payout_order(&mut circle, &env);

    let start_timestamp = circle.start_date.unwrap_or(env.block.time);
    circle.start_date = Some(start_timestamp);
    circle.first_cycle_date = Some(start_timestamp);
    circle.next_payout_date = Some(start_timestamp);

    let total_rounds = circle.max_members * circle.total_cycles;
    let total_duration_seconds = circle.cycle_duration_days as u64 * total_rounds as u64 * 86400;
    let end_timestamp =
        Timestamp::from_seconds(start_timestamp.seconds() + total_duration_seconds);
    circle.end_date = Some(end_timestamp);

    let archived_timestamp = Timestamp::from_seconds(
        end_timestamp.seconds() + (circle.grace_period_hours as u64 * 3600),
    );

    circle.circle_status = CircleStatus::Running;
    circle.current_cycle_index = 1;
    circle.updated_at = env.block.time;

    // Set fee params at start: members_at_start and max_missed (dynamically from % penalty and late fee)
    let members_at_start = circle.members_list.len() as u32;
    circle.members_at_start = Some(members_at_start);
    circle.max_missed_payments_allowed = cap_max_missed_by_rounds(
        compute_max_missed_scaled(
            circle.exit_penalty_percent,
            circle.late_fee_percent,
            Some(members_at_start),
            members_at_start,
        ),
        total_rounds,
    );

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    let calendar_data = build_distribution_calendar(&circle, start_timestamp);

    log_event(
        &mut deps,
        &env,
        circle_id,
        "circle_started",
        &format!("Circle {} started at {}", circle_id, start_timestamp.seconds()),
    )?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "distribution_calendar",
        &format!(
            "{{start_date:{},end_date:{},archived_date:{},calendar:[{}]}}",
            start_timestamp.seconds(),
            end_timestamp.seconds(),
            archived_timestamp.seconds(),
            calendar_data
        ),
    )?;

    Ok(Response::new()
        .add_attribute("action", "start_circle")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("start_date", start_timestamp.seconds().to_string())
        .add_attribute("end_date", end_timestamp.seconds().to_string())
        .add_attribute("archived_date", archived_timestamp.seconds().to_string())
        .add_attribute("total_rounds", total_rounds.to_string()))
}

// ---------------------------------------------------------------------------
// Deposit Contribution
// ---------------------------------------------------------------------------

fn execute_deposit_contribution(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if !matches!(circle.circle_status, CircleStatus::Running) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Running".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    if !circle.members_list.contains(&info.sender) {
        return Err(ContractError::Unauthorized {
            msg: "Not a member of this circle".to_string(),
        });
    }

    if let Ok(Some(blocked_from_cycle)) =
        BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, info.sender.clone()))
    {
        if blocked_from_cycle <= circle.current_cycle_index {
            return Err(ContractError::InvalidParameters {
                msg: format!("Member is blocked from cycle {} onwards", blocked_from_cycle),
            });
        }
    }

    if DEPOSITS
        .may_load(
            deps.storage,
            (circle_id, info.sender.clone(), circle.current_cycle_index),
        )?
        .is_some()
    {
        return Err(ContractError::AlreadyDeposited {
            address: info.sender.to_string(),
            cycle: circle.current_cycle_index,
        });
    }

    // Require exactly contribution_amount — no extra for fees (fees tracked against locked amount)
    let payment = must_pay(&info, &circle.denomination).map_err(|_| {
        ContractError::InsufficientFunds {
            required: circle.contribution_amount.to_string(),
            sent: "0".to_string(),
        }
    })?;

    if payment < circle.contribution_amount {
        return Err(ContractError::InsufficientFunds {
            required: circle.contribution_amount.to_string(),
            sent: payment.to_string(),
        });
    }

    // Determine if late
    let mut is_late = false;
    if let Some(next_payout) = circle.next_payout_date {
        let grace_end = next_payout.plus_seconds(circle.grace_period_hours as u64 * 3600);
        if env.block.time > grace_end {
            is_late = true;

            // Strict mode: in strict mode, ejection is automatic but deposit is still accepted
            // (we record the late fee against locked amount and let CheckAndEject handle ejection)
            let late_fee_per_round =
                compute_late_fee_per_round(circle.contribution_amount, circle.late_fee_percent);

            let mut accumulated = MEMBER_ACCUMULATED_LATE_FEES
                .may_load(deps.storage, (circle_id, info.sender.clone()))?
                .unwrap_or(Uint128::zero());
            accumulated = accumulated
                .checked_add(late_fee_per_round)
                .unwrap_or(accumulated);
            MEMBER_ACCUMULATED_LATE_FEES.save(
                deps.storage,
                (circle_id, info.sender.clone()),
                &accumulated,
            )?;

            // Track missed payment count
            let mut missed = MEMBER_MISSED_PAYMENTS
                .may_load(deps.storage, (circle_id, info.sender.clone()))?
                .unwrap_or(MemberMissedPayments {
                    member: info.sender.clone(),
                    missed_count: 0,
                    last_missed_cycle: None,
                });
            missed.missed_count += 1;
            missed.last_missed_cycle = Some(circle.current_cycle_index);
            MEMBER_MISSED_PAYMENTS.save(
                deps.storage,
                (circle_id, info.sender.clone()),
                &missed,
            )?;

            // Record late fee as penalty record for history
            PENALTIES.save(
                deps.storage,
                (circle_id, info.sender.clone(), circle.current_cycle_index),
                &PenaltyRecord {
                    member: info.sender.clone(),
                    cycle: circle.current_cycle_index,
                    amount: late_fee_per_round,
                    reason: format!(
                        "Late payment ({}% of contribution deducted from locked amount)",
                        circle.late_fee_percent / 100
                    ),
                    timestamp: env.block.time,
                },
            )?;

            // Check ejection condition AFTER accumulating late fee
            let locked = MEMBER_LOCKED_AMOUNTS
                .may_load(deps.storage, (circle_id, info.sender.clone()))?
                .unwrap_or(Uint128::zero());

            if should_eject_member(
                deps.storage,
                circle_id,
                &info.sender,
                locked,
                circle.exit_penalty_percent,
            ) {
                eject_member_from_circle(&mut deps, &env, &mut circle, &info.sender)?;
                CIRCLES.save(deps.storage, circle_id, &circle)?;
                // Deposit is rejected since member is now ejected
                return Err(ContractError::MaxMissedPaymentsExceeded {
                    max: circle.max_missed_payments_allowed,
                });
            }
        } else if env.block.time > next_payout {
            // Within grace period: still on-time from penalty perspective but flagged as late
            is_late = true;
        }
    }

    // Record deposit
    DEPOSITS.save(
        deps.storage,
        (circle_id, info.sender.clone(), circle.current_cycle_index),
        &DepositRecord {
            member: info.sender.clone(),
            cycle: circle.current_cycle_index,
            amount: circle.contribution_amount,
            timestamp: env.block.time,
            on_time: !is_late,
        },
    )?;

    circle.total_amount_locked = circle
        .total_amount_locked
        .checked_add(circle.contribution_amount)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Total amount overflow".to_string(),
        })?;

    if is_late {
        circle.members_late_this_cycle.push(info.sender.clone());
    } else {
        circle.members_paid_this_cycle.push(info.sender.clone());
    }

    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "contribution_deposited",
        &format!(
            "Member {} deposited {} for cycle {} (on_time: {})",
            info.sender, circle.contribution_amount, circle.current_cycle_index, !is_late
        ),
    )?;

    let staking_msgs = rebalance_staking(deps.branch(), &env, circle_id)?;
    let mut resp = Response::new()
        .add_attribute("action", "deposit_contribution")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("cycle", circle.current_cycle_index.to_string())
        .add_attribute("amount", circle.contribution_amount.to_string())
        .add_attribute("on_time", (!is_late).to_string());
    for msg in staking_msgs {
        resp = resp.add_message(msg);
    }
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Process Payout — stores to PENDING_PAYOUTS instead of direct send
// ---------------------------------------------------------------------------

fn execute_process_payout(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if !matches!(circle.circle_status, CircleStatus::Running) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Running".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    // Authorization: manual_trigger_enabled means only creator can call
    if circle.manual_trigger_enabled && info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can trigger payout (manual_trigger_enabled)".to_string(),
        });
    }

    if let Some(next_payout) = circle.next_payout_date {
        if env.block.time < next_payout {
            return Err(ContractError::CycleNotReady {
                next_date: next_payout.seconds(),
            });
        }
    }

    // Active members (not blocked)
    let mut active_members: Vec<Addr> = circle
        .members_list
        .iter()
        .filter(|m| {
            BLOCKED_MEMBERS
                .may_load(deps.storage, (circle_id, (*m).clone()))
                .unwrap_or(None)
                .map(|bc| bc > circle.current_cycle_index)
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    let missing_members: Vec<Addr> = active_members
        .iter()
        .filter(|m| {
            DEPOSITS
                .may_load(deps.storage, (circle_id, (*m).clone(), circle.current_cycle_index))
                .unwrap_or(None)
                .is_none()
        })
        .cloned()
        .collect();

    // If any missing and grace period still active, block payout
    if let Some(next_payout) = circle.next_payout_date {
        let grace_end = next_payout.plus_seconds(circle.grace_period_hours as u64 * 3600);
        if env.block.time <= grace_end && !missing_members.is_empty() {
            return Err(ContractError::InvalidParameters {
                msg: "Grace period not ended for all missing members".to_string(),
            });
        }
    }

    let mut locked_used_total = Uint128::zero();

    // Handle missing members (past grace): accumulate late fees, check ejection, use locked funds
    for member in &missing_members {
        let late_fee_per_round =
            compute_late_fee_per_round(circle.contribution_amount, circle.late_fee_percent);

        let mut accumulated = MEMBER_ACCUMULATED_LATE_FEES
            .may_load(deps.storage, (circle_id, member.clone()))?
            .unwrap_or(Uint128::zero());

        // Only count once per cycle
        let mut missed = MEMBER_MISSED_PAYMENTS
            .may_load(deps.storage, (circle_id, member.clone()))?
            .unwrap_or(MemberMissedPayments {
                member: member.clone(),
                missed_count: 0,
                last_missed_cycle: None,
            });

        if missed.last_missed_cycle != Some(circle.current_cycle_index) {
            missed.missed_count += 1;
            missed.last_missed_cycle = Some(circle.current_cycle_index);
            accumulated = accumulated
                .checked_add(late_fee_per_round)
                .unwrap_or(accumulated);

            MEMBER_ACCUMULATED_LATE_FEES.save(
                deps.storage,
                (circle_id, member.clone()),
                &accumulated,
            )?;
            MEMBER_MISSED_PAYMENTS.save(
                deps.storage,
                (circle_id, member.clone()),
                &missed,
            )?;
        }

        let locked = MEMBER_LOCKED_AMOUNTS
            .may_load(deps.storage, (circle_id, member.clone()))?
            .unwrap_or(Uint128::zero());

        // Check ejection condition
        if should_eject_member(
            deps.storage,
            circle_id,
            member,
            locked,
            circle.exit_penalty_percent,
        ) {
            eject_member_from_circle(&mut deps, &env, &mut circle, member)?;
        }

        // Use locked funds to cover missed deposit
        let used = use_locked_amount_for_member(
            deps.storage,
            circle_id,
            member,
            circle.contribution_amount,
        )?;
        if !used.is_zero() {
            locked_used_total = locked_used_total
                .checked_add(used)
                .map_err(|_| ContractError::InvalidParameters {
                    msg: "Locked funds overflow".to_string(),
                })?;
        }
    }

    // Also use locked funds from previously blocked members
    let blocked_members_list: Vec<Addr> = BLOCKED_MEMBERS
        .prefix(circle_id)
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|res| res.ok().map(|(m, _)| m))
        .collect();

    for blocked_member in blocked_members_list {
        if missing_members.iter().any(|m| m == &blocked_member) {
            continue;
        }
        if let Ok(Some(bc)) =
            BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, blocked_member.clone()))
        {
            if bc <= circle.current_cycle_index {
                let used = use_locked_amount_for_member(
                    deps.storage,
                    circle_id,
                    &blocked_member,
                    circle.contribution_amount,
                )?;
                if !used.is_zero() {
                    locked_used_total = locked_used_total
                        .checked_add(used)
                        .map_err(|_| ContractError::InvalidParameters {
                            msg: "Locked funds overflow".to_string(),
                        })?;
                }
            }
        }
    }

    // Recompute active members after ejections
    active_members = circle
        .members_list
        .iter()
        .filter(|m| {
            BLOCKED_MEMBERS
                .may_load(deps.storage, (circle_id, (*m).clone()))
                .unwrap_or(None)
                .map(|bc| bc > circle.current_cycle_index)
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    let deposits_count = active_members
        .iter()
        .filter(|m| {
            DEPOSITS
                .may_load(deps.storage, (circle_id, (*m).clone(), circle.current_cycle_index))
                .unwrap_or(None)
                .is_some()
        })
        .count();

    // Distribution threshold check
    let round_in_cycle = ((circle.current_cycle_index - 1) % circle.max_members) + 1;
    let min_round_for_distribution = match circle.distribution_threshold {
        None => 1u32,
        Some(DistributionThreshold::Total) => circle.max_members,
        Some(DistributionThreshold::MinMembers { count }) => count,
    };

    if round_in_cycle < min_round_for_distribution {
        return Err(ContractError::InvalidParameters {
            msg: format!(
                "Distribution only from round {} (current round in cycle: {})",
                min_round_for_distribution, round_in_cycle
            ),
        });
    }

    if deposits_count < active_members.len() {
        return Err(ContractError::InvalidParameters {
            msg: format!(
                "Not all active members have deposited: need {}, have {}",
                active_members.len(),
                deposits_count
            ),
        });
    }

    // Get recipient
    let recipient = if let Some(ref order_list) = circle.payout_order_list {
        let index = (circle.current_cycle_index as usize - 1) % order_list.len();
        order_list[index].clone()
    } else {
        return Err(ContractError::InvalidParameters {
            msg: "Payout order not set".to_string(),
        });
    };

    // Calculate payout amount
    let base_payout = circle
        .contribution_amount
        .checked_mul(Uint128::from(deposits_count as u128))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount overflow".to_string(),
        })?;

    let mut payout_amount = base_payout
        .checked_add(locked_used_total)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount overflow".to_string(),
        })?;

    let platform_fee = payout_amount.multiply_ratio(circle.platform_fee_percent, 10000u64);
    payout_amount = payout_amount
        .checked_sub(platform_fee)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount underflow".to_string(),
        })?;

    circle.total_platform_fees_collected = circle
        .total_platform_fees_collected
        .checked_add(platform_fee)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Platform fees overflow".to_string(),
        })?;

    // Record payout
    PAYOUTS.save(
        deps.storage,
        (circle_id, circle.current_cycle_index),
        &PayoutRecord {
            cycle: circle.current_cycle_index,
            recipient: recipient.clone(),
            amount: payout_amount,
            timestamp: env.block.time,
            transaction_hash: None,
        },
    )?;

    // Store in PENDING_PAYOUTS instead of direct BankMsg (decouples withdraw from deposit cycle)
    let existing_pending = PENDING_PAYOUTS
        .may_load(deps.storage, (circle_id, recipient.clone()))?
        .unwrap_or(Uint128::zero());
    let new_pending = existing_pending
        .checked_add(payout_amount)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Pending payout overflow".to_string(),
        })?;
    PENDING_PAYOUTS.save(deps.storage, (circle_id, recipient.clone()), &new_pending)?;

    // Subtract the contributed deposits from total_amount_locked (excluding locked join deposits used above — those were already deducted by use_locked_amount_for_member)
    let deposit_outflow = base_payout
        .checked_add(locked_used_total)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Outflow overflow".to_string(),
        })?;

    circle.total_amount_locked = circle
        .total_amount_locked
        .checked_sub(deposit_outflow)
        .unwrap_or(Uint128::zero());

    circle.total_pending_payouts = circle
        .total_pending_payouts
        .checked_add(payout_amount)
        .unwrap_or(circle.total_pending_payouts);

    circle.cycles_completed += 1;
    circle.members_paid_this_cycle.clear();
    circle.members_late_this_cycle.clear();

    // Check if last round across all cycles
    let total_rounds = circle.max_members * circle.total_cycles;
    if circle.current_cycle_index >= total_rounds {
        circle.circle_status = CircleStatus::Completed;

        // Final distribution: all remaining locked + penalties + staking rewards distributed equally to active members via PENDING_PAYOUTS
        let mut remaining = circle
            .total_amount_locked
            .checked_add(circle.total_penalties_collected)
            .unwrap_or(circle.total_amount_locked);
        if let Some(mut staking_cfg) = CIRCLE_STAKING.may_load(deps.storage, circle_id)? {
            remaining = remaining
                .checked_add(staking_cfg.rewards_accumulated)
                .unwrap_or(remaining);
            staking_cfg.rewards_accumulated = Uint128::zero();
            CIRCLE_STAKING.save(deps.storage, circle_id, &staking_cfg)?;
        }

        if !remaining.is_zero() && !active_members.is_empty() {
            let member_count = Uint128::from(active_members.len() as u128);
            let amount_per_member = remaining.multiply_ratio(1u128, member_count.u128());
            let remainder = remaining
                .checked_sub(amount_per_member * member_count)
                .unwrap_or(Uint128::zero());

            for (idx, member) in active_members.iter().enumerate() {
                let mut bonus = amount_per_member;
                if idx == 0 {
                    bonus = bonus.checked_add(remainder).unwrap_or(bonus);
                }
                if !bonus.is_zero() {
                    let ep = PENDING_PAYOUTS
                        .may_load(deps.storage, (circle_id, member.clone()))?
                        .unwrap_or(Uint128::zero());
                    PENDING_PAYOUTS.save(
                        deps.storage,
                        (circle_id, member.clone()),
                        &(ep + bonus),
                    )?;
                }
            }

            circle.total_pending_payouts = circle
                .total_pending_payouts
                .checked_add(remaining)
                .unwrap_or(circle.total_pending_payouts);

            // Clear locked amounts and penalties (they're now in PENDING_PAYOUTS)
            let locked_keys: Vec<Addr> = MEMBER_LOCKED_AMOUNTS
                .prefix(circle_id)
                .range(deps.storage, None, None, Order::Ascending)
                .filter_map(|res| res.ok().map(|(m, _)| m))
                .collect();
            for m in locked_keys {
                MEMBER_LOCKED_AMOUNTS.remove(deps.storage, (circle_id, m));
            }
            circle.total_amount_locked = Uint128::zero();
            circle.total_penalties_collected = Uint128::zero();
        }

        log_event(
            &mut deps,
            &env,
            circle_id,
            "circle_completed",
            &format!(
                "Circle {} completed. All payouts stored in PENDING_PAYOUTS.",
                circle_id
            ),
        )?;
    } else {
        circle.current_cycle_index += 1;
        if let Some(current_date) = circle.next_payout_date {
            circle.next_payout_date = Some(Timestamp::from_seconds(
                current_date.seconds() + (circle.cycle_duration_days as u64 * 86400),
            ));
        }
    }

    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "payout_processed",
        &format!(
            "Payout processed for round {} to {} ({} usaf pending withdrawal)",
            circle.cycles_completed, recipient, payout_amount
        ),
    )?;

    let claim_msgs = claim_staking_rewards(deps.branch(), &env, circle_id)?;
    let rebalance_msgs = rebalance_staking(deps.branch(), &env, circle_id)?;
    let mut resp = Response::new()
        .add_attribute("action", "process_payout")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("cycle", circle.cycles_completed.to_string())
        .add_attribute("recipient", recipient)
        .add_attribute("amount", payout_amount.to_string())
        .add_attribute("pending_withdrawal", "true");
    for msg in claim_msgs {
        resp = resp.add_message(msg);
    }
    for msg in rebalance_msgs {
        resp = resp.add_message(msg);
    }
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Withdraw — member claims all pending payouts at once
// ---------------------------------------------------------------------------

fn execute_withdraw(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    // Validate circle exists
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    let pending = PENDING_PAYOUTS
        .may_load(deps.storage, (circle_id, info.sender.clone()))?
        .unwrap_or(Uint128::zero());

    if pending.is_zero() {
        return Err(ContractError::NoPendingPayouts {});
    }

    // Clear pending payout
    PENDING_PAYOUTS.remove(deps.storage, (circle_id, info.sender.clone()));

    circle.total_pending_payouts = circle
        .total_pending_payouts
        .checked_sub(pending)
        .unwrap_or(Uint128::zero());

    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "withdrawal",
        &format!("Member {} withdrew {} usaf", info.sender, pending),
    )?;

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: circle.denomination.clone(),
                amount: pending,
            }],
        })
        .add_attribute("action", "withdraw")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("amount", pending.to_string()))
}

// ---------------------------------------------------------------------------
// Check And Eject — permissionless ejection check
// ---------------------------------------------------------------------------

fn execute_check_and_eject(
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if !matches!(circle.circle_status, CircleStatus::Running | CircleStatus::Paused) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Running or Paused".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    let members_snapshot: Vec<Addr> = circle.members_list.clone();
    let mut ejected_count = 0u32;

    for member in &members_snapshot {
        let locked = MEMBER_LOCKED_AMOUNTS
            .may_load(deps.storage, (circle_id, member.clone()))?
            .unwrap_or(Uint128::zero());

        if should_eject_member(
            deps.storage,
            circle_id,
            member,
            locked,
            circle.exit_penalty_percent,
        ) {
            eject_member_from_circle(&mut deps, &env, &mut circle, member)?;
            ejected_count += 1;
        }
    }

    if ejected_count > 0 {
        // Recalculate payout order
        if let Some(ref mut order) = circle.payout_order_list {
            let blocked: Vec<Addr> = BLOCKED_MEMBERS
                .prefix(circle_id)
                .range(deps.storage, None, None, Order::Ascending)
                .filter_map(|res| res.ok().map(|(m, _)| m))
                .collect();
            order.retain(|m| !blocked.contains(m));
        }
        circle.payout_amount = circle
            .contribution_amount
            .checked_mul(Uint128::from(circle.members_list.len() as u128))
            .unwrap_or(circle.payout_amount);

        circle.updated_at = env.block.time;
        CIRCLES.save(deps.storage, circle_id, &circle)?;
    }

    Ok(Response::new()
        .add_attribute("action", "check_and_eject")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("ejected_count", ejected_count.to_string()))
}

// ---------------------------------------------------------------------------
// Cancel Circle — extended to allow running circles (creator forfeits lock)
// ---------------------------------------------------------------------------

fn execute_cancel_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can cancel circle".to_string(),
        });
    }

    let is_running = matches!(
        circle.circle_status,
        CircleStatus::Running | CircleStatus::Paused
    );

    if !matches!(
        circle.circle_status,
        CircleStatus::Draft | CircleStatus::Open | CircleStatus::Running | CircleStatus::Paused
    ) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Draft, Open, Running or Paused".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    circle.circle_status = CircleStatus::Cancelled;
    circle.updated_at = env.block.time;

    let mut messages: Vec<CosmosMsg> = Vec::new();

    if is_running {
        // Creator forfeits creator_lock_amount — distributed to active members via PENDING_PAYOUTS
        let active_members: Vec<Addr> = circle
            .members_list
            .iter()
            .filter(|m| m.as_ref() != circle.creator_address.as_str())
            .filter(|m| {
                BLOCKED_MEMBERS
                    .may_load(deps.storage, (circle_id, (*m).clone()))
                    .unwrap_or(None)
                    .is_none()
            })
            .cloned()
            .collect();

        if !circle.creator_lock_amount.is_zero() && !active_members.is_empty() {
            let count = Uint128::from(active_members.len() as u128);
            let per_member = circle.creator_lock_amount.multiply_ratio(1u128, count.u128());
            let remainder = circle
                .creator_lock_amount
                .checked_sub(per_member * count)
                .unwrap_or(Uint128::zero());

            for (idx, member) in active_members.iter().enumerate() {
                let mut share = per_member;
                if idx == 0 {
                    share = share.checked_add(remainder).unwrap_or(share);
                }
                if !share.is_zero() {
                    let ep = PENDING_PAYOUTS
                        .may_load(deps.storage, (circle_id, member.clone()))?
                        .unwrap_or(Uint128::zero());
                    PENDING_PAYOUTS.save(
                        deps.storage,
                        (circle_id, member.clone()),
                        &(ep + share),
                    )?;
                    circle.total_pending_payouts = circle
                        .total_pending_payouts
                        .checked_add(share)
                        .unwrap_or(circle.total_pending_payouts);
                }
            }
        }

        // Refund each member's locked join deposit (minus accumulated fees)
        let member_list_snapshot: Vec<Addr> = circle.members_list.clone();
        for member in &member_list_snapshot {
            let locked = MEMBER_LOCKED_AMOUNTS
                .may_load(deps.storage, (circle_id, member.clone()))?
                .unwrap_or(Uint128::zero());
            let accumulated_fees = MEMBER_ACCUMULATED_LATE_FEES
                .may_load(deps.storage, (circle_id, member.clone()))?
                .unwrap_or(Uint128::zero());
            let exit_penalty = compute_exit_penalty(locked, circle.exit_penalty_percent);
            let deduction = accumulated_fees + exit_penalty;
            let refund = if deduction >= locked {
                Uint128::zero()
            } else {
                locked - deduction
            };

            if !refund.is_zero() {
                circle.total_amount_locked = circle
                    .total_amount_locked
                    .checked_sub(refund)
                    .unwrap_or(Uint128::zero());
                let (refund_msgs, _) = safe_refund_or_queue(
                    deps.branch(),
                    &env,
                    circle_id,
                    member,
                    refund,
                    &circle.denomination,
                )?;
                messages.extend(refund_msgs);
            }

            MEMBER_LOCKED_AMOUNTS.remove(deps.storage, (circle_id, member.clone()));
            MEMBER_ACCUMULATED_LATE_FEES.remove(deps.storage, (circle_id, member.clone()));
        }
    } else {
        // Before start: refund all join deposits
        let locked_entries: Vec<(Addr, Uint128)> = MEMBER_LOCKED_AMOUNTS
            .prefix(circle_id)
            .range(deps.storage, None, None, Order::Ascending)
            .filter_map(|res| res.ok().map(|(m, a)| (m, a)))
            .collect();

        let mut join_refund_total = Uint128::zero();
        for (member, amount) in locked_entries {
            if !amount.is_zero() {
                let (refund_msgs, _) = safe_refund_or_queue(
                    deps.branch(),
                    &env,
                    circle_id,
                    &member,
                    amount,
                    &circle.denomination,
                )?;
                messages.extend(refund_msgs);
                join_refund_total += amount;
            }
            MEMBER_LOCKED_AMOUNTS.remove(deps.storage, (circle_id, member));
        }

        if !join_refund_total.is_zero() {
            circle.total_amount_locked = circle
                .total_amount_locked
                .checked_sub(join_refund_total)
                .unwrap_or(Uint128::zero());
        }

        // Refund creator lock
        if !circle.creator_lock_amount.is_zero() {
            let creator_amount = circle.creator_lock_amount;
            let (refund_msgs, _) = safe_refund_or_queue(
                deps.branch(),
                &env,
                circle_id,
                &circle.creator_address,
                creator_amount,
                &circle.denomination,
            )?;
            messages.extend(refund_msgs);
        }
    }

    let staking_msgs = rebalance_staking(deps.branch(), &env, circle_id)?;
    messages.extend(staking_msgs);

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "circle_cancelled",
        &format!(
            "Circle {} cancelled by {} (was_running: {})",
            circle_id, info.sender, is_running
        ),
    )?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "cancel_circle")
        .add_attribute("circle_id", circle_id.to_string()))
}

// ---------------------------------------------------------------------------
// Pause / Unpause
// ---------------------------------------------------------------------------

fn execute_pause_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can pause circle".to_string(),
        });
    }

    if !matches!(circle.circle_status, CircleStatus::Running) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Running".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    circle.circle_status = CircleStatus::Paused;
    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(&mut deps, &env, circle_id, "circle_paused", &format!("Circle {} paused", circle_id))?;

    Ok(Response::new()
        .add_attribute("action", "pause_circle")
        .add_attribute("circle_id", circle_id.to_string()))
}

fn execute_unpause_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can unpause circle".to_string(),
        });
    }

    if !matches!(circle.circle_status, CircleStatus::Paused) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Paused".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    circle.circle_status = CircleStatus::Running;
    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(&mut deps, &env, circle_id, "circle_unpaused", &format!("Circle {} unpaused", circle_id))?;

    Ok(Response::new()
        .add_attribute("action", "unpause_circle")
        .add_attribute("circle_id", circle_id.to_string()))
}

// ---------------------------------------------------------------------------
// Emergency Stop
// ---------------------------------------------------------------------------

fn execute_emergency_stop(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if !circle.emergency_stop_enabled {
        return Err(ContractError::InvalidParameters {
            msg: "Emergency stop not enabled".to_string(),
        });
    }

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can trigger emergency stop".to_string(),
        });
    }

    circle.emergency_stop_triggered = true;
    circle.circle_status = CircleStatus::Paused;
    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(&mut deps, &env, circle_id, "emergency_stop", &format!("Emergency stop triggered for circle {}", circle_id))?;

    Ok(Response::new()
        .add_attribute("action", "emergency_stop")
        .add_attribute("circle_id", circle_id.to_string()))
}

// ---------------------------------------------------------------------------
// Update Circle
// ---------------------------------------------------------------------------

fn execute_update_circle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    circle_name: Option<String>,
    circle_description: Option<String>,
    circle_image: Option<String>,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can update circle".to_string(),
        });
    }

    if matches!(
        circle.circle_status,
        CircleStatus::Running | CircleStatus::Completed
    ) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Not Running or Completed".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    if let Some(name) = circle_name {
        circle.circle_name = name;
    }
    if let Some(desc) = circle_description {
        circle.circle_description = desc;
    }
    if let Some(img) = circle_image {
        circle.circle_image = Some(img);
    }

    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    Ok(Response::new()
        .add_attribute("action", "update_circle")
        .add_attribute("circle_id", circle_id.to_string()))
}

// ---------------------------------------------------------------------------
// Withdraw Platform Fees (stub)
// ---------------------------------------------------------------------------

fn execute_withdraw_platform_fees(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _circle_id: Option<u64>,
) -> Result<Response, ContractError> {
    Err(ContractError::InvalidParameters {
        msg: "Platform fee withdrawal not yet implemented".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Private Circle Management
// ---------------------------------------------------------------------------

fn execute_add_private_member(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    member_address: Addr,
    pseudonym: Option<String>,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can add private members".to_string(),
        });
    }

    if !matches!(circle.visibility, Visibility::Private) {
        return Err(ContractError::InvalidParameters {
            msg: "Circle must be private to use AddPrivateMember".to_string(),
        });
    }

    if circle.members_list.len() as u32 >= circle.max_members {
        return Err(ContractError::CircleFull {
            max: circle.max_members,
        });
    }

    let validated = deps.api.addr_validate(member_address.as_str())?;

    if circle.members_list.contains(&validated) {
        return Err(ContractError::AlreadyMember {
            address: validated.to_string(),
        });
    }

    circle.members_list.push(validated.clone());

    let mut private_members = PRIVATE_MEMBER_LIST
        .may_load(deps.storage, circle_id)?
        .unwrap_or_default();
    private_members.push(validated.clone());
    PRIVATE_MEMBER_LIST.save(deps.storage, circle_id, &private_members)?;

    if let Some(pseudo) = pseudonym {
        MEMBER_PSEUDONYMS.save(deps.storage, (circle_id, validated.clone()), &pseudo)?;
    }

    circle.updated_at = env.block.time;

    if circle.members_list.len() as u32 >= circle.max_members {
        circle.circle_status = CircleStatus::Full;
    } else if circle.circle_status == CircleStatus::Draft {
        circle.circle_status = CircleStatus::Open;
    }

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "private_member_added",
        &format!("Private member {} added by {}", validated, info.sender),
    )?;

    Ok(Response::new()
        .add_attribute("action", "add_private_member")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", validated))
}

fn execute_update_member_pseudonym(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    member_address: Addr,
    pseudonym: String,
) -> Result<Response, ContractError> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can update pseudonyms".to_string(),
        });
    }

    let validated = deps.api.addr_validate(member_address.as_str())?;
    let is_member = circle.members_list.contains(&validated);
    let is_pending = circle.pending_members.contains(&validated);

    if !is_member && !is_pending {
        return Err(ContractError::InvalidParameters {
            msg: "Address not found in circle members or pending invitations".to_string(),
        });
    }

    MEMBER_PSEUDONYMS.save(deps.storage, (circle_id, validated.clone()), &pseudonym)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "member_pseudonym_updated",
        &format!("Pseudonym '{}' set for {}", pseudonym, validated),
    )?;

    Ok(Response::new()
        .add_attribute("action", "update_member_pseudonym")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", validated)
        .add_attribute("pseudonym", pseudonym))
}

fn execute_block_member(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    member_address: Addr,
) -> Result<Response, ContractError> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can block members".to_string(),
        });
    }

    let validated = deps.api.addr_validate(member_address.as_str())?;

    if !circle.members_list.contains(&validated) {
        return Err(ContractError::InvalidParameters {
            msg: "Member not found in circle".to_string(),
        });
    }

    let blocked_from_cycle = circle.current_cycle_index + 1;
    BLOCKED_MEMBERS.save(deps.storage, (circle_id, validated.clone()), &blocked_from_cycle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "member_blocked",
        &format!("Member {} blocked from cycle {}", validated, blocked_from_cycle),
    )?;

    Ok(Response::new()
        .add_attribute("action", "block_member")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", validated)
        .add_attribute("blocked_from_cycle", blocked_from_cycle.to_string()))
}

fn execute_distribute_blocked_funds(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    cycle: u32,
) -> Result<Response, ContractError> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can distribute blocked funds".to_string(),
        });
    }

    let mut total_blocked_funds = Uint128::zero();
    let mut blocked_in_cycle: Vec<(Addr, Uint128)> = Vec::new();

    for member in &circle.members_list {
        if let Some(blocked_cycle) =
            BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, member.clone()))?
        {
            if blocked_cycle <= cycle {
                if let Some(locked) =
                    MEMBER_LOCKED_AMOUNTS.may_load(deps.storage, (circle_id, member.clone()))?
                {
                    total_blocked_funds += locked;
                    blocked_in_cycle.push((member.clone(), locked));
                }
            }
        }
    }

    if total_blocked_funds.is_zero() {
        return Err(ContractError::InvalidParameters {
            msg: "No blocked funds to distribute".to_string(),
        });
    }

    let active_members: Vec<Addr> = circle
        .members_list
        .iter()
        .filter(|m| {
            BLOCKED_MEMBERS
                .may_load(deps.storage, (circle_id, (*m).clone()))
                .unwrap_or(None)
                .map(|bc| bc > cycle)
                .unwrap_or(true)
        })
        .filter(|m| {
            DEPOSITS
                .may_load(deps.storage, (circle_id, (*m).clone(), cycle))
                .map(|d| d.is_some())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    if active_members.is_empty() {
        return Err(ContractError::InvalidParameters {
            msg: "No active members to distribute to".to_string(),
        });
    }

    let amount_per_member =
        total_blocked_funds.multiply_ratio(1u128, active_members.len() as u128);
    let remainder = total_blocked_funds
        .checked_sub(amount_per_member * Uint128::from(active_members.len() as u128))
        .unwrap_or(Uint128::zero());

    let mut messages = Vec::new();
    for (idx, member) in active_members.iter().enumerate() {
        let mut amount = amount_per_member;
        if idx == 0 {
            amount = amount.checked_add(remainder).unwrap_or(amount);
        }
        messages.push(BankMsg::Send {
            to_address: member.to_string(),
            amount: vec![Coin {
                denom: circle.denomination.clone(),
                amount,
            }],
        });
    }

    log_event(
        &mut deps,
        &env,
        circle_id,
        "blocked_funds_distributed",
        &format!(
            "Blocked funds {} distributed to {} active members",
            total_blocked_funds,
            active_members.len()
        ),
    )?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "distribute_blocked_funds")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("cycle", cycle.to_string())
        .add_attribute("total_distributed", total_blocked_funds.to_string()))
}

// ---------------------------------------------------------------------------
// Staking
// ---------------------------------------------------------------------------

/// Unbonding period in seconds (21 days typical for Cosmos chains)
const UNBONDING_PERIOD_SECS: u64 = 21 * 24 * 60 * 60;

fn execute_enable_staking(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    validator_address: String,
) -> Result<Response, ContractError> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can enable staking".to_string(),
        });
    }

    if CIRCLE_STAKING.may_load(deps.storage, circle_id)?.is_some() {
        return Err(ContractError::InvalidParameters {
            msg: "Staking already configured for this circle".to_string(),
        });
    }

    if !matches!(
        circle.circle_status,
        CircleStatus::Open | CircleStatus::Full | CircleStatus::Running
    ) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Open, Full or Running".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    if validator_address.trim().is_empty() {
        return Err(ContractError::InvalidParameters {
            msg: "Validator address cannot be empty".to_string(),
        });
    }

    let config = CircleStakingConfig {
        enabled: true,
        validator_address: validator_address.clone(),
        staked_amount: Uint128::zero(),
        total_rewards_earned: Uint128::zero(),
        rewards_accumulated: Uint128::zero(),
        last_claim_at: None,
        pending_undelegations: vec![],
    };
    CIRCLE_STAKING.save(deps.storage, circle_id, &config)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "staking_enabled",
        &format!("Staking enabled for circle {} with validator {}", circle_id, validator_address),
    )?;

    let staking_msgs = rebalance_staking(deps.branch(), &env, circle_id)?;
    let mut resp = Response::new()
        .add_attribute("action", "enable_staking")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("validator", validator_address);
    for msg in staking_msgs {
        resp = resp.add_message(msg);
    }
    Ok(resp)
}

fn execute_disable_staking(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;

    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can disable staking".to_string(),
        });
    }

    let Some(mut config) = CIRCLE_STAKING.may_load(deps.storage, circle_id)? else {
        return Err(ContractError::InvalidParameters {
            msg: "Staking not enabled for this circle".to_string(),
        });
    };

    config.enabled = false;
    CIRCLE_STAKING.save(deps.storage, circle_id, &config)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    if !config.staked_amount.is_zero() {
        messages.push(CosmosMsg::Staking(StakingMsg::Undelegate {
            validator: config.validator_address.clone(),
            amount: Coin {
                denom: circle.denomination.clone(),
                amount: config.staked_amount,
            },
        }));
        config.pending_undelegations.push(PendingUndelegation {
            amount: config.staked_amount,
            available_at: Timestamp::from_seconds(
                env.block.time.seconds() + UNBONDING_PERIOD_SECS,
            ),
            reason: "disable_staking".to_string(),
        });
        config.staked_amount = Uint128::zero();
        CIRCLE_STAKING.save(deps.storage, circle_id, &config)?;
    }

    log_event(
        &mut deps,
        &env,
        circle_id,
        "staking_disabled",
        &format!("Staking disabled for circle {}", circle_id),
    )?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "disable_staking")
        .add_attribute("circle_id", circle_id.to_string()))
}

fn execute_claim_pending_refund(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let circle = CIRCLES.load(deps.storage, circle_id)?;

    let Some(record) = PENDING_REFUNDS.may_load(deps.storage, (circle_id, info.sender.clone()))? else {
        return Err(ContractError::InvalidParameters {
            msg: "No pending refund for this member".to_string(),
        });
    };

    if record.member != info.sender {
        return Err(ContractError::Unauthorized {
            msg: "Not your pending refund".to_string(),
        });
    }

    if env.block.time < record.available_at {
        return Err(ContractError::InvalidParameters {
            msg: format!(
                "Refund available at {} (unbonding in progress)",
                record.available_at.seconds()
            ),
        });
    }

    PENDING_REFUNDS.remove(deps.storage, (circle_id, info.sender.clone()));

    log_event(
        &mut deps,
        &env,
        circle_id,
        "pending_refund_claimed",
        &format!("Member {} claimed {} usaf", info.sender, record.amount),
    )?;

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: circle.denomination,
                amount: record.amount,
            }],
        })
        .add_attribute("action", "claim_pending_refund")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("amount", record.amount.to_string()))
}

/// Returns messages to rebalance staking. Call after lock/deposit/exit/cancel/payout.
fn rebalance_staking(
    deps: DepsMut,
    env: &Env,
    circle_id: u64,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let Some(mut config) = CIRCLE_STAKING.may_load(deps.storage, circle_id)? else {
        return Ok(vec![]);
    };
    if !config.enabled {
        return Ok(vec![]);
    }

    let circle = CIRCLES.load(deps.storage, circle_id)?;

    if !matches!(
        circle.circle_status,
        CircleStatus::Open | CircleStatus::Full | CircleStatus::Running
    ) {
        return Ok(vec![]);
    }

    let active_count = circle
        .members_list
        .iter()
        .filter(|m| {
            BLOCKED_MEMBERS
                .may_load(deps.storage, (circle_id, (*m).clone()))
                .unwrap_or(None)
                .map(|bc| bc > circle.current_cycle_index)
                .unwrap_or(true)
        })
        .count() as u32;

    if active_count == 0 {
        return Ok(vec![]);
    }

    let liquid_reserve = circle
        .payout_amount
        .checked_mul(Uint128::from(2u128))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Reserve overflow".to_string(),
        })?;

    let total_locked = circle.total_amount_locked;

    let mut msgs: Vec<CosmosMsg> = vec![];

    if total_locked > liquid_reserve {
        let ideal_staked = total_locked
            .checked_sub(liquid_reserve)
            .map_err(|_| ContractError::InvalidParameters {
                msg: "Stakeable underflow".to_string(),
            })?;

        if ideal_staked > config.staked_amount {
            let to_stake = ideal_staked
                .checked_sub(config.staked_amount)
                .map_err(|_| ContractError::InvalidParameters {
                    msg: "Stake amount underflow".to_string(),
                })?;

            if !to_stake.is_zero() {
                let balance = deps
                    .querier
                    .query_balance(&env.contract.address, &circle.denomination)
                    .map_err(|e| ContractError::InvalidParameters {
                        msg: format!("Balance query failed: {}", e),
                    })?;
                let available = balance.amount;
                let actual_stake = if to_stake > available {
                    available
                } else {
                    to_stake
                };

                if !actual_stake.is_zero() {
                    msgs.push(CosmosMsg::Staking(StakingMsg::Delegate {
                        validator: config.validator_address.clone(),
                        amount: Coin {
                            denom: circle.denomination.clone(),
                            amount: actual_stake,
                        },
                    }));
                    config.staked_amount = config
                        .staked_amount
                        .checked_add(actual_stake)
                        .map_err(|_| ContractError::InvalidParameters {
                            msg: "Staked amount overflow".to_string(),
                        })?;
                    CIRCLE_STAKING.save(deps.storage, circle_id, &config)?;
                }
            }
        } else if config.staked_amount > ideal_staked {
            let to_undelegate = config
                .staked_amount
                .checked_sub(ideal_staked)
                .map_err(|_| ContractError::InvalidParameters {
                    msg: "Undelegate amount underflow".to_string(),
                })?;

            if !to_undelegate.is_zero() {
                msgs.push(CosmosMsg::Staking(StakingMsg::Undelegate {
                    validator: config.validator_address.clone(),
                    amount: Coin {
                        denom: circle.denomination.clone(),
                        amount: to_undelegate,
                    },
                }));
                config.staked_amount = ideal_staked;
                config.pending_undelegations.push(PendingUndelegation {
                    amount: to_undelegate,
                    available_at: Timestamp::from_seconds(
                        env.block.time.seconds() + UNBONDING_PERIOD_SECS,
                    ),
                    reason: "rebalance".to_string(),
                });
                CIRCLE_STAKING.save(deps.storage, circle_id, &config)?;
            }
        }
    } else if !config.staked_amount.is_zero() {
        msgs.push(CosmosMsg::Staking(StakingMsg::Undelegate {
            validator: config.validator_address.clone(),
            amount: Coin {
                denom: circle.denomination.clone(),
                amount: config.staked_amount,
            },
        }));
        config.pending_undelegations.push(PendingUndelegation {
            amount: config.staked_amount,
            available_at: Timestamp::from_seconds(
                env.block.time.seconds() + UNBONDING_PERIOD_SECS,
            ),
            reason: "below_reserve".to_string(),
        });
        config.staked_amount = Uint128::zero();
        CIRCLE_STAKING.save(deps.storage, circle_id, &config)?;
    }

    Ok(msgs)
}

/// Returns messages to claim staking rewards. Rewards go to rewards_accumulated for end distribution.
fn claim_staking_rewards(
    deps: DepsMut,
    _env: &Env,
    circle_id: u64,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let Some(config) = CIRCLE_STAKING.may_load(deps.storage, circle_id)? else {
        return Ok(vec![]);
    };
    if !config.enabled || config.staked_amount.is_zero() {
        return Ok(vec![]);
    }

    Ok(vec![CosmosMsg::Distribution(
        DistributionMsg::WithdrawDelegatorReward {
            validator: config.validator_address,
        },
    )])
}

/// Try immediate refund or queue. Returns (messages, was_immediate).
fn safe_refund_or_queue(
    deps: DepsMut,
    env: &Env,
    circle_id: u64,
    member: &Addr,
    amount: Uint128,
    denom: &str,
) -> Result<(Vec<CosmosMsg>, bool), ContractError> {
    if amount.is_zero() {
        return Ok((vec![], true));
    }

    let balance = deps
        .querier
        .query_balance(&env.contract.address, denom)
        .map_err(|e| ContractError::InvalidParameters {
            msg: format!("Balance query failed: {}", e),
        })?;

    if balance.amount >= amount {
        return Ok((
            vec![CosmosMsg::Bank(BankMsg::Send {
                to_address: member.to_string(),
                amount: vec![Coin {
                    denom: denom.to_string(),
                    amount,
                }],
            })],
            true,
        ));
    }

    let Some(mut config) = CIRCLE_STAKING.may_load(deps.storage, circle_id)? else {
        return Err(ContractError::InvalidParameters {
            msg: "Insufficient liquid balance and staking not enabled".to_string(),
        });
    };

    if !config.enabled || config.staked_amount.is_zero() {
        return Err(ContractError::InvalidParameters {
            msg: "Insufficient liquid balance for refund".to_string(),
        });
    }

    let validator = config.validator_address.clone();
    let to_undelegate = amount.min(config.staked_amount);
    let available_at =
        Timestamp::from_seconds(env.block.time.seconds() + UNBONDING_PERIOD_SECS);

    PENDING_REFUNDS.save(
        deps.storage,
        (circle_id, member.clone()),
        &PendingRefundRecord {
            member: member.clone(),
            amount,
            available_at,
        },
    )?;

    config.staked_amount = config
        .staked_amount
        .checked_sub(to_undelegate)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Staked amount underflow".to_string(),
        })?;
    config.pending_undelegations.push(PendingUndelegation {
        amount: to_undelegate,
        available_at,
        reason: "refund".to_string(),
    });
    CIRCLE_STAKING.save(deps.storage, circle_id, &config)?;

    Ok((
        vec![CosmosMsg::Staking(StakingMsg::Undelegate {
            validator,
            amount: Coin {
                denom: denom.to_string(),
                amount: to_undelegate,
            },
        })],
        false,
    ))
}

// ---------------------------------------------------------------------------
// Internal Helpers
// ---------------------------------------------------------------------------

fn generate_payout_order(circle: &mut Circle, env: &Env) {
    if circle.payout_order_list.is_none() {
        let members = circle.members_list.clone();
        circle.payout_order_list = Some(match circle.payout_order_type {
            PayoutOrderType::RandomOrder => {
                let mut shuffled = members;
                let seed = env.block.time.seconds() + circle.circle_id;
                for i in 0..shuffled.len() {
                    let j = (seed as usize + i * 7) % shuffled.len();
                    shuffled.swap(i, j);
                }
                shuffled
            }
            PayoutOrderType::PredefinedOrder => members,
        });
    }
}

fn build_distribution_calendar(circle: &Circle, start_timestamp: Timestamp) -> String {
    let min_round_for_distribution: u32 = match circle.distribution_threshold {
        None => 1,
        Some(DistributionThreshold::Total) => circle.max_members,
        Some(DistributionThreshold::MinMembers { count }) => count,
    };

    let mut calendar_data = String::new();
    if let Some(payout_order) = &circle.payout_order_list {
        let mut round_number = 1u32;
        for cycle in 1..=circle.total_cycles {
            for recipient in payout_order.iter() {
                let round_in_cycle = ((round_number - 1) % circle.max_members) + 1;
                let distribution_occurs = round_in_cycle >= min_round_for_distribution;
                let round_offset_seconds =
                    ((round_number - 1) * circle.cycle_duration_days) as u64 * 86400;
                let deposit_deadline = Timestamp::from_seconds(
                    start_timestamp.seconds() + round_offset_seconds,
                );
                let distribution_date = Timestamp::from_seconds(
                    start_timestamp.seconds()
                        + round_offset_seconds
                        + (circle.cycle_duration_days as u64 * 86400),
                );
                if !calendar_data.is_empty() {
                    calendar_data.push(',');
                }
                calendar_data.push_str(&format!(
                    "{{round:{},cycle:{},deposit_deadline:{},distribution_date:{},distribution_occurs:{},recipient:\"{}\"}}",
                    round_number,
                    cycle,
                    deposit_deadline.seconds(),
                    distribution_date.seconds(),
                    distribution_occurs,
                    recipient
                ));
                round_number += 1;
            }
        }
    }
    calendar_data
}

fn use_locked_amount_for_member(
    storage: &mut dyn Storage,
    circle_id: u64,
    member: &Addr,
    amount_needed: Uint128,
) -> Result<Uint128, ContractError> {
    if let Some(locked) = MEMBER_LOCKED_AMOUNTS.may_load(storage, (circle_id, member.clone()))? {
        let used = if locked > amount_needed {
            amount_needed
        } else {
            locked
        };
        let remaining = locked
            .checked_sub(used)
            .map_err(|_| ContractError::InvalidParameters {
                msg: "Locked amount underflow".to_string(),
            })?;
        if remaining.is_zero() {
            MEMBER_LOCKED_AMOUNTS.remove(storage, (circle_id, member.clone()));
        } else {
            MEMBER_LOCKED_AMOUNTS.save(storage, (circle_id, member.clone()), &remaining)?;
        }
        return Ok(used);
    }
    Ok(Uint128::zero())
}

fn log_event(
    deps: &mut DepsMut,
    env: &Env,
    circle_id: u64,
    event_type: &str,
    data: &str,
) -> StdResult<()> {
    let event_id = EVENT_COUNTER
        .may_load(deps.storage, circle_id)?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| cosmwasm_std::StdError::generic_err("Event ID overflow"))?;

    EVENTS.save(
        deps.storage,
        (circle_id, event_id),
        &EventLog {
            event_type: event_type.to_string(),
            circle_id,
            data: data.to_string(),
            timestamp: env.block.time,
        },
    )?;
    EVENT_COUNTER.save(deps.storage, circle_id, &event_id)?;

    Ok(())
}
