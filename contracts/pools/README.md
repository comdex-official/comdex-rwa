
## Instantiate

```sh
MSG='{"admins":[""],"token_issuer":"","usdc_denom":"ucmdx","code_id":27}'
```

## Update KYC

```sh
MSG='{"update_kyc":{"user":"","kyc_status":false}}'
```

## Create Pool

- principal_grace_period is 90 days,
- drawdown_period is 14 days,
- term length is 365 days
All in seconds of course.

```sh
MSG='{"new_pool":{"msg":{"borrower":"<>","uid_token":"<>","interest_apr":500,"borrow_limit":"100000000000","interest_payment_frequency":"monthly","principal_payment_frequency":"quaterly","principal_grace_period":7776000,"drawdown_period":1209600,"term_length":31536000}}}'
```

## Deposit

```sh
MSG='{"deposit":{"msg":{"amount":"","pool_id":null}}}'
```

## Drawdown

```sh
MSG='{"drawdown":{"msg":{"pool_id":null,"amount":""}}}'
```

## Repay

```sh
MSG='{"repay":{"msg":{"pool_id":null,"amount":""}}}'
```

## Get Pool Info

```sh
MSG='{"get_pool_info":{"id":1}}'
```
