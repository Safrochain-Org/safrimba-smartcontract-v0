use cosmwasm_std::{Addr, Uint128, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{CircleStatus, DistributionThreshold, PayoutOrderType, Visibility};

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
        /// Exit penalty in basis points of locked amount (e.g. 2000 = 20%)
        exit_penalty_percent: u64,
        /// Late fee per missed round, in basis points of contribution_amount (e.g. 1000 = 10%)
        late_fee_percent: u64,
        total_cycles: u32,
        cycle_duration_days: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        start_date: Option<Timestamp>,
        grace_period_hours: u32,
        auto_start_when_full: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        auto_start_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        auto_start_date: Option<Timestamp>,
        payout_order_type: PayoutOrderType,
        #[serde(skip_serializing_if = "Option::is_none")]
        payout_order_list: Option<Vec<Addr>>,
        auto_payout_enabled: bool,
        manual_trigger_enabled: bool,
        emergency_stop_enabled: bool,
        auto_refund_if_min_not_met: bool,
        strict_mode: bool,
        visibility: Visibility,
        show_member_identities: bool,
        /// When the first distribution happens each cycle (round-based). None = round 1; Total = 100% of all members (last round only); MinMembers(count) = wait count rounds.
        /// For Public circles this is forced to Total regardless of input.
        #[serde(skip_serializing_if = "Option::is_none")]
        distribution_threshold: Option<DistributionThreshold>,
    },
    /// Join a circle — must attach exactly contribution_amount in usaf as join deposit (locked as security)
    JoinCircle {
        circle_id: u64,
    },
    /// Accept an invite — must attach exactly contribution_amount in usaf as join deposit
    AcceptInvite {
        circle_id: u64,
    },
    InviteMember {
        circle_id: u64,
        member_address: Addr,
    },
    /// Exit circle. Before start: full refund. After start (strict_mode=false only): refund locked minus accumulated late fees minus exit penalty.
    ExitCircle {
        circle_id: u64,
    },
    StartCircle {
        circle_id: u64,
    },
    /// Deposit contribution for current round. Attach exactly contribution_amount usaf. Late deposits are accepted — late fee is tracked against locked amount.
    DepositContribution {
        circle_id: u64,
    },
    /// Trigger round payout. Anyone can call when manual_trigger_enabled=false and now>=next_payout_date.
    ProcessPayout {
        circle_id: u64,
    },
    /// Withdraw all pending (accumulated) payouts owed to caller. Callable anytime by any member who has pending payouts.
    Withdraw {
        circle_id: u64,
    },
    /// Permissionless: check all members for ejection condition (accumulated_late_fees + exit_penalty >= locked) and eject automatically.
    CheckAndEject {
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
    /// Cancel circle. Before start: full refunds. During Running: creator forfeits creator_lock_amount distributed to active members; all deposits for current cycle refunded.
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
        circle_id: Option<u64>,
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
    // Staking
    EnableStaking {
        circle_id: u64,
        validator_address: String,
    },
    DisableStaking {
        circle_id: u64,
    },
    ClaimPendingRefund {
        circle_id: u64,
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

    // Pending payouts and late fees
    GetPendingPayout { circle_id: u64, member: Addr },
    GetMemberAccumulatedLateFees { circle_id: u64, member: Addr },

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
    // Staking
    GetCircleStakingInfo { circle_id: u64 },
    GetPendingRefunds {
        circle_id: u64,
        member: Option<Addr>,
    },
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
    pub total_pending_payouts: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MemberStatsResponse {
    pub member: Addr,
    pub circles_joined: u32,
    pub total_contributed: Uint128,
    pub total_received: Uint128,
    pub total_penalties: Uint128,
    pub missed_payments: u32,
    pub pending_payout: Uint128,
    pub accumulated_late_fees: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MemberLockedAmountResponse {
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct BlockedMembersResponse {
    pub blocked_members: Vec<(Addr, u32)>,
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
    pub distribution_occurs: bool,
    pub recipient: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ArchivedDateResponse {
    pub archived_date: Option<Timestamp>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PendingPayoutResponse {
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct AccumulatedLateFeesResponse {
    pub amount: Uint128,
    /// Number of rounds currently used to calculate (missed_rounds * late_fee_per_round)
    pub missed_rounds: u32,
    pub late_fee_per_round: Uint128,
    pub exit_penalty: Uint128,
    pub locked_amount: Uint128,
    /// Rounds remaining before ejection
    pub rounds_until_ejection: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CircleStakingInfoResponse {
    pub enabled: bool,
    pub validator_address: Option<String>,
    pub staked_amount: Uint128,
    pub total_rewards_earned: Uint128,
    pub rewards_accumulated: Uint128,
    pub last_claim_at: Option<Timestamp>,
    pub pending_undelegations: Vec<crate::state::PendingUndelegation>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PendingRefundsResponse {
    pub refunds: Vec<crate::state::PendingRefundRecord>,
}
