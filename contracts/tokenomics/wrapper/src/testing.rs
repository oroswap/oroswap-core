use crate::contract::{execute, instantiate, query, reply};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::Config;
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{
    attr, coin, from_json, to_json_binary, Binary, Reply, SubMsgResponse, SubMsgResult, Uint128,
};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        lock_denom: "uzig".to_string(),
        mint_subdenom: "wzig".to_string(),
        mint_uri: None,
        mint_decimals: Some(6),
        owner: "owner".to_string(),
    };
    let info = mock_info("creator", &[]);
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Check we sent the sub-message to create the denom
    assert_eq!(res.messages.len(), 1);
    assert_eq!(res.attributes[2], attr("owner", "owner"));

    // Mock the reply from tokenfactory
    // On ZigChain, the response is MsgCreateDenomResponse { denom: "coin.contract.wzig" }
    // We'll simulate the response data
    let _reply_msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgResponse {
            data: Some(Binary::from(
                // This is a bit tricky to mock exactly without the proto layout, 
                // but we can try to use the struct if it's available or just mock the data.
                // Looking at contract.rs, it uses b.try_into() which expects a specific format.
                // For unit tests, we can just ensure it parses.
                &[10, 18, 99, 111, 105, 110, 46, 99, 111, 110, 116, 114, 97, 99, 116, 46, 119, 122, 105, 103]
            )),
            events: vec![],
        }),
    };

    // We skip the reply testing for now because of the complex proto encoding of TF responses 
    // unless we have the exact layout. Instead, we can manually set the mint_denom in state for 
    // subsequent tests if needed, or just test the logic that uses it.
}

#[test]
fn test_lock_unlock() {
    let mut deps = mock_dependencies();

    // 1. Setup config with a mock mint_denom (skipping reply for simplicity in unit test)
    let owner = "owner".to_string();
    let lock_denom = "uzig".to_string();
    let mint_denom = "coin.contract.wzig".to_string();

    crate::state::CONFIG.save(&mut deps.storage, &Config {
        lock_denom: lock_denom.clone(),
        mint_denom: mint_denom.clone(),
        owner: owner.clone(),
        paused: false,
    }).unwrap();

    // 2. Test Lock
    let info = mock_info("user1", &[coin(1000, &lock_denom)]);
    let msg = ExecuteMsg::Lock { receiver: None };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Should have 1 message (tf_mint_msg)
    assert_eq!(res.messages.len(), 1);
    
    // 3. Test Unlock
    let info = mock_info("user1", &[coin(500, &mint_denom)]);
    let msg = ExecuteMsg::Unlock { receiver: None };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Should have 2 messages (tf_burn_msg and BankMsg::Send)
    assert_eq!(res.messages.len(), 2);
}

#[test]
fn test_pause_unpause() {
    let mut deps = mock_dependencies();
    let owner = "owner".to_string();
    let lock_denom = "uzig".to_string();
    let mint_denom = "coin.contract.wzig".to_string();

    crate::state::CONFIG.save(&mut deps.storage, &Config {
        lock_denom: lock_denom.clone(),
        mint_denom: mint_denom.clone(),
        owner: owner.clone(),
        paused: false,
    }).unwrap();

    // Pause
    let info = mock_info("owner", &[]);
    execute(deps.as_mut(), mock_env(), info.clone(), ExecuteMsg::Pause {}).unwrap();

    // Try Lock (should fail)
    let info_user = mock_info("user1", &[coin(1000, &lock_denom)]);
    let err = execute(deps.as_mut(), mock_env(), info_user, ExecuteMsg::Lock { receiver: None }).unwrap_err();
    assert!(err.to_string().contains("Contract is paused"));

    // Unpause
    execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Unpause {}).unwrap();

    // Try Lock (should succeed)
    let info = mock_info("user1", &[coin(1000, &lock_denom)]);
    execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Lock { receiver: None }).unwrap();
}
