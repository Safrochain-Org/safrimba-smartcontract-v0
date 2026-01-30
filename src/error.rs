use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized: {msg}")]
    Unauthorized { msg: String },

    #[error("Circle not found: {circle_id}")]
    CircleNotFound { circle_id: u64 },

    #[error("Circle is not in the correct status. Expected: {expected}, Got: {actual}")]
    InvalidCircleStatus { expected: String, actual: String },

    #[error("Circle is full. Max members: {max}")]
    CircleFull { max: u32 },

    #[error("Circle is invite-only")]
    InviteOnly { circle_id: u64 },

    #[error("Member already joined")]
    AlreadyMember { address: String },

    #[error("Insufficient funds. Required: {required}, Sent: {sent}")]
    InsufficientFunds { required: String, sent: String },

    #[error("Circle has not started yet")]
    CircleNotStarted { circle_id: u64 },

    #[error("Circle has already started. Member exit not allowed")]
    ExitNotAllowed { circle_id: u64 },

    #[error("Minimum members not met. Required: {required}, Current: {current}")]
    MinMembersNotMet { required: u32, current: u32 },

    #[error("Cycle not ready. Next payout date: {next_date}")]
    CycleNotReady { next_date: u64 },

    #[error("Member has not paid for this cycle")]
    PaymentMissing { address: String, cycle: u32 },

    #[error("Member is late. Grace period ended")]
    MemberLate { address: String },

    #[error("Maximum missed payments exceeded: {max}")]
    MaxMissedPaymentsExceeded { max: u32 },

    #[error("Invalid payout order type")]
    InvalidPayoutOrderType {},

    #[error("Invalid cycle index: {index}")]
    InvalidCycleIndex { index: u32 },

    #[error("Emergency stop is active")]
    EmergencyStopActive {},

    #[error("Arbiter only action")]
    ArbiterOnly {},

    #[error("Invalid parameters: {msg}")]
    InvalidParameters { msg: String },

    #[error("Deposit already made for this cycle")]
    AlreadyDeposited { address: String, cycle: u32 },
}
