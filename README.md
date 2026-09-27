# Building a Blockchain from Scratch — Development Plan

**Assumptions:** learning-oriented but real (networked, persistent) blockchain, written in Rust, account-based state model, proof of work first, upgraded to proof of stake + BFT finality later, optional Wasm smart contracts at the end. Swap Rust for Go or Python if you prefer; the module breakdown stays the same.

---

## 1. Architecture at a glance

```
            ┌─────────────── NODE ────────────────┐
 Wallet ──► │ RPC ─► Mempool ─► Consensus         │
  (CLI)     │         ▲            │              │
            │ P2P ◄───┴──── Chain manager         │ ◄──► other nodes
            │                      │              │
            │           Validation / Execution    │
            │                      │              │
            │                   Storage           │
            └─────────────────────────────────────┘
   Shared by all: crypto, types/encoding, merkle, config/genesis
```

---

## 2. Modules

| # | Module | Responsibility | Key outputs | Suggested crates |
|---|---|---|---|---|
| 1 | `crypto` | Hashing, keypairs, signatures, addresses | `hash()`, `sign()`, `verify()`, `Address` | `sha2` / `blake3`, `ed25519-dalek` or `k256` |
| 2 | `types` | Core data structures + canonical binary encoding | `Transaction`, `SignedTransaction`, `BlockHeader`, `Block`, `Receipt` | `serde`, `borsh` or `bincode` |
| 3 | `merkle` | Merkle tree over txs; later a state trie | `merkle_root()`, inclusion proofs | own code |
| 4 | `state` | Account state: balance, nonce, (later) code + storage | `State`, `get/set_account`, `state_root()` | own code |
| 5 | `storage` | Persist blocks, index, state, undo data | `BlockStore`, `StateDB` | `rocksdb` or `sled` |
| 6 | `execution` | State transition function: apply tx / block to state | `apply_block(state, block) -> (new_state, receipts)` | own code |
| 7 | `validation` | Stateless + stateful checks for txs and blocks | `validate_tx()`, `validate_block()` | own code |
| 8 | `mempool` | Pending tx pool, fee ordering, replacement, eviction | `add()`, `select_for_block()`, `remove_included()` | own code |
| 9 | `consensus` | Block production + finality rules (PoW → PoS/BFT) | `produce_block()`, `verify_seal()` | own code |
| 10 | `chain` | Chain manager: tip, fork choice, reorgs | `import_block()`, `head()`, `reorg()` | own code |
| 11 | `p2p` | Peer discovery, connections, gossip, request/response | tx & block gossip, `GetBlocks` | `libp2p` or raw `tokio` TCP |
| 12 | `sync` | Catch up with network (headers-first, then bodies) | `SyncManager` | own code |
| 13 | `rpc` | JSON-RPC API for wallets/apps | `send_tx`, `get_balance`, `get_block` | `jsonrpsee` or `axum` |
| 14 | `node` | Wires everything; config, genesis, lifecycle | `node` binary | `tokio`, `clap`, `tracing` |
| 15 | `wallet` | Key management CLI, tx building/signing | `wallet` binary | `clap` |
| 16 | `vm` (optional) | Smart contract execution with gas metering | Wasm runtime | `wasmtime` / `wasmer` |
| 17 | `explorer` (optional) | Web UI reading from RPC | small web app | any |

**Repo layout (Cargo workspace):**
```
chain/
├── crates/
│   ├── crypto/  types/  merkle/  state/  storage/
│   ├── execution/  validation/  mempool/  consensus/
│   ├── chain/  p2p/  sync/  rpc/  vm/
├── bin/
│   ├── node/    wallet/
├── config/genesis.json
└── tests/       (integration + multi-node)
```

---

## 3. Phases, end to end

Each phase ends with a working, testable system. Don't start the next phase until the exit criteria pass.

### Phase 0 — Design & setup (≈1 week)
- Write a short spec: account model, tx fields, block header fields, hash function, signature scheme, block time target, reward/fee rules.
- Define genesis format (initial accounts + balances, chain ID, consensus params).
- Set up workspace, CI (fmt, clippy, tests), logging with `tracing`.
- **Exit:** spec doc + empty crates compiling in CI.

### Phase 1 — Core primitives (≈1–2 weeks)
- `crypto`: keygen, sign/verify, address = hash(pubkey) truncated.
- `types`: `Transaction {chain_id, from, to, amount, fee, nonce}`, signed wrapper, `BlockHeader {parent_hash, height, timestamp, tx_root, state_root, difficulty, nonce}`, `Block`.
- Canonical encoding — the same struct must always produce the same bytes (critical: hashes and signatures depend on it).
- `merkle`: tx root + inclusion proof verification.
- **Exit:** unit tests for sign/verify round-trips, deterministic hashes, Merkle proofs; fuzz the decoder.

### Phase 2 — Single-node chain in memory (≈2 weeks)
- `state`: in-memory map `Address → Account {balance, nonce}`; compute `state_root` (start with hash of sorted accounts; upgrade to a sparse Merkle / Patricia trie later).
- `execution`: apply tx (check nonce, deduct amount + fee, credit recipient, increment nonce), apply block (all txs + block reward to producer).
- `validation`: stateless (signature, sizes, chain_id) and stateful (balance, nonce, parent exists, roots match).
- Genesis loading; build blocks manually in tests.
- **Exit:** a test that creates genesis, builds 100 blocks of random transfers, and checks balances and roots are consistent.

### Phase 3 — Persistence (≈1 week)
- `storage`: column families for `blocks`, `height→hash` index, `state`, `undo`, `metadata (head)`.
- Write state changes + block atomically (batch writes).
- Store undo data per block so you can revert state.
- **Exit:** kill the node mid-run, restart, and it resumes at the same head with identical state root.

### Phase 4 — Mempool + proof of work (≈1–2 weeks)
- `mempool`: validate on entry, index by `(sender, nonce)` and fee, replace-by-fee, size cap with eviction, drop txs once included or invalid.
- `consensus` (PoW): target from difficulty, nonce search loop, difficulty adjustment every N blocks toward target block time.
- `chain`: import block → validate → execute → store → update head; fork choice = most cumulative work; implement reorg (revert via undo data, apply new branch, return orphaned txs to mempool).
- **Exit:** a single node mines continuously; a test feeding two competing branches reorgs correctly.

### Phase 5 — RPC + wallet (≈1 week)
- `rpc`: `send_raw_transaction`, `get_account`, `get_block_by_height/hash`, `get_tx`, `chain_head`.
- `wallet` CLI: `keygen`, `balance`, `send --to --amount --fee` (fetches nonce, signs, submits).
- **Exit:** send a transfer from the CLI, see it mined, and query the new balance.

### Phase 6 — Networking (≈2–3 weeks)
- `p2p`: bootstrap peers from config, handshake (chain ID, genesis hash, head height), peer scoring/banning.
- Gossip: new txs → peers' mempools; new blocks → peers' chain import.
- Request/response: `GetHeaders`, `GetBlocks`.
- `sync`: on startup or when behind, download headers from best peer, validate the chain of headers, then fetch and import bodies.
- **Exit:** run 3–5 nodes locally (Docker Compose or separate ports); all converge on the same head; kill one, restart it, and it syncs back.

### Phase 7 — Hardening (≈2 weeks)
- Limits everywhere: max tx size, block size, mempool size, message size, peer count.
- DoS protection: rate-limit RPC and peers, reject malformed messages early.
- Replay protection (chain ID in signed payload), timestamp rules (not too far in future, > median of past blocks).
- Property tests and fuzzing for decoding, execution, and reorgs.
- Metrics (Prometheus) + structured logs.
- **Exit:** multi-node chaos test (random restarts, network partitions, invalid-block injection) runs for hours without divergence.

### Phase 8 — Proof of stake + BFT finality (≈3–4 weeks)
- Staking txs: `stake`, `unstake` (with unbonding delay); validator set updated per epoch.
- Proposer selection: stake-weighted, deterministic from a seed (e.g. previous block hash, later a VRF).
- Replace nonce seal with proposer signature; add a Tendermint/HotStuff-style voting round (prevote → precommit, >2/3 stake) for finality.
- Slashing for double-signing (evidence tx).
- Fork choice: never revert finalized blocks.
- **Exit:** 4-validator network finalizes blocks with 1 validator offline; stops (safely) with 2 offline; a double-sign gets slashed.

### Phase 9 — Smart contracts (optional, ≈3–4 weeks)
- New tx types: `deploy(code)`, `call(contract, method, args)`.
- `vm`: run Wasm with host functions (`storage_read/write`, `caller`, `transfer`, `log`); gas metering; deterministic execution (no floats, no clocks, no randomness from the host).
- Contract storage inside state, included in `state_root`.
- **Exit:** deploy and call a counter and a token contract written in Rust → Wasm.

### Phase 10 — Tooling & ecosystem (ongoing)
- Block explorer, light client (header sync + Merkle proofs), testnet deployment on a few cloud VMs, documentation.

---

## 4. Testing strategy
- **Unit tests** per module (crypto, encoding, execution rules).
- **Property tests** (`proptest`): e.g. total supply is conserved except for block rewards; apply-then-revert returns identical state.
- **Fuzzing** (`cargo-fuzz`): decoders and P2P message handlers.
- **Integration tests:** in-process multi-node network with simulated latency and partitions.
- **Determinism check:** two nodes executing the same blocks must produce byte-identical state roots.

---

## 5. Rough timeline (one developer, part-time)
| Milestone | After phase | Cumulative |
|---|---|---|
| Working local chain with persistence | 3 | ~1 month |
| Mining node + wallet | 5 | ~2 months |
| Multi-node network that syncs | 6–7 | ~3.5 months |
| PoS with finality | 8 | ~4.5 months |
| Smart contracts | 9 | ~5.5 months |

---

## 6. Common pitfalls
- **Non-deterministic encoding or execution** (HashMap iteration order, floats, system time) → nodes silently fork. Use `BTreeMap` and canonical encoding.
- **Forgetting reorgs** until late — design undo data in Phase 3.
- **Trusting peer data** — validate everything received over P2P before acting on it.
- **Unbounded resources** — every queue, message, and cache needs a limit.
- **Signing the wrong bytes** — sign a hash of the canonical encoding that includes chain ID and nonce.

## 7. References to consult per phase
- Phases 1–4: *Programming Bitcoin* (Jimmy Song), *Mastering Bitcoin*, learnmeabitcoin.com
- Phase 6: Bitcoin P2P protocol docs, libp2p docs
- Phase 8: Tendermint thesis (Buchman), HotStuff paper, Casper FFG
- Phase 9: Ethereum Yellow Paper (gas model), NEAR Nomicon runtime spec (Wasm host functions)
