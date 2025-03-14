
DROP TABLE lnd_payments;
DROP INDEX claim_inputs_claim_tx_id_idx;
DROP TABLE claim_inputs;
DROP INDEX claims_swap_hash_creation_time;
DROP TABLE claims;
DROP TABLE filter_addresses;
DROP INDEX tx_inputs_tx_id_output_index_idx;
DROP TABLE tx_inputs;
DROP INDEX tx_blocks_block_hash_idx;
DROP INDEX tx_blocks_tx_id_output_index_idx;
DROP TABLE tx_blocks;
DROP INDEX tx_outputs_address_idx;
DROP TABLE tx_outputs;
DROP TABLE watch_addresses;
DROP INDEX blocks_height_idx;
DROP TABLE blocks;
DROP INDEX payment_attempt_tx_outputs_tx_id_output_index_idx;
DROP INDEX payment_attempt_tx_outputs_payment_attempt_id_idx;
DROP TABLE payment_attempt_tx_outputs;
DROP INDEX payment_attempts_destination_idx;
DROP INDEX payment_attempts_payment_request_idx;
DROP INDEX payment_attempts_label_idx;
DROP INDEX payment_attempts_swap_payment_hash_idx;
DROP TABLE payment_attempts;
DROP INDEX swaps_address_idx;
DROP TABLE swaps;
