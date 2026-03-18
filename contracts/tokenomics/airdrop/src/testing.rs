use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_json, Uint128, BankMsg, CosmosMsg};

use crate::contract::{execute, instantiate, query};
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
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100, // 100% immediate for simplicity
        apy_bonus_bps: 0,
        eligibility: vec![("user".to_string(), Uint128::new(500))],
    };
    execute(deps.as_mut(), mock_env(), mock_info("admin", &[]), msg).unwrap();

    // Season 2
    let msg = ExecuteMsg::AddAirdrop {
        token_address: "oro2".to_string(),
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 50,
        apy_bonus_bps: 0,
        eligibility: vec![("user".to_string(), Uint128::new(1000))],
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
        start_time: now,
        vesting_start_time: now,
        vesting_end_time: now + 1000,
        initial_claim_percent: 100,
        apy_bonus_bps: 0,
        eligibility: vec![("user".to_string(), Uint128::new(1000))],
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
        start_time,
        vesting_start_time,
        vesting_end_time,
        initial_claim_percent: 40,
        apy_bonus_bps: 1000, // 10%
        eligibility: vec![
            ("user1".to_string(), Uint128::new(1000)),
        ],
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
