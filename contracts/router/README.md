# Oroswap Router

The Router contract contains logic to facilitate multi-hop swaps for ZIGChain native & Oroswap tokens.

---

### Operations Assertion

For every swap, the contract checks if the resulting token is the one that was asked for and whether the receiving amount exceeds the minimum to receive.

## InstantiateMsg

Initializes the contract with the Oroswap factory contract address.

```json
{
  "oroswap_factory": "zig..."
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

### `execute_swap_operation`

Swaps one token to another. _single_ defines whether this swap is single or part of a multi hop route.
This message is for internal use.

### Example

Swap UZIG => CWBNB

```json
{
   "execute_swap_operation": {
     "operation": {
        "oro_swap": {
          "offer_asset_info": {
            "native_token": {
              "denom": "uzig"
            }
          },
          "ask_asset_info": {
            "token": {
              "contract_addr": "zig..."
            }
          }
        }
      },
     "to": "zig...",
     "max_spread": "0.05",
     "single": false
   }
}
```

### `execute_swap_operations`

Performs multi-hop swap operations for native & Oroswap tokens. Swaps execute one-by-one and the last swap will return the ask token. This function is public (can be called by anyone).
Contract sets total 'return_amount' in response data after all routes are processed. See `SwapResponseData` type for more info.
Note: Response data makes sense ONLY if the first token in multi-hop swap is native. Otherwise, cw20::send message resets response data.

### Example

Swap UZIG => CWBNB

```json
{
  "execute_swap_operations": {
    "operations": [
      {
        "native_swap":{
          "offer_denom":"uzig",
          "ask_denom":"uzig"
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": {
            "native_token": {
              "denom": "uzig"
            }
          },
          "ask_asset_info": {
            "token": {
              "contract_addr": "zig..."
            }
          }
        }
      }
    ],
    "minimum_receive": "123",
    "to": "zig...",
    "max_spread": "0.05"
  }
}
```

### `assert_minimum_receive`

Checks that an amount of ask tokens exceeds `minimum_receive`. This message is for internal use.

```json
{
  "assert_minimum_receive": {
    "asset_info": {
      "token": {
        "contract_addr": "zig..."
      }
    },
    "prev_balance": "123",
    "minimum_receive": "123",
    "receiver": "zig..."
  }
}
```

## QueryMsg

All query messages are described below. A custom struct is defined for each query response.

### `config`

Returns the general configuration for the router contract.

```json
{
  "config": {}
}
```

### `simulate_swap_operations`

Simulates multi-hop swap operations. Examples:

- UZIG => CWBNB

```json
{
  "simulate_swap_operations" : {
    "offer_amount": "123",
    "operations": [
      {
        "native_swap": {
          "offer_denom": "uzig",
          "ask_denom": "uzig"
        }
      },
      {
        "oro_swap": {
          "offer_asset_info": {
            "native_token": {
              "denom": "uzig"
            }
          },
          "ask_asset_info": {
            "token": {
              "contract_addr": "zig..."
            }
          }
        }
      }
    ]
  }
}
```

- CWBNB => UZIG

```json
{
  "simulate_swap_operations" : {
    "offer_amount": "123",
    "operations": [
    {
      "native_swap": {
        "offer_denom": "uzig",
        "ask_denom": "uzig"
      }
    },
    {
      "oro_swap": {
        "offer_asset_info": {
          "token": {
            "contract_addr": "zig..."
          }
        },
        "ask_asset_info": {
          "native_token": {
            "denom": "uzig"
          }
        }
      }
    }
  ]
  }
}
```
