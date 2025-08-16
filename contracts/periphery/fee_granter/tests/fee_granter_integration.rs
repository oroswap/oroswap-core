use oroswap::fee_granter::{Config, ExecuteMsg, InstantiateMsg, QueryMsg};
use oroswap_fee_granter::contract::{execute, instantiate};
use oroswap_fee_granter::error::ContractError;
use oroswap_fee_granter::query::query;
use oroswap_fee_granter::state::MAX_ADMINS;
use cosmwasm_std::{coins, Addr, Empty};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

fn fee_granter_contract() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new_with_empty(execute, instantiate, query))
}

const GAS_DENOM: &str = "inj";

#[test]
fn test_init() {
    let owner = Addr::unchecked("owner");
    let mut app = App::new(|router, _, store| {
        router
            .bank
            .init_balance(store, &owner, coins(1000000, GAS_DENOM))
            .unwrap();
    });

    let fee_granter_code_id = app.store_code(fee_granter_contract());
    let mut init_msg = InstantiateMsg {
        owner: owner.to_string(),
        admins: vec![
            "admin1".to_string(),
            "admin2".to_string(),
            "admin3".to_string(),
        ],
        gas_denom: GAS_DENOM.to_string(),
    };
    let err = app
        .instantiate_contract(
            fee_granter_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "Test contract",
            None,
        )
        .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        format!("Generic error: Maximum allowed number of admins is {MAX_ADMINS}")
    );
    init_msg.admins = vec!["admin1".to_string(), "admin2".to_string()];
    let fee_granter = app
        .instantiate_contract(
            fee_granter_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "Test contract",
            None,
        )
        .unwrap();

    app.send_tokens(owner.clone(), fee_granter.clone(), &coins(10, GAS_DENOM))
        .unwrap();

    let inj_bal = app
        .wrap()
        .query_balance(&fee_granter, GAS_DENOM)
        .unwrap()
        .amount
        .u128();

    assert_eq!(inj_bal, 10);

    let inj_bal_before = app
        .wrap()
        .query_balance(&owner, GAS_DENOM)
        .unwrap()
        .amount
        .u128();

    app.execute_contract(
        owner.clone(),
        fee_granter.clone(),
        &ExecuteMsg::TransferCoins {
            amount: 5u128.into(),
            receiver: None,
        },
        &[],
    )
    .unwrap();

    let inj_bal_after = app
        .wrap()
        .query_balance(&owner, GAS_DENOM)
        .unwrap()
        .amount
        .u128();

    assert_eq!(inj_bal_after - inj_bal_before, 5);

    let receiver_addr = "receiver".to_string();
    app.execute_contract(
        owner.clone(),
        fee_granter.clone(),
        &ExecuteMsg::TransferCoins {
            amount: 5u128.into(),
            receiver: Some(receiver_addr.clone()),
        },
        &[],
    )
    .unwrap();

    let inj_bal_receiver = app
        .wrap()
        .query_balance(&receiver_addr, GAS_DENOM)
        .unwrap()
        .amount
        .u128();

    assert_eq!(inj_bal_receiver, 5);

    let err = app
        .execute_contract(
            owner,
            fee_granter,
            &ExecuteMsg::TransferCoins {
                amount: 0u8.into(),
                receiver: Some(receiver_addr.clone()),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        format!("Generic error: Can't send 0 amount")
    );
}

#[test]
fn test_update_admins() {
    let owner = Addr::unchecked("owner");
    let admin = Addr::unchecked("admin");
    let mut app = App::new(|router, _, store| {
        router
            .bank
            .init_balance(store, &owner, coins(1000000, GAS_DENOM))
            .unwrap();
    });

    let fee_granter_code_id = app.store_code(fee_granter_contract());
    let err = app
        .instantiate_contract(
            fee_granter_code_id,
            owner.clone(),
            &InstantiateMsg {
                owner: owner.to_string(),
                admins: vec![admin.to_string(), "user1".to_string(), "user2".to_string()],
                gas_denom: GAS_DENOM.to_string(),
            },
            &[],
            "Test contract",
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        format!("Generic error: Maximum allowed number of admins is {MAX_ADMINS}")
    );

    let err = app
        .instantiate_contract(
            fee_granter_code_id,
            owner.clone(),
            &InstantiateMsg {
                owner: owner.to_string(),
                admins: vec![admin.to_string(), admin.to_string()],
                gas_denom: GAS_DENOM.to_string(),
            },
            &[],
            "Test contract",
            None,
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        format!("Generic error: Admin {admin} already exists")
    );

    let fee_granter = app
        .instantiate_contract(
            fee_granter_code_id,
            owner.clone(),
            &InstantiateMsg {
                owner: owner.to_string(),
                admins: vec![admin.to_string()],
                gas_denom: GAS_DENOM.to_string(),
            },
            &[],
            "Test contract",
            None,
        )
        .unwrap();

    app.send_tokens(owner.clone(), admin.clone(), &coins(10, GAS_DENOM))
        .unwrap();
    app.send_tokens(owner.clone(), fee_granter.clone(), &coins(5, GAS_DENOM))
        .unwrap();

    // Admin can only create, revoke grants and transfer coins
    app.execute_contract(
        admin.clone(),
        fee_granter.clone(),
        &ExecuteMsg::TransferCoins {
            amount: 5u128.into(),
            receiver: None,
        },
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            owner.clone(),
            fee_granter.clone(),
            &ExecuteMsg::UpdateAdmins {
                add: vec!["admin2".to_string(), "admin3".to_string()],
                remove: vec![],
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        format!("Generic error: Maximum allowed number of admins is {MAX_ADMINS}")
    );

    // Stargate messages are not implemented in cw-multitest thus we assert that we receive exact cw-multitest error
    let err = app
        .execute_contract(
            admin.clone(),
            fee_granter.clone(),
            &ExecuteMsg::Grant {
                grantee_contract: "test".to_string(),
                amount: 10u128.into(),
                bypass_amount_check: false,
            },
            &coins(10, GAS_DENOM),
        )
        .unwrap_err();
    assert!(
        err.root_cause().to_string().contains("Unexpected exec msg"),
        "{err}"
    );

    // only owner is able to update admins
    let err = app
        .execute_contract(
            admin.clone(),
            fee_granter.clone(),
            &ExecuteMsg::UpdateAdmins {
                add: vec!["admin2".to_string()],
                remove: vec![],
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err.downcast().unwrap());

    let err = app
        .execute_contract(
            owner.clone(),
            fee_granter.clone(),
            &ExecuteMsg::UpdateAdmins {
                add: vec![admin.to_string()],
                remove: vec![],
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        format!("Generic error: Admin {} already exists", admin)
    );

    app.execute_contract(
        owner.clone(),
        fee_granter.clone(),
        &ExecuteMsg::UpdateAdmins {
            add: vec!["admin2".to_string()],
            remove: vec![],
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner,
        fee_granter,
        &ExecuteMsg::UpdateAdmins {
            add: vec![],
            remove: vec!["admin2".to_string(), "random".to_string()], // random is not admin thus it should be ignored
        },
        &[],
    )
    .unwrap();
}

#[test]
fn test_change_ownership() {
    let owner = Addr::unchecked("owner");
    let admin = Addr::unchecked("admin");
    let mut app = App::default();

    let fee_granter_code_id = app.store_code(fee_granter_contract());
    let fee_granter = app
        .instantiate_contract(
            fee_granter_code_id,
            owner.clone(),
            &InstantiateMsg {
                owner: owner.to_string(),
                admins: vec![admin.to_string()],
                gas_denom: GAS_DENOM.to_string(),
            },
            &[],
            "Test contract",
            None,
        )
        .unwrap();

    let new_owner = Addr::unchecked("new_owner".to_string());

    // New owner
    let msg = ExecuteMsg::ProposeNewOwner {
        owner: new_owner.to_string(),
        expires_in: 100, // seconds
    };

    // Unauthorized check
    let err = app
        .execute_contract(Addr::unchecked("not_owner"), fee_granter.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim before proposal
    let err = app
        .execute_contract(
            new_owner.clone(),
            fee_granter.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Ownership proposal not found"
    );

    // Propose new owner
    app.execute_contract(owner, fee_granter.clone(), &msg, &[])
        .unwrap();

    // Claim from invalid addr
    let err = app
        .execute_contract(
            Addr::unchecked("invalid_addr"),
            fee_granter.clone(),
            &ExecuteMsg::ClaimOwnership {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.root_cause().to_string(), "Generic error: Unauthorized");

    // Claim ownership
    app.execute_contract(
        new_owner.clone(),
        fee_granter.clone(),
        &ExecuteMsg::ClaimOwnership {},
        &[],
    )
    .unwrap();

    let res: Config = app
        .wrap()
        .query_wasm_smart(&fee_granter, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(res.owner, new_owner)
}

#[test]
fn test_insufficient_balance_prevention() {
    let owner = Addr::unchecked("owner");
    let grantee = Addr::unchecked("grantee_contract");
    let mut app = App::new(|router, _, store| {
        router
            .bank
            .init_balance(store, &owner, coins(1000000, GAS_DENOM))
            .unwrap();
    });

    let fee_granter_code_id = app.store_code(fee_granter_contract());
    let fee_granter = app
        .instantiate_contract(
            fee_granter_code_id,
            owner.clone(),
            &InstantiateMsg {
                owner: owner.to_string(),
                admins: vec![],
                gas_denom: GAS_DENOM.to_string(),
            },
            &[],
            "Test contract",
            None,
        )
        .unwrap();

    // Give the contract a small balance
    app.send_tokens(owner.clone(), fee_granter.clone(), &coins(100, GAS_DENOM))
        .unwrap();

    // Verify contract has 100 tokens
    let contract_balance = app
        .wrap()
        .query_balance(&fee_granter, GAS_DENOM)
        .unwrap()
        .amount
        .u128();
    assert_eq!(contract_balance, 100);

    // Test 1: Try to grant more than contract has with bypass_amount_check = true
    // This should fail with InsufficientBalance error
    let err = app
        .execute_contract(
            owner.clone(),
            fee_granter.clone(),
            &ExecuteMsg::Grant {
                grantee_contract: grantee.to_string(),
                amount: 1000u128.into(), // Try to grant 1000 when contract only has 100
                bypass_amount_check: true,
            },
            &[],
        )
        .unwrap_err();

    // Should fail with InsufficientBalance error
    let error_msg = err.root_cause().to_string();
    println!("Test 1 error message: {}", error_msg);
    assert!(error_msg.contains("Insufficient balance:"), 
        "Expected 'Insufficient balance:' error, got: {}", error_msg);

    // Test 2: Try to grant exactly what the contract has (should pass balance check)
    // Note: This will fail in mock environment due to Stargate message, but the balance check passes
    let result = app.execute_contract(
        owner.clone(),
        fee_granter.clone(),
        &ExecuteMsg::Grant {
            grantee_contract: Addr::unchecked("grantee3").to_string(),
            amount: 100u128.into(), // Grant exactly what contract has
            bypass_amount_check: true,
        },
        &[],
    );
    
    // The balance check should pass, but the Stargate message will fail in mock environment
    if let Err(err) = result {
        let error_msg = err.root_cause().to_string();
        assert!(!error_msg.contains("Insufficient balance"), 
            "Should not fail with InsufficientBalance when granting exact balance amount");
        println!("Test 2 passed: Balance check passed, failed on StargateMsg as expected");
    }

    println!("✅ Insufficient balance prevention test passed!");
    println!("✅ Grants cannot be created for amounts exceeding contract balance");
    println!("✅ Fix prevents creation of unspendable fee grants");
    println!("✅ Balance check works correctly with bypass_amount_check = true");
}
