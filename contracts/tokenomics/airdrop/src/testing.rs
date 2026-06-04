use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_json, Uint128, BankMsg, CosmosMsg};

use crate::contract::{execute, instantiate, query};
use crate::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    AirdropResponse, UserAirdropStatusResponse,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        admin: "admin".to_string(),
    };
    let info = mock_info("creator", &[]);

    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_json(&res).unwrap();
    assert_eq!("admin", value.admin);
}

#[test]
fn test_single_claim_timeline() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg { admin: "admin".to_string() };
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

    let env = mock_env();
    let start_time = env.block.time.seconds();
    let vesting_start_time = start_time + 100;
    let vesting_end_time = vesting_start_time + 1000; // 1000s duration

    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time,
        vesting_start_time,
        vesting_end_time,
        initial_claim_percent: 40,
        apy_bonus_bps: 1000, // 10%
        eligibility: vec![
            ("user1".to_string(), Uint128::new(1000)),
            ("user2".to_string(), Uint128::new(1000)),
            ("user3".to_string(), Uint128::new(1000)),
        ],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), env.clone(), mock_info("admin", &[]), msg).unwrap();

    // Scenario 1: Claim at Day 0 (before vesting starts)
    // Should get exactly 40% = 400
    let msg = ExecuteMsg::Claim { airdrop_id: 1 };
    let res = execute(deps.as_mut(), env.clone(), mock_info("user1", &[]), msg).unwrap();
    
    let expected_payout = Uint128::new(400);
    assert_eq!(res.messages.len(), 1);
    if let CosmosMsg::Bank(BankMsg::Send { amount, .. }) = &res.messages[0].msg {
        assert_eq!(amount[0].amount, expected_payout);
    } else { panic!("Wrong message type"); }

    // Try to claim again
    let msg = ExecuteMsg::Claim { airdrop_id: 1 };
    let err = execute(deps.as_mut(), env.clone(), mock_info("user1", &[]), msg).unwrap_err();
    assert!(format!("{}", err).to_lowercase().contains("already claimed"));

    // Scenario 2: Claim midway (500s into 1000s vesting)
    let mut env_mid = mock_env();
    env_mid.block.time = env_mid.block.time.plus_seconds(100 + 500); 
    
    let msg = ExecuteMsg::Claim { airdrop_id: 1 };
    let res = execute(deps.as_mut(), env_mid.clone(), mock_info("user2", &[]), msg).unwrap();
    
    // Initial: 400
    // Vesting principal: 600
    // Unlocked: 600 * 500/1000 = 300
    // Bonus: 0 (Bonus is now a cliff at the end)
    let expected_total = Uint128::new(400 + 300); 
    
    let total_attr = res.attributes.iter().find(|a| a.key == "total_payout").unwrap();
    assert_eq!(total_attr.value, expected_total.to_string());
    let bonus_attr = res.attributes.iter().find(|a| a.key == "bonus").unwrap();
    assert_eq!(bonus_attr.value, "0");

    // Scenario 3: Claim after vesting ends
    let mut env_end = mock_env();
    env_end.block.time = env_end.block.time.plus_seconds(2000); 
    
    let msg = ExecuteMsg::Claim { airdrop_id: 1 };
    let res = execute(deps.as_mut(), env_end.clone(), mock_info("user3", &[]), msg).unwrap();
    
    // Initial: 400
    // Vesting: 600
    // Bonus: 600 * 0.10 = 60
    // Total should be 1000 + 60 = 1060
    let total_attr = res.attributes.iter().find(|a| a.key == "total_payout").unwrap();
    assert_eq!(total_attr.value, "1060");
}

#[test]
fn test_multi_season() {
    let mut deps = mock_dependencies();
    let msg = InstantiateMsg { admin: "admin".to_string() };
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

    let now = mock_env().block.time.seconds();
    // Season 1
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "oro1".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100, // 100% immediate for simplicity
        apy_bonus_bps: 0,
        eligibility: vec![("user".to_string(), Uint128::new(500))],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Season 2
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "oro2".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 50,
        apy_bonus_bps: 0,
        eligibility: vec![("user".to_string(), Uint128::new(1000))],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Claim Season 1
    execute(deps.as_mut(), mock_env(), mock_info("user", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();
    
    // Claim Season 2
    execute(deps.as_mut(), mock_env(), mock_info("user", &[]), ExecuteMsg::Claim { airdrop_id: 2 }).unwrap();

    // Verify both are claimed via UserAirdropStatus
    let res1: UserAirdropStatusResponse = from_json(&query(deps.as_ref(), mock_env(), QueryMsg::UserAirdropStatus { id: 1, address: "user".to_string() }).unwrap()).unwrap();
    let res2: UserAirdropStatusResponse = from_json(&query(deps.as_ref(), mock_env(), QueryMsg::UserAirdropStatus { id: 2, address: "user".to_string() }).unwrap()).unwrap();
    
    assert_eq!(res1.is_claimed, true);
    assert_eq!(res1.amount_claimed, Uint128::new(500));
    assert_eq!(res2.is_claimed, true);
    assert_eq!(res2.amount_claimed, Uint128::new(500)); // Claimed at T=0, so only initial 50%
}

#[test]
fn test_admin_toggle() {
    let mut deps = mock_dependencies();
    let msg = InstantiateMsg { admin: "admin".to_string() };
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

    let now = mock_env().block.time.seconds();
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![("user".to_string(), Uint128::new(1000))],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Toggle inactive
    let msg = ExecuteMsg::ToggleAirdrop { id: 1, active: false };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Check status via query
    let res: AirdropResponse = from_json(&query(deps.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap()).unwrap();
    assert_eq!(res.is_active, false);

    // Try to claim
    let msg = ExecuteMsg::Claim { airdrop_id: 1 };
    let err = execute(deps.as_mut(), mock_env(), mock_info("user", &[]), msg).unwrap_err();
    assert!(format!("{}", err).to_lowercase().contains("inactive"));

    // Toggle back active
    let msg = ExecuteMsg::ToggleAirdrop { id: 1, active: true };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Claim succeeds
    execute(deps.as_mut(), mock_env(), mock_info("user", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();
}

#[test]
fn test_user_airdrop_status_query() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg { admin: "admin".to_string() };
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

    let env = mock_env();
    let start_time = env.block.time.seconds() + 50; // starts in 50s
    let vesting_start_time = start_time + 100;       // vesting starts 100s after start
    let vesting_end_time = vesting_start_time + 1000; // 1000s vesting duration

    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time,
        vesting_start_time,
        vesting_end_time,
        initial_claim_percent: 40,
        apy_bonus_bps: 1000, // 10%
        eligibility: vec![
            ("user1".to_string(), Uint128::new(1000)),
        ],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), env.clone(), mock_info("admin", &[]), msg).unwrap();

    // --- Scenario 1: Before airdrop starts ---
    let res: UserAirdropStatusResponse = from_json(
        &query(deps.as_ref(), env.clone(), QueryMsg::UserAirdropStatus { id: 1, address: "user1".to_string() }).unwrap()
    ).unwrap();
    assert_eq!(res.total_allocation, Uint128::new(1000));
    assert_eq!(res.current_claimable, Uint128::zero());
    assert_eq!(res.is_claimed, false);
    assert_eq!(res.vesting_status, "not_started");

    // --- Scenario 2: After start, before vesting begins ---
    let mut env2 = mock_env();
    env2.block.time = env2.block.time.plus_seconds(60); // 60s in, start_time was +50
    let res: UserAirdropStatusResponse = from_json(
        &query(deps.as_ref(), env2.clone(), QueryMsg::UserAirdropStatus { id: 1, address: "user1".to_string() }).unwrap()
    ).unwrap();
    assert_eq!(res.total_allocation, Uint128::new(1000));
    assert_eq!(res.current_claimable, Uint128::new(400)); // 40% initial only
    assert_eq!(res.is_claimed, false);
    assert_eq!(res.vesting_status, "not_started");

    // --- Scenario 3: Mid-vesting (500s into 1000s vesting) ---
    let mut env3 = mock_env();
    env3.block.time = env3.block.time.plus_seconds(50 + 100 + 500); // start + gap + 500s vesting
    let res: UserAirdropStatusResponse = from_json(
        &query(deps.as_ref(), env3.clone(), QueryMsg::UserAirdropStatus { id: 1, address: "user1".to_string() }).unwrap()
    ).unwrap();
    assert_eq!(res.total_allocation, Uint128::new(1000));
    // initial 400 + vesting 600*500/1000 = 400 + 300 = 700
    assert_eq!(res.current_claimable, Uint128::new(700));
    assert_eq!(res.is_claimed, false);
    assert_eq!(res.vesting_status, "in_progress");

    // --- Scenario 4: After vesting ends ---
    let mut env4 = mock_env();
    env4.block.time = env4.block.time.plus_seconds(50 + 100 + 1000 + 100); // well past end
    let res: UserAirdropStatusResponse = from_json(
        &query(deps.as_ref(), env4.clone(), QueryMsg::UserAirdropStatus { id: 1, address: "user1".to_string() }).unwrap()
    ).unwrap();
    assert_eq!(res.total_allocation, Uint128::new(1000));
    // initial 400 + vesting 600 + bonus 600*10% = 1060
    assert_eq!(res.current_claimable, Uint128::new(1060));
    assert_eq!(res.is_claimed, false);
    assert_eq!(res.vesting_status, "completed");

    // --- Scenario 5: After user claims ---
    // Claim at env3 time (mid-vesting)
    execute(deps.as_mut(), env3.clone(), mock_info("user1", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();

    let res: UserAirdropStatusResponse = from_json(
        &query(deps.as_ref(), env4.clone(), QueryMsg::UserAirdropStatus { id: 1, address: "user1".to_string() }).unwrap()
    ).unwrap();
    assert_eq!(res.total_allocation, Uint128::new(1000));
    assert_eq!(res.current_claimable, Uint128::zero());
    assert_eq!(res.is_claimed, true);
    assert_eq!(res.amount_claimed, Uint128::new(700)); // claimed at mid-vesting
    assert!(res.claim_time.is_some());
    assert_eq!(res.vesting_status, "claimed");

    // --- Scenario 6: Non-eligible user ---
    let res: UserAirdropStatusResponse = from_json(
        &query(deps.as_ref(), env4.clone(), QueryMsg::UserAirdropStatus { id: 1, address: "nobody".to_string() }).unwrap()
    ).unwrap();
    assert_eq!(res.total_allocation, Uint128::zero());
    assert_eq!(res.current_claimable, Uint128::zero());
    assert_eq!(res.is_claimed, false);
}

#[test]
fn test_ibc_and_cw20_handling() {
    let mut deps = mock_dependencies();
    let msg = InstantiateMsg { admin: "admin".to_string() };
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

    let now = mock_env().block.time.seconds();
    
    // 1. IBC Denom
    let ibc_denom = "ibc/F1073E5895A25347EF10A7B50783D3048882726895AEE1176C498C45D0576F2B".to_string();
    let msg = ExecuteMsg::AddAirdrop {
        token_address: ibc_denom.clone(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![("user1".to_string(), Uint128::new(1000))],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Claim IBC token
    let res = execute(deps.as_mut(), mock_env(), mock_info("user1", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();
    assert_eq!(res.messages.len(), 1);
    if let CosmosMsg::Bank(BankMsg::Send { to_address, amount }) = &res.messages[0].msg {
        assert_eq!(to_address, "user1");
        assert_eq!(amount[0].denom, ibc_denom);
    } else { panic!("Expected BankMsg::Send for IBC denom"); }

    // 2. CW20 Token
    let cw20_address = "wasm1cw20tokenaddress".to_string();
    let msg = ExecuteMsg::AddAirdrop {
        token_address: cw20_address.clone(),
        is_native: false, // Treat as CW20
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![("user2".to_string(), Uint128::new(1000))],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Claim CW20 token
    let res = execute(deps.as_mut(), mock_env(), mock_info("user2", &[]), ExecuteMsg::Claim { airdrop_id: 2 }).unwrap();
    assert_eq!(res.messages.len(), 1);
    if let CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute { contract_addr, msg, .. }) = &res.messages[0].msg {
        assert_eq!(contract_addr, &cw20_address);
        // Verify it's a Transfer msg
        let transfer_msg: cw20::Cw20ExecuteMsg = from_json(msg).unwrap();
        match transfer_msg {
            cw20::Cw20ExecuteMsg::Transfer { recipient, amount } => {
                assert_eq!(recipient, "user2");
                assert_eq!(amount, Uint128::new(1000));
            },
            _ => panic!("Expected Transfer message"),
        }
    } else { panic!("Expected WasmMsg::Execute for CW20"); }
}

#[test]
fn test_deposit_and_withdraw() {
    let mut deps = mock_dependencies();
    let msg = InstantiateMsg { admin: "admin".to_string() };
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

    let denom = "native.oro".to_string();
    let amount = Uint128::new(5000);

    // Deposit Native (success)
    let info = mock_info("admin", &[cosmwasm_std::Coin { denom: denom.clone(), amount }]);
    let msg = ExecuteMsg::DepositFunds { token_address: denom.clone(), amount, is_native: true };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Deposit Native with extra coins must reject
    let info_wrong = mock_info("admin", &[
        cosmwasm_std::Coin { denom: denom.clone(), amount },
        cosmwasm_std::Coin { denom: "other".to_string(), amount: Uint128::new(100) } // Extra funds!
    ]);
    let msg = ExecuteMsg::DepositFunds { token_address: denom.clone(), amount, is_native: true };
    let err = execute(deps.as_mut(), mock_env(), info_wrong, msg).unwrap_err();
    match err {
        ContractError::PaymentError(cw_utils::PaymentError::MultipleDenoms {}) => {}
        _ => panic!("Expected MultipleDenoms error, got: {:?}", err),
    }

    // Withdraw Native
    let msg = ExecuteMsg::WithdrawFunds { token_address: denom.clone(), amount: Uint128::new(2000), is_native: true };
    let res = execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    if let CosmosMsg::Bank(BankMsg::Send { amount, .. }) = &res.messages[0].msg {
        assert_eq!(amount[0].amount, Uint128::new(2000));
    } else { panic!("Expected BankMsg::Send"); }

    // Deposit CW20
    let cw20_address = "wasm1cw20".to_string();
    let msg = ExecuteMsg::DepositFunds { token_address: cw20_address.clone(), amount: Uint128::new(3000), is_native: false };
    let res = execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();
    assert_eq!(res.messages.len(), 1);
    if let CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute { contract_addr, msg, .. }) = &res.messages[0].msg {
        assert_eq!(contract_addr, &cw20_address);
        let transfer_from: cw20::Cw20ExecuteMsg = from_json(msg).unwrap();
        match transfer_from {
            cw20::Cw20ExecuteMsg::TransferFrom { owner, recipient, amount: amt } => {
                assert_eq!(owner, "admin");
                assert_eq!(recipient, mock_env().contract.address);
                assert_eq!(amt, Uint128::new(3000));
            },
            _ => panic!("Expected TransferFrom message"),
        }
    } else { panic!("Expected WasmMsg::Execute"); }
}

#[test]
fn test_update_eligibility_and_zero_claim() {
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now + 100, // Starts in future
        vesting_start_time: now + 100,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![], // Empty initially
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Update eligibility in batches
    let msg = ExecuteMsg::UpdateEligibility {
        airdrop_id: 1,
        eligibility: vec![("user1".to_string(), Uint128::new(1000))],
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    let msg = ExecuteMsg::UpdateEligibility {
        airdrop_id: 1,
        // user2 gets 0 — must be rejected on claim
        eligibility: vec![("user2".to_string(), Uint128::zero())],
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Move to start time
    let mut env_start = mock_env();
    env_start.block.time = env_start.block.time.plus_seconds(100);

    // Normal claim
    execute(deps.as_mut(), env_start.clone(), mock_info("user1", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();

    // Zero-eligible claim must fail with NotEligible
    let err = execute(deps.as_mut(), env_start.clone(), mock_info("user2", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap_err();
    assert!(format!("{}", err).contains("Not eligible"));
}

#[test]
fn test_edge_cases_and_unauthorized() {
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    
    // Unauthorized action
    let msg = ExecuteMsg::ToggleAirdrop { id: 1, active: false };
    let err = execute(deps.as_mut(), mock_env(), mock_info("user", &[]), msg).unwrap_err();
    assert!(format!("{}", err).contains("Unauthorized"));

    // Cap: outrageous apy_bonus_bps is rejected
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 0,
        apy_bonus_bps: 1_000_000_000,
        eligibility: vec![("user1".to_string(), Uint128::MAX)],
        expiry_time: 9_999_999_999u64,
    };
    let err = execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap_err();
    match err {
        ContractError::ApyBonusTooHigh { got, max } => {
            assert_eq!(got, 1_000_000_000);
            assert_eq!(max, crate::contract::MAX_APY_BONUS_BPS);
        }
        other => panic!("expected ApyBonusTooHigh, got: {other:?}"),
    }

    // Edge case at the cap: max-allowed bonus + Uint128::MAX eligibility
    // must NOT overflow multiply_ratio. 0% initial → all in the vesting base.
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 0,
        apy_bonus_bps: crate::contract::MAX_APY_BONUS_BPS,
        eligibility: vec![("user1".to_string(), Uint128::MAX)],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Claim at start time (0% initial + vesting not elapsed → 0 payout) must
    // revert with NothingToClaim. Settling a zero-payout claim would lock the
    // user out of their entire allocation via the AlreadyClaimed gate, so the
    // user is expected to retry once at least one portion has unlocked.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user1", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::NothingToClaim {}));
}

#[test]
fn test_airdrop_expiry() {
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    let expiry_time = now + 1500;
    
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![("user1".to_string(), Uint128::new(1000))],
        expiry_time: expiry_time,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // 1. Claim before expiry should succeed
    let mut env_before = mock_env();
    env_before.block.time = env_before.block.time.plus_seconds(400);
    execute(deps.as_mut(), env_before.clone(), mock_info("user1", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();

    // 2. Add another airdrop for user2 that will expire
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![("user2".to_string(), Uint128::new(1000))],
        expiry_time: expiry_time,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // 3. Claim after expiry should fail
    let mut env_after = mock_env();
    env_after.block.time = env_after.block.time.plus_seconds(1501);
    let err = execute(deps.as_mut(), env_after.clone(), mock_info("user2", &[]), ExecuteMsg::Claim { airdrop_id: 2 }).unwrap_err();
    assert!(format!("{}", err).to_lowercase().contains("expired"));

    // 4. Query status after expiry should show "expired"
    let res: UserAirdropStatusResponse = from_json(
        &query(deps.as_ref(), env_after.clone(), QueryMsg::UserAirdropStatus { id: 2, address: "user2".to_string() }).unwrap()
    ).unwrap();
    assert_eq!(res.vesting_status, "expired");
    assert_eq!(res.current_claimable, Uint128::zero());
}

// ── Eligibility batch cap ─────────────────────────────────────────────────────

#[test]
fn test_add_airdrop_eligibility_batch_cap() {
    use crate::contract::MAX_ELIGIBILITY_BATCH;
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    // exactly MAX_ELIGIBILITY_BATCH entries — allowed
    let ok_eligibility: Vec<(String, Uint128)> = (0..MAX_ELIGIBILITY_BATCH)
        .map(|i| (format!("user{i}"), Uint128::new(100)))
        .collect();
    let now = mock_env().block.time.seconds();
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 100,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: ok_eligibility,
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // MAX + 1 entries — rejected
    let too_many: Vec<(String, Uint128)> = (0..=MAX_ELIGIBILITY_BATCH)
        .map(|i| (format!("user{i}"), Uint128::new(100)))
        .collect();
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 100,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: too_many,
        expiry_time: 9_999_999_999u64,
    };
    let err = execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap_err();
    match err {
        ContractError::EligibilityBatchTooLarge { got, max } => {
            assert_eq!(got, MAX_ELIGIBILITY_BATCH + 1);
            assert_eq!(max, MAX_ELIGIBILITY_BATCH);
        }
        other => panic!("expected EligibilityBatchTooLarge, got: {other:?}"),
    }
}

#[test]
fn test_update_eligibility_batch_cap() {
    use crate::contract::MAX_ELIGIBILITY_BATCH;
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    // Create an airdrop in the future so UpdateEligibility is allowed
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now + 1000,
        vesting_start_time: now + 1000,
        vesting_end_time: now + 2000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    let too_many: Vec<(String, Uint128)> = (0..=MAX_ELIGIBILITY_BATCH)
        .map(|i| (format!("user{i}"), Uint128::new(100)))
        .collect();
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateEligibility { airdrop_id: 1, eligibility: too_many },
    )
    .unwrap_err();
    match err {
        ContractError::EligibilityBatchTooLarge { got, max } => {
            assert_eq!(got, MAX_ELIGIBILITY_BATCH + 1);
            assert_eq!(max, MAX_ELIGIBILITY_BATCH);
        }
        other => panic!("expected EligibilityBatchTooLarge, got: {other:?}"),
    }
}

// ── Solvency query ────────────────────────────────────────────────────────────

#[test]
fn test_solvency_query_totals_and_funding() {
    use crate::msg::SolvencyResponse;

    let denom = "native.oro";
    // Pre-fund the contract with 1000 native.oro (mock dep helper)
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(1000, denom),
    );
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    // Airdrop committing 1500 total (1000 + 500), 50% initial, 10% apy bonus
    let msg = ExecuteMsg::AddAirdrop {
        token_address: denom.to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 50,
        apy_bonus_bps: 1000, // 10%
        eligibility: vec![
            ("user1".to_string(), Uint128::new(1000)),
            ("user2".to_string(), Uint128::new(500)),
        ],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Solvency snapshot before any claim
    let res: SolvencyResponse = from_json(
        &query(deps.as_ref(), mock_env(), QueryMsg::Solvency { id: 1 }).unwrap(),
    )
    .unwrap();
    assert_eq!(res.total_eligibility.u128(), 1500);
    assert_eq!(res.total_claimed.u128(), 0);
    assert_eq!(res.outstanding.u128(), 1500);
    // Worst case: vesting_base = 50% of 1500 = 750; bonus = 750 * 10% = 75
    // max_outstanding = 1500 + 75 = 1575
    assert_eq!(res.max_outstanding.u128(), 1575);
    assert_eq!(res.contract_balance.u128(), 1000);
    assert!(!res.is_fully_funded, "1000 balance can't cover 1575 worst case");

    // user1 claims at time = now (only initial 50% = 500 paid out)
    execute(deps.as_mut(), mock_env(), mock_info("user1", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();

    let res: SolvencyResponse = from_json(
        &query(deps.as_ref(), mock_env(), QueryMsg::Solvency { id: 1 }).unwrap(),
    )
    .unwrap();
    assert_eq!(res.total_claimed.u128(), 500);
    // user1's full eligibility (1000) is now settled — they can't claim
    // again, so the remaining liability is just user2's 500. The earlier
    // formulation `total_eligibility - total_claimed = 1000` overstated
    // this because it conflated paid-out tokens with claimed eligibility.
    assert_eq!(res.outstanding.u128(), 500);
    // Worst case for the one remaining user: 500 + (500 * 50% vesting * 10% apy) = 525.
    // Critically, user1's 1000 eligibility is NOT included in the bonus base.
    assert_eq!(res.max_outstanding.u128(), 525);
}

#[test]
fn test_solvency_tracks_update_eligibility_delta() {
    use crate::msg::SolvencyResponse;
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now + 1000,
            vesting_start_time: now + 1000,
            vesting_end_time: now + 2000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![("user1".to_string(), Uint128::new(500))],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();

    let res: SolvencyResponse = from_json(
        &query(deps.as_ref(), mock_env(), QueryMsg::Solvency { id: 1 }).unwrap(),
    )
    .unwrap();
    assert_eq!(res.total_eligibility.u128(), 500);

    // Overwrite user1 to 800, add user2 at 200 — total should be 1000
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateEligibility {
            airdrop_id: 1,
            eligibility: vec![
                ("user1".to_string(), Uint128::new(800)),
                ("user2".to_string(), Uint128::new(200)),
            ],
        },
    )
    .unwrap();

    let res: SolvencyResponse = from_json(
        &query(deps.as_ref(), mock_env(), QueryMsg::Solvency { id: 1 }).unwrap(),
    )
    .unwrap();
    assert_eq!(res.total_eligibility.u128(), 1000);
}

// ── WithdrawFunds restrictions ────────────────────────────────────────────────

#[test]
fn test_withdraw_blocked_while_active() {
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 2000,
        },
    )
    .unwrap();

    // Try to withdraw while the airdrop is still active
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::WithdrawFunds {
            token_address: "native.oro".to_string(),
            amount: Uint128::new(10),
            is_native: true,
        },
    )
    .unwrap_err();
    match err {
        ContractError::WithdrawalBlocked { airdrop_id } => assert_eq!(airdrop_id, 1),
        other => panic!("expected WithdrawalBlocked, got: {other:?}"),
    }
}

/// Audit follow-up: `expiry_time` is now mandatory on `AddAirdrop` AND must
/// be strictly greater than `vesting_end_time`. This removes the
/// previously-possible "expiry_time = None" foot-gun where leftover
/// tokens were permanently unreclaimable, and replaces it with a
/// deterministic `InvalidExpiry` rejection at creation time.
#[test]
fn test_add_airdrop_rejects_expiry_le_vesting_end() {
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    // expiry_time exactly equal to vesting_end_time → rejected.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 1000,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::InvalidExpiry {}));

    // expiry_time below vesting_end_time → also rejected.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 500,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::InvalidExpiry {}));

    // expiry_time > vesting_end_time → accepted.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 1001,
        },
    )
    .unwrap();
}

#[test]
fn test_withdraw_blocked_before_expiry() {
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 2000,
        },
    )
    .unwrap();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::ToggleAirdrop { id: 1, active: false },
    )
    .unwrap();

    // Inactive but expiry not yet reached → blocked
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::WithdrawFunds {
            token_address: "native.oro".to_string(),
            amount: Uint128::new(10),
            is_native: true,
        },
    )
    .unwrap_err();
    match err {
        ContractError::WithdrawalBlocked { airdrop_id } => assert_eq!(airdrop_id, 1),
        other => panic!("expected WithdrawalBlocked, got: {other:?}"),
    }
}

#[test]
fn test_withdraw_allowed_after_inactive_and_expired() {
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 2000,
        },
    )
    .unwrap();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::ToggleAirdrop { id: 1, active: false },
    )
    .unwrap();

    // Fast-forward past expiry
    let mut env_past = mock_env();
    env_past.block.time = env_past.block.time.plus_seconds(3000);

    let res = execute(
        deps.as_mut(),
        env_past,
        mock_info("admin", &[]),
        ExecuteMsg::WithdrawFunds {
            token_address: "native.oro".to_string(),
            amount: Uint128::new(10),
            is_native: true,
        },
    )
    .unwrap();
    assert_eq!(res.messages.len(), 1);
}

#[test]
fn test_claim_and_status_at_vesting_start_boundary() {
    // Claim uses strict `>` against vesting_start_time; UserAirdropStatus
    // uses `<=` to declare "not_started". They agree at the boundary:
    //   at current_time == vesting_start_time
    //     -> Claim pays only initial (no vesting unlock yet)
    //     -> UserAirdropStatus reports "not_started" with current_claimable = initial
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    let vesting_start = now + 100;
    let vesting_end = vesting_start + 1000;
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: vesting_start,
            vesting_end_time: vesting_end,
            initial_claim_percent: 40,
            apy_bonus_bps: 1000,
            eligibility: vec![
                ("user_query".to_string(), Uint128::new(1000)),
                ("user_claim".to_string(), Uint128::new(1000)),
            ],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();

    // Tick exactly to vesting_start_time
    let mut env_boundary = mock_env();
    env_boundary.block.time = cosmwasm_std::Timestamp::from_seconds(vesting_start);

    // (a) Status query at the boundary reports "not_started" with claimable = initial
    let res: UserAirdropStatusResponse = from_json(
        &query(
            deps.as_ref(),
            env_boundary.clone(),
            QueryMsg::UserAirdropStatus { id: 1, address: "user_query".to_string() },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(res.vesting_status, "not_started");
    assert_eq!(res.current_claimable.u128(), 400, "initial 40% of 1000");

    // (b) Claim at the boundary pays exactly the initial portion (no vesting unlock)
    let res = execute(
        deps.as_mut(),
        env_boundary,
        mock_info("user_claim", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap();
    let total_attr = res.attributes.iter().find(|a| a.key == "total_payout").unwrap();
    assert_eq!(total_attr.value, "400");
    let unlocked_attr = res.attributes.iter().find(|a| a.key == "unlocked_vesting").unwrap();
    assert_eq!(unlocked_attr.value, "0");
    let bonus_attr = res.attributes.iter().find(|a| a.key == "bonus").unwrap();
    assert_eq!(bonus_attr.value, "0");
}

#[test]
fn test_withdraw_allowed_for_unrelated_token() {
    // No airdrops use "other.token" → withdrawal goes through.
    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), InstantiateMsg { admin: "admin".to_string() }).unwrap();

    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 2000,
        },
    )
    .unwrap();

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::WithdrawFunds {
            token_address: "other.token".to_string(),
            amount: Uint128::new(10),
            is_native: true,
        },
    )
    .unwrap();
    assert_eq!(res.messages.len(), 1);
}

/// Regression guard for N-1: AddAirdrop with duplicate addresses must not
/// inflate ELIGIBILITY_TOTAL above what actually lands in storage. Last-wins
/// dedupe semantics (matches `execute_update_eligibility`).
#[test]
fn test_add_airdrop_deduplicates_repeated_addresses() {
    use crate::msg::SolvencyResponse;
    let mut deps = mock_dependencies();
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();
    // user1 appears twice (100, then 200 — last wins). user2 distinct.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now + 1,
            vesting_start_time: now + 1,
            vesting_end_time: now + 1000,
            initial_claim_percent: 0,
            apy_bonus_bps: 0,
            eligibility: vec![
                ("user1".to_string(), Uint128::new(100)),
                ("user1".to_string(), Uint128::new(200)),
                ("user2".to_string(), Uint128::new(50)),
            ],
            expiry_time: now + 2000,
        },
    )
    .unwrap();

    // Stored eligibility: user1 = 200 (last wins), user2 = 50 → total = 250.
    // Buggy behaviour would have produced total = 100 + 200 + 50 = 350.
    let res: SolvencyResponse = from_json(
        &query(deps.as_ref(), mock_env(), QueryMsg::Solvency { id: 1 }).unwrap(),
    )
    .unwrap();
    assert_eq!(
        res.total_eligibility.u128(),
        250,
        "ELIGIBILITY_TOTAL must reflect deduped storage, not the inflated sum",
    );

    // Cross-check user1's actual eligibility is the last-supplied amount.
    let status: UserAirdropStatusResponse = from_json(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::UserAirdropStatus {
                id: 1,
                address: "user1".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(status.total_allocation.u128(), 200);
}

/// Regression guard for N-2: `Solvency.max_outstanding` must NOT abort the
/// query when `outstanding + max_bonus` would overflow Uint128. It should
/// saturate to Uint128::MAX with `is_fully_funded = false`.
#[test]
fn test_solvency_saturates_on_max_outstanding_overflow() {
    use crate::msg::SolvencyResponse;
    use crate::state::{AirdropInfo, AIRDROPS, ELIGIBILITY_TOTAL, CLAIMED_TOTAL};

    let mut deps = mock_dependencies();
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();
    // Seed an extreme state directly: total_eligibility = Uint128::MAX,
    // initial_claim_percent = 0 (so vesting_base = MAX), apy = 10_000 bps
    // (so max_bonus = MAX). outstanding + max_bonus overflows.
    AIRDROPS
        .save(
            deps.as_mut().storage,
            1,
            &AirdropInfo {
                id: 1,
                token_address: "native.oro".to_string(),
                start_time: now,
                vesting_start_time: now,
                vesting_end_time: now + 1000,
                initial_claim_percent: 0,
                apy_bonus_bps: 10_000,
                is_active: true,
                is_native: true,
                expiry_time: 9_999_999_999u64,
            },
        )
        .unwrap();
    ELIGIBILITY_TOTAL
        .save(deps.as_mut().storage, 1, &Uint128::MAX)
        .unwrap();
    CLAIMED_TOTAL
        .save(deps.as_mut().storage, 1, &Uint128::zero())
        .unwrap();

    let res: SolvencyResponse = from_json(
        &query(deps.as_ref(), mock_env(), QueryMsg::Solvency { id: 1 }).unwrap(),
    )
    .expect("query must not abort on overflow");
    assert_eq!(res.max_outstanding, Uint128::MAX);
    assert!(!res.is_fully_funded);
}

// ─── UpdateApyBonus ──────────────────────────────────────────────────────────

/// Helper: instantiate + add a season starting `start_offset` seconds from
/// `mock_env().block.time`, vesting `duration` seconds long, with the given
/// `initial_pct` and `apy_bps`. Eligibility is a single 1000-unit user
/// (so totals are easy to reason about). Returns the start_time.
fn setup_season(
    deps: &mut cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    >,
    denom: &str,
    start_offset: u64,
    duration: u64,
    initial_pct: u64,
    apy_bps: u64,
    eligibility: Uint128,
) -> u64 {
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();
    let start_time = now + start_offset;
    let msg = ExecuteMsg::AddAirdrop {
        token_address: denom.to_string(),
        is_native: true,
        start_time,
        vesting_start_time: start_time,
        vesting_end_time: start_time + duration,
        initial_claim_percent: initial_pct,
        apy_bonus_bps: apy_bps,
        eligibility: vec![("user".to_string(), eligibility)],
        expiry_time: 9_999_999_999u64,
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();
    start_time
}

#[test]
fn test_update_apy_bonus_unauthorized_and_capped() {
    let mut deps = mock_dependencies();
    setup_season(&mut deps, "native.oro", 100, 1000, 40, 1000, Uint128::new(1000));

    // Non-admin rejected.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 1500 },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));

    // Above the cap rejected.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 10_001 },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::ApyBonusTooHigh { got: 10_001, max: 10_000 }));
}

#[test]
fn test_update_apy_bonus_pre_start_free_direction() {
    let mut deps = mock_dependencies();
    setup_season(&mut deps, "native.oro", 100, 1000, 40, 1000, Uint128::new(1000));

    // Before start: increase allowed (with empty balance is fine — outstanding
    // is reachable but we're checking direction logic; the funding gate runs
    // and will fail unless balance is set. Use the same denom-balance helper
    // to ensure the funding gate passes for the increase case).
    let mut deps_funded = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(2_000, "native.oro"),
    );
    setup_season(&mut deps_funded, "native.oro", 100, 1000, 40, 1000, Uint128::new(1000));

    // Pre-start increase: 1000 → 2000.
    execute(
        deps_funded.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 2000 },
    )
    .unwrap();
    let res: AirdropResponse =
        from_json(&query(deps_funded.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap())
            .unwrap();
    assert_eq!(res.apy_bonus_bps, 2000);

    // Pre-start decrease: 2000 → 500 (allowed because pre-start).
    execute(
        deps_funded.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 500 },
    )
    .unwrap();
    let res: AirdropResponse =
        from_json(&query(deps_funded.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap())
            .unwrap();
    assert_eq!(res.apy_bonus_bps, 500);
}

#[test]
fn test_update_apy_bonus_decrease_blocked_after_start() {
    let mut deps = mock_dependencies();
    let start_time = setup_season(&mut deps, "native.oro", 0, 1000, 40, 1000, Uint128::new(1000));

    // Advance to after start_time (we set start_offset=0, so default env is past start).
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(10);
    let _ = start_time;

    let err = execute(
        deps.as_mut(),
        env,
        mock_info("admin", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 500 },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ContractError::ApyDecreaseNotAllowed { old: 1000, new: 500 }
    ));
}

#[test]
fn test_update_apy_bonus_increase_after_start_with_funding() {
    // Pre-fund the contract so the solvency gate passes for an increase.
    // allocation = 1000, initial = 40% → vesting_base = 600.
    // outstanding = 1000. New max bonus at 2000 bps = 600 * 20% = 120.
    // max_outstanding = 1120. Balance 2000 is enough.
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(2_000, "native.oro"),
    );
    setup_season(&mut deps, "native.oro", 0, 1000, 40, 1000, Uint128::new(1000));

    // Just after start, before vesting end.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(10);

    execute(
        deps.as_mut(),
        env.clone(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 2000 },
    )
    .unwrap();

    let res: AirdropResponse =
        from_json(&query(deps.as_ref(), env, QueryMsg::Airdrop { id: 1 }).unwrap()).unwrap();
    assert_eq!(res.apy_bonus_bps, 2000);
}

#[test]
fn test_update_apy_bonus_increase_underfunded() {
    // allocation = 1000, initial = 40% → vesting_base = 600.
    // outstanding = 1000. Target new bps = 5000 (50%). Max bonus = 300.
    // max_outstanding = 1300. Balance 500 is too low.
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(500, "native.oro"),
    );
    setup_season(&mut deps, "native.oro", 0, 1000, 40, 1000, Uint128::new(1000));

    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(10);

    let err = execute(
        deps.as_mut(),
        env,
        mock_info("admin", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 5000 },
    )
    .unwrap_err();
    match err {
        ContractError::ApyUpdateUnderfunded { required, available } => {
            assert_eq!(required.u128(), 1300);
            assert_eq!(available.u128(), 500);
        }
        other => panic!("expected ApyUpdateUnderfunded, got {other:?}"),
    }
}

#[test]
fn test_update_apy_bonus_after_vesting_end_rejected() {
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(10_000, "native.oro"),
    );
    setup_season(&mut deps, "native.oro", 0, 1000, 40, 1000, Uint128::new(1000));

    // Jump well past vesting_end_time.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(2000);

    // Increase, decrease, and no-op are all rejected once vesting has ended.
    for new_bps in [500, 1000, 2000] {
        let err = execute(
            deps.as_mut(),
            env.clone(),
            mock_info("admin", &[]),
            ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: new_bps },
        )
        .unwrap_err();
        assert!(
            matches!(err, ContractError::ApyUpdateAfterVestingEnd {}),
            "unexpected error for new_bps={new_bps}: {err:?}"
        );
    }
}

/// Audit regression: the APY-bonus solvency gate must compute the worst-case
/// bonus from the *unclaimed* eligibility only — settled users no longer
/// carry a future bonus obligation. A version that anchored `max_bonus` to
/// `total_eligibility` would reject this increase as underfunded even though
/// the contract holds more than enough for everyone still able to claim.
#[test]
fn test_update_apy_bonus_excludes_claimed_users_from_bonus_base() {
    let denom = "native.oro";
    // 2 users × 1000 eligibility = 2000 total, 40% initial, 1000 bps (10%) starting APY.
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(1_500, denom),
    );
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: denom.to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 40,
            apy_bonus_bps: 1000,
            eligibility: vec![
                ("user1".to_string(), Uint128::new(1000)),
                ("user2".to_string(), Uint128::new(1000)),
            ],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();

    // user1 settles at t = now + 10: initial 400 + 10/1000 of 600 vesting = 406.
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(10);
    execute(
        deps.as_mut(),
        env.clone(),
        mock_info("user1", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap();

    // Admin raises APY 1000 → 5000 bps. Only user2 (1000 eligibility) still has
    // a bonus claim against the contract:
    //   remaining_eligibility    = 1000
    //   remaining_vesting_base   = 1000 * 60% = 600
    //   max_bonus @ 5000 bps     = 600 * 50% = 300
    //   max_outstanding          = 1000 + 300 = 1300  ≤  balance 1500 → OK.
    // An eligibility-based gate would compute max_bonus over 2000 (both users)
    // → vesting_base 1200 → bonus 600 → max_outstanding ≈ 2194, blocking the
    // increase even though user1's obligation is already settled.
    execute(
        deps.as_mut(),
        env.clone(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateApyBonus { airdrop_id: 1, apy_bonus_bps: 5000 },
    )
    .unwrap();

    let res: AirdropResponse =
        from_json(&query(deps.as_ref(), env.clone(), QueryMsg::Airdrop { id: 1 }).unwrap())
            .unwrap();
    assert_eq!(res.apy_bonus_bps, 5000);

    // Solvency snapshot reflects the same accounting: outstanding == remaining
    // eligibility, and max_outstanding uses the remaining vesting base.
    use crate::msg::SolvencyResponse;
    let solvency: SolvencyResponse =
        from_json(&query(deps.as_ref(), env, QueryMsg::Solvency { id: 1 }).unwrap()).unwrap();
    assert_eq!(solvency.outstanding.u128(), 1000);
    assert_eq!(solvency.max_outstanding.u128(), 1300);
    assert!(solvency.is_fully_funded);
}

/// Audit regression: with `initial_claim_percent = 0` and no vesting unlocked
/// yet, a `Claim` between `start_time` and `vesting_start_time` would have
/// permanently settled the user with `amount_claimed = 0`, locking them out
/// of their real allocation. The contract now rejects the empty claim and
/// the user can come back once vesting has unlocked.
#[test]
fn test_claim_rejected_when_payout_is_zero_then_succeeds_later() {
    let denom = "native.oro";
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(1_000, denom),
    );
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();
    let start = now;
    let vesting_start = now + 100;
    let vesting_end = vesting_start + 1000;
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: denom.to_string(),
            is_native: true,
            start_time: start,
            vesting_start_time: vesting_start,
            vesting_end_time: vesting_end,
            initial_claim_percent: 0,
            apy_bonus_bps: 0,
            eligibility: vec![("user".to_string(), Uint128::new(1000))],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();

    // (a) Premature claim — past start, before vesting begins, 0% initial.
    // Total payout = 0. Must revert without touching USER_CLAIMS so the user
    // can try again later.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::NothingToClaim {}));

    // No state should have been written: claimed totals stay at zero.
    use crate::msg::SolvencyResponse;
    let solvency: SolvencyResponse =
        from_json(&query(deps.as_ref(), mock_env(), QueryMsg::Solvency { id: 1 }).unwrap()).unwrap();
    assert_eq!(solvency.total_claimed.u128(), 0);
    assert_eq!(solvency.outstanding.u128(), 1000);

    // (b) Same user retries at vesting end — full allocation paid out.
    let mut env_end = mock_env();
    env_end.block.time = cosmwasm_std::Timestamp::from_seconds(vesting_end);
    let res = execute(
        deps.as_mut(),
        env_end,
        mock_info("user", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap();
    let total_attr = res.attributes.iter().find(|a| a.key == "total_payout").unwrap();
    assert_eq!(total_attr.value, "1000");
}

/// Audit regression: `Claim` must reject attached native funds. The claim
/// path only reads `info.sender`; without a `nonpayable` guard, any coins
/// sent by a faulty frontend/wallet would be silently kept by the contract
/// with no record of who deposited them.
#[test]
fn test_claim_rejects_attached_funds() {
    let denom = "native.oro";
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(1_000, denom),
    );
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: denom.to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![("user".to_string(), Uint128::new(500))],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();

    // Sanity: a clean (no-funds) claim succeeds.
    let mut clean_deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(1_000, denom),
    );
    instantiate(
        clean_deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();
    execute(
        clean_deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: denom.to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![("user".to_string(), Uint128::new(500))],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();
    execute(
        clean_deps.as_mut(),
        mock_env(),
        mock_info("user", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap();

    // Wrong-denom attached: PaymentError::ExtraDenom.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user", &cosmwasm_std::coins(5, "uatom")),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap_err();
    assert!(
        matches!(err, ContractError::PaymentError(_)),
        "expected PaymentError on attached funds, got: {err:?}"
    );

    // Right denom but still attached: PaymentError::NonPayable.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user", &cosmwasm_std::coins(5, denom)),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap_err();
    assert!(
        matches!(err, ContractError::PaymentError(_)),
        "expected PaymentError on attached funds, got: {err:?}"
    );

    // After the two rejected attempts, the user must still be eligible: the
    // guard runs before any state mutation, so no USER_CLAIMS entry was saved.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap();
}

/// Audit regression: `query_user_airdrop_status` must mirror `execute_claim`'s
/// `is_active` gate. After `ToggleAirdrop(active=false)`, the query must
/// report `current_claimable = 0` with a distinct `"inactive"` status string
/// — otherwise frontends/bots would advertise a claim that any Claim tx
/// would revert with `AirdropInactive`.
#[test]
fn test_user_airdrop_status_returns_inactive_when_disabled() {
    let mut deps = mock_dependencies();
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();
    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 40,
            apy_bonus_bps: 0,
            eligibility: vec![("user".to_string(), Uint128::new(1000))],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::ToggleAirdrop { id: 1, active: false },
    )
    .unwrap();

    let res: UserAirdropStatusResponse = from_json(
        &query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::UserAirdropStatus { id: 1, address: "user".to_string() },
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(res.vesting_status, "inactive");
    assert_eq!(res.current_claimable, Uint128::zero());

    // Parity check: a Claim now would also revert.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("user", &[]),
        ExecuteMsg::Claim { airdrop_id: 1 },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::AirdropInactive {}));
}

/// FIND-008 regression: `execute_withdraw_funds` must not scan the entire
/// AIRDROPS map. With many active airdrops on UNRELATED tokens, withdrawing
/// a settled token must still succeed without iterating over the unrelated
/// entries. Pin the secondary-index invariant by also asserting the bucket
/// contains exactly the right ids per (token, is_native).
#[test]
fn test_withdraw_scans_only_token_bucket() {
    use crate::state::{AIRDROPS_BY_TOKEN, AIRDROPS};

    let target_denom = "native.target";
    // Pre-fund the contract so the actual BankMsg::Send the withdraw emits
    // would be satisfiable (mock_dependencies_with_balance only affects the
    // balance the contract sees; the BankMsg itself is emitted as a sub-message
    // and not executed in this test environment, but seeding is hygienic).
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(10_000, target_denom),
    );
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();

    // The settled airdrop on the target token: inactive, expiry passed → not blocking.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: target_denom.to_string(),
            is_native: true,
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 100,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: now + 200,
        },
    )
    .unwrap();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::ToggleAirdrop { id: 1, active: false },
    )
    .unwrap();

    // 50 decoy airdrops on OTHER tokens (active, no expiry — would all be
    // "blocking" if the scan touched them, but they should be skipped because
    // they live in different (token, is_native) buckets).
    for i in 0..50u64 {
        let denom = format!("native.decoy{i}");
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("admin", &[]),
            ExecuteMsg::AddAirdrop {
                token_address: denom,
                is_native: true,
                start_time: now,
                vesting_start_time: now,
                vesting_end_time: now + 1000,
                initial_claim_percent: 100,
                apy_bonus_bps: 0,
                eligibility: vec![],
                expiry_time: 9_999_999_999u64,
            },
        )
        .unwrap();
    }

    // AIRDROPS now has 51 entries total; the target-token bucket must hold exactly id=1.
    let total_airdrops = AIRDROPS
        .range(&deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .count();
    assert_eq!(total_airdrops, 51, "all 51 airdrops should be stored in AIRDROPS");

    let target_bucket: Vec<u64> = AIRDROPS_BY_TOKEN
        .prefix(target_denom)
        .range(&deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .map(|kv| kv.unwrap().0)
        .collect();
    assert_eq!(
        target_bucket,
        vec![1u64],
        "AIRDROPS_BY_TOKEN[target_denom] must contain exactly the target airdrop id",
    );

    // Advance past the target airdrop's expiry so it is no longer blocking.
    let mut env_after_expiry = mock_env();
    env_after_expiry.block.time = env_after_expiry.block.time.plus_seconds(300);

    // Withdraw the target token. With the FIND-008 fix the scan only touches
    // id=1 (the settled airdrop); the 50 active decoys on other tokens are
    // skipped at the prefix level, not at the per-entry filter.
    let res = execute(
        deps.as_mut(),
        env_after_expiry,
        mock_info("admin", &[]),
        ExecuteMsg::WithdrawFunds {
            token_address: target_denom.to_string(),
            amount: cosmwasm_std::Uint128::new(1_000),
            is_native: true,
        },
    )
    .unwrap();
    assert_eq!(res.messages.len(), 1, "withdraw must emit exactly one BankMsg");
}

/// FIND-011 regression: the withdraw guard must derive token type from
/// stored `AirdropInfo`, not from the caller-supplied `is_native`. With an
/// active CW20 airdrop on a `token_address`, the admin passing
/// `is_native = true` for the SAME `token_address` must still be blocked
/// — even though the payout branch would otherwise route through
/// `BankMsg::Send` and (harmlessly) fail. The guard now sees the airdrop
/// regardless of the caller's flag.
#[test]
fn test_withdraw_guard_uses_stored_is_native_not_caller_flag() {
    // Use a string that's syntactically both a plausible native denom AND
    // a "contract address" — what matters is the GUARD treats both
    // `is_native` modes as the same key for matching.
    let shared = "uoro";
    let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(
        &cosmwasm_std::coins(1_000, shared),
    );
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();

    let now = mock_env().block.time.seconds();

    // Add an ACTIVE CW20 airdrop on `shared`. The CW20 deposit pool and
    // native bank balance are separate, but the guard should still block
    // any withdraw with the same token_address string.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: shared.to_string(),
            is_native: false, // CW20
            start_time: now,
            vesting_start_time: now,
            vesting_end_time: now + 1000,
            initial_claim_percent: 100,
            apy_bonus_bps: 0,
            eligibility: vec![],
            expiry_time: 9_999_999_999u64,
        },
    )
    .unwrap();

    // Admin tries to withdraw with the WRONG is_native (true instead of false).
    // Pre-FIND-011 behaviour: the guard's (token, is_native=true) filter would
    // skip the CW20 airdrop, allow the call to proceed, and a BankMsg::Send
    // would (harmlessly) fail at sub-message execution.
    // Post-FIND-011: the guard finds the active airdrop on `shared` regardless
    // of caller-supplied is_native and blocks with WithdrawalBlocked.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::WithdrawFunds {
            token_address: shared.to_string(),
            amount: cosmwasm_std::Uint128::new(100),
            is_native: true, // <-- WRONG: the airdrop is CW20
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, ContractError::WithdrawalBlocked { airdrop_id: 1 }),
        "guard must block on stored is_native, got {err:?}"
    );

    // And the correct is_native is also blocked (sanity check — same airdrop
    // is still active).
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::WithdrawFunds {
            token_address: shared.to_string(),
            amount: cosmwasm_std::Uint128::new(100),
            is_native: false,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::WithdrawalBlocked { airdrop_id: 1 }));
}

// ─── UpdateTimes / UpdateExpiry (admin retiming handlers) ─────────────────────

/// Helper: instantiate + add a single 1-user airdrop with predictable
/// timings. Used by the UpdateTimes / UpdateExpiry tests below.
fn setup_retiming_airdrop(
    deps: &mut cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    >,
) -> u64 {
    instantiate(
        deps.as_mut(),
        mock_env(),
        mock_info("creator", &[]),
        InstantiateMsg { admin: "admin".to_string() },
    )
    .unwrap();
    let now = mock_env().block.time.seconds();
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::AddAirdrop {
            token_address: "native.oro".to_string(),
            is_native: true,
            start_time: now + 100,
            vesting_start_time: now + 200,
            vesting_end_time: now + 1000,
            initial_claim_percent: 40,
            apy_bonus_bps: 0,
            eligibility: vec![("user".to_string(), Uint128::new(1000))],
            expiry_time: now + 2000,
        },
    )
    .unwrap();
    now
}

#[test]
fn test_update_times_unauthorized() {
    let mut deps = mock_dependencies();
    let _ = setup_retiming_airdrop(&mut deps);

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("not-admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: Some(mock_env().block.time.seconds() + 200),
            vesting_start_time: None,
            vesting_end_time: None,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
}

#[test]
fn test_update_times_requires_at_least_one_field() {
    let mut deps = mock_dependencies();
    let _ = setup_retiming_airdrop(&mut deps);

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: None,
            vesting_start_time: None,
            vesting_end_time: None,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::NoTimeFieldsProvided {}));
}

#[test]
fn test_update_times_happy_path_pushes_all_three() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: Some(now + 150),
            vesting_start_time: Some(now + 300),
            vesting_end_time: Some(now + 1500),
        },
    )
    .unwrap();

    let res: AirdropResponse =
        from_json(&query(deps.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap()).unwrap();
    assert_eq!(res.start_time, now + 150);
    assert_eq!(res.vesting_start_time, now + 300);
    assert_eq!(res.vesting_end_time, now + 1500);
    // Expiry untouched.
    assert_eq!(res.expiry_time, now + 2000);
}

#[test]
fn test_update_times_partial_only_start_time() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Only push start_time. vesting_start_time stays at now+200 (which is
    // still >= new start_time of now+150 — invariant holds).
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: Some(now + 150),
            vesting_start_time: None,
            vesting_end_time: None,
        },
    )
    .unwrap();

    let res: AirdropResponse =
        from_json(&query(deps.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap()).unwrap();
    assert_eq!(res.start_time, now + 150);
    assert_eq!(res.vesting_start_time, now + 200); // unchanged
    assert_eq!(res.vesting_end_time, now + 1000); // unchanged
}

#[test]
fn test_update_times_rejects_non_forward_start() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Try to MOVE start_time BACKWARD (now + 50, current is now + 100).
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: Some(now + 50),
            vesting_start_time: None,
            vesting_end_time: None,
        },
    )
    .unwrap_err();
    match err {
        ContractError::NonForwardUpdate { field, old, new } => {
            assert_eq!(field, "start_time");
            assert_eq!(old, now + 100);
            assert_eq!(new, now + 50);
        }
        other => panic!("expected NonForwardUpdate, got {other:?}"),
    }
}

#[test]
fn test_update_times_equal_value_also_rejected() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // "Forward-only" is STRICT — equality is rejected too.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: Some(now + 100),
            vesting_start_time: None,
            vesting_end_time: None,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::NonForwardUpdate { .. }));
}

#[test]
fn test_update_times_rejected_after_start() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Jump past start_time and try to retime.
    let mut env = mock_env();
    env.block.time = cosmwasm_std::Timestamp::from_seconds(now + 150);

    let err = execute(
        deps.as_mut(),
        env,
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: Some(now + 200),
            vesting_start_time: None,
            vesting_end_time: None,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::AirdropAlreadyStarted {}));
}

#[test]
fn test_update_times_rejects_broken_vesting_window() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Push start_time PAST vesting_start_time without also updating vesting_start.
    // current vesting_start = now+200; new start = now+300 → invariant broken.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: Some(now + 300),
            vesting_start_time: None,
            vesting_end_time: None,
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::InvalidVestingWindow {}));
}

#[test]
fn test_update_times_rejects_pushing_vesting_end_past_expiry() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Try to push vesting_end_time to now+2000 (== current expiry). Invariant
    // requires expiry > vesting_end strictly, so even == is rejected via
    // InvalidExpiry. Admin must extend expiry first via UpdateExpiry.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: None,
            vesting_start_time: None,
            vesting_end_time: Some(now + 2000),
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::InvalidExpiry {}));
}

#[test]
fn test_update_expiry_unauthorized() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("not-admin", &[]),
        ExecuteMsg::UpdateExpiry { airdrop_id: 1, expiry_time: now + 3000 },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::Unauthorized {}));
}

#[test]
fn test_update_expiry_happy_path() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateExpiry { airdrop_id: 1, expiry_time: now + 3000 },
    )
    .unwrap();

    let res: AirdropResponse =
        from_json(&query(deps.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap()).unwrap();
    assert_eq!(res.expiry_time, now + 3000);
}

#[test]
fn test_update_expiry_forward_only_rejects_shrink() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Current expiry = now + 2000. Try to shorten to now + 1500.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateExpiry { airdrop_id: 1, expiry_time: now + 1500 },
    )
    .unwrap_err();
    match err {
        ContractError::NonForwardUpdate { field, old, new } => {
            assert_eq!(field, "expiry_time");
            assert_eq!(old, now + 2000);
            assert_eq!(new, now + 1500);
        }
        other => panic!("expected NonForwardUpdate, got {other:?}"),
    }
}

#[test]
fn test_update_expiry_rejected_after_expiry_passes() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Jump past current expiry.
    let mut env = mock_env();
    env.block.time = cosmwasm_std::Timestamp::from_seconds(now + 2500);

    let err = execute(
        deps.as_mut(),
        env,
        mock_info("admin", &[]),
        ExecuteMsg::UpdateExpiry { airdrop_id: 1, expiry_time: now + 5000 },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::AirdropExpired {}));
}

#[test]
fn test_update_expiry_allowed_mid_vesting() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Advance to mid-vesting (start has passed, vesting in progress).
    let mut env = mock_env();
    env.block.time = cosmwasm_std::Timestamp::from_seconds(now + 500);

    // UpdateExpiry should still work — extending the reclaim window only
    // gives unclaimed users more time, never less.
    execute(
        deps.as_mut(),
        env.clone(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateExpiry { airdrop_id: 1, expiry_time: now + 3000 },
    )
    .unwrap();

    let res: AirdropResponse =
        from_json(&query(deps.as_ref(), env, QueryMsg::Airdrop { id: 1 }).unwrap()).unwrap();
    assert_eq!(res.expiry_time, now + 3000);
}

#[test]
fn test_update_times_then_update_expiry_combined_flow() {
    let mut deps = mock_dependencies();
    let now = setup_retiming_airdrop(&mut deps);

    // Admin wants to push vesting_end to now+2500 — would violate the
    // expiry invariant (current expiry = now+2000). They must extend
    // expiry first, then push vesting_end.
    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: None,
            vesting_start_time: None,
            vesting_end_time: Some(now + 2500),
        },
    )
    .unwrap_err();
    assert!(matches!(err, ContractError::InvalidExpiry {}));

    // Extend expiry first.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateExpiry { airdrop_id: 1, expiry_time: now + 3000 },
    )
    .unwrap();

    // Now push vesting_end — succeeds.
    execute(
        deps.as_mut(),
        mock_env(),
        mock_info("admin", &[]),
        ExecuteMsg::UpdateTimes {
            airdrop_id: 1,
            start_time: None,
            vesting_start_time: None,
            vesting_end_time: Some(now + 2500),
        },
    )
    .unwrap();

    let res: AirdropResponse =
        from_json(&query(deps.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap()).unwrap();
    assert_eq!(res.expiry_time, now + 3000);
    assert_eq!(res.vesting_end_time, now + 2500);
}
