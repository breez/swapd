- [ ] address the single-utxo model in add_fund_status
- [ ] add a minimum percentage profit / maximum percentage loss for claiming 
      utxos and sending the payment in get_swap_payment
- [ ] resync the chain in the background periodically
- [ ] add cli commands
  - in-progress-swaps
  - in-progress-redeems
  - get-info
  - list-swaps --address
  - list-swaps --txid:outnum
  - list-swaps --invoice
  - list-swaps --payment-hash
  - list-swaps --destination
- [ ] let chainservice monitor redeem transactions
- [ ] ensure chain is syncing before accepting payments
- [ ] ensure redeem is working before accepting payments
- [ ] make redeem logic runnable in separate binary
- [ ] make redeem service also include info about which utxos were included in
      the payment.