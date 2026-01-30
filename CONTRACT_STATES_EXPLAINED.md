# 🔄 Safrimba Contract States - Professional Flow

## State Machine Overview

```
Draft → Open → Full → Running → (Paused) → Completed
  ↓                              ↓
Cancelled ←─────────────────────┘
```

## States Explained

### 1. Draft (Initial State)
**When**: Circle just created
**Characteristics**:
- Only creator is a member
- No other members joined yet
- Settings can be updated
- Can be cancelled

**Allowed Actions**:
- InviteMember (if invite_only)
- JoinCircle (public circles)
- UpdateCircle (metadata)
- CancelCircle
- ExitCircle (if allowed)

**Transition To**:
- **Open**: When first non-creator member joins
- **Cancelled**: If creator cancels

### 2. Open (Accepting Members)
**When**: At least one member joined (not full yet)
**Characteristics**:
- Members < max_members
- Members >= min_members_required can trigger start
- Still accepting new members
- Contributions can be prepared

**Allowed Actions**:
- InviteMember (if invite_only)
- JoinCircle / AcceptInvite
- LockJoinDeposit
- StartCircle (if min members met)
- ExitCircle (if allowed)
- CancelCircle

**Transition To**:
- **Full**: When members == max_members
- **Running**: Manual start by creator (if min met)
- **Draft**: If members drop below 1 (exits)
- **Cancelled**: If cancelled

### 3. Full (Ready to Start)
**When**: members == max_members
**Characteristics**:
- Maximum capacity reached
- Ready to start
- No more joins allowed
- Waiting for start trigger

**Allowed Actions**:
- StartCircle (by creator)
- ExitCircle (if allowed, drops to Open)
- CancelCircle

**Auto-Transition**:
- **Running**: Automatically if `auto_start_when_full = true` AND `auto_start_type = "by_members"`

**Manual Transition To**:
- **Running**: Creator calls StartCircle
- **Open**: Member exits
- **Cancelled**: If cancelled

### 4. Running (Active Cycle)
**When**: Circle has started, distributions happening
**Characteristics**:
- Cycles in progress
- Members making deposits
- Distributions being processed
- Cannot exit or join
- Current cycle tracked

**Allowed Actions**:
- DepositContribution (by members)
- ProcessPayout (by contract/arbiter/creator)
- PauseCircle (if emergency_stop_enabled)
- BlockMember (by creator/arbiter)
- DistributeBlockedFunds

**Transition To**:
- **Paused**: Emergency stop or manual pause
- **Completed**: All cycles finished
- **Cancelled**: Emergency cancellation

### 5. Paused (Temporary Halt)
**When**: Circle paused due to emergency or manual intervention
**Characteristics**:
- All operations frozen
- Funds locked
- No deposits or payouts
- Requires manual unpause

**Allowed Actions**:
- UnpauseCircle (by creator/arbiter)
- EmergencyStop (if enabled)
- CancelCircle (to refund)

**Transition To**:
- **Running**: UnpauseCircle
- **Cancelled**: If refund needed

### 6. Completed (Success!)
**When**: All cycles finished successfully
**Characteristics**:
- All distributions made
- cycles_completed == total_cycles
- Circle archived
- Historical record

**Allowed Actions**:
- Query data
- View history
- Platform fee withdrawal

**Final State**: No transitions

### 7. Cancelled (Terminated)
**When**: Circle cancelled before completion
**Characteristics**:
- Refunds processed
- Circle terminated
- Historical record

**Allowed Actions**:
- Query data
- View refund info

**Final State**: No transitions

---

## State Transition Rules

### Auto-Start Triggers

**By Members** (`auto_start_type = "by_members"`):
```rust
// In execute_join_circle when last member joins:
if circle.members_list.len() >= circle.max_members 
    && circle.auto_start_when_full
    && auto_start_type == "by_members"
    && members.len() >= min_members_required
{
    circle.circle_status = CircleStatus::Running;
    circle.current_cycle_index = 1;
    // Generate payout order
}
```

**By Date** (`auto_start_type = "by_date"`):
- Requires external trigger (cron/keeper)
- Frontend shows countdown
- Manual StartCircle call at scheduled time

### Membership Changes Affect State

**Member Joins**:
- Draft → Open (first join)
- Open → Full (reaches max)
- Full → Running (if auto-start)

**Member Exits**:
- Full → Open (below max)
- Open → Draft (last member exits)
- Open → Cancelled (below min, if auto_refund)

---

## Frontend State Display

### Status Badges (Color-Coded)

```typescript
Draft      → Gray badge
Open       → Blue badge (accepting members)
Full       → Yellow badge (ready to start)
Running    → Green badge (active)
Paused     → Orange badge (frozen)
Completed  → Purple badge (finished)
Cancelled  → Red badge (terminated)
```

### Progress Bar Updates

```
Draft:
[✅ Creation] [⏳ Registration] [⏳ Deposit] [⏳ Distribution] [⏳ Completion]

Open (3/10 members):
[✅ Creation] [🔄 Registration] [⏳ Deposit] [⏳ Distribution] [⏳ Completion]

Full (10/10 members):
[✅ Creation] [✅ Registration] [🔄 Deposit] [⏳ Distribution] [⏳ Completion]

Running:
[✅ Creation] [✅ Registration] [✅ Deposit] [🔄 Distribution] [⏳ Completion]

Completed:
[✅ Creation] [✅ Registration] [✅ Deposit] [✅ Distribution] [✅ Completion]
```

### Buttons Shown by State

**Draft** (creator only):
- [Invite Member] (if private)
- [Update Circle]
- [Cancel Circle]

**Open**:
- [Invite Member] (if private, creator/arbiter)
- [Join Circle] (if public, non-members)
- [Lock Deposit] (if invited, shows SAF amount)
- [Start Circle] (creator, if min members met)
- [Exit Circle] (if allowed)

**Full**:
- [Start Circle] (creator)
- [Exit Circle] (if allowed)

**Running**:
- [Deposit Now] (members, if cycle active)
- [Process Payout] (if manual_trigger)
- [Pause Circle] (creator/arbiter)

**Paused**:
- [Unpause Circle] (creator/arbiter)
- [Emergency Stop] (arbiter, if enabled)

**Completed**:
- [View History]
- [View Stats]

**Cancelled**:
- [View Refunds]
- [View History]

---

## Contract State Checks (Validation)

### Status-Based Validations

```rust
// Joining allowed only in Draft/Open
JoinCircle: requires Draft | Open

// Locking deposit allowed in Draft/Open  
LockJoinDeposit: requires Draft | Open

// Starting allowed only in Open/Full
StartCircle: requires Open | Full

// Deposits only in Running
DepositContribution: requires Running

// Payouts only in Running
ProcessPayout: requires Running

// Pause/Unpause only in Running/Paused
PauseCircle: requires Running
UnpauseCircle: requires Paused
```

### Member Count Validations

```rust
// Can't start without minimum
StartCircle: requires members >= min_members_required

// Can't join if full
JoinCircle: requires members < max_members

// Auto-refund if below minimum
ExitCircle: if members < min && auto_refund_if_min_not_met
    → CircleStatus::Cancelled
```

---

## Professional State Management

### Atomic State Updates

All state transitions happen atomically:
```rust
// Example: Full → Running
circle.circle_status = CircleStatus::Running;
circle.current_cycle_index = 1;
circle.start_date = Some(env.block.time);
circle.payout_order_list = Some(generated_order);
CIRCLES.save(deps.storage, circle_id, &circle)?;
// Log event after save succeeds
```

### Event Logging

Every state transition logs an event:
```rust
log_event(&mut deps, &env, circle_id, "circle_started", &data)?;
log_event(&mut deps, &env, circle_id, "circle_paused", &data)?;
log_event(&mut deps, &env, circle_id, "circle_completed", &data)?;
```

### Error Recovery

If state transition fails:
- No partial updates (transaction reverted)
- State remains unchanged
- Clear error message returned
- Frontend shows user-friendly error

---

## Frontend Integration

### Real-Time State Display

```typescript
// Query contract for current state
const contractCircle = await getCircleFromContract(address, circleId);
const status = contractCircle.circle_status; // "Running", "Open", etc.

// Display with color-coded badge
<span className={statusColors[status]}>
  {status}
</span>
```

### State-Based UI

```typescript
// Show different buttons based on state
{status === 'Open' && canJoin && <JoinButton />}
{status === 'Open' && canStart && <StartButton />}
{status === 'Running' && userIsMember && <DepositButton />}
{status === 'Completed' && <ViewHistoryButton />}
```

### Progress Tracking

```typescript
// Dynamic progress bar
const getRegistrationStatus = () => {
  if (currentMembers >= minRequired) return 'completed';
  if (currentMembers > 0) return 'in-progress';
  return 'upcoming';
};
```

---

## State Persistence

### On-Chain Storage

```rust
pub struct Circle {
    pub circle_status: CircleStatus,      // Current state
    pub current_cycle_index: u32,          // Active cycle
    pub cycles_completed: u32,             // Finished cycles
    pub members_list: Vec<Addr>,          // Active members
    pub pending_members: Vec<Addr>,       // Invited
    // ... more fields
}
```

### State is Immutable

- State changes only via execute messages
- All transitions logged
- Complete audit trail
- Transparent and verifiable

---

## Summary

✅ **7 distinct states** with clear transitions
✅ **Atomic updates** prevent inconsistencies  
✅ **Event logging** for full audit trail
✅ **Validation** at every step
✅ **Auto-transitions** when conditions met
✅ **Frontend reflects** real-time contract state
✅ **Color-coded badges** for quick recognition
✅ **Dynamic buttons** based on state
✅ **Progress tracking** visual feedback

**The contract state machine is production-ready and working professionally!** 🚀

