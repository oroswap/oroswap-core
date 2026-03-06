use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{from_json, Uint128, WasmMsg, to_json_binary};
use cw20::Cw20ExecuteMsg;

use crate::contract::{execute, instantiate, query};
use crate::msg::{
    UserClaimResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    AirdropResponse, ClaimChoice,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        admin: "admin".to_string(),
    };
    let info = mock_info("creator", &[]);

    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let value: ConfigResponse = from_json(&res).unwrap();
    assert_eq!("admin", value.admin);
}

#[test]
fn add_airdrop_and_claim() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        admin: "admin".to_string(),
    };
    let info = mock_info("creator", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let start_time = mock_env().block.time.seconds() + 100;
    let end_time = start_time + 1000;

    let msg = ExecuteMsg::AddAirdrop {
        token_address: "contract_address_long_enough_12345".to_string(),
        start_time,
        end_time,
        vesting_duration: 3600, // 1 hour
        initial_claim_percent: 40,
        apy_bonus_bps: 1000, // 10%
        eligibility: vec![
            ("user1".to_string(), Uint128::new(100)),
            ("user2".to_string(), Uint128::new(200)),
        ],
    };
    let info = mock_info("admin", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Check airdrop info
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Airdrop { id: 1 }).unwrap();
    let value: AirdropResponse = from_json(&res).unwrap();
    assert_eq!("contract_address_long_enough_12345", value.token_address);
    assert_eq!(40, value.initial_claim_percent);

    // Try to claim before start time
    let msg = ExecuteMsg::Claim { airdrop_id: 1, choice: ClaimChoice::InitialPortion };
    let info = mock_info("user1", &[]);
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert!(format!("{}", err).contains("Airdrop has not started yet"));

    // Move time forward
    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(150);

    // Claim successfully (Initial Portion)
    let msg = ExecuteMsg::Claim { airdrop_id: 1, choice: ClaimChoice::InitialPortion };
    let info = mock_info("user1", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    
    // Initial 40% of 100 is 40.
    assert_eq!(1, res.messages.len());
    let expected_msg = WasmMsg::Execute {
        contract_addr: "contract_address_long_enough_12345".to_string(),
        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
            recipient: "user1".to_string(),
            amount: Uint128::new(40),
        }).unwrap(),
        funds: vec![],
    };
    assert_eq!(res.messages[0].msg, expected_msg.into());

    // Check claim status
    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::ClaimStatus {
            id: 1,
            address: "user1".to_string(),
        },
    )
    .unwrap();
    let value: UserClaimResponse = from_json(&res).unwrap();
    assert!(value.claim.is_some());
    let claim = value.claim.unwrap();
    assert_eq!(claim.initial_claimed, Uint128::new(40));
    assert_eq!(claim.vesting_amount, Uint128::new(0));
}

#[test]
fn deposit_and_withdraw_funds() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        admin: "admin".to_string(),
    };
    let info = mock_info("creator", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Deposit funds (CW20)
    let msg = ExecuteMsg::DepositFunds {
        token_address: "contract_address_long_enough_12345".to_string(),
        amount: Uint128::new(1000),
    };
    let info = mock_info("admin", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    
    let expected_msg = WasmMsg::Execute {
        contract_addr: "contract_address_long_enough_12345".to_string(),
        msg: to_json_binary(&Cw20ExecuteMsg::TransferFrom {
            owner: "admin".to_string(),
            recipient: "cosmos2contract".to_string(),
            amount: Uint128::new(1000),
        }).unwrap(),
        funds: vec![],
    };
    assert_eq!(res.messages[0].msg, expected_msg.into());

    // Deposit funds (Native)
    let msg = ExecuteMsg::DepositFunds {
        token_address: "native.oro".to_string(),
        amount: Uint128::new(1000),
    };
    let info = mock_info("admin", &[cosmwasm_std::Coin {
        denom: "native.oro".to_string(),
        amount: Uint128::new(1000),
    }]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // Withdraw funds (CW20)
    let msg = ExecuteMsg::WithdrawFunds {
        token_address: "contract_address_long_enough_12345".to_string(),
        amount: Uint128::new(500),
    };
    let info = mock_info("admin", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    
    let expected_msg = WasmMsg::Execute {
        contract_addr: "contract_address_long_enough_12345".to_string(),
        msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
            recipient: "admin".to_string(),
            amount: Uint128::new(500),
        }).unwrap(),
        funds: vec![],
    };
    assert_eq!(res.messages[0].msg, expected_msg.into());

    // Withdraw funds (Native)
    let msg = ExecuteMsg::WithdrawFunds {
        token_address: "native.oro".to_string(),
        amount: Uint128::new(500),
    };
    let info = mock_info("admin", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    
    let expected_msg = cosmwasm_std::BankMsg::Send {
        to_address: "admin".to_string(),
        amount: vec![cosmwasm_std::Coin {
            denom: "native.oro".to_string(),
            amount: Uint128::new(500),
        }],
    };
    assert_eq!(res.messages[0].msg, expected_msg.into());
}

#[test]
fn test_vesting_and_apy() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        admin: "admin".to_string(),
    };
    let info = mock_info("creator", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let start_time = mock_env().block.time.seconds() + 100;
    let end_time = start_time + 1000;
    let vesting_duration = 3600; // 1 hour

    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        start_time,
        end_time,
        vesting_duration,
        initial_claim_percent: 40,
        apy_bonus_bps: 1000, // 10%
        eligibility: vec![
            ("user1".to_string(), Uint128::new(1000)),
        ],
    };
    let info = mock_info("admin", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(150);

    // Path 2: FullVesting (0 immediately, 1000 vested)
    let msg = ExecuteMsg::Claim { airdrop_id: 1, choice: ClaimChoice::FullVesting };
    let info = mock_info("user1", &[]);
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Move forward 1800 seconds (50% of 1 hour vesting)
    env.block.time = env.block.time.plus_seconds(1800);
    
    // Withdraw Vested
    let msg = ExecuteMsg::WithdrawVested { airdrop_id: 1 };
    let info = mock_info("user1", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    
    // Expected claimable_unlocked: 1000 * 50% = 500
    // Expected APY bonus: Principal (1000) * 10% * (1800 / SECONDS_PER_YEAR)
    let expected_unlocked = Uint128::new(500);
    let seconds_per_year: u128 = 365 * 24 * 3600;
    let expected_apy = Uint128::new(1000).multiply_ratio(1000u128, 10000u128).multiply_ratio(1800u128, seconds_per_year);
    
    let apy_attr = res.attributes.iter().find(|a| a.key == "apy_bonus").unwrap();
    assert_eq!(apy_attr.value, expected_apy.to_string());
    
    let payout_attr = res.attributes.iter().find(|a| a.key == "payout").unwrap();
    assert_eq!(payout_attr.value, (expected_unlocked + expected_apy).to_string());

    // Check updated state
    let res = query(deps.as_ref(), env.clone(), QueryMsg::ClaimStatus { id: 1, address: "user1".to_string() }).unwrap();
    let value: UserClaimResponse = from_json(&res).unwrap();
    let claim = value.claim.unwrap();
    assert_eq!(claim.withdrawn_amount, expected_unlocked);
}

#[test]
fn test_initial_portion_forfeiture() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        admin: "admin".to_string(),
    };
    let info = mock_info("creator", &[]);
    instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let start_time = mock_env().block.time.seconds() + 100;
    let end_time = start_time + 1000;
    let vesting_duration = 3600; // 1 hour

    let msg = ExecuteMsg::AddAirdrop {
        token_address: "native.oro".to_string(),
        start_time,
        end_time,
        vesting_duration,
        initial_claim_percent: 40,
        apy_bonus_bps: 1000, // 10%
        eligibility: vec![
            ("user1".to_string(), Uint128::new(1000)),
        ],
    };
    let info = mock_info("admin", &[]);
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let mut env = mock_env();
    env.block.time = env.block.time.plus_seconds(150);

    // Path 1: InitialPortion (400 immediately, 600 forfeited)
    let msg = ExecuteMsg::Claim { airdrop_id: 1, choice: ClaimChoice::InitialPortion };
    let info = mock_info("user1", &[]);
    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    
    // Check attributes
    let initial_claimed_attr = res.attributes.iter().find(|a| a.key == "initial_claimed").unwrap();
    assert_eq!(initial_claimed_attr.value, "400");
    let vesting_amount_attr = res.attributes.iter().find(|a| a.key == "vesting_amount").unwrap();
    assert_eq!(vesting_amount_attr.value, "0");

    // Try to withdraw (should fail or return zero payout)
    env.block.time = env.block.time.plus_seconds(1800);
    let msg = ExecuteMsg::WithdrawVested { airdrop_id: 1 };
    let info = mock_info("user1", &[]);
    let err = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
    assert!(format!("{}", err).contains("Nothing to withdraw at this time"));
}
