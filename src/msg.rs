use cosmwasm_std::{Addr, Uint128, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{CircleStatus, PayoutOrderType, Visibility};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub platform_fee_percent: u64, // Basis points
    pub platform_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Circle Management
    CreateCircle {
        circle_name: String,
        circle_description: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        circle_image: Option<String>,
        max_members: u32,
        min_members_required: u32,
        invite_only: bool,
        contribution_amount: Uint128,
        penalty_fee_amount: Uint128,
        late_fee_amount: Uint128,
        total_cycles: u32,
        cycle_duration_days: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        start_date: Option<Timestamp>,
        grace_period_hours: u32,
        auto_start_when_full: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        auto_start_type: Option<String>, // "by_members" or "by_date"
        #[serde(skip_serializing_if = "Option::is_none")]
        auto_start_date: Option<Timestamp>, // Date for auto-start if type is "by_date"
        payout_order_type: PayoutOrderType,
        #[serde(skip_serializing_if = "Option::is_none")]
        payout_order_list: Option<Vec<Addr>>,
        auto_payout_enabled: bool,
        manual_trigger_enabled: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        arbiter_address: Option<Addr>,
        emergency_stop_enabled: bool,
        auto_refund_if_min_not_met: bool,
        max_missed_payments_allowed: u32,
        strict_mode: bool,
        member_exit_allowed_before_start: bool,
        visibility: Visibility,
        show_member_identities: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        arbiter_fee_percent: Option<u64>,
        creator_lock_amount: Uint128, // Minimum 10 SAF
        #[serde(skip_serializing_if = "Option::is_none")]
        first_distribution_threshold_percent: Option<u64>, // Max 60%
    },
    JoinCircle {
        circle_id: u64,
    },
    LockJoinDeposit {
        circle_id: u64,
    },
    InviteMember {
        circle_id: u64,
        member_address: Addr,
    },
    AcceptInvite {
        circle_id: u64,
    },
    ExitCircle {
        circle_id: u64,
    },
    StartCircle {
        circle_id: u64,
    },
    DepositContribution {
        circle_id: u64,
    },
    ProcessPayout {
        circle_id: u64,
    },
    PauseCircle {
        circle_id: u64,
    },
    UnpauseCircle {
        circle_id: u64,
    },
    EmergencyStop {
        circle_id: u64,
    },
    CancelCircle {
        circle_id: u64,
    },
    UpdateCircle {
        circle_id: u64,
        circle_name: Option<String>,
        circle_description: Option<String>,
        circle_image: Option<String>,
    },
    WithdrawPlatformFees {
        circle_id: Option<u64>, // If None, withdraw all
    },
    // Private Circle and Member Management
    AddPrivateMember {
        circle_id: u64,
        member_address: Addr,
        #[serde(skip_serializing_if = "Option::is_none")]
        pseudonym: Option<String>,
    },
    UpdateMemberPseudonym {
        circle_id: u64,
        member_address: Addr,
        pseudonym: String,
    },
    BlockMember {
        circle_id: u64,
        member_address: Addr,
    },
    DistributeBlockedFunds {
        circle_id: u64,
        cycle: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // Circle Queries
    GetCircle { circle_id: u64 },
    GetCircles {
        start_after: Option<u64>,
        limit: Option<u32>,
        status: Option<CircleStatus>,
        creator: Option<Addr>,
    },
    GetCircleMembers { circle_id: u64 },
    GetCircleStatus { circle_id: u64 },
    
    // Cycle Queries
    GetCurrentCycle { circle_id: u64 },
    GetCycleDeposits { circle_id: u64, cycle: u32 },
    GetMemberDeposits { circle_id: u64, member: Addr },
    
    // Payout Queries
    GetPayouts { circle_id: u64 },
    GetPayoutHistory { circle_id: u64, cycle: Option<u32> },
    
    // Financial Queries
    GetCircleBalance { circle_id: u64 },
    GetMemberBalance { circle_id: u64, member: Addr },
    GetPenalties { circle_id: u64, member: Option<Addr> },
    GetRefunds { circle_id: u64 },
    
    // Event Queries
    GetEvents { circle_id: u64, limit: Option<u32> },
    
    // Statistics
    GetCircleStats { circle_id: u64 },
    GetMemberStats { circle_id: u64, member: Addr },
    
    // Locking and Private Circle Queries
    GetMemberLockedAmount { circle_id: u64, member: Addr },
    GetBlockedMembers { circle_id: u64 },
    GetMemberPseudonym { circle_id: u64, member: Addr },
    GetPrivateMembers { circle_id: u64 },
    GetDistributionCalendar { circle_id: u64 },
    GetArchivedDate { circle_id: u64 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CircleResponse {
    pub circle: crate::state::Circle,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CirclesResponse {
    pub circles: Vec<crate::state::Circle>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MembersResponse {
    pub members: Vec<Addr>,
    pub pending_members: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct StatusResponse {
    pub status: CircleStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CycleResponse {
    pub current_cycle: u32,
    pub total_cycles: u32,
    pub next_payout_date: Option<Timestamp>,
    pub members_paid: Vec<Addr>,
    pub members_late: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct DepositsResponse {
    pub deposits: Vec<crate::state::DepositRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PayoutsResponse {
    pub payouts: Vec<crate::state::PayoutRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct BalanceResponse {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PenaltiesResponse {
    pub penalties: Vec<crate::state::PenaltyRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RefundsResponse {
    pub refunds: Vec<crate::state::RefundRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct EventsResponse {
    pub events: Vec<crate::state::EventLog>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CircleStatsResponse {
    pub circle_id: u64,
    pub total_members: u32,
    pub total_cycles: u32,
    pub cycles_completed: u32,
    pub total_amount_locked: Uint128,
    pub total_payouts: Uint128,
    pub total_penalties: Uint128,
    pub total_platform_fees: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MemberStatsResponse {
    pub member: Addr,
    pub circles_joined: u32,
    pub total_contributed: Uint128,
    pub total_received: Uint128,
    pub total_penalties: Uint128,
    pub missed_payments: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MemberLockedAmountResponse {
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct BlockedMembersResponse {
    pub blocked_members: Vec<(Addr, u32)>, // (member_address, blocked_from_cycle)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MemberPseudonymResponse {
    pub pseudonym: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PrivateMembersResponse {
    pub members: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct DistributionCalendarResponse {
    pub rounds: Vec<CalendarRound>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CalendarRound {
    pub round_number: u32,
    pub cycle_number: u32,
    pub deposit_deadline: Timestamp,
    pub distribution_date: Timestamp,
    pub recipient: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ArchivedDateResponse {
    pub archived_date: Option<Timestamp>,
}

