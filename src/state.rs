use cosmwasm_std::{Addr, Uint128, Timestamp};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Circle {
    // General Information
    pub circle_id: u64,
    pub circle_name: String,
    pub circle_description: String,
    pub circle_image: Option<String>,
    pub creator_address: Addr,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,

    // Membership Parameters
    pub max_members: u32,
    pub min_members_required: u32,
    pub invite_only: bool,
    pub members_list: Vec<Addr>,
    pub pending_members: Vec<Addr>,

    // Financial Parameters (using SAF, fees in basis points)
    pub contribution_amount: Uint128,
    pub denomination: String, // Always "usaf"
    pub payout_amount: Uint128, // contribution_amount * max_members
    /// Exit penalty in basis points (e.g. 2000 = 20% of locked amount). Applied on ejection or voluntary exit before all cycles end.
    pub exit_penalty_percent: u64,
    /// Late fee per missed round, in basis points of contribution_amount (e.g. 1000 = 10%). Accumulated against locked amount.
    pub late_fee_percent: u64,
    pub platform_fee_percent: u64, // Basis points (10000 = 100%)
    /// Dynamically calculated from % penalty and late fee. Set at start, updated on exit/ejection.
    /// Formula: floor((10000 - exit_penalty_percent) / late_fee_percent) * active_members / members_at_start
    pub max_missed_payments_allowed: u32,
    /// Number of active members when circle started. Set at StartCircle, used to scale max_missed on exit/ejection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members_at_start: Option<u32>,

    // Cycle & Time Parameters
    pub total_cycles: u32,
    pub cycle_duration_days: u32,
    pub start_date: Option<Timestamp>,
    pub first_cycle_date: Option<Timestamp>,
    pub next_payout_date: Option<Timestamp>,
    pub end_date: Option<Timestamp>,
    pub grace_period_hours: u32,
    pub auto_start_when_full: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_start_type: Option<String>, // "by_members" or "by_date"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_start_date: Option<Timestamp>,

    // Payout Logic Parameters
    pub payout_order_type: PayoutOrderType,
    pub payout_order_list: Option<Vec<Addr>>,
    pub auto_payout_enabled: bool,
    pub manual_trigger_enabled: bool,

    // Security & Risk Controls
    pub emergency_stop_enabled: bool,
    pub emergency_stop_triggered: bool,
    pub auto_refund_if_min_not_met: bool,
    pub strict_mode: bool,

    // Escrow and Funds Management
    pub escrow_address: Option<Addr>,
    pub total_amount_locked: Uint128,
    pub total_penalties_collected: Uint128,
    pub total_platform_fees_collected: Uint128,
    pub total_pending_payouts: Uint128,
    pub withdrawal_lock: bool,
    pub refund_mode: RefundMode,

    // Locking and Security Features
    pub creator_lock_amount: Uint128, // = contribution_amount * (1 + max_members * 0.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution_threshold: Option<DistributionThreshold>,

    // Internal State Parameters
    pub circle_status: CircleStatus,
    pub current_cycle_index: u32,
    pub cycles_completed: u32,
    pub members_paid_this_cycle: Vec<Addr>,
    pub members_late_this_cycle: Vec<Addr>,

    // Optional UX / Customization Parameters
    pub visibility: Visibility,
    pub show_member_identities: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum CircleStatus {
    Draft,
    Open,
    Full,
    Running,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum PayoutOrderType {
    PredefinedOrder,
    RandomOrder,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum RefundMode {
    FullRefund,
    PartialRefund,
    AutoDistribute,
}

/// When the first distribution happens each cycle (round-based, not deposit count).
/// Applied to every cycle: determines which rounds have a payout.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DistributionThreshold {
    /// 100% of all members: one distribution only at the last round of each cycle.
    Total,
    /// Wait `count` rounds, then one distribution per round through end of cycle.
    MinMembers { count: u32 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PayoutRecord {
    pub cycle: u32,
    pub recipient: Addr,
    pub amount: Uint128,
    pub timestamp: Timestamp,
    pub transaction_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct DepositRecord {
    pub member: Addr,
    pub cycle: u32,
    pub amount: Uint128,
    pub timestamp: Timestamp,
    pub on_time: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PenaltyRecord {
    pub member: Addr,
    pub cycle: u32,
    pub amount: Uint128,
    pub reason: String,
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RefundRecord {
    pub member: Addr,
    pub amount: Uint128,
    pub reason: String,
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct EventLog {
    pub event_type: String,
    pub circle_id: u64,
    pub data: String,
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MemberMissedPayments {
    pub member: Addr,
    pub missed_count: u32,
    pub last_missed_cycle: Option<u32>,
}

// Platform configuration stored at contract level
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PlatformConfig {
    pub platform_fee_percent: u64,
    pub platform_address: Addr,
}

// Staking configuration per circle
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CircleStakingConfig {
    pub enabled: bool,
    pub validator_address: String,
    pub staked_amount: Uint128,
    pub total_rewards_earned: Uint128,
    pub rewards_accumulated: Uint128,
    pub last_claim_at: Option<Timestamp>,
    pub pending_undelegations: Vec<PendingUndelegation>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PendingUndelegation {
    pub amount: Uint128,
    pub available_at: Timestamp,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PendingRefundRecord {
    pub member: Addr,
    pub amount: Uint128,
    pub available_at: Timestamp,
}

// Storage
pub const PLATFORM_CONFIG: Item<PlatformConfig> = Item::new("platform_config");
pub const CIRCLE_COUNTER: Item<u64> = Item::new("circle_counter");
pub const CIRCLES: Map<u64, Circle> = Map::new("circles");
pub const PAYOUTS: Map<(u64, u32), PayoutRecord> = Map::new("payouts");
pub const DEPOSITS: Map<(u64, Addr, u32), DepositRecord> = Map::new("deposits");
pub const PENALTIES: Map<(u64, Addr, u32), PenaltyRecord> = Map::new("penalties");
pub const REFUNDS: Map<(u64, Addr), RefundRecord> = Map::new("refunds");
pub const EVENTS: Map<(u64, u64), EventLog> = Map::new("events");
pub const MEMBER_MISSED_PAYMENTS: Map<(u64, Addr), MemberMissedPayments> = Map::new("missed_payments");
pub const EVENT_COUNTER: Map<u64, u64> = Map::new("event_counter");

// Locking and Private Circle Storage
/// Join deposits locked per member (one contribution_amount per member; creator uses circle.creator_lock_amount)
pub const MEMBER_LOCKED_AMOUNTS: Map<(u64, Addr), Uint128> = Map::new("member_locked_amounts");
/// Cumulative late fees accumulated against each member's locked amount (tracked in-contract, no extra funds required from member at deposit time)
pub const MEMBER_ACCUMULATED_LATE_FEES: Map<(u64, Addr), Uint128> = Map::new("member_accum_late_fees");
/// Pending payout amounts waiting for member to call Withdraw
pub const PENDING_PAYOUTS: Map<(u64, Addr), Uint128> = Map::new("pending_payouts");
pub const BLOCKED_MEMBERS: Map<(u64, Addr), u32> = Map::new("blocked_members");
pub const MEMBER_PSEUDONYMS: Map<(u64, Addr), String> = Map::new("member_pseudonyms");
pub const PRIVATE_MEMBER_LIST: Map<u64, Vec<Addr>> = Map::new("private_member_list");

// Staking: per-circle staking config; refunds queued when liquid balance insufficient
pub const CIRCLE_STAKING: Map<u64, CircleStakingConfig> = Map::new("circle_staking");
pub const PENDING_REFUNDS: Map<(u64, Addr), PendingRefundRecord> = Map::new("pending_refunds");
