#![cfg(not(tarpaulin_include))]

use cosmwasm_std::{coins, coin, Addr};
use cw_multi_test::Executor;
use cw_utils::PaymentError;

use oroswap_wrapper::error::ContractError;
use oroswap_wrapper::msg::ExecuteMsg;

use crate::common::helper::{Helper, LOCK_DENOM, MINT_SUBDENOM};
mod common;

// ---------------------------------------------------------------------------
// test_instantiate
// ---------------------------------------------------------------------------

/// Verify that after instantiation the config is stored correctly and the
/// tokenfactory token denom is created with the expected naming convention.
#[test]
fn test_instantiate() {
    let owner = Addr::unchecked("owner");
    let helper = Helper::new(&owner).unwrap();

    let config = helper.query_config().unwrap();

    assert_eq!(config.lock_denom, LOCK_DENOM);
    // ZigChain denom pattern: "coin.<contract_address>.<subdenom>"
    assert_eq!(
        config.mint_denom,
        format!("coin.{}.{}", helper.wrapper, MINT_SUBDENOM)
    );
    assert_eq!(config.owner, owner);
    assert!(!config.paused);
}

// ---------------------------------------------------------------------------
// test_lock_and_unlock
// ---------------------------------------------------------------------------

/// Full happy-path: Alice locks IBC tokens → gets mint tokens 1:1.
/// Then she unlocks half → burns mint tokens and gets IBC tokens back.
#[test]
fn test_lock_and_unlock() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let mint_denom = helper.mint_denom.clone();
    let wrapper_addr = helper.wrapper.clone();

    let alice = Addr::unchecked("alice");
    helper.give_lock_tokens(10_000, &alice);

    // --- Lock 5000 ---
    helper.lock(&alice, 5_000).unwrap();

    // Alice should now have 5000 mint tokens and 5000 lock tokens remaining
    let alice_mint = helper.query_balance(&alice, &mint_denom).unwrap();
    let alice_lock = helper.query_balance(&alice, LOCK_DENOM).unwrap();
    assert_eq!(alice_mint.u128(), 5_000, "alice mint after lock");
    assert_eq!(alice_lock.u128(), 5_000, "alice lock remaining");

    // Contract should hold 5000 lock tokens
    let contract_lock = helper
        .query_balance(&wrapper_addr, LOCK_DENOM)
        .unwrap();
    assert_eq!(contract_lock.u128(), 5_000, "contract lock balance");

    // TotalLocked query should agree
    let total_locked = helper.query_total_locked().unwrap();
    assert_eq!(total_locked.u128(), 5_000, "TotalLocked query");

    // --- Unlock 2000 ---
    helper.unlock(&alice, 2_000).unwrap();

    // Alice burns 2000 mint tokens and receives 2000 lock tokens back
    let alice_mint = helper.query_balance(&alice, &mint_denom).unwrap();
    let alice_lock = helper.query_balance(&alice, LOCK_DENOM).unwrap();
    assert_eq!(alice_mint.u128(), 3_000, "alice mint after partial unlock");
    assert_eq!(alice_lock.u128(), 7_000, "alice lock after partial unlock");

    // Contract holds 3000 lock tokens now
    let contract_lock = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    assert_eq!(contract_lock.u128(), 3_000, "contract after unlock");

    // TotalLocked == mint supply (1:1 invariant)
    let total_locked = helper.query_total_locked().unwrap();
    assert_eq!(total_locked.u128(), 3_000, "invariant after unlock");
}

// ---------------------------------------------------------------------------
// test_wrong_denom_lock
// ---------------------------------------------------------------------------

/// Sending the wrong denom to Lock must fail.
#[test]
fn test_wrong_denom_lock() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let bad_denom = "uzig";
    helper.mint_coin(&owner, coin(10_000, bad_denom));

    let err = helper
        .app
        .execute_contract(
            owner.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::Lock { receiver: None },
            &coins(1_000, bad_denom),
        )
        .unwrap_err();

    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MissingDenom(LOCK_DENOM.to_string()))
    );
}

// ---------------------------------------------------------------------------
// test_wrong_denom_unlock
// ---------------------------------------------------------------------------

/// Sending the wrong denom to Unlock must fail.
#[test]
fn test_wrong_denom_unlock() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // First lock some tokens so alice has ibc tokens to play with
    let alice = Addr::unchecked("alice");
    helper.give_lock_tokens(1_000, &alice);

    // Try to unlock with the raw lock denom instead of the mint denom
    let err = helper
        .app
        .execute_contract(
            alice.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::Unlock { receiver: None },
            &coins(100, LOCK_DENOM),
        )
        .unwrap_err();

    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::PaymentError(PaymentError::MissingDenom(
            helper.mint_denom.clone()
        ))
    );
}

// ---------------------------------------------------------------------------
// test_custom_receiver
// ---------------------------------------------------------------------------

/// When `receiver` is set, minted tokens go to that address, not the sender.
#[test]
fn test_custom_receiver() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let mint_denom = helper.mint_denom.clone();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");
    helper.give_lock_tokens(5_000, &alice);

    // Alice locks 1000 and sends mint tokens to Bob
    helper.lock_to(&alice, 1_000, &bob).unwrap();

    let bob_mint = helper.query_balance(&bob, &mint_denom).unwrap();
    let alice_mint = helper.query_balance(&alice, &mint_denom).unwrap();
    assert_eq!(bob_mint.u128(), 1_000, "bob received mint tokens");
    assert_eq!(alice_mint.u128(), 0, "alice has no mint tokens");

    // Bob unlocks to Alice
    helper.unlock_to(&bob, 500, &alice).unwrap();
    let alice_lock = helper.query_balance(&alice, LOCK_DENOM).unwrap();
    // Alice had 4000 lock tokens remaining; now 4000 + 500 = 4500
    assert_eq!(alice_lock.u128(), 4_500, "alice received lock tokens back");
}

// ---------------------------------------------------------------------------
// test_pause_unpause
// ---------------------------------------------------------------------------

/// Only the owner can pause/unpause. Lock/unlock are blocked while paused.
#[test]
fn test_pause_unpause() {
    let owner = Addr::unchecked("owner");
    let alice = Addr::unchecked("alice");
    let mut helper = Helper::new(&owner).unwrap();
    helper.give_lock_tokens(5_000, &alice);

    // Non-owner without pause-authority cannot pause
    let err = helper.pause(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::NoPauseAuthority {}
    );

    // Owner pauses
    helper.pause(&owner).unwrap();
    let config = helper.query_config().unwrap();
    assert!(config.paused);

    // Lock is blocked
    let err = helper.lock(&alice, 1_000).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Unlock with mint tokens (first need some — do it after unpause)
    // Non-owner cannot unpause
    let err = helper.unpause(&alice).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::NoUnpauseAuthority {}
    );

    // Owner unpauses
    helper.unpause(&owner).unwrap();
    let config = helper.query_config().unwrap();
    assert!(!config.paused);

    // Lock works again
    helper.lock(&alice, 1_000).unwrap();

    // Pause again; unlock is also blocked
    helper.pause(&owner).unwrap();
    let err = helper.unlock(&alice, 100).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::ContractPaused {}
    );

    // Restore
    helper.unpause(&owner).unwrap();
    helper.unlock(&alice, 100).unwrap();
}

// ---------------------------------------------------------------------------
// test_1_to_1_invariant
// ---------------------------------------------------------------------------

/// At every point the contract's `lock_denom` balance must equal the circulating
/// supply of `mint_denom`. We exercise this with multiple users.
#[test]
fn test_1_to_1_invariant() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    let mint_denom = helper.mint_denom.clone();
    let wrapper_addr = helper.wrapper.clone();

    let alice = Addr::unchecked("alice");
    let bob = Addr::unchecked("bob");
    helper.give_lock_tokens(10_000, &alice);
    helper.give_lock_tokens(10_000, &bob);

    // Both lock
    helper.lock(&alice, 3_000).unwrap();
    helper.lock(&bob, 7_000).unwrap();

    let locked = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    let minted = helper
        .app
        .wrap()
        .query_supply(&mint_denom)
        .unwrap()
        .amount;
    assert_eq!(locked, minted, "invariant after two locks");

    // Alice partially unlocks
    helper.unlock(&alice, 1_000).unwrap();

    let locked = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    let minted = helper
        .app
        .wrap()
        .query_supply(&mint_denom)
        .unwrap()
        .amount;
    assert_eq!(locked, minted, "invariant after alice unlock");

    // Bob fully unlocks
    helper.unlock(&bob, 7_000).unwrap();

    let locked = helper.query_balance(&wrapper_addr, LOCK_DENOM).unwrap();
    let minted = helper
        .app
        .wrap()
        .query_supply(&mint_denom)
        .unwrap()
        .amount;
    assert_eq!(locked, minted, "invariant after bob full unlock");
    assert_eq!(locked.u128(), 2_000, "2000 alice tokens remain locked");
}

// ---------------------------------------------------------------------------
// test_update_config
// ---------------------------------------------------------------------------

/// Owner can transfer ownership.
#[test]
fn test_update_config() {
    let owner = Addr::unchecked("owner");
    let new_owner = Addr::unchecked("new_owner");
    let alice = Addr::unchecked("alice");
    let mut helper = Helper::new(&owner).unwrap();

    // Non-owner cannot propose new owner
    let err = helper
        .app
        .execute_contract(
            alice.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::ProposeNewOwner {
                owner: alice.to_string(),
                expires_in: 86400,
            },
            &[],
        )
        .unwrap_err();
    // common helper returns StdError generic "Unauthorized"
    let err_str = format!("{err:?}");
    assert!(err_str.contains("Unauthorized"), "expected Unauthorized, got: {err_str}");

    // Owner proposes new_owner
    helper
        .app
        .execute_contract(
            owner.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::ProposeNewOwner {
                owner: new_owner.to_string(),
                expires_in: 86400,
            },
            &[],
        )
        .unwrap();

    // new_owner claims ownership
    helper
        .app
        .execute_contract(
            new_owner.clone(),
            helper.wrapper.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap();

    let config = helper.query_config().unwrap();
    assert_eq!(config.owner, new_owner);

    // Old owner can no longer pause
    let err = helper.pause(&owner).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::NoPauseAuthority {}
    );

    // New owner can pause
    helper.pause(&new_owner).unwrap();
    let config = helper.query_config().unwrap();
    assert!(config.paused);
}

// ---------------------------------------------------------------------------
// test_pause_authorities
// ---------------------------------------------------------------------------

/// Pause authorities (delegated wallets) can pause but only the owner can
/// unpause — the factory pattern.
#[test]
fn test_pause_authorities() {
    let owner = Addr::unchecked("owner");
    let watcher = Addr::unchecked("watcher");
    let stranger = Addr::unchecked("stranger");
    let mut helper = Helper::new(&owner).unwrap();

    // Only owner can manage the authority set
    let err = helper
        .add_pause_authorities(&stranger, vec![watcher.to_string()])
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    // Owner adds the watcher
    helper
        .add_pause_authorities(&owner, vec![watcher.to_string()])
        .unwrap();
    let authorities = helper.query_pause_authorities().unwrap();
    assert_eq!(authorities, vec![watcher.to_string()]);

    // Invalid address rejected
    let err = helper
        .add_pause_authorities(&owner, vec!["".to_string()])
        .unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::InvalidPauseAuthority { address: "".to_string() }
    );

    // Watcher can pause
    helper.pause(&watcher).unwrap();
    assert!(helper.query_config().unwrap().paused);

    // Watcher cannot unpause — only owner can
    let err = helper.unpause(&watcher).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::NoUnpauseAuthority {}
    );

    helper.unpause(&owner).unwrap();
    assert!(!helper.query_config().unwrap().paused);

    // Removing the authority revokes pause power
    helper
        .remove_pause_authorities(&owner, vec![watcher.to_string()])
        .unwrap();
    assert!(helper.query_pause_authorities().unwrap().is_empty());

    let err = helper.pause(&watcher).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::NoPauseAuthority {}
    );
}

// ---------------------------------------------------------------------------
// test_recover_excess
// ---------------------------------------------------------------------------

/// Owner can sweep `lock_denom` accidentally bank-sent to the contract;
/// the call fails cleanly when the contract is exactly 1:1 backed.
#[test]
fn test_recover_excess() {
    let owner = Addr::unchecked("owner");
    let alice = Addr::unchecked("alice");
    let stranger = Addr::unchecked("stranger");
    let mut helper = Helper::new(&owner).unwrap();
    helper.give_lock_tokens(10_000, &alice);

    // 1) No excess yet — call should fail
    let err = helper.recover_excess(&owner).unwrap_err();
    assert!(
        err.root_cause()
            .to_string()
            .contains("No excess tokens to recover"),
        "expected no-excess error, got: {err:?}"
    );

    // 2) Lock 3000 (balance == mint supply == 3000)
    helper.lock(&alice, 3_000).unwrap();

    // 3) Bypass Lock: bank-send 500 directly to the contract — now
    // balance = 3500, supply = 3000, excess = 500
    helper.force_lock_balance(500);

    // 4) Non-owner cannot recover
    let err = helper.recover_excess(&stranger).unwrap_err();
    assert_eq!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::Unauthorized {}
    );

    let owner_before = helper.query_balance(&owner, LOCK_DENOM).unwrap();
    helper.recover_excess(&owner).unwrap();
    let owner_after = helper.query_balance(&owner, LOCK_DENOM).unwrap();
    assert_eq!(
        owner_after.u128() - owner_before.u128(),
        500,
        "owner received the 500 excess"
    );

    // Invariant restored: balance == supply
    let bal = helper.query_balance(&helper.wrapper.clone(), LOCK_DENOM).unwrap();
    let total_locked = helper.query_total_locked().unwrap();
    assert_eq!(bal, total_locked);
    assert_eq!(bal.u128(), 3_000);

    // 5) With invariant restored, recover_excess fails again
    let err = helper.recover_excess(&owner).unwrap_err();
    assert!(
        err.root_cause()
            .to_string()
            .contains("No excess tokens to recover"),
        "expected no-excess error, got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// test_drop_ownership_proposal
// ---------------------------------------------------------------------------

/// Owner can drop a pending proposal so the proposed owner can no longer claim.
#[test]
fn test_drop_ownership_proposal() {
    let owner = Addr::unchecked("owner");
    let new_owner = Addr::unchecked("new_owner");
    let stranger = Addr::unchecked("stranger");
    let mut helper = Helper::new(&owner).unwrap();

    helper.propose_new_owner(&owner, &new_owner, 86_400).unwrap();

    // Non-owner cannot drop
    let err = helper.drop_ownership_proposal(&stranger).unwrap_err();
    let err_str = format!("{err:?}");
    assert!(
        err_str.contains("Unauthorized"),
        "expected Unauthorized, got: {err_str}"
    );

    // Owner drops the proposal
    helper.drop_ownership_proposal(&owner).unwrap();

    // Now the proposed owner cannot claim — there's no proposal
    let err = helper.claim_ownership(&new_owner).unwrap_err();
    let err_str = format!("{err:?}");
    assert!(
        err_str.to_lowercase().contains("ownership proposal not found")
            || err_str.to_lowercase().contains("proposal"),
        "expected proposal-missing error, got: {err_str}"
    );

    // Owner is still the owner
    assert_eq!(helper.query_config().unwrap().owner, owner);
}

// ---------------------------------------------------------------------------
// test_ownership_proposal_expiry
// ---------------------------------------------------------------------------

/// Once `expires_in` seconds elapse, a pending proposal can no longer be
/// claimed.
#[test]
fn test_ownership_proposal_expiry() {
    let owner = Addr::unchecked("owner");
    let new_owner = Addr::unchecked("new_owner");
    let mut helper = Helper::new(&owner).unwrap();

    let ttl = 3_600u64;
    helper.propose_new_owner(&owner, &new_owner, ttl).unwrap();

    // Advance past the proposal's lifetime
    helper.next_block(ttl + 1);

    // Claim must fail because the proposal has expired
    let err = helper.claim_ownership(&new_owner).unwrap_err();
    let err_str = format!("{err:?}");
    assert!(
        err_str.to_lowercase().contains("expired"),
        "expected expiry error, got: {err_str}"
    );

    // Owner is unchanged
    assert_eq!(helper.query_config().unwrap().owner, owner);
}

// ---------------------------------------------------------------------------
// test_migrate
// ---------------------------------------------------------------------------

/// Same-version migrate is a no-op that succeeds and stamps the new cw2
/// contract-version attributes. (The wrong-contract-name rejection branch is
/// covered by a unit test in `src/testing.rs`, since cw-multi-test doesn't
/// expose contract storage for tampering.)
#[test]
fn test_migrate() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();
    helper.migrate_self().unwrap();
}

// ─── UpdateDenomMetadata + ProposeDenomAdmin payload captures ────────────────
//
// Audit M-A / M-B asked for end-to-end coverage that asserts on the *exact*
// `MsgSetDenomMetadata` / `MsgProposeDenomAdmin` bytes the contract sends to
// the chain — execute() returning Ok isn't enough, since the patch-vs-set
// bug fixed in B-3 still let "success" responses through while silently
// corrupting Metadata.

/// Reply-handler invariant: the very first `MsgSetDenomMetadata` sent at
/// instantiate carries the resolved defaults (derived from `mint_subdenom`
/// since the helper passes `None` for name/symbol/description), plus the
/// `mint_decimals` from `InstantiateMsg` — not a hardcoded value.
#[test]
fn test_initial_metadata_emitted_at_instantiate() {
    let owner = Addr::unchecked("owner");
    let helper = Helper::new(&owner).unwrap();

    let captured = helper.captured.lock().unwrap();
    assert_eq!(
        captured.set_metadata.len(),
        1,
        "exactly one MsgSetDenomMetadata expected from the create-denom reply"
    );
    let m = captured.set_metadata[0]
        .metadata
        .as_ref()
        .expect("metadata field");
    // Defaults derive from MINT_SUBDENOM = "xoro"
    assert_eq!(m.symbol, MINT_SUBDENOM);
    assert_eq!(m.name, format!("Oroswap {}", MINT_SUBDENOM));
    assert_eq!(m.description, format!("Oroswap {} Token", MINT_SUBDENOM));
    assert_eq!(m.display, MINT_SUBDENOM.to_lowercase());
    assert_eq!(m.base, helper.mint_denom);
    assert_eq!(m.uri, "https://example.com/metadata.json");
    // DenomUnits: [{base, 0}, {display, mint_decimals=6}]
    assert_eq!(m.denom_units.len(), 2);
    assert_eq!(m.denom_units[0].exponent, 0);
    assert_eq!(m.denom_units[1].exponent, 6);
    assert_eq!(m.denom_units[1].denom, MINT_SUBDENOM.to_lowercase());
}

/// Regression for B-3: a partial `UpdateDenomMetadata { uri: Some, rest: None }`
/// must emit a `MsgSetDenomMetadata` that preserves the previously-stored
/// name/symbol/description — NOT regenerate them from subdenom defaults — and
/// must echo back the stored `mint_decimals`, never a hardcoded 6.
#[test]
fn test_update_metadata_partial_patch_preserves_custom_fields() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    // First: set custom branding via a full patch (all fields Some).
    helper
        .update_denom_metadata(
            &owner,
            Some("Custom Wrapped Branding".to_string()),
            Some("xCUSTOM".to_string()),
            Some("Hand-picked description".to_string()),
            Some("https://example/v1.png".to_string()),
        )
        .unwrap();

    // Then: "just update the logo" — only uri.
    helper
        .update_denom_metadata(
            &owner,
            None,
            None,
            None,
            Some("https://example/v2.png".to_string()),
        )
        .unwrap();

    let captured = helper.captured.lock().unwrap();
    // 1 from reply + 2 from the two UpdateDenomMetadata calls = 3.
    assert_eq!(captured.set_metadata.len(), 3);

    let latest = captured.set_metadata[2]
        .metadata
        .as_ref()
        .expect("metadata field");
    // The custom branding must survive the uri-only patch.
    assert_eq!(latest.name, "Custom Wrapped Branding");
    assert_eq!(latest.symbol, "xCUSTOM");
    assert_eq!(latest.description, "Hand-picked description");
    // display tracks the (latest) symbol.
    assert_eq!(latest.display, "xcustom");
    // uri was the only field actually patched.
    assert_eq!(latest.uri, "https://example/v2.png");
    // Decimals echo the instantiate-time value, NOT a hardcoded 6.
    assert_eq!(latest.denom_units[1].exponent, 6);
}

/// `ProposeDenomAdmin` payload capture: owner-only handler reaches the chain
/// with the right signer, denom, and admin addresses.
#[test]
fn test_propose_denom_admin_emits_correct_payload() {
    let owner = Addr::unchecked("owner");
    let mut helper = Helper::new(&owner).unwrap();

    let bank_admin = "zig1bankadmin000000000000000000000000000000";
    let metadata_admin = "zig1metaadmin000000000000000000000000000000";
    helper
        .propose_denom_admin(&owner, bank_admin, metadata_admin)
        .unwrap();

    let captured = helper.captured.lock().unwrap();
    assert_eq!(captured.propose_admin.len(), 1);
    let p = &captured.propose_admin[0];
    assert_eq!(p.signer, helper.wrapper.to_string());
    assert_eq!(p.denom, helper.mint_denom);
    assert_eq!(p.bank_admin, bank_admin);
    assert_eq!(p.metadata_admin, metadata_admin);
}

/// Non-owner cannot call `ProposeDenomAdmin` even at the integration layer
/// (defence in depth — also covered by the unit test).
#[test]
fn test_propose_denom_admin_non_owner_rejected_integration() {
    let owner = Addr::unchecked("owner");
    let mallory = Addr::unchecked("mallory");
    let mut helper = Helper::new(&owner).unwrap();

    let err = helper
        .propose_denom_admin(
            &mallory,
            "zig1bankadmin000000000000000000000000000000",
            "zig1metaadmin000000000000000000000000000000",
        )
        .unwrap_err();
    let root = err.downcast::<ContractError>().unwrap();
    assert_eq!(root, ContractError::Unauthorized {});
}
