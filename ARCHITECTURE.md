# CTS-ARCHITECTURE

The CTS is an on-chain order-book market trade platform for ICRC-1 token ledgers on the world-computer blockchain.

## General architecture overview
Each token/ledger listed for trading on the market trades against the CYCLES. CYCLES is the native stablecoin on the internet-computer payed by all canisters to the ICP network for computation and storage costs. The CYCLES are held at a stable value 1-XDR = 1-TRILLION-CYCLES. This makes for a perfect native stablecoin.
The market tokens trade against the CYCLES, creating a stable trading scenario, helping tokens find their current price based on the merits of that token alone, and not being swayed by the current price of ICP.

For users to hold CYCLES, the CTS-CYCLES-BANK is an ICRC-1 ledger that hold CYCLES for users 1:1 without each user needing a 'cycles-wallet' canister. The DFINITY foundation is currently working on a cycles-ledger to be managed and controlled by the NNS for this purpose that accomplishes the same goal as the CTS-CYCLES-BANK, however there is no telling when it will be released. If the cycles-ledger built by DFINITY does get released, the CTS will switch to using it, as it is better for the ecosystem to be using the same ledger for the cycles.

For each token/ledger listed for trading on the CTS, the system creates a new trade-contract canister, known as the cm_tc (cycles-market-trade-contract) in this codebase. This canister facilitates the trading of this token by receiving and matching the orders. 
 
In the cm_tc, orders are known as 'positions' and order-matches are known as 'trades'. A single position can have many trades.
 
Each cm_tc canister creates two types of storage canisters, one for storing the logs of each position, known as the cm_positions_storage canisters, and one for storing the logs of each trade, known as the cm_trades_storage canisters. A single cm_tc can create many cm_positions_storage canisters and many cm_trades_storage canisters.

## CANISTERS

### 'cts
The canister referred to in this codebase as the 'cts' canister is the frontend "asset" canister, serving certified frontend files to the browser. 
At this time this is it's only purpose and has no connection to the trading market. The code for this 'cts' canister is located at `rust/canisters/cts` in this repo. This is a top-level canister and will be controlled by the SNS root canister.

### bank
The `bank` canister is the CTS-CYCLES-BANK and is located at `rust/canisters/bank` in this repo. This canister is an ICRC-1 ledger (ICRC-2 and ICRC-3 coming soon) that holds cycles for the users 1:1.
The bank can be used to mint cycles using ICP straight into the user's ledger account, and send-out and receive cycles to and from canisters. This is a top-level canister and will be controlled by the SNS root canister.

### 'cm_main'
The market starts with the canister referred to in this codebase as the 'cm_main' canister located at `rust/canisters/market/cm_main`. This canister creates and manages the trade-contract canisters of each token/ledger listed on the market. This is a top-level canister and will be controlled by the SNS root canister.
To create a new trade-contract, the cm_main has a method that only the controller can call. The wasm-modules for the trade-contract canister and positions-storage and trades-storage canisters are held on this canister, the cm_main.

### cm_tc
Location: `rust/canisters/market/cm_tc`

#### Trade Flow
Lets walk through a sample of a user creating a position (order) to trade some XTKN for CYCLES. The first step is to transfer the amount of XTKN for the trade plus the XTKN transfer-fee into the user's subaccount of the cm_tc canister of the XTKN trade-contract. Next, the user calls the `trade_tokens` method on the cm_tc, setting the trade-amount and the trade-rate. The cm_tc transfers the trade-amount from the user's subaccount into the cm_tc's central positions-subaccount. If the transfer goes through, the cm_tc creates a TokenPosition for the user with the amount and rate of the trade. The cm_tc then checks the current cycles-positions (those trading CYCLES for XTKN) and if positions with a compatible rate are found, the cm_tc matches the positions, creating a TradeLog for each match. If there are still XTKN left in the order after the matching-process, the remaining trade-amount is put on the position-book and waits for a compatible position to come in. The trades are then payed out in the background, and logs are put into the storage.

The storage mechanism is implemented with a buffer in the cm_tc that when full, creates storage canisters when needed, and flushes the buffer to the storage canisters.

### cm_positions_storage
Location: `rust/canisters/market/cm_positions_storage`

### cm_trades_storage
Location: `rust/canisters/market/cm_trades_storage`


## Tests 
Integration-tests are located in the `rust/pic_tests/tests` directory. To run these tests using cargo, cd into the rust/pic_tests directory and run cargo test.



