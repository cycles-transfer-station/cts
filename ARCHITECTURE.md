# CTS-ARCHITECTURE

The CTS is an on-chain order-book market trade platform for ICRC-1 token ledgers on the world-computer blockchain.

The CTS lives 100% on the internet-computer blockchain with zero off-chain dependencies.

## General architecture overview
Each token/ledger listed for trading on the market trades against the CYCLES. CYCLES is the native stablecoin on the internet-computer payed by all canisters to the ICP network for computation and storage costs. The CYCLES are held at a stable value 1-XDR = 1-TRILLION-CYCLES. This makes for a perfect native stablecoin.
The market tokens trade against the CYCLES, creating a stable trading scenario, helping tokens find their current price based on the merits of that token alone, and not being swayed by the current price of ICP.

For users to hold CYCLES, the CTS-CYCLES-BANK is an ICRC-1 ledger that hold CYCLES for users 1:1 without each user needing a 'cycles-wallet' canister.

For each token/ledger listed for trading on the CTS, the system creates a new trade-contract canister, known as the cm_tc (cycles-market-trade-contract) in this codebase. This canister facilitates the trading of this token by receiving and matching the orders. 
 
In the cm_tc, orders are known as 'positions' and order-matches are known as 'trades'. A single position can have many trades.
 
Each cm_tc canister creates two types of storage canisters, one for storing the position-logs, known as the cm_positions_storage canisters, and one for storing the trade-logs, known as the cm_trades_storage canisters. A single cm_tc can create many cm_positions_storage canisters and many cm_trades_storage canisters.

## CANISTERS

### cts - em3jm-bqaaa-aaaar-qabxa-cai
The canister referred to in this codebase as the 'cts' canister is the frontend "asset" canister, serving certified frontend files to the browser. 
At this time this is it's only purpose and has no connection to the trading market. The code for this 'cts' canister is located at `rust/canisters/cts` in this repo. This is a top-level canister and will be controlled by the SNS root canister.

### bank - wwikr-gqaaa-aaaar-qacva-cai
The `bank` canister is the CTS-CYCLES-BANK and is located at `rust/canisters/bank` in this repo. This canister is an ICRC-1 ledger (ICRC-2 and ICRC-3 coming soon) that holds cycles for the users 1:1.
The bank can be used to mint cycles using ICP straight into the user's ledger account, and send-out and receive cycles to and from canisters. This is a top-level canister and will be controlled by the SNS root canister.

### cm_main - el2py-miaaa-aaaar-qabxq-cai
The market starts with the canister referred to in this codebase as the 'cm_main' canister located at `rust/canisters/market/cm_main`. This canister creates and manages the trade-contract canisters of each token/ledger listed on the market. This is a top-level canister and will be controlled by the SNS root canister.
To create a new trade-contract, the cm_main has a method that only the controller can call. The wasm-modules for the trade-contract canister and positions-storage and trades-storage canisters are held on this canister, the cm_main.

### cm_tc
Location: `rust/canisters/market/cm_tc`. These canisters are controlled and upgraded by the cm_main canister.

#### Trade Flow
Lets walk through a sample of a user creating a position (order) to trade some XTKN for CYCLES. The first step is to transfer the amount of XTKN for the trade plus the XTKN transfer-fee into the user's subaccount of the cm_tc canister of the XTKN trade-contract. Next, the user calls the `trade_tokens` method on the cm_tc, setting the trade-amount and the trade-rate. The cm_tc transfers the trade-amount from the user's subaccount into the cm_tc's central positions-subaccount. If the transfer goes through, the cm_tc creates a TokenPosition for the user with the amount and rate of the trade. The cm_tc then checks the current CyclesPositions (those trading CYCLES for XTKN) and if positions with a compatible rate are found, the cm_tc matches the positions, creating a TradeLog for each match. If there are still XTKN left in the order after the matching-process, the remaining trade-amount is put on the position-book and waits for a compatible position to come in. The trades are then payed out in the background, and logs are put into the storage.

#### Storage
The storage mechanism is implemented with a buffer in the cm_tc that when full, creates storage canisters when needed, and flushes the buffer to the storage canisters.

For the positions-storage, as soon as a position is created, it goes into the storage logs as it's initial state, but also stays on the cm_tc in the current-positions book. Then when the position is either filled or canceled, the position's log in the storage-logs is updated with it's final state. When viewing a user's positions, if a position-log is in the current-positions and in the storage-logs, the version in the current-positions is the correct current state of the position. This is done because the positions are put into the logs in the order of their creation but the order of their completion is not the same as the order of their creation, since a position can be waiting to be filled while other positions get created after but filled before the first one is filled, and we need to save a spot in the storage-logs for the first position.

For the trades-storage, as soon as a trade is made, the system can do the payouts and put it into the storage logs. Therefore trades get put into the storage-logs only once when the payouts are complete, and they are put into the storage only once in the order of their creation.

### cm_positions_storage
Location: `rust/canisters/market/cm_positions_storage`. These canisters are controlled and upgraded by their cm_tc canister.

### cm_trades_storage
Location: `rust/canisters/market/cm_trades_storage`. These canisters are controlled and upgraded by their cm_tc canister.




