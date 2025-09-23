# Oroswap xORO Staking

This staking contract allows ORO holders to stake their tokens in exchange for xORO. The amount of ORO they can claim later increases as accrued fees in the Maker contract get swapped to ORO which is then sent to stakers.

---

## InstantiateMsg

Initializes the contract with the token code ID used by ORO and the ORO token address.

```json
{
  "token_code_id": 123,
  "deposit_token_addr": "zig..."
}
```

## ExecuteMsg

### `receive`

CW20 receive msg.

```json
{
  "receive": {
    "sender": "zig...",
    "amount": "123",
    "msg": "<base64_encoded_json_string>"
  }
}
```

#### `Enter`

Deposits ORO in the xORO staking contract.

Execute this message by calling the ORO token contract and use a message like this:
```json
{
  "send": {
    "contract": "oroswap_staking",
    "amount": "1000000",
    "msg": "eyJlbnRlciI6eyJyZWNlaXZlciI6InVzZXIxIn19"
  }
}
```

#### `leave`

Burns xORO and unstakes underlying ORO (initial staked amount + accrued ORO since staking).

Execute this message by calling the xORO token contract and use a message like this:
```json
{
  "send": {
    "contract": "oroswap_staking",
    "amount": "1000000",
    "msg": "eyJsZWF2ZSI6eyJyZWNlaXZlciI6InVzZXIxIn19"
  }
}
```

## QueryMsg

All query messages are described below. A custom struct is defined for each query response.

### `config`

Returns the ORO and xORO addresses.

```json
{
  "config": {}
}
```

### `get_total_shares`

Returns the total amount of xORO tokens.

```json
{
  "get_total_shares": {}
}
```

### `get_total_deposit`

Returns the total amount of ORO deposits in the staking contract.

```json
{
  "get_total_deposit": {}
}
```
