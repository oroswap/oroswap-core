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
        expiry_time: None,
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
        expiry_time: None,
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
        expiry_time: None,
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
        expiry_time: None,
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
        expiry_time: None,
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
        is_native: true, // Crucial: C-1 fix in action
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![("user1".to_string(), Uint128::new(1000))],
        expiry_time: None,
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
        expiry_time: None,
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

    // Deposit Native with wrong funds (M-5 fix)
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
        expiry_time: None,
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
        // user2 gets 0 (L-4 test)
        eligibility: vec![("user2".to_string(), Uint128::zero())],
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Move to start time
    let mut env_start = mock_env();
    env_start.block.time = env_start.block.time.plus_seconds(100);

    // Normal claim
    execute(deps.as_mut(), env_start.clone(), mock_info("user1", &[]), ExecuteMsg::Claim { airdrop_id: 1 }).unwrap();

    // Zero eligible claim (L-4 fix) should fail with NotEligible
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

    // Edge case: initial_claim_percent = 0, and large apy_bonus_bps (H-1 fix test)
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        is_native: true,
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 0, // 0% initial
        apy_bonus_bps: 1_000_000_000, // Huuuuge bonus, would overflow u128 if not using multiply_ratio!
        eligibility: vec![("user1".to_string(), Uint128::MAX)], // Max allocation to force overflow bounds
        expiry_time: None,
    };
    // Should pass AddAirdrop without panicking
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Claim at start time (should get 0 because 0% initial and vesting hasn't elapsed)
    let msg = ExecuteMsg::Claim { airdrop_id: 1 };
    let res = execute(deps.as_mut(), mock_env(), mock_info("user1", &[]), msg).unwrap();
    // Claiming 0 doesn't send messages
    assert_eq!(res.messages.len(), 0);
    let total_attr = res.attributes.iter().find(|a| a.key == "total_payout").unwrap();
    assert_eq!(total_attr.value, "0");
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
        expiry_time: Some(expiry_time),
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
        expiry_time: Some(expiry_time),
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
