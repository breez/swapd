from helpers import *
import grpc
import musig2
import os
from bitcoinutils.constants import TYPE_RELATIVE_TIMELOCK
from bitcoinutils.transactions import Locktime, Sequence
from bitcoinutils.ripemd160 import ripemd160
from bitcoinutils.script import Script, OP_CODES
from bitcoinutils.utils import ControlBlock, get_tag_hashed_merkle_root, tagged_hash
from bitcoinutils.setup import setup
from bitcoinutils.keys import P2trAddress, PublicKey
from bitcoinutils.transactions import Transaction, TxInput, TxOutput, TxWitnessInput
from decimal import Decimal
import struct


def unilateral_refund(
    user,
    swapper,
    address,
    h,
    refund_privkey,
    claim_pubkey,
    lock_time,
    to_spend_txid,
    refund_amount,
    height,
):
    to_spend_tx = user.bitcoin.rpc.getrawtransaction(to_spend_txid, True)
    to_spend_output_index = 0
    if to_spend_tx["vout"][1]["value"] == Decimal("0.00100000"):
        to_spend_output_index = 1

    refund_address = P2trAddress(user.new_address())
    extra_in = os.urandom(32)

    tx_lock_time = struct.pack("<q", height).hex()[2:].zfill(8)
    sequence = Sequence(TYPE_RELATIVE_TIMELOCK, lock_time).for_input_sequence()
    print("AAAAA", sequence, tx_lock_time)
    tx_in = TxInput(to_spend_txid, to_spend_output_index, sequence=sequence)
    tx_out = TxOutput(refund_amount, refund_address.to_script_pub_key())
    tx = Transaction([tx_in], [tx_out], has_segwit=True)

    scripts = [
        Script(
            [
                "OP_HASH160",
                ripemd160(bytes.fromhex(h)).hex(),
                "OP_EQUALVERIFY",
                PublicKey(claim_pubkey.hex()).to_x_only_hex(),
                "OP_CHECKSIG",
            ]
        ),
        Script(
            [
                refund_privkey.get_public_key().to_x_only_hex(),
                "OP_CHECKSIGVERIFY",
                Sequence(TYPE_RELATIVE_TIMELOCK, lock_time).for_script(),
                "OP_CHECKSEQUENCEVERIFY",
            ]
        ),
    ]
    sig = refund_privkey.sign_taproot_input(
        tx,
        0,
        [P2trAddress(address).to_script_pub_key()],
        [100_000],
        script_path=True,
        tapleaf_script=scripts[1],
        tweak=False,
    )

    refund_pubkey_bytes = bytes.fromhex(refund_privkey.get_public_key().to_hex())
    pubkeys = musig2.key_sort([claim_pubkey, refund_pubkey_bytes])
    agg_ctx = musig2.key_agg(pubkeys)
    aggpk = musig2.get_pk(agg_ctx)

    control_block = ControlBlock(PublicKey(aggpk.hex()), scripts, 1, is_odd=True)
    tx.witnesses.append(
        TxWitnessInput([sig, scripts[1].to_hex(), control_block.to_hex()])
    )

    tx_odd = tx.to_hex()

    control_block = ControlBlock(PublicKey(aggpk.hex()), scripts, 1, is_odd=False)
    tx.witnesses.clear()
    tx.witnesses.append(
        TxWitnessInput([sig, scripts[1].to_hex(), control_block.to_hex()])
    )

    tx_even = tx.to_hex()

    # TODO(JssDWt): I could not figure out which value to use for is_odd. Decided to use both.
    return tx_odd, tx_even


def test_unilateral_refund_success(node_factory, swapd_factory):
    setup("regtest")
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    address, _, h, refund_privkey, claim_pubkey, lock_time = (
        create_swap_no_invoice_extended(user, swapper)
    )
    to_spend_txid = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    height = user.bitcoin.rpc.getblockcount()
    blocks_to_add = lock_time - 1
    new_height = height + blocks_to_add
    user.bitcoin.generate_block(blocks_to_add)
    user.bitcoin.wait_for_log(r"UpdateTip: new best=.* height={}".format(new_height))
    tx_odd, tx_even = unilateral_refund(
        user,
        swapper,
        address,
        h,
        refund_privkey,
        claim_pubkey,
        lock_time,
        to_spend_txid,
        99_000,
        new_height,
    )

    expected_utxos = len(user.list_utxos()) + 1
    try:
        user.bitcoin.rpc.sendrawtransaction(tx_odd)
    except Exception as e:
        if "Witness program hash mismatch" in str(e):
            user.bitcoin.rpc.sendrawtransaction(tx_even)
        else:
            raise
    user.bitcoin.generate_block(1)
    wait_for(lambda: len(user.list_utxos()) == expected_utxos)


def test_unilateral_refund_too_soon(node_factory, swapd_factory):
    setup("regtest")
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    address, _, h, refund_privkey, claim_pubkey, lock_time = (
        create_swap_no_invoice_extended(user, swapper)
    )
    to_spend_txid = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    height = user.bitcoin.rpc.getblockcount()
    blocks_to_add = lock_time - 2
    new_height = height + blocks_to_add
    user.bitcoin.generate_block(blocks_to_add)
    user.bitcoin.wait_for_log(r"UpdateTip: new best=.* height={}".format(new_height))
    tx_odd, tx_even = unilateral_refund(
        user,
        swapper,
        address,
        h,
        refund_privkey,
        claim_pubkey,
        lock_time,
        to_spend_txid,
        99_000,
        new_height,
    )

    expected_utxos = len(user.list_utxos()) + 1
    try:
        user.bitcoin.rpc.sendrawtransaction(tx_odd)
        assert False
    except Exception as e:
        if "Witness program hash mismatch" in str(e):
            try:
                user.bitcoin.rpc.sendrawtransaction(tx_even)
                assert False
            except Exception as e:
                if "non-BIP68-final" in str(e):
                    pass
                else:
                    raise
        elif "non-BIP68-final" in str(e):
            pass
        else:
            raise
