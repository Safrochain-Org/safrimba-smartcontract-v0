use cosmwasm_std::{
    BankMsg, Coin, DepsMut, Env, MessageInfo, Response, StdResult, Timestamp, Uint128,
    Addr,
};
use cw_utils::must_pay;

use crate::error::ContractError;
use crate::msg::ExecuteMsg;
use crate::state::{
    Circle, CircleStatus, DepositRecord, EventLog, PayoutOrderType,
    PayoutRecord, PenaltyRecord, RefundMode, Visibility, CIRCLE_COUNTER, CIRCLES, DEPOSITS, EVENTS,
    EVENT_COUNTER, PAYOUTS, PENALTIES, PLATFORM_CONFIG, MEMBER_LOCKED_AMOUNTS, BLOCKED_MEMBERS,
    MEMBER_PSEUDONYMS, PRIVATE_MEMBER_LIST,
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
            penalty_fee_amount,
            late_fee_amount,
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
            arbiter_address,
            emergency_stop_enabled,
            auto_refund_if_min_not_met,
            max_missed_payments_allowed,
            strict_mode,
            member_exit_allowed_before_start,
            visibility,
            show_member_identities,
            arbiter_fee_percent,
            creator_lock_amount,
            first_distribution_threshold_percent,
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
            penalty_fee_amount,
            late_fee_amount,
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
            arbiter_address,
            emergency_stop_enabled,
            auto_refund_if_min_not_met,
            max_missed_payments_allowed,
            strict_mode,
            member_exit_allowed_before_start,
            visibility,
            show_member_identities,
            arbiter_fee_percent,
            creator_lock_amount,
            first_distribution_threshold_percent,
        ),
        ExecuteMsg::JoinCircle { circle_id } => execute_join_circle(deps, env, info, circle_id),
        ExecuteMsg::LockJoinDeposit { circle_id } => {
            execute_lock_join_deposit(deps, env, info, circle_id)
        }
        ExecuteMsg::InviteMember {
            circle_id,
            member_address,
        } => execute_invite_member(deps, env, info, circle_id, member_address),
        ExecuteMsg::AcceptInvite { circle_id } => execute_accept_invite(deps, env, info, circle_id),
        ExecuteMsg::ExitCircle { circle_id } => execute_exit_circle(deps, env, info, circle_id),
        ExecuteMsg::StartCircle { circle_id } => execute_start_circle(deps, env, info, circle_id),
        ExecuteMsg::DepositContribution { circle_id } => {
            execute_deposit_contribution(deps, env, info, circle_id)
        }
        ExecuteMsg::ProcessPayout { circle_id } => {
            execute_process_payout(deps, env, info, circle_id)
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
    }
}

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
    penalty_fee_amount: Uint128,
    late_fee_amount: Uint128,
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
    arbiter_address: Option<Addr>,
    emergency_stop_enabled: bool,
    auto_refund_if_min_not_met: bool,
    max_missed_payments_allowed: u32,
    strict_mode: bool,
    member_exit_allowed_before_start: bool,
    visibility: Visibility,
    show_member_identities: bool,
    arbiter_fee_percent: Option<u64>,
    creator_lock_amount: Uint128,
    first_distribution_threshold_percent: Option<u64>,
) -> Result<Response, ContractError> {
    // Validation
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
    
    // Validate creator_lock_amount: minimum 10 SAF (10000000 usaf with 6 decimals)
    const MIN_CREATOR_LOCK: Uint128 = Uint128::new(10_000_000u128); // 10 SAF
    if creator_lock_amount < MIN_CREATOR_LOCK {
        return Err(ContractError::InvalidParameters {
            msg: format!("creator_lock_amount must be at least 10 SAF ({} usaf)", MIN_CREATOR_LOCK),
        });
    }
    
    // Validate first_distribution_threshold_percent: maximum 60%
    if let Some(threshold) = first_distribution_threshold_percent {
        if threshold > 60 {
            return Err(ContractError::InvalidParameters {
                msg: "first_distribution_threshold_percent cannot exceed 60%".to_string(),
            });
        }
    }
    
    // Require creator to send the lock amount in minimal denom (usaf)
    let payment = must_pay(&info, "usaf")
        .map_err(|_| ContractError::InsufficientFunds {
            required: creator_lock_amount.to_string(),
            sent: "0".to_string(),
        })?;
    
    if payment < creator_lock_amount {
        return Err(ContractError::InsufficientFunds {
            required: creator_lock_amount.to_string(),
            sent: payment.to_string(),
        });
    }
    // Validate payout_order_list if provided
    // For PredefinedOrder (FIFO), if list is None, we'll use join order when circle starts
    if let Some(ref order_list) = payout_order_list {
        if order_list.len() as u32 != max_members {
            return Err(ContractError::InvalidParameters {
                msg: "payout_order_list length must match max_members".to_string(),
            });
        }
    }

    // Get next circle ID
    let circle_id = CIRCLE_COUNTER
        .may_load(deps.storage)?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| ContractError::InvalidParameters {
            msg: "Circle ID overflow".to_string(),
        })?;

    // Calculate payout amount
    let payout_amount = contribution_amount
        .checked_mul(Uint128::from(max_members))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount overflow".to_string(),
        })?;

    // Calculate end date if start_date is provided
    // One cycle = all members receive once = max_members rounds
    // Total duration = cycle_duration_days * max_members * total_cycles
    let end_date = start_date.map(|start| {
        Timestamp::from_seconds(
            start.seconds()
                + (cycle_duration_days as u64 * max_members as u64 * total_cycles as u64 * 86400),
        )
    });

    // Initialize payout order list
    // For PredefinedOrder: if list is provided, use it; if None, will use join order (FIFO) when circle starts
    // For RandomOrder: will be generated when circle starts
    let final_payout_order = match payout_order_type {
        PayoutOrderType::RandomOrder => None, // Will be generated when circle starts
        PayoutOrderType::PredefinedOrder => payout_order_list, // If None, will use members_list (FIFO) when circle starts
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
        members_list: vec![info.sender.clone()], // Creator is first member
        pending_members: vec![],
        member_exit_allowed_before_start,
        contribution_amount,
        denomination: "usaf".to_string(),
        payout_amount,
        penalty_fee_amount,
        late_fee_amount,
        platform_fee_percent: PLATFORM_CONFIG.load(deps.storage)?.platform_fee_percent,
        arbiter_fee_percent,
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
        manual_trigger_enabled,
        arbiter_address: if let Some(addr) = arbiter_address {
            Some(deps.api.addr_validate(addr.as_str())?)
        } else {
            None
        },
        emergency_stop_enabled,
        emergency_stop_triggered: false,
        auto_refund_if_min_not_met,
        max_missed_payments_allowed,
        strict_mode,
        escrow_address: Some(env.contract.address.clone()),
        total_amount_locked: Uint128::zero(),
        total_penalties_collected: Uint128::zero(),
        total_platform_fees_collected: Uint128::zero(),
        withdrawal_lock: false,
        refund_mode: RefundMode::FullRefund,
        circle_status: CircleStatus::Draft,
        current_cycle_index: 0,
        cycles_completed: 0,
        members_paid_this_cycle: vec![],
        members_late_this_cycle: vec![],
        visibility,
        show_member_identities,
        creator_lock_amount,
        first_distribution_threshold_percent,
    };

    CIRCLES.save(deps.storage, circle_id, &circle)?;
    CIRCLE_COUNTER.save(deps.storage, &circle_id)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "circle_created",
        &format!("Circle {} created by {}", circle_id, info.sender),
    )?;

    Ok(Response::new()
        .add_attribute("action", "create_circle")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("creator", info.sender))
}

fn execute_join_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Check if circle is open for joining
    if !matches!(circle.circle_status, CircleStatus::Draft | CircleStatus::Open) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Draft or Open".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    // Check if circle is full
    if circle.members_list.len() as u32 >= circle.max_members {
        return Err(ContractError::CircleFull {
            max: circle.max_members,
        });
    }

    // Check if already a member
    if circle.members_list.contains(&info.sender) {
        return Err(ContractError::AlreadyMember {
            address: info.sender.to_string(),
        });
    }

    // Check if invite-only (including private circles)
    if circle.invite_only || matches!(circle.visibility, Visibility::Private) {
        if !circle.pending_members.contains(&info.sender) {
            return Err(ContractError::InviteOnly { circle_id });
        }
        // Remove from pending
        circle.pending_members.retain(|m| m != &info.sender);
    }
    
    // Note: Join deposit is separate - user must call LockJoinDeposit separately

    // Add member
    circle.members_list.push(info.sender.clone());
    circle.updated_at = env.block.time;

    // Update status
    if circle.members_list.len() as u32 >= circle.max_members {
        circle.circle_status = CircleStatus::Full;
        
        // Auto-start by members: start immediately when last member joins
        if circle.auto_start_when_full {
            if let Some(ref auto_type) = circle.auto_start_type {
                if auto_type == "by_members" {
                    // Check minimum members requirement
                    if (circle.members_list.len() as u32) >= circle.min_members_required {
                        // Generate payout order if needed
                        if circle.payout_order_list.is_none() {
                            match circle.payout_order_type {
                                PayoutOrderType::RandomOrder => {
                                    // Generate random order
                                    let mut members = circle.members_list.clone();
                                    let seed = env.block.time.seconds() + circle_id as u64;
                                    for i in 0..members.len() {
                                        let j = (seed as usize + i * 7) % members.len();
                                        members.swap(i, j);
                                    }
                                    circle.payout_order_list = Some(members);
                                }
                                PayoutOrderType::PredefinedOrder => {
                                    // FIFO: Use join order (members_list as-is)
                                    circle.payout_order_list = Some(circle.members_list.clone());
                                }
                            }
                        }

                        // Set start date if not set
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
        }
    } else if circle.circle_status == CircleStatus::Draft {
        circle.circle_status = CircleStatus::Open;
    }

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    // Determine event type
    let event_type = if matches!(circle.circle_status, CircleStatus::Running) && circle.current_cycle_index == 1 {
        "circle_auto_started"
    } else {
        "member_joined"
    };
    
    let event_data = if event_type == "circle_auto_started" {
        format!("Circle {} auto-started when last member joined", circle_id)
    } else {
        format!("Member {} joined circle {}", info.sender, circle_id)
    };

    // Log event (single call to avoid moving deps multiple times)
    log_event(
        &mut deps,
        &env,
        circle_id,
        event_type,
        &event_data,
    )?;

    Ok(Response::new()
        .add_attribute("action", "join_circle")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender))
}

fn execute_invite_member(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    member_address: Addr,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Only creator or arbiter can invite
    if info.sender != circle.creator_address
        && circle.arbiter_address.as_ref() != Some(&info.sender)
    {
        return Err(ContractError::Unauthorized {
            msg: "Only creator or arbiter can invite members".to_string(),
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

fn execute_accept_invite(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    // Same as join_circle but with invite check
    execute_join_circle(deps, env, info, circle_id)
}

fn execute_exit_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Check if exit is allowed
    if !circle.member_exit_allowed_before_start {
        return Err(ContractError::ExitNotAllowed { circle_id });
    }

    // Check if circle has started - can't exit from Running/Paused/Completed
    if matches!(
        circle.circle_status,
        CircleStatus::Running | CircleStatus::Paused | CircleStatus::Completed
    ) {
        return Err(ContractError::ExitNotAllowed { circle_id });
    }

    // Check if member is in the circle
    if !circle.members_list.contains(&info.sender) {
        return Err(ContractError::Unauthorized {
            msg: "Not a member of this circle".to_string(),
        });
    }

    // Check if member has locked deposit and refund it
    let mut refund_amount = Uint128::zero();
    let mut messages = vec![];
    
    if let Ok(Some(locked_amount)) = MEMBER_LOCKED_AMOUNTS.may_load(deps.storage, (circle_id, info.sender.clone())) {
        refund_amount = locked_amount;
        
        // Remove locked amount record
        MEMBER_LOCKED_AMOUNTS.remove(deps.storage, (circle_id, info.sender.clone()));
        
        // Update total locked amount
        circle.total_amount_locked = circle
            .total_amount_locked
            .checked_sub(locked_amount)
            .unwrap_or(circle.total_amount_locked);
        
        // Create refund message
        messages.push(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: circle.denomination.clone(),
                amount: refund_amount,
            }],
        });
    }

    // Remove member
    circle.members_list.retain(|m| m != &info.sender);
    circle.updated_at = env.block.time;

    // Update status based on member count
    // Transition 9: Full → Open (if member exits and below max)
    if (circle.members_list.len() as u32) < circle.max_members
        && circle.circle_status == CircleStatus::Full
    {
        circle.circle_status = CircleStatus::Open;
    }
    
    // Transition 10: Open → Cancelled (if below min and auto_refund enabled)
    if (circle.members_list.len() as u32) < circle.min_members_required {
        if circle.auto_refund_if_min_not_met {
            circle.circle_status = CircleStatus::Cancelled;
        }
    }
    
    // Transition: Open → Draft (if only creator remains)
    if circle.members_list.len() == 1 && circle.circle_status == CircleStatus::Open {
        circle.circle_status = CircleStatus::Draft;
    }

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "member_exited",
        &format!("Member {} exited circle {} (refunded: {})", info.sender, circle_id, refund_amount),
    )?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "exit_circle")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("refund_amount", refund_amount.to_string()))
}

fn execute_start_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Only creator can start
    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can start circle".to_string(),
        });
    }

    // Check status
    if !matches!(
        circle.circle_status,
        CircleStatus::Open | CircleStatus::Full
    ) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Open or Full".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    // Check minimum members
    if (circle.members_list.len() as u32) < circle.min_members_required {
        return Err(ContractError::MinMembersNotMet {
            required: circle.min_members_required,
            current: circle.members_list.len() as u32,
        });
    }

    // Generate payout order if needed
    if circle.payout_order_list.is_none() {
        match circle.payout_order_type {
            PayoutOrderType::RandomOrder => {
                // Generate random order
                // Note: In production, use a proper random oracle or commit-reveal scheme
                // For now, we'll use a deterministic shuffle based on block time and circle_id
                let mut members = circle.members_list.clone();
                let seed = env.block.time.seconds() + circle_id as u64;
                for i in 0..members.len() {
                    let j = (seed as usize + i * 7) % members.len();
                    members.swap(i, j);
                }
                circle.payout_order_list = Some(members);
            }
            PayoutOrderType::PredefinedOrder => {
                // FIFO: Use join order (members_list as-is)
                circle.payout_order_list = Some(circle.members_list.clone());
            }
        }
    }

    // Set start date - always set it when starting
    let start_timestamp = circle.start_date.unwrap_or(env.block.time);
    circle.start_date = Some(start_timestamp);
    circle.first_cycle_date = Some(start_timestamp);
    circle.next_payout_date = Some(start_timestamp);

    // Calculate end date and archived date
    // Total rounds = max_members * total_cycles (one round per member per cycle)
    let total_rounds = circle.max_members * circle.total_cycles;
    let total_duration_seconds = circle.cycle_duration_days as u64 * total_rounds as u64 * 86400;
    let end_timestamp = Timestamp::from_seconds(start_timestamp.seconds() + total_duration_seconds);
    circle.end_date = Some(end_timestamp);
    
    // Calculate archived date = end_date + grace period
    let archived_timestamp = Timestamp::from_seconds(
        end_timestamp.seconds() + (circle.grace_period_hours as u64 * 3600)
    );

    circle.circle_status = CircleStatus::Running;
    circle.current_cycle_index = 1;
    circle.updated_at = env.block.time;

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    // Calculate full distribution calendar
    // Calculate all deposit deadlines and distribution dates
    let mut calendar_data = String::new();
    if let Some(payout_order) = &circle.payout_order_list {
        let mut round_number = 1u32;
        for cycle in 1..=circle.total_cycles {
            for (_member_idx, recipient) in payout_order.iter().enumerate() {
                // Calculate dates for this round
                let round_offset_seconds = ((round_number - 1) * circle.cycle_duration_days as u32) * 86400;
                let deposit_deadline = Timestamp::from_seconds(
                    start_timestamp.seconds() + round_offset_seconds as u64
                );
                let distribution_date = Timestamp::from_seconds(
                    start_timestamp.seconds() + round_offset_seconds as u64 + (circle.cycle_duration_days as u64 * 86400)
                );
                
                // Build calendar entry
                if !calendar_data.is_empty() {
                    calendar_data.push_str(",");
                }
                calendar_data.push_str(&format!(
                    "{{round:{},cycle:{},deposit_deadline:{},distribution_date:{},recipient:\"{}\"}}",
                    round_number,
                    cycle,
                    deposit_deadline.seconds(),
                    distribution_date.seconds(),
                    recipient
                ));
                
                round_number += 1;
            }
        }
    }
    
    // Emit events with calendar information
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
        &format!("{{start_date:{},end_date:{},archived_date:{},calendar:[{}]}}", 
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
        .add_attribute("total_rounds", (circle.max_members * circle.total_cycles).to_string()))
}

fn execute_deposit_contribution(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Check if circle is running
    if !matches!(circle.circle_status, CircleStatus::Running) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Running".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    // Check if member
    if !circle.members_list.contains(&info.sender) {
        return Err(ContractError::Unauthorized {
            msg: "Not a member of this circle".to_string(),
        });
    }

    // Check if member is blocked from this cycle
    if let Ok(Some(blocked_from_cycle)) = BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, info.sender.clone())) {
        if blocked_from_cycle <= circle.current_cycle_index {
            return Err(ContractError::InvalidParameters {
                msg: format!("Member is blocked from cycle {} onwards", blocked_from_cycle),
            });
        }
    }

    // Check if already deposited for this cycle
    if DEPOSITS
        .may_load(deps.storage, (circle_id, info.sender.clone(), circle.current_cycle_index))?
        .is_some()
    {
        return Err(ContractError::AlreadyDeposited {
            address: info.sender.to_string(),
            cycle: circle.current_cycle_index,
        });
    }

    // Check payment amount
    let payment = must_pay(&info, &circle.denomination)
        .map_err(|_| ContractError::InsufficientFunds {
            required: circle.contribution_amount.to_string(),
            sent: "0".to_string(),
        })?;

    let mut amount_to_pay = circle.contribution_amount;
    let mut is_late = false;

    // Check if late
    if let Some(next_payout) = circle.next_payout_date {
        let grace_period_end = next_payout.plus_seconds(circle.grace_period_hours as u64 * 3600);
        if env.block.time > grace_period_end {
            is_late = true;
            if circle.strict_mode {
                return Err(ContractError::MemberLate {
                    address: info.sender.to_string(),
                });
            }
            amount_to_pay = amount_to_pay
                .checked_add(circle.late_fee_amount)
                .map_err(|_| ContractError::InvalidParameters {
                    msg: "Amount overflow".to_string(),
                })?;
        }
    }

    if payment < amount_to_pay {
        return Err(ContractError::InsufficientFunds {
            required: amount_to_pay.to_string(),
            sent: payment.to_string(),
        });
    }

    // Record deposit
    let deposit = DepositRecord {
        member: info.sender.clone(),
        cycle: circle.current_cycle_index,
        amount: circle.contribution_amount,
        timestamp: env.block.time,
        on_time: !is_late,
    };

    DEPOSITS.save(
        deps.storage,
        (circle_id, info.sender.clone(), circle.current_cycle_index),
        &deposit,
    )?;

    // Update circle state
    circle.total_amount_locked = circle
        .total_amount_locked
        .checked_add(circle.contribution_amount)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Total amount overflow".to_string(),
        })?;

    if is_late {
        circle.members_late_this_cycle.push(info.sender.clone());
        circle.total_penalties_collected = circle
            .total_penalties_collected
            .checked_add(circle.late_fee_amount)
            .map_err(|_| ContractError::InvalidParameters {
                msg: "Penalties overflow".to_string(),
            })?;

        // Record penalty
        let penalty = PenaltyRecord {
            member: info.sender.clone(),
            cycle: circle.current_cycle_index,
            amount: circle.late_fee_amount,
            reason: "Late payment".to_string(),
            timestamp: env.block.time,
        };
        PENALTIES.save(
            deps.storage,
            (circle_id, info.sender.clone(), circle.current_cycle_index),
            &penalty,
        )?;
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
            "Member {} deposited for cycle {}",
            info.sender, circle.current_cycle_index
        ),
    )?;

    Ok(Response::new()
        .add_attribute("action", "deposit_contribution")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("cycle", circle.current_cycle_index.to_string())
        .add_attribute("amount", circle.contribution_amount.to_string()))
}

fn execute_process_payout(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Check if circle is running
    if !matches!(circle.circle_status, CircleStatus::Running) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Running".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    // Check authorization
    if circle.manual_trigger_enabled {
        if info.sender != circle.creator_address
            && circle.arbiter_address.as_ref() != Some(&info.sender)
        {
            return Err(ContractError::Unauthorized {
                msg: "Only creator or arbiter can trigger payout".to_string(),
            });
        }
    }

    // Check if cycle is ready
    if let Some(next_payout) = circle.next_payout_date {
        if env.block.time < next_payout {
            return Err(ContractError::CycleNotReady {
                next_date: next_payout.seconds(),
            });
        }
    }

    // For first distribution, check first_distribution_threshold_percent if set
    if circle.current_cycle_index == 1 {
        if let Some(threshold_percent) = circle.first_distribution_threshold_percent {
            // Count deposits for current cycle (excluding blocked members)
            let active_members: Vec<Addr> = circle.members_list
                .iter()
                .filter(|member| {
                    // Check if member is not blocked for this cycle
                    if let Ok(Some(blocked_from_cycle)) = BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, (*member).clone())) {
                        blocked_from_cycle > circle.current_cycle_index
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();
            
            let deposits_count = active_members
                .iter()
                .filter(|member| {
                    DEPOSITS
                        .may_load(deps.storage, (circle_id, (*member).clone(), circle.current_cycle_index))
                        .unwrap_or(None)
                        .is_some()
                })
                .count();
            // Compute required deposits as usize for direct comparison with deposits_count
            let required_deposits = (((active_members.len() as u64) * (threshold_percent as u64)) / 100) as usize;
            
            if deposits_count < required_deposits {
                return Err(ContractError::InvalidParameters {
                    msg: format!(
                        "First distribution threshold not met: need {}% ({} deposits), have {}",
                        threshold_percent,
                        required_deposits,
                        deposits_count
                    ),
                });
            }
        }
    }

    // Verify all active (non-blocked) members have paid
    let active_members: Vec<Addr> = circle.members_list
        .iter()
        .filter(|member| {
            // Check if member is not blocked for this cycle
            if let Ok(Some(blocked_from_cycle)) = BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, (*member).clone())) {
                blocked_from_cycle > circle.current_cycle_index
            } else {
                true
            }
        })
        .cloned()
        .collect();

    let all_paid = active_members
        .iter()
        .all(|member| {
            DEPOSITS
                .may_load(
                    deps.storage,
                    (circle_id, member.clone(), circle.current_cycle_index),
                )
                .unwrap_or(None)
                .is_some()
        });

    if !all_paid {
        return Err(ContractError::InvalidParameters {
            msg: "Not all active members have paid for this cycle".to_string(),
        });
    }
    
    // Include blocked member funds: use their locked join deposits if they didn't deposit
    // This is handled when calculating payout amount below

    // Get payout recipient
    let recipient = if let Some(ref order_list) = circle.payout_order_list {
        let index = (circle.current_cycle_index as usize - 1) % order_list.len();
        order_list[index].clone()
    } else {
        return Err(ContractError::InvalidParameters {
            msg: "Payout order not set".to_string(),
        });
    };

    // Calculate payout amount including blocked member funds
    // Base payout is contribution_amount * active_members
    let active_member_count = active_members.len() as u32;
    let base_payout = circle.contribution_amount
        .checked_mul(Uint128::from(active_member_count))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount overflow".to_string(),
        })?;
    
    // Add blocked member funds (their locked join deposits)
    let mut total_blocked_funds = Uint128::zero();
    let blocked_for_cycle: Vec<Addr> = circle.members_list
        .iter()
        .filter(|member| {
            // Check if member is blocked for this cycle
            if let Ok(Some(blocked_from_cycle)) = BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, (*member).clone())) {
                blocked_from_cycle <= circle.current_cycle_index
            } else {
                false
            }
        })
        .cloned()
        .collect();
    
    for blocked_member in blocked_for_cycle {
        // Check if they didn't deposit (use their locked funds)
        let has_deposited = DEPOSITS
            .may_load(deps.storage, (circle_id, blocked_member.clone(), circle.current_cycle_index))
            .unwrap_or(None)
            .is_some();
        
        if !has_deposited {
            // Use their locked join deposit
            if let Some(locked) = MEMBER_LOCKED_AMOUNTS.may_load(deps.storage, (circle_id, blocked_member.clone()))? {
                total_blocked_funds = total_blocked_funds
                    .checked_add(locked)
                    .map_err(|_| ContractError::InvalidParameters {
                        msg: "Blocked funds overflow".to_string(),
                    })?;
            }
        }
    }
    
    let mut payout_amount = base_payout
        .checked_add(total_blocked_funds)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount overflow".to_string(),
        })?;

    // Calculate fees on the final payout amount
    let platform_fee = payout_amount
        .multiply_ratio(circle.platform_fee_percent, 10000u64);
    let arbiter_fee = circle.arbiter_fee_percent.map(|percent| {
        payout_amount
            .multiply_ratio(percent, 10000u64)
    });
    payout_amount = payout_amount
        .checked_sub(platform_fee)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Payout amount underflow".to_string(),
        })?;

    if let Some(arb_fee) = arbiter_fee {
        payout_amount = payout_amount
            .checked_sub(arb_fee)
            .map_err(|_| ContractError::InvalidParameters {
                msg: "Payout amount underflow".to_string(),
            })?;
    }

    // Record payout
    let payout = PayoutRecord {
        cycle: circle.current_cycle_index,
        recipient: recipient.clone(),
        amount: payout_amount,
        timestamp: env.block.time,
        transaction_hash: None,
    };

    PAYOUTS.save(
        deps.storage,
        (circle_id, circle.current_cycle_index),
        &payout,
    )?;

    // Update circle state
    // Subtract the base payout amount (active member deposits)
    circle.total_amount_locked = circle
        .total_amount_locked
        .checked_sub(base_payout)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Total amount underflow".to_string(),
        })?;
    
    // Also subtract blocked funds that were used
    if !total_blocked_funds.is_zero() {
        circle.total_amount_locked = circle
            .total_amount_locked
            .checked_sub(total_blocked_funds)
            .map_err(|_| ContractError::InvalidParameters {
                msg: "Total amount underflow".to_string(),
            })?;
    }

    circle.total_platform_fees_collected = circle
        .total_platform_fees_collected
        .checked_add(platform_fee)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Platform fees overflow".to_string(),
        })?;

    circle.cycles_completed += 1;
    circle.members_paid_this_cycle.clear();
    circle.members_late_this_cycle.clear();

    // Check if last cycle
    if circle.current_cycle_index >= circle.total_cycles {
        circle.circle_status = CircleStatus::Completed;
    } else {
        circle.current_cycle_index += 1;
        // Set next payout date
        if let Some(current_date) = circle.next_payout_date {
            circle.next_payout_date = Some(Timestamp::from_seconds(
                current_date.seconds() + (circle.cycle_duration_days as u64 * 86400),
            ));
        }
    }

    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    // Build bank message for payout
    let messages = vec![BankMsg::Send {
        to_address: recipient.to_string(),
        amount: vec![Coin {
            denom: circle.denomination.clone(),
            amount: payout_amount,
        }],
    }];

    log_event(
        &mut deps,
        &env,
        circle_id,
        "payout_processed",
        &format!(
            "Payout processed for cycle {} to {}",
            payout.cycle, recipient
        ),
    )?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "process_payout")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("cycle", payout.cycle.to_string())
        .add_attribute("recipient", recipient)
        .add_attribute("amount", payout_amount.to_string()))
}

fn execute_pause_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Only creator or arbiter can pause
    if info.sender != circle.creator_address
        && circle.arbiter_address.as_ref() != Some(&info.sender)
    {
        return Err(ContractError::Unauthorized {
            msg: "Only creator or arbiter can pause circle".to_string(),
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

    log_event(
        &mut deps,
        &env,
        circle_id,
        "circle_paused",
        &format!("Circle {} paused by {}", circle_id, info.sender),
    )?;

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

    // Only creator or arbiter can unpause
    if info.sender != circle.creator_address
        && circle.arbiter_address.as_ref() != Some(&info.sender)
    {
        return Err(ContractError::Unauthorized {
            msg: "Only creator or arbiter can unpause circle".to_string(),
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

    log_event(
        &mut deps,
        &env,
        circle_id,
        "circle_unpaused",
        &format!("Circle {} unpaused by {}", circle_id, info.sender),
    )?;

    Ok(Response::new()
        .add_attribute("action", "unpause_circle")
        .add_attribute("circle_id", circle_id.to_string()))
}

fn execute_emergency_stop(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    if !circle.emergency_stop_enabled {
        return Err(ContractError::InvalidParameters {
            msg: "Emergency stop not enabled for this circle".to_string(),
        });
    }

    // Only arbiter can trigger emergency stop
    if circle.arbiter_address.as_ref() != Some(&info.sender) {
        return Err(ContractError::ArbiterOnly {});
    }

    circle.emergency_stop_triggered = true;
    circle.circle_status = CircleStatus::Paused;
    circle.updated_at = env.block.time;

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "emergency_stop",
        &format!("Emergency stop triggered for circle {}", circle_id),
    )?;

    Ok(Response::new()
        .add_attribute("action", "emergency_stop")
        .add_attribute("circle_id", circle_id.to_string()))
}

fn execute_cancel_circle(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Only creator can cancel before start, creator or arbiter after start
    if info.sender != circle.creator_address
        && circle.arbiter_address.as_ref() != Some(&info.sender)
    {
        return Err(ContractError::Unauthorized {
            msg: "Only creator or arbiter can cancel circle".to_string(),
        });
    }

    // Can only cancel if not completed
    if matches!(circle.circle_status, CircleStatus::Completed) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Not Completed".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    circle.circle_status = CircleStatus::Cancelled;
    circle.updated_at = env.block.time;

    CIRCLES.save(deps.storage, circle_id, &circle)?;

    // Process refunds based on refund mode
    // This is a simplified version - full implementation would handle all refunds
    log_event(
        &mut deps,
        &env,
        circle_id,
        "circle_cancelled",
        &format!("Circle {} cancelled by {}", circle_id, info.sender),
    )?;

    Ok(Response::new()
        .add_attribute("action", "cancel_circle")
        .add_attribute("circle_id", circle_id.to_string()))
}

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

    // Only creator can update
    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can update circle".to_string(),
        });
    }

    // Can only update if not started
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

fn execute_withdraw_platform_fees(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _circle_id: Option<u64>,
) -> Result<Response, ContractError> {
    // This would require storing platform address in config
    // Simplified implementation
    Err(ContractError::InvalidParameters {
        msg: "Platform fee withdrawal not yet implemented".to_string(),
    })
}

fn execute_lock_join_deposit(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Check if circle status allows locking (Open or Draft)
    if !matches!(circle.circle_status, CircleStatus::Draft | CircleStatus::Open) {
        return Err(ContractError::InvalidCircleStatus {
            expected: "Draft or Open".to_string(),
            actual: format!("{:?}", circle.circle_status),
        });
    }

    // Check if user is member OR pending member (auto-accept if pending)
    let is_member = circle.members_list.contains(&info.sender);
    let is_pending = circle.pending_members.contains(&info.sender);
    
    if !is_member && !is_pending {
        return Err(ContractError::Unauthorized {
            msg: "Not a member or invited to this circle".to_string(),
        });
    }

    // If pending, accept the invite automatically
    if is_pending {
        // Remove from pending and add to members
        circle.pending_members.retain(|m| m != &info.sender);
        circle.members_list.push(info.sender.clone());
        
        // Update status if needed
        if circle.members_list.len() as u32 >= circle.max_members {
            circle.circle_status = CircleStatus::Full;
        } else if circle.circle_status == CircleStatus::Draft {
            circle.circle_status = CircleStatus::Open;
        }
    }

    // Check if already locked
    if MEMBER_LOCKED_AMOUNTS
        .may_load(deps.storage, (circle_id, info.sender.clone()))?
        .is_some()
    {
        return Err(ContractError::InvalidParameters {
            msg: "Join deposit already locked".to_string(),
        });
    }

    // Require user to send the contribution amount as join deposit
    let payment = must_pay(&info, &circle.denomination)
        .map_err(|_| ContractError::InsufficientFunds {
            required: circle.contribution_amount.to_string(),
            sent: "0".to_string(),
        })?;

    if payment < circle.contribution_amount {
        return Err(ContractError::InsufficientFunds {
            required: circle.contribution_amount.to_string(),
            sent: payment.to_string(),
        });
    }

    // Store locked amount
    MEMBER_LOCKED_AMOUNTS.save(
        deps.storage,
        (circle_id, info.sender.clone()),
        &circle.contribution_amount,
    )?;

    // Update total locked amount
    circle.total_amount_locked = circle
        .total_amount_locked
        .checked_add(circle.contribution_amount)
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Total amount overflow".to_string(),
        })?;

    circle.updated_at = env.block.time;
    CIRCLES.save(deps.storage, circle_id, &circle)?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "join_deposit_locked",
        &format!("Member {} locked join deposit {}", info.sender, circle.contribution_amount),
    )?;

    Ok(Response::new()
        .add_attribute("action", "lock_join_deposit")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", info.sender)
        .add_attribute("amount", circle.contribution_amount.to_string())
        .add_attribute("auto_accepted", is_pending.to_string()))
}

fn execute_add_private_member(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    circle_id: u64,
    member_address: Addr,
    pseudonym: Option<String>,
) -> Result<Response, ContractError> {
    let mut circle = CIRCLES.load(deps.storage, circle_id)?;

    // Require caller is creator
    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can add private members".to_string(),
        });
    }

    // Require circle is private
    if !matches!(circle.visibility, Visibility::Private) {
        return Err(ContractError::InvalidParameters {
            msg: "Circle must be private to use AddPrivateMember".to_string(),
        });
    }

    // Check if circle is full
    if circle.members_list.len() as u32 >= circle.max_members {
        return Err(ContractError::CircleFull {
            max: circle.max_members,
        });
    }

    // Check if already a member
    if circle.members_list.contains(&member_address) {
        return Err(ContractError::AlreadyMember {
            address: member_address.to_string(),
        });
    }

    // Validate member address
    let validated_address = deps.api.addr_validate(member_address.as_str())?;

    // Add to members list
    circle.members_list.push(validated_address.clone());

    // Update private member list
    let mut private_members = PRIVATE_MEMBER_LIST
        .may_load(deps.storage, circle_id)?
        .unwrap_or_default();
    private_members.push(validated_address.clone());
    PRIVATE_MEMBER_LIST.save(deps.storage, circle_id, &private_members)?;

    // Set pseudonym if provided
    if let Some(pseudo) = pseudonym {
        MEMBER_PSEUDONYMS.save(
            deps.storage,
            (circle_id, validated_address.clone()),
            &pseudo,
        )?;
    }

    circle.updated_at = env.block.time;
    
    // Update status
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
        &format!("Private member {} added by {}", validated_address, info.sender),
    )?;

    Ok(Response::new()
        .add_attribute("action", "add_private_member")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", validated_address))
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

    // Require caller is creator or arbiter
    if info.sender != circle.creator_address 
        && circle.arbiter_address.as_ref() != Some(&info.sender) {
        return Err(ContractError::Unauthorized {
            msg: "Only creator or arbiter can update pseudonyms".to_string(),
        });
    }

    // Validate member address
    let validated_address = deps.api.addr_validate(member_address.as_str())?;

    // Check if member exists (in members_list OR pending_members)
    let is_member = circle.members_list.contains(&validated_address);
    let is_pending = circle.pending_members.contains(&validated_address);
    
    if !is_member && !is_pending {
        return Err(ContractError::InvalidParameters {
            msg: "Address not found in circle members or pending invitations".to_string(),
        });
    }

    // Update pseudonym (can be set for both members and pending members)
    MEMBER_PSEUDONYMS.save(
        deps.storage,
        (circle_id, validated_address.clone()),
        &pseudonym,
    )?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "member_pseudonym_updated",
        &format!("Pseudonym '{}' set for {}", pseudonym, validated_address),
    )?;

    Ok(Response::new()
        .add_attribute("action", "update_member_pseudonym")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", validated_address)
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

    // Require caller is creator/admin or automatic (for missed deposits)
    // For now, only creator/admin can block
    if info.sender != circle.creator_address {
        // TODO: Check if admin in future
        return Err(ContractError::Unauthorized {
            msg: "Only creator can block members".to_string(),
        });
    }

    // Validate member address
    let validated_address = deps.api.addr_validate(member_address.as_str())?;

    // Check if member exists
    if !circle.members_list.contains(&validated_address) {
        return Err(ContractError::InvalidParameters {
            msg: "Member not found in circle".to_string(),
        });
    }

    // Block member - record which cycle they were blocked from
    let blocked_from_cycle = circle.current_cycle_index + 1; // Block from next cycle
    BLOCKED_MEMBERS.save(
        deps.storage,
        (circle_id, validated_address.clone()),
        &blocked_from_cycle,
    )?;

    log_event(
        &mut deps,
        &env,
        circle_id,
        "member_blocked",
        &format!("Member {} blocked from cycle {}", validated_address, blocked_from_cycle),
    )?;

    Ok(Response::new()
        .add_attribute("action", "block_member")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("member", validated_address)
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

    // Only creator/admin can trigger distribution
    if info.sender != circle.creator_address {
        return Err(ContractError::Unauthorized {
            msg: "Only creator can distribute blocked funds".to_string(),
        });
    }

    // Get all blocked members for this circle
    let all_members = circle.members_list.clone();
    let mut total_blocked_funds = Uint128::zero();
    let mut blocked_in_cycle = Vec::new();

    for member in all_members {
        if let Some(blocked_cycle) = BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, member.clone()))? {
            if blocked_cycle <= cycle {
                // Get their locked amount
                if let Some(locked) = MEMBER_LOCKED_AMOUNTS.may_load(deps.storage, (circle_id, member.clone()))? {
                    total_blocked_funds = total_blocked_funds
                        .checked_add(locked)
                        .map_err(|_| ContractError::InvalidParameters {
                            msg: "Amount overflow".to_string(),
                        })?;
                    blocked_in_cycle.push((member, locked));
                }
            }
        }
    }

    if total_blocked_funds.is_zero() {
        return Err(ContractError::InvalidParameters {
            msg: "No blocked funds to distribute".to_string(),
        });
    }

    // Get active members who have deposited (for proportional distribution)
    let active_members: Vec<Addr> = circle.members_list
        .iter()
        .filter(|member| {
            // Check if member is not blocked or blocked after this cycle
            if let Ok(Some(blocked_cycle)) = BLOCKED_MEMBERS.may_load(deps.storage, (circle_id, (*member).clone())) {
                blocked_cycle > cycle
            } else {
                true
            }
        })
        .filter(|member| {
            // Check if member has deposited for this cycle
            DEPOSITS.may_load(deps.storage, (circle_id, (*member).clone(), cycle))
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

    // Distribute proportionally to active members
    let amount_per_member = total_blocked_funds
        .checked_div(Uint128::from(active_members.len() as u128))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Division error".to_string(),
        })?;
    let remainder = total_blocked_funds
        .checked_rem(Uint128::from(active_members.len() as u128))
        .map_err(|_| ContractError::InvalidParameters {
            msg: "Remainder calculation error".to_string(),
        })?;

    let mut messages = Vec::new();
    for (idx, member) in active_members.iter().enumerate() {
        let mut amount = amount_per_member;
        // Give remainder to first member
        if idx == 0 {
            amount = amount
                .checked_add(remainder)
                .map_err(|_| ContractError::InvalidParameters {
                    msg: "Amount overflow".to_string(),
                })?;
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
        &format!("Blocked funds {} distributed to {} active members", total_blocked_funds, active_members.len()),
    )?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "distribute_blocked_funds")
        .add_attribute("circle_id", circle_id.to_string())
        .add_attribute("cycle", cycle.to_string())
        .add_attribute("total_distributed", total_blocked_funds.to_string()))
}

// Helper function to log events
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

    let event = EventLog {
        event_type: event_type.to_string(),
        circle_id,
        data: data.to_string(),
        timestamp: env.block.time,
    };

    EVENTS.save(deps.storage, (circle_id, event_id), &event)?;
    EVENT_COUNTER.save(deps.storage, circle_id, &event_id)?;

    Ok(())
}

