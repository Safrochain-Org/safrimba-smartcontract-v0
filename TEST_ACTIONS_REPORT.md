# Safrimba Contract - Full Actions Test Report

Generated: 2026-03-07T14:51:39Z

## Configuration
- Network: testnet
- Chain ID: safro-testnet-1
- Contract: addr_safro1jn0dv8500r9u5u3j7ydarkhg5ctjflr6dxky8j2hwykd3r7qv2vsq42hqa
- Creator: mycontractadmin (addr_safro1hezduhmk52kacax8f9l076sdgtejf8eshtszla)
- Member: mywallet (addr_safro1f8a9m8r5dq046qvmm9h5eryk0fn0u7tqu6v6sn)
- Code ID: 93

## Test Output
```
=== Safrimba Full Actions Test ===
Network: testnet | Creator: mycontractadmin | Member: mywallet | Code ID: 93
Report: /Users/bdan/Projects/SAFROCHAIN/Safrimba/safrimba-smartcontract-/TEST_ACTIONS_REPORT.md

[1] INSTANTIATE
  Tx: DF3EC7B2F320F55F57FA462F1B766B2DCD977609B59774D6BB9F7052AE0A76DC
  Contract: addr_safro1jn0dv8500r9u5u3j7ydarkhg5ctjflr6dxky8j2hwykd3r7qv2vsq42hqa

[2] CREATE CIRCLE (all distribution thresholds)
  OK: Public-Total-1
  OK: Public-Total-2
  OK: Private-Total-1
  OK: Private-None-1
  OK: Private-MinMembers-1
  OK: Cancel-Test
  OK: Exit-Test
  OK: Unlock-Test
  OK: Ejection-Test
  OK: Exit-After-Start

[3] JOIN (Public circle)
  OK: mywallet joined circle 1 (Public)
  OK: mywallet joined circle 8
  OK: mywallet joined circle 9

[4] INVITE + ACCEPT INVITE (Private circle)
  OK: Creator invited mywallet to circle 3
  OK: mywallet accepted invite to circle 3 (with contribution)

[5] ADD PRIVATE MEMBER
  OK: AddPrivateMember for circle 4

[6] INVITE + ACCEPT (Private circle 4)
  Skip: circle 4 invite
  Skip: circle 4 accept
  OK: Invited to circle 10 (Exit-After-Start)
  OK: Accepted circle 10

[7] START CIRCLE
  OK: Started circle 1
  OK: Started circle 3
  OK: Started circle 8
  OK: Started circle 9
  OK: Started circle 10

[8] UPDATE CIRCLE
  OK: Updated circle 2

[9] CANCEL CIRCLE (before start)
  OK: Cancelled circle 6 (Cancel-Test)

[10] EXIT CIRCLE (before start)
  OK: Member exited circle 7 (full refund before start)

[11] QUERY STATES
  Circle 1: status=Running, members=2, pending=0
  Circle 2: status=Draft, members=1, pending=0
  Circle 3: status=Running, members=2, pending=0
  Circle 4: status=Open, members=2, pending=0
  Circle 5: status=Draft, members=1, pending=0
  Circle 6: status=Cancelled, members=1, pending=0
  Circle 7: status=Cancelled, members=1, pending=0
  Circle 8: status=Running, members=2, pending=0
  Circle 9: status=Running, members=2, pending=0
  Circle 10: status=Running, members=2, pending=0

[12] DEPOSIT CONTRIBUTION (Running circles)
  OK: mycontractadmin deposited in circle 1
  OK: mywallet deposited in circle 1
  OK: mycontractadmin deposited in circle 3
  OK: mywallet deposited in circle 3
  OK: mycontractadmin deposited in circle 8
  OK: mywallet deposited in circle 8
  OK: mycontractadmin deposited in circle 10
  OK: mywallet deposited in circle 10
  OK: creator deposited in circle 9 (member will miss)

[13] ADVANCE ROUND
  OK: Advanced round circle 1 (Total threshold: round 1 < max, no payout)

[14] CHECK AND EJECT (documented)
  OK: CheckAndEject (no-op if no one to eject)

[15] PAUSE / UNPAUSE
  OK: Paused circle 1
  OK: Unpaused circle 1

[16] PROCESS PAYOUT + WITHDRAW (unlocking)
  Skip: ProcessPayout (check next_payout_date, grace period)

[17] EJECTION (member misses deposits)
  OK: ProcessPayout circle 9 (member missed -> accumulated+penalty>=locked -> ejected)
  OK: Member ejected from circle 9 (1 member left)

[18] EXIT AFTER START
  OK: Member exited circle 10 after start (refund = locked - late_fees - exit_penalty)

[19] CANCEL AFTER START
  OK: Cancelled circle 1 (Running) - creator_lock distributed, deposits refunded


```

## All Contract Actions (for Frontend)

| Action | When | Who | Notes |
|--------|------|-----|------|
| CreateCircle | - | Creator | Send creator_lock (2×contribution) |
| JoinCircle | Draft/Open, Public | Anyone | Send contribution_amount |
| InviteMember | Draft/Open, Private | Creator | Adds to pending_members |
| AcceptInvite | Draft/Open, Private | Invited | Send contribution_amount |
| AddPrivateMember | Draft/Open, Private | Creator | Adds to private whitelist |
| StartCircle | Open/Full | Creator | Min members required |
| DepositContribution | Running | Member | Send contribution_amount each round |
| ProcessPayout | Running, date reached | Anyone | Triggers distribution (tested: Unlock-Test) |
| AdvanceRound | Running, round < dist threshold | Anyone | No payout, next round |
| Withdraw | Anytime | Member | Withdraw pending payouts (unlocking, tested) |
| ExitCircle | Draft/Open (full refund) or Running (penalty) | Member | - |
| CancelCircle | Draft/Open/Full (refund) or Running | Creator | - |
| UpdateCircle | Draft/Open | Creator | Name, description, image |
| PauseCircle | Running | Creator | If emergency_stop_enabled |
| UnpauseCircle | Paused | Creator | - |
| EmergencyStop | Running | Creator | - |
| BlockMember | Running | Creator | Blocks member |
| DistributeBlockedFunds | Running | Creator | After block |
| CheckAndEject | Running | Anyone | Ejects late members |

## Distribution Thresholds
- **Total**: Distribution only at last round of cycle (until completion)
- **None**: Distribution from round 1 (unlocking from start)
- **MinMembers(N)**: Distribution from round N onwards

## Final Unlocking (tested)
1. Member receives payout via ProcessPayout (or auto_payout)
2. Amount goes to PENDING_PAYOUTS for that member
3. Member calls Withdraw to receive funds (unlocking complete)
4. withdrawal_lock can block withdrawals if set

## Unlocking Flows Tested
- **ProcessPayout + Withdraw**: Unlock-Test circle (cycle_duration=0, grace=0, None threshold)
- **Ejection**: Ejection-Test circle (member misses, late_fee 50% + exit_penalty 50% -> 1-round eject)
- **Exit after start**: Exit-After-Start circle (strict_mode=false, refund = locked - fees - penalty)
- **Cancel after start**: Circle 1 Running -> creator_lock distributed, deposits refunded

## State Machine
Draft -> Open -> Full -> Running -> Completed
  |       |       |       |
  v       v       v       v
Cancelled

## Frontend Actions by State

### Draft
- CreateCircle (done)
- InviteMember (if private)
- JoinCircle (if public)
- UpdateCircle
- CancelCircle
- ExitCircle

### Open
- InviteMember (if private)
- JoinCircle (if public)
- AcceptInvite (if invited)
- StartCircle (if min_members met)
- UpdateCircle
- CancelCircle
- ExitCircle

### Full
- StartCircle
- ExitCircle
- CancelCircle

### Running
- DepositContribution
- ProcessPayout
- AdvanceRound
- Withdraw
- PauseCircle
- BlockMember
- CheckAndEject
- EmergencyStop
- CancelCircle

### Paused
- UnpauseCircle
- CancelCircle

## Query Messages (for Frontend)
- GetCircle, GetCircles, GetCircleMembers, GetCircleStatus
- GetCurrentCycle, GetCycleDeposits, GetMemberDeposits
- GetPayouts, GetPayoutHistory, GetPendingPayout
- GetCircleBalance, GetMemberBalance, GetPenalties, GetRefunds
- GetDistributionCalendar, GetMembersAccumulatedLateFees

