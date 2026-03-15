# Safrimba Contract Test Report

Generated: 2026-03-12T09:53:23Z
Network: testnet | Chain: safro-testnet-1 | Code ID: 102
Contract: `addr_safro1v5fzm0mtfe23glgg0rh47slxd3ajknn06tfpctnuhpw252yvjk0q0wrlx8`
Creator: `addr_safro1hezduhmk52kacax8f9l076sdgtejf8eshtszla` | Member: `addr_safro1f8a9m8r5dq046qvmm9h5eryk0fn0u7tqu6v6sn`

## Results: 89 passed, 1 failed out of 90 tests

```
════════════════════════════════════════════════════════════════
  SAFRIMBA COMPREHENSIVE CONTRACT TEST
════════════════════════════════════════════════════════════════
Network: testnet | Code ID: 102
Creator: mycontractadmin (addr_safro1hezduhmk52kacax8f9l076sdgtejf8eshtszla)
Member:  mywallet (addr_safro1f8a9m8r5dq046qvmm9h5eryk0fn0u7tqu6v6sn)

[1] INSTANTIATE CONTRACT
  Contract: addr_safro1v5fzm0mtfe23glgg0rh47slxd3ajknn06tfpctnuhpw252yvjk0q0wrlx8

[2] CREATE CIRCLES (7 different configs)
  ✓ PASS: Circle 1: Public + Total, 2 members, 60s cycle
  ✓ PASS: Circle 2: Private + None, 2 members, fast
  ✓ PASS: Circle 3: Public + MinMembers(2), 3 members
  ✓ PASS: Circle 4: Public + Total, manual_trigger
  ✓ PASS: Circle 5: Public + None, ejection params
  ✓ PASS: Circle 6: Private + Total (cancel test)
  ✓ PASS: Circle 7: Private + None (exit test)

[3] UPDATE CIRCLE (before start)
  ✓ PASS: UpdateCircle on circle 6

[4] JOIN PUBLIC CIRCLES
  ✓ PASS: Join circle 1 (Public)
  ✓ PASS: Join circle 3 (Public)
  ✓ PASS: Join circle 4 (Public)
  ✓ PASS: Join circle 5 (Public)

[5] INVITE + ACCEPT (Private circles)
  ✓ PASS: Invite+Accept circle 2 (Private)
  ✓ PASS: Invite+Accept circle 6 (Private)
  ✓ PASS: Invite+Accept circle 7 (Private)

[6] CANCEL CIRCLE BEFORE START
  ✓ PASS: Cancel circle 6 (before start, full refund)
  ✓ PASS: Verify circle 6 status = Cancelled

[7] EXIT CIRCLE BEFORE START
  ✓ PASS: Exit circle 7 as member (full refund)

[8] START CIRCLES
  ✓ PASS: Start circle 1
  ✓ PASS: Start circle 2
  ✓ PASS: Start circle 4
  ✓ PASS: Start circle 5
  ✓ PASS: Start circle 3 (2/3 members, min=2)

[9] QUERY CIRCLE STATES
  Circle 1: status=Running, members=2, threshold={
  "total": {}
}
  Circle 2: status=Running, members=2, threshold=none
  Circle 3: status=Running, members=2, threshold={
  "total": {}
}
  Circle 4: status=Running, members=2, threshold={
  "total": {}
}
  Circle 5: status=Running, members=2, threshold={
  "total": {}
}
  Circle 6: status=Cancelled, members=2, threshold={
  "total": {}
}
  Circle 7: status=Cancelled, members=1, threshold=none

[10] DEPOSIT CONTRIBUTION (Round 1 for all)
  ✓ PASS: Circle 1: creator deposits R1
  ✓ PASS: Circle 1: member deposits R1
  ✓ PASS: Circle 3: creator deposits R1
  ✓ PASS: Circle 3: member deposits R1
  ✓ PASS: Circle 4: creator deposits R1
  ✓ PASS: Circle 4: member deposits R1
  ✓ PASS: Circle 2: creator deposits R1
  ✓ PASS: Circle 2: member deposits R1
  ✓ PASS: Circle 5: creator deposits R1 (member will miss)

[11] ADVANCE ROUND R1→R2 (Public=Total, need all rounds)
  ✓ PASS: Advance circle 1 R1→R2 (Total forced)
  ✓ PASS: Advance circle 3 R1→R2 (Total forced)
  ✓ PASS: Advance circle 4 R1→R2 (Total forced)
  ✓ PASS: Advance circle 5 R1→R2 (Total forced)
  ✓ PASS: Circle 2: ProcessPayout R1 (None threshold, instant)

[12] PAUSE / UNPAUSE
  ✓ PASS: Pause circle 3
  ✓ PASS: Verify circle 3 status = Paused
  ✓ PASS: Unpause circle 3
  ✓ PASS: Verify circle 3 status = Running

[13] DEPOSIT R2 + PROCESS PAYOUT
  ✓ PASS: Circle 1: creator deposits R2
  ✓ PASS: Circle 1: member deposits R2
  ✓ PASS: Circle 3: creator deposits R2
  ✓ PASS: Circle 3: member deposits R2
  ✓ PASS: Circle 4: creator deposits R2
  ✓ PASS: Circle 4: member deposits R2
  ✓ PASS: Circle 5: creator deposits R2 (member still missing)
  ✓ PASS: Circle 1: ProcessPayout (R2/2, cycle 1)
  ✓ PASS: Circle 3: ProcessPayout (R2/2)
  ✓ PASS: Circle 4: ProcessPayout (manual, R2/2)
  ✓ PASS: Circle 5: ProcessPayout (R2/2, member missed both)
  ✓ PASS: Circle 2: creator deposits R2
  ✓ PASS: Circle 2: member deposits R2
  ✓ PASS: Circle 2: ProcessPayout R2 (cycle complete)

[14] WITHDRAW (pending payouts)
  ✓ PASS: Withdraw circle 1 (creator)
  ✓ PASS: Withdraw circle 1 (member)
  ✓ PASS: Withdraw circle 2 (creator)
  ✓ PASS: Withdraw circle 2 (member)
  ✓ PASS: Withdraw circle 3 (creator)
  ✗ FAIL: Withdraw circle 3 (member)
  ✓ PASS: Withdraw circle 4 (creator)
  ✓ PASS: Withdraw circle 4 (member)

[15] CHECK EJECTION RESULTS (circle 5)
  Circle 5 status: Finalizing, members: 1
  (Member missed 2 rounds → late fees accumulated during advance+payout)
  ✓ Member was auto-ejected during ProcessPayout

[16] EMERGENCY STOP
  ✓ PASS: EmergencyStop circle 1

[17] EXIT AFTER START
  Circle 1 status: Paused

[18] BLOCK MEMBER + DISTRIBUTE BLOCKED FUNDS
  Circle 1: status=Paused, members=2

[19] CANCEL AFTER START
  Circle 1 status: Paused
  ✓ PASS: Cancel circle 1 after distribution should fail

[20] COMPREHENSIVE QUERIES
  ✓ PASS: GetCircle(1)
  ✓ PASS: GetCircles(limit=5)
  ✓ PASS: GetCircleMembers(1)
  ✓ PASS: GetCircleStatus(1)
  ✓ PASS: GetCurrentCycle(1)
  ✓ PASS: GetCycleDeposits(1,1)
  ✓ PASS: GetMemberDeposits
  ✓ PASS: GetPayouts(1)
  ✓ PASS: GetPayoutHistory(1)
  ✓ PASS: GetCircleBalance(1)
  ✓ PASS: GetMemberBalance
  ✓ PASS: GetPenalties(1)
  ✓ PASS: GetRefunds(1)
  ✓ PASS: GetPendingPayout
  ✓ PASS: GetMemberAccumLateFees
  ✓ PASS: GetDepositRequirement
  ✓ PASS: GetEvents(1)
  ✓ PASS: GetCircleStats(1)
  ✓ PASS: GetMemberStats
  ✓ PASS: GetMemberLockedAmount
  ✓ PASS: GetBlockedMembers(1)
  ✓ PASS: GetDistributionCalendar
  ✓ PASS: GetCircleStakingInfo
  ✓ PASS: GetPendingRefunds(1)

[21] FINAL STATUS CHECK
  Circle 1: status=Paused, members=2, cycle=4, pending_payouts=0
  Circle 2: status=Completed, members=2, cycle=2, pending_payouts=0
  Circle 3: status=Running, members=2, cycle=3, pending_payouts=200000
  Circle 4: status=Completed, members=2, cycle=2, pending_payouts=0
  Circle 5: status=Finalizing, members=1, cycle=2, pending_payouts=300000
  Circle 6: status=Cancelled, members=2, cycle=0, pending_payouts=0
  Circle 7: status=Cancelled, members=1, cycle=0, pending_payouts=0

════════════════════════════════════════════════════════════════
  1/90 TESTS FAILED
  89 passed, 1 failed
════════════════════════════════════════════════════════════════
Contract: addr_safro1v5fzm0mtfe23glgg0rh47slxd3ajknn06tfpctnuhpw252yvjk0q0wrlx8


```
