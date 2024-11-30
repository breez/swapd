import lightning_pb2 as _lightning_pb2
import signer_pb2 as _signer_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import (
    ClassVar as _ClassVar,
    Iterable as _Iterable,
    Mapping as _Mapping,
    Optional as _Optional,
    Union as _Union,
)

DESCRIPTOR: _descriptor.FileDescriptor

class AddressType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    UNKNOWN: _ClassVar[AddressType]
    WITNESS_PUBKEY_HASH: _ClassVar[AddressType]
    NESTED_WITNESS_PUBKEY_HASH: _ClassVar[AddressType]
    HYBRID_NESTED_WITNESS_PUBKEY_HASH: _ClassVar[AddressType]
    TAPROOT_PUBKEY: _ClassVar[AddressType]

class WitnessType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    UNKNOWN_WITNESS: _ClassVar[WitnessType]
    COMMITMENT_TIME_LOCK: _ClassVar[WitnessType]
    COMMITMENT_NO_DELAY: _ClassVar[WitnessType]
    COMMITMENT_REVOKE: _ClassVar[WitnessType]
    HTLC_OFFERED_REVOKE: _ClassVar[WitnessType]
    HTLC_ACCEPTED_REVOKE: _ClassVar[WitnessType]
    HTLC_OFFERED_TIMEOUT_SECOND_LEVEL: _ClassVar[WitnessType]
    HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL: _ClassVar[WitnessType]
    HTLC_OFFERED_REMOTE_TIMEOUT: _ClassVar[WitnessType]
    HTLC_ACCEPTED_REMOTE_SUCCESS: _ClassVar[WitnessType]
    HTLC_SECOND_LEVEL_REVOKE: _ClassVar[WitnessType]
    WITNESS_KEY_HASH: _ClassVar[WitnessType]
    NESTED_WITNESS_KEY_HASH: _ClassVar[WitnessType]
    COMMITMENT_ANCHOR: _ClassVar[WitnessType]
    COMMITMENT_NO_DELAY_TWEAKLESS: _ClassVar[WitnessType]
    COMMITMENT_TO_REMOTE_CONFIRMED: _ClassVar[WitnessType]
    HTLC_OFFERED_TIMEOUT_SECOND_LEVEL_INPUT_CONFIRMED: _ClassVar[WitnessType]
    HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL_INPUT_CONFIRMED: _ClassVar[WitnessType]
    LEASE_COMMITMENT_TIME_LOCK: _ClassVar[WitnessType]
    LEASE_COMMITMENT_TO_REMOTE_CONFIRMED: _ClassVar[WitnessType]
    LEASE_HTLC_OFFERED_TIMEOUT_SECOND_LEVEL: _ClassVar[WitnessType]
    LEASE_HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL: _ClassVar[WitnessType]
    TAPROOT_PUB_KEY_SPEND: _ClassVar[WitnessType]
    TAPROOT_LOCAL_COMMIT_SPEND: _ClassVar[WitnessType]
    TAPROOT_REMOTE_COMMIT_SPEND: _ClassVar[WitnessType]
    TAPROOT_ANCHOR_SWEEP_SPEND: _ClassVar[WitnessType]
    TAPROOT_HTLC_OFFERED_TIMEOUT_SECOND_LEVEL: _ClassVar[WitnessType]
    TAPROOT_HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL: _ClassVar[WitnessType]
    TAPROOT_HTLC_SECOND_LEVEL_REVOKE: _ClassVar[WitnessType]
    TAPROOT_HTLC_ACCEPTED_REVOKE: _ClassVar[WitnessType]
    TAPROOT_HTLC_OFFERED_REVOKE: _ClassVar[WitnessType]
    TAPROOT_HTLC_OFFERED_REMOTE_TIMEOUT: _ClassVar[WitnessType]
    TAPROOT_HTLC_LOCAL_OFFERED_TIMEOUT: _ClassVar[WitnessType]
    TAPROOT_HTLC_ACCEPTED_REMOTE_SUCCESS: _ClassVar[WitnessType]
    TAPROOT_HTLC_ACCEPTED_LOCAL_SUCCESS: _ClassVar[WitnessType]
    TAPROOT_COMMITMENT_REVOKE: _ClassVar[WitnessType]

class ChangeAddressType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CHANGE_ADDRESS_TYPE_UNSPECIFIED: _ClassVar[ChangeAddressType]
    CHANGE_ADDRESS_TYPE_P2TR: _ClassVar[ChangeAddressType]

UNKNOWN: AddressType
WITNESS_PUBKEY_HASH: AddressType
NESTED_WITNESS_PUBKEY_HASH: AddressType
HYBRID_NESTED_WITNESS_PUBKEY_HASH: AddressType
TAPROOT_PUBKEY: AddressType
UNKNOWN_WITNESS: WitnessType
COMMITMENT_TIME_LOCK: WitnessType
COMMITMENT_NO_DELAY: WitnessType
COMMITMENT_REVOKE: WitnessType
HTLC_OFFERED_REVOKE: WitnessType
HTLC_ACCEPTED_REVOKE: WitnessType
HTLC_OFFERED_TIMEOUT_SECOND_LEVEL: WitnessType
HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL: WitnessType
HTLC_OFFERED_REMOTE_TIMEOUT: WitnessType
HTLC_ACCEPTED_REMOTE_SUCCESS: WitnessType
HTLC_SECOND_LEVEL_REVOKE: WitnessType
WITNESS_KEY_HASH: WitnessType
NESTED_WITNESS_KEY_HASH: WitnessType
COMMITMENT_ANCHOR: WitnessType
COMMITMENT_NO_DELAY_TWEAKLESS: WitnessType
COMMITMENT_TO_REMOTE_CONFIRMED: WitnessType
HTLC_OFFERED_TIMEOUT_SECOND_LEVEL_INPUT_CONFIRMED: WitnessType
HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL_INPUT_CONFIRMED: WitnessType
LEASE_COMMITMENT_TIME_LOCK: WitnessType
LEASE_COMMITMENT_TO_REMOTE_CONFIRMED: WitnessType
LEASE_HTLC_OFFERED_TIMEOUT_SECOND_LEVEL: WitnessType
LEASE_HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL: WitnessType
TAPROOT_PUB_KEY_SPEND: WitnessType
TAPROOT_LOCAL_COMMIT_SPEND: WitnessType
TAPROOT_REMOTE_COMMIT_SPEND: WitnessType
TAPROOT_ANCHOR_SWEEP_SPEND: WitnessType
TAPROOT_HTLC_OFFERED_TIMEOUT_SECOND_LEVEL: WitnessType
TAPROOT_HTLC_ACCEPTED_SUCCESS_SECOND_LEVEL: WitnessType
TAPROOT_HTLC_SECOND_LEVEL_REVOKE: WitnessType
TAPROOT_HTLC_ACCEPTED_REVOKE: WitnessType
TAPROOT_HTLC_OFFERED_REVOKE: WitnessType
TAPROOT_HTLC_OFFERED_REMOTE_TIMEOUT: WitnessType
TAPROOT_HTLC_LOCAL_OFFERED_TIMEOUT: WitnessType
TAPROOT_HTLC_ACCEPTED_REMOTE_SUCCESS: WitnessType
TAPROOT_HTLC_ACCEPTED_LOCAL_SUCCESS: WitnessType
TAPROOT_COMMITMENT_REVOKE: WitnessType
CHANGE_ADDRESS_TYPE_UNSPECIFIED: ChangeAddressType
CHANGE_ADDRESS_TYPE_P2TR: ChangeAddressType

class ListUnspentRequest(_message.Message):
    __slots__ = ("min_confs", "max_confs", "account", "unconfirmed_only")
    MIN_CONFS_FIELD_NUMBER: _ClassVar[int]
    MAX_CONFS_FIELD_NUMBER: _ClassVar[int]
    ACCOUNT_FIELD_NUMBER: _ClassVar[int]
    UNCONFIRMED_ONLY_FIELD_NUMBER: _ClassVar[int]
    min_confs: int
    max_confs: int
    account: str
    unconfirmed_only: bool
    def __init__(
        self,
        min_confs: _Optional[int] = ...,
        max_confs: _Optional[int] = ...,
        account: _Optional[str] = ...,
        unconfirmed_only: bool = ...,
    ) -> None: ...

class ListUnspentResponse(_message.Message):
    __slots__ = ("utxos",)
    UTXOS_FIELD_NUMBER: _ClassVar[int]
    utxos: _containers.RepeatedCompositeFieldContainer[_lightning_pb2.Utxo]
    def __init__(
        self, utxos: _Optional[_Iterable[_Union[_lightning_pb2.Utxo, _Mapping]]] = ...
    ) -> None: ...

class LeaseOutputRequest(_message.Message):
    __slots__ = ("id", "outpoint", "expiration_seconds")
    ID_FIELD_NUMBER: _ClassVar[int]
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    EXPIRATION_SECONDS_FIELD_NUMBER: _ClassVar[int]
    id: bytes
    outpoint: _lightning_pb2.OutPoint
    expiration_seconds: int
    def __init__(
        self,
        id: _Optional[bytes] = ...,
        outpoint: _Optional[_Union[_lightning_pb2.OutPoint, _Mapping]] = ...,
        expiration_seconds: _Optional[int] = ...,
    ) -> None: ...

class LeaseOutputResponse(_message.Message):
    __slots__ = ("expiration",)
    EXPIRATION_FIELD_NUMBER: _ClassVar[int]
    expiration: int
    def __init__(self, expiration: _Optional[int] = ...) -> None: ...

class ReleaseOutputRequest(_message.Message):
    __slots__ = ("id", "outpoint")
    ID_FIELD_NUMBER: _ClassVar[int]
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    id: bytes
    outpoint: _lightning_pb2.OutPoint
    def __init__(
        self,
        id: _Optional[bytes] = ...,
        outpoint: _Optional[_Union[_lightning_pb2.OutPoint, _Mapping]] = ...,
    ) -> None: ...

class ReleaseOutputResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class KeyReq(_message.Message):
    __slots__ = ("key_finger_print", "key_family")
    KEY_FINGER_PRINT_FIELD_NUMBER: _ClassVar[int]
    KEY_FAMILY_FIELD_NUMBER: _ClassVar[int]
    key_finger_print: int
    key_family: int
    def __init__(
        self, key_finger_print: _Optional[int] = ..., key_family: _Optional[int] = ...
    ) -> None: ...

class AddrRequest(_message.Message):
    __slots__ = ("account", "type", "change")
    ACCOUNT_FIELD_NUMBER: _ClassVar[int]
    TYPE_FIELD_NUMBER: _ClassVar[int]
    CHANGE_FIELD_NUMBER: _ClassVar[int]
    account: str
    type: AddressType
    change: bool
    def __init__(
        self,
        account: _Optional[str] = ...,
        type: _Optional[_Union[AddressType, str]] = ...,
        change: bool = ...,
    ) -> None: ...

class AddrResponse(_message.Message):
    __slots__ = ("addr",)
    ADDR_FIELD_NUMBER: _ClassVar[int]
    addr: str
    def __init__(self, addr: _Optional[str] = ...) -> None: ...

class Account(_message.Message):
    __slots__ = (
        "name",
        "address_type",
        "extended_public_key",
        "master_key_fingerprint",
        "derivation_path",
        "external_key_count",
        "internal_key_count",
        "watch_only",
    )
    NAME_FIELD_NUMBER: _ClassVar[int]
    ADDRESS_TYPE_FIELD_NUMBER: _ClassVar[int]
    EXTENDED_PUBLIC_KEY_FIELD_NUMBER: _ClassVar[int]
    MASTER_KEY_FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    DERIVATION_PATH_FIELD_NUMBER: _ClassVar[int]
    EXTERNAL_KEY_COUNT_FIELD_NUMBER: _ClassVar[int]
    INTERNAL_KEY_COUNT_FIELD_NUMBER: _ClassVar[int]
    WATCH_ONLY_FIELD_NUMBER: _ClassVar[int]
    name: str
    address_type: AddressType
    extended_public_key: str
    master_key_fingerprint: bytes
    derivation_path: str
    external_key_count: int
    internal_key_count: int
    watch_only: bool
    def __init__(
        self,
        name: _Optional[str] = ...,
        address_type: _Optional[_Union[AddressType, str]] = ...,
        extended_public_key: _Optional[str] = ...,
        master_key_fingerprint: _Optional[bytes] = ...,
        derivation_path: _Optional[str] = ...,
        external_key_count: _Optional[int] = ...,
        internal_key_count: _Optional[int] = ...,
        watch_only: bool = ...,
    ) -> None: ...

class AddressProperty(_message.Message):
    __slots__ = ("address", "is_internal", "balance", "derivation_path", "public_key")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    IS_INTERNAL_FIELD_NUMBER: _ClassVar[int]
    BALANCE_FIELD_NUMBER: _ClassVar[int]
    DERIVATION_PATH_FIELD_NUMBER: _ClassVar[int]
    PUBLIC_KEY_FIELD_NUMBER: _ClassVar[int]
    address: str
    is_internal: bool
    balance: int
    derivation_path: str
    public_key: bytes
    def __init__(
        self,
        address: _Optional[str] = ...,
        is_internal: bool = ...,
        balance: _Optional[int] = ...,
        derivation_path: _Optional[str] = ...,
        public_key: _Optional[bytes] = ...,
    ) -> None: ...

class AccountWithAddresses(_message.Message):
    __slots__ = ("name", "address_type", "derivation_path", "addresses")
    NAME_FIELD_NUMBER: _ClassVar[int]
    ADDRESS_TYPE_FIELD_NUMBER: _ClassVar[int]
    DERIVATION_PATH_FIELD_NUMBER: _ClassVar[int]
    ADDRESSES_FIELD_NUMBER: _ClassVar[int]
    name: str
    address_type: AddressType
    derivation_path: str
    addresses: _containers.RepeatedCompositeFieldContainer[AddressProperty]
    def __init__(
        self,
        name: _Optional[str] = ...,
        address_type: _Optional[_Union[AddressType, str]] = ...,
        derivation_path: _Optional[str] = ...,
        addresses: _Optional[_Iterable[_Union[AddressProperty, _Mapping]]] = ...,
    ) -> None: ...

class ListAccountsRequest(_message.Message):
    __slots__ = ("name", "address_type")
    NAME_FIELD_NUMBER: _ClassVar[int]
    ADDRESS_TYPE_FIELD_NUMBER: _ClassVar[int]
    name: str
    address_type: AddressType
    def __init__(
        self,
        name: _Optional[str] = ...,
        address_type: _Optional[_Union[AddressType, str]] = ...,
    ) -> None: ...

class ListAccountsResponse(_message.Message):
    __slots__ = ("accounts",)
    ACCOUNTS_FIELD_NUMBER: _ClassVar[int]
    accounts: _containers.RepeatedCompositeFieldContainer[Account]
    def __init__(
        self, accounts: _Optional[_Iterable[_Union[Account, _Mapping]]] = ...
    ) -> None: ...

class RequiredReserveRequest(_message.Message):
    __slots__ = ("additional_public_channels",)
    ADDITIONAL_PUBLIC_CHANNELS_FIELD_NUMBER: _ClassVar[int]
    additional_public_channels: int
    def __init__(self, additional_public_channels: _Optional[int] = ...) -> None: ...

class RequiredReserveResponse(_message.Message):
    __slots__ = ("required_reserve",)
    REQUIRED_RESERVE_FIELD_NUMBER: _ClassVar[int]
    required_reserve: int
    def __init__(self, required_reserve: _Optional[int] = ...) -> None: ...

class ListAddressesRequest(_message.Message):
    __slots__ = ("account_name", "show_custom_accounts")
    ACCOUNT_NAME_FIELD_NUMBER: _ClassVar[int]
    SHOW_CUSTOM_ACCOUNTS_FIELD_NUMBER: _ClassVar[int]
    account_name: str
    show_custom_accounts: bool
    def __init__(
        self, account_name: _Optional[str] = ..., show_custom_accounts: bool = ...
    ) -> None: ...

class ListAddressesResponse(_message.Message):
    __slots__ = ("account_with_addresses",)
    ACCOUNT_WITH_ADDRESSES_FIELD_NUMBER: _ClassVar[int]
    account_with_addresses: _containers.RepeatedCompositeFieldContainer[
        AccountWithAddresses
    ]
    def __init__(
        self,
        account_with_addresses: _Optional[
            _Iterable[_Union[AccountWithAddresses, _Mapping]]
        ] = ...,
    ) -> None: ...

class GetTransactionRequest(_message.Message):
    __slots__ = ("txid",)
    TXID_FIELD_NUMBER: _ClassVar[int]
    txid: str
    def __init__(self, txid: _Optional[str] = ...) -> None: ...

class SignMessageWithAddrRequest(_message.Message):
    __slots__ = ("msg", "addr")
    MSG_FIELD_NUMBER: _ClassVar[int]
    ADDR_FIELD_NUMBER: _ClassVar[int]
    msg: bytes
    addr: str
    def __init__(
        self, msg: _Optional[bytes] = ..., addr: _Optional[str] = ...
    ) -> None: ...

class SignMessageWithAddrResponse(_message.Message):
    __slots__ = ("signature",)
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    signature: str
    def __init__(self, signature: _Optional[str] = ...) -> None: ...

class VerifyMessageWithAddrRequest(_message.Message):
    __slots__ = ("msg", "signature", "addr")
    MSG_FIELD_NUMBER: _ClassVar[int]
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    ADDR_FIELD_NUMBER: _ClassVar[int]
    msg: bytes
    signature: str
    addr: str
    def __init__(
        self,
        msg: _Optional[bytes] = ...,
        signature: _Optional[str] = ...,
        addr: _Optional[str] = ...,
    ) -> None: ...

class VerifyMessageWithAddrResponse(_message.Message):
    __slots__ = ("valid", "pubkey")
    VALID_FIELD_NUMBER: _ClassVar[int]
    PUBKEY_FIELD_NUMBER: _ClassVar[int]
    valid: bool
    pubkey: bytes
    def __init__(self, valid: bool = ..., pubkey: _Optional[bytes] = ...) -> None: ...

class ImportAccountRequest(_message.Message):
    __slots__ = (
        "name",
        "extended_public_key",
        "master_key_fingerprint",
        "address_type",
        "dry_run",
    )
    NAME_FIELD_NUMBER: _ClassVar[int]
    EXTENDED_PUBLIC_KEY_FIELD_NUMBER: _ClassVar[int]
    MASTER_KEY_FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    ADDRESS_TYPE_FIELD_NUMBER: _ClassVar[int]
    DRY_RUN_FIELD_NUMBER: _ClassVar[int]
    name: str
    extended_public_key: str
    master_key_fingerprint: bytes
    address_type: AddressType
    dry_run: bool
    def __init__(
        self,
        name: _Optional[str] = ...,
        extended_public_key: _Optional[str] = ...,
        master_key_fingerprint: _Optional[bytes] = ...,
        address_type: _Optional[_Union[AddressType, str]] = ...,
        dry_run: bool = ...,
    ) -> None: ...

class ImportAccountResponse(_message.Message):
    __slots__ = ("account", "dry_run_external_addrs", "dry_run_internal_addrs")
    ACCOUNT_FIELD_NUMBER: _ClassVar[int]
    DRY_RUN_EXTERNAL_ADDRS_FIELD_NUMBER: _ClassVar[int]
    DRY_RUN_INTERNAL_ADDRS_FIELD_NUMBER: _ClassVar[int]
    account: Account
    dry_run_external_addrs: _containers.RepeatedScalarFieldContainer[str]
    dry_run_internal_addrs: _containers.RepeatedScalarFieldContainer[str]
    def __init__(
        self,
        account: _Optional[_Union[Account, _Mapping]] = ...,
        dry_run_external_addrs: _Optional[_Iterable[str]] = ...,
        dry_run_internal_addrs: _Optional[_Iterable[str]] = ...,
    ) -> None: ...

class ImportPublicKeyRequest(_message.Message):
    __slots__ = ("public_key", "address_type")
    PUBLIC_KEY_FIELD_NUMBER: _ClassVar[int]
    ADDRESS_TYPE_FIELD_NUMBER: _ClassVar[int]
    public_key: bytes
    address_type: AddressType
    def __init__(
        self,
        public_key: _Optional[bytes] = ...,
        address_type: _Optional[_Union[AddressType, str]] = ...,
    ) -> None: ...

class ImportPublicKeyResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ImportTapscriptRequest(_message.Message):
    __slots__ = (
        "internal_public_key",
        "full_tree",
        "partial_reveal",
        "root_hash_only",
        "full_key_only",
    )
    INTERNAL_PUBLIC_KEY_FIELD_NUMBER: _ClassVar[int]
    FULL_TREE_FIELD_NUMBER: _ClassVar[int]
    PARTIAL_REVEAL_FIELD_NUMBER: _ClassVar[int]
    ROOT_HASH_ONLY_FIELD_NUMBER: _ClassVar[int]
    FULL_KEY_ONLY_FIELD_NUMBER: _ClassVar[int]
    internal_public_key: bytes
    full_tree: TapscriptFullTree
    partial_reveal: TapscriptPartialReveal
    root_hash_only: bytes
    full_key_only: bool
    def __init__(
        self,
        internal_public_key: _Optional[bytes] = ...,
        full_tree: _Optional[_Union[TapscriptFullTree, _Mapping]] = ...,
        partial_reveal: _Optional[_Union[TapscriptPartialReveal, _Mapping]] = ...,
        root_hash_only: _Optional[bytes] = ...,
        full_key_only: bool = ...,
    ) -> None: ...

class TapscriptFullTree(_message.Message):
    __slots__ = ("all_leaves",)
    ALL_LEAVES_FIELD_NUMBER: _ClassVar[int]
    all_leaves: _containers.RepeatedCompositeFieldContainer[TapLeaf]
    def __init__(
        self, all_leaves: _Optional[_Iterable[_Union[TapLeaf, _Mapping]]] = ...
    ) -> None: ...

class TapLeaf(_message.Message):
    __slots__ = ("leaf_version", "script")
    LEAF_VERSION_FIELD_NUMBER: _ClassVar[int]
    SCRIPT_FIELD_NUMBER: _ClassVar[int]
    leaf_version: int
    script: bytes
    def __init__(
        self, leaf_version: _Optional[int] = ..., script: _Optional[bytes] = ...
    ) -> None: ...

class TapscriptPartialReveal(_message.Message):
    __slots__ = ("revealed_leaf", "full_inclusion_proof")
    REVEALED_LEAF_FIELD_NUMBER: _ClassVar[int]
    FULL_INCLUSION_PROOF_FIELD_NUMBER: _ClassVar[int]
    revealed_leaf: TapLeaf
    full_inclusion_proof: bytes
    def __init__(
        self,
        revealed_leaf: _Optional[_Union[TapLeaf, _Mapping]] = ...,
        full_inclusion_proof: _Optional[bytes] = ...,
    ) -> None: ...

class ImportTapscriptResponse(_message.Message):
    __slots__ = ("p2tr_address",)
    P2TR_ADDRESS_FIELD_NUMBER: _ClassVar[int]
    p2tr_address: str
    def __init__(self, p2tr_address: _Optional[str] = ...) -> None: ...

class Transaction(_message.Message):
    __slots__ = ("tx_hex", "label")
    TX_HEX_FIELD_NUMBER: _ClassVar[int]
    LABEL_FIELD_NUMBER: _ClassVar[int]
    tx_hex: bytes
    label: str
    def __init__(
        self, tx_hex: _Optional[bytes] = ..., label: _Optional[str] = ...
    ) -> None: ...

class PublishResponse(_message.Message):
    __slots__ = ("publish_error",)
    PUBLISH_ERROR_FIELD_NUMBER: _ClassVar[int]
    publish_error: str
    def __init__(self, publish_error: _Optional[str] = ...) -> None: ...

class RemoveTransactionResponse(_message.Message):
    __slots__ = ("status",)
    STATUS_FIELD_NUMBER: _ClassVar[int]
    status: str
    def __init__(self, status: _Optional[str] = ...) -> None: ...

class SendOutputsRequest(_message.Message):
    __slots__ = (
        "sat_per_kw",
        "outputs",
        "label",
        "min_confs",
        "spend_unconfirmed",
        "coin_selection_strategy",
    )
    SAT_PER_KW_FIELD_NUMBER: _ClassVar[int]
    OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    LABEL_FIELD_NUMBER: _ClassVar[int]
    MIN_CONFS_FIELD_NUMBER: _ClassVar[int]
    SPEND_UNCONFIRMED_FIELD_NUMBER: _ClassVar[int]
    COIN_SELECTION_STRATEGY_FIELD_NUMBER: _ClassVar[int]
    sat_per_kw: int
    outputs: _containers.RepeatedCompositeFieldContainer[_signer_pb2.TxOut]
    label: str
    min_confs: int
    spend_unconfirmed: bool
    coin_selection_strategy: _lightning_pb2.CoinSelectionStrategy
    def __init__(
        self,
        sat_per_kw: _Optional[int] = ...,
        outputs: _Optional[_Iterable[_Union[_signer_pb2.TxOut, _Mapping]]] = ...,
        label: _Optional[str] = ...,
        min_confs: _Optional[int] = ...,
        spend_unconfirmed: bool = ...,
        coin_selection_strategy: _Optional[
            _Union[_lightning_pb2.CoinSelectionStrategy, str]
        ] = ...,
    ) -> None: ...

class SendOutputsResponse(_message.Message):
    __slots__ = ("raw_tx",)
    RAW_TX_FIELD_NUMBER: _ClassVar[int]
    raw_tx: bytes
    def __init__(self, raw_tx: _Optional[bytes] = ...) -> None: ...

class EstimateFeeRequest(_message.Message):
    __slots__ = ("conf_target",)
    CONF_TARGET_FIELD_NUMBER: _ClassVar[int]
    conf_target: int
    def __init__(self, conf_target: _Optional[int] = ...) -> None: ...

class EstimateFeeResponse(_message.Message):
    __slots__ = ("sat_per_kw", "min_relay_fee_sat_per_kw")
    SAT_PER_KW_FIELD_NUMBER: _ClassVar[int]
    MIN_RELAY_FEE_SAT_PER_KW_FIELD_NUMBER: _ClassVar[int]
    sat_per_kw: int
    min_relay_fee_sat_per_kw: int
    def __init__(
        self,
        sat_per_kw: _Optional[int] = ...,
        min_relay_fee_sat_per_kw: _Optional[int] = ...,
    ) -> None: ...

class PendingSweep(_message.Message):
    __slots__ = (
        "outpoint",
        "witness_type",
        "amount_sat",
        "sat_per_byte",
        "broadcast_attempts",
        "next_broadcast_height",
        "force",
        "requested_conf_target",
        "requested_sat_per_byte",
        "sat_per_vbyte",
        "requested_sat_per_vbyte",
        "immediate",
        "budget",
        "deadline_height",
    )
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    WITNESS_TYPE_FIELD_NUMBER: _ClassVar[int]
    AMOUNT_SAT_FIELD_NUMBER: _ClassVar[int]
    SAT_PER_BYTE_FIELD_NUMBER: _ClassVar[int]
    BROADCAST_ATTEMPTS_FIELD_NUMBER: _ClassVar[int]
    NEXT_BROADCAST_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    FORCE_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_CONF_TARGET_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_SAT_PER_BYTE_FIELD_NUMBER: _ClassVar[int]
    SAT_PER_VBYTE_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_SAT_PER_VBYTE_FIELD_NUMBER: _ClassVar[int]
    IMMEDIATE_FIELD_NUMBER: _ClassVar[int]
    BUDGET_FIELD_NUMBER: _ClassVar[int]
    DEADLINE_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    outpoint: _lightning_pb2.OutPoint
    witness_type: WitnessType
    amount_sat: int
    sat_per_byte: int
    broadcast_attempts: int
    next_broadcast_height: int
    force: bool
    requested_conf_target: int
    requested_sat_per_byte: int
    sat_per_vbyte: int
    requested_sat_per_vbyte: int
    immediate: bool
    budget: int
    deadline_height: int
    def __init__(
        self,
        outpoint: _Optional[_Union[_lightning_pb2.OutPoint, _Mapping]] = ...,
        witness_type: _Optional[_Union[WitnessType, str]] = ...,
        amount_sat: _Optional[int] = ...,
        sat_per_byte: _Optional[int] = ...,
        broadcast_attempts: _Optional[int] = ...,
        next_broadcast_height: _Optional[int] = ...,
        force: bool = ...,
        requested_conf_target: _Optional[int] = ...,
        requested_sat_per_byte: _Optional[int] = ...,
        sat_per_vbyte: _Optional[int] = ...,
        requested_sat_per_vbyte: _Optional[int] = ...,
        immediate: bool = ...,
        budget: _Optional[int] = ...,
        deadline_height: _Optional[int] = ...,
    ) -> None: ...

class PendingSweepsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class PendingSweepsResponse(_message.Message):
    __slots__ = ("pending_sweeps",)
    PENDING_SWEEPS_FIELD_NUMBER: _ClassVar[int]
    pending_sweeps: _containers.RepeatedCompositeFieldContainer[PendingSweep]
    def __init__(
        self, pending_sweeps: _Optional[_Iterable[_Union[PendingSweep, _Mapping]]] = ...
    ) -> None: ...

class BumpFeeRequest(_message.Message):
    __slots__ = (
        "outpoint",
        "target_conf",
        "sat_per_byte",
        "force",
        "sat_per_vbyte",
        "immediate",
        "budget",
    )
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    TARGET_CONF_FIELD_NUMBER: _ClassVar[int]
    SAT_PER_BYTE_FIELD_NUMBER: _ClassVar[int]
    FORCE_FIELD_NUMBER: _ClassVar[int]
    SAT_PER_VBYTE_FIELD_NUMBER: _ClassVar[int]
    IMMEDIATE_FIELD_NUMBER: _ClassVar[int]
    BUDGET_FIELD_NUMBER: _ClassVar[int]
    outpoint: _lightning_pb2.OutPoint
    target_conf: int
    sat_per_byte: int
    force: bool
    sat_per_vbyte: int
    immediate: bool
    budget: int
    def __init__(
        self,
        outpoint: _Optional[_Union[_lightning_pb2.OutPoint, _Mapping]] = ...,
        target_conf: _Optional[int] = ...,
        sat_per_byte: _Optional[int] = ...,
        force: bool = ...,
        sat_per_vbyte: _Optional[int] = ...,
        immediate: bool = ...,
        budget: _Optional[int] = ...,
    ) -> None: ...

class BumpFeeResponse(_message.Message):
    __slots__ = ("status",)
    STATUS_FIELD_NUMBER: _ClassVar[int]
    status: str
    def __init__(self, status: _Optional[str] = ...) -> None: ...

class ListSweepsRequest(_message.Message):
    __slots__ = ("verbose", "start_height")
    VERBOSE_FIELD_NUMBER: _ClassVar[int]
    START_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    verbose: bool
    start_height: int
    def __init__(
        self, verbose: bool = ..., start_height: _Optional[int] = ...
    ) -> None: ...

class ListSweepsResponse(_message.Message):
    __slots__ = ("transaction_details", "transaction_ids")

    class TransactionIDs(_message.Message):
        __slots__ = ("transaction_ids",)
        TRANSACTION_IDS_FIELD_NUMBER: _ClassVar[int]
        transaction_ids: _containers.RepeatedScalarFieldContainer[str]
        def __init__(
            self, transaction_ids: _Optional[_Iterable[str]] = ...
        ) -> None: ...

    TRANSACTION_DETAILS_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_IDS_FIELD_NUMBER: _ClassVar[int]
    transaction_details: _lightning_pb2.TransactionDetails
    transaction_ids: ListSweepsResponse.TransactionIDs
    def __init__(
        self,
        transaction_details: _Optional[
            _Union[_lightning_pb2.TransactionDetails, _Mapping]
        ] = ...,
        transaction_ids: _Optional[
            _Union[ListSweepsResponse.TransactionIDs, _Mapping]
        ] = ...,
    ) -> None: ...

class LabelTransactionRequest(_message.Message):
    __slots__ = ("txid", "label", "overwrite")
    TXID_FIELD_NUMBER: _ClassVar[int]
    LABEL_FIELD_NUMBER: _ClassVar[int]
    OVERWRITE_FIELD_NUMBER: _ClassVar[int]
    txid: bytes
    label: str
    overwrite: bool
    def __init__(
        self,
        txid: _Optional[bytes] = ...,
        label: _Optional[str] = ...,
        overwrite: bool = ...,
    ) -> None: ...

class LabelTransactionResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class FundPsbtRequest(_message.Message):
    __slots__ = (
        "psbt",
        "raw",
        "coin_select",
        "target_conf",
        "sat_per_vbyte",
        "account",
        "min_confs",
        "spend_unconfirmed",
        "change_type",
        "coin_selection_strategy",
    )
    PSBT_FIELD_NUMBER: _ClassVar[int]
    RAW_FIELD_NUMBER: _ClassVar[int]
    COIN_SELECT_FIELD_NUMBER: _ClassVar[int]
    TARGET_CONF_FIELD_NUMBER: _ClassVar[int]
    SAT_PER_VBYTE_FIELD_NUMBER: _ClassVar[int]
    ACCOUNT_FIELD_NUMBER: _ClassVar[int]
    MIN_CONFS_FIELD_NUMBER: _ClassVar[int]
    SPEND_UNCONFIRMED_FIELD_NUMBER: _ClassVar[int]
    CHANGE_TYPE_FIELD_NUMBER: _ClassVar[int]
    COIN_SELECTION_STRATEGY_FIELD_NUMBER: _ClassVar[int]
    psbt: bytes
    raw: TxTemplate
    coin_select: PsbtCoinSelect
    target_conf: int
    sat_per_vbyte: int
    account: str
    min_confs: int
    spend_unconfirmed: bool
    change_type: ChangeAddressType
    coin_selection_strategy: _lightning_pb2.CoinSelectionStrategy
    def __init__(
        self,
        psbt: _Optional[bytes] = ...,
        raw: _Optional[_Union[TxTemplate, _Mapping]] = ...,
        coin_select: _Optional[_Union[PsbtCoinSelect, _Mapping]] = ...,
        target_conf: _Optional[int] = ...,
        sat_per_vbyte: _Optional[int] = ...,
        account: _Optional[str] = ...,
        min_confs: _Optional[int] = ...,
        spend_unconfirmed: bool = ...,
        change_type: _Optional[_Union[ChangeAddressType, str]] = ...,
        coin_selection_strategy: _Optional[
            _Union[_lightning_pb2.CoinSelectionStrategy, str]
        ] = ...,
    ) -> None: ...

class FundPsbtResponse(_message.Message):
    __slots__ = ("funded_psbt", "change_output_index", "locked_utxos")
    FUNDED_PSBT_FIELD_NUMBER: _ClassVar[int]
    CHANGE_OUTPUT_INDEX_FIELD_NUMBER: _ClassVar[int]
    LOCKED_UTXOS_FIELD_NUMBER: _ClassVar[int]
    funded_psbt: bytes
    change_output_index: int
    locked_utxos: _containers.RepeatedCompositeFieldContainer[UtxoLease]
    def __init__(
        self,
        funded_psbt: _Optional[bytes] = ...,
        change_output_index: _Optional[int] = ...,
        locked_utxos: _Optional[_Iterable[_Union[UtxoLease, _Mapping]]] = ...,
    ) -> None: ...

class TxTemplate(_message.Message):
    __slots__ = ("inputs", "outputs")

    class OutputsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: int
        def __init__(
            self, key: _Optional[str] = ..., value: _Optional[int] = ...
        ) -> None: ...

    INPUTS_FIELD_NUMBER: _ClassVar[int]
    OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    inputs: _containers.RepeatedCompositeFieldContainer[_lightning_pb2.OutPoint]
    outputs: _containers.ScalarMap[str, int]
    def __init__(
        self,
        inputs: _Optional[_Iterable[_Union[_lightning_pb2.OutPoint, _Mapping]]] = ...,
        outputs: _Optional[_Mapping[str, int]] = ...,
    ) -> None: ...

class PsbtCoinSelect(_message.Message):
    __slots__ = ("psbt", "existing_output_index", "add")
    PSBT_FIELD_NUMBER: _ClassVar[int]
    EXISTING_OUTPUT_INDEX_FIELD_NUMBER: _ClassVar[int]
    ADD_FIELD_NUMBER: _ClassVar[int]
    psbt: bytes
    existing_output_index: int
    add: bool
    def __init__(
        self,
        psbt: _Optional[bytes] = ...,
        existing_output_index: _Optional[int] = ...,
        add: bool = ...,
    ) -> None: ...

class UtxoLease(_message.Message):
    __slots__ = ("id", "outpoint", "expiration", "pk_script", "value")
    ID_FIELD_NUMBER: _ClassVar[int]
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    EXPIRATION_FIELD_NUMBER: _ClassVar[int]
    PK_SCRIPT_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    id: bytes
    outpoint: _lightning_pb2.OutPoint
    expiration: int
    pk_script: bytes
    value: int
    def __init__(
        self,
        id: _Optional[bytes] = ...,
        outpoint: _Optional[_Union[_lightning_pb2.OutPoint, _Mapping]] = ...,
        expiration: _Optional[int] = ...,
        pk_script: _Optional[bytes] = ...,
        value: _Optional[int] = ...,
    ) -> None: ...

class SignPsbtRequest(_message.Message):
    __slots__ = ("funded_psbt",)
    FUNDED_PSBT_FIELD_NUMBER: _ClassVar[int]
    funded_psbt: bytes
    def __init__(self, funded_psbt: _Optional[bytes] = ...) -> None: ...

class SignPsbtResponse(_message.Message):
    __slots__ = ("signed_psbt", "signed_inputs")
    SIGNED_PSBT_FIELD_NUMBER: _ClassVar[int]
    SIGNED_INPUTS_FIELD_NUMBER: _ClassVar[int]
    signed_psbt: bytes
    signed_inputs: _containers.RepeatedScalarFieldContainer[int]
    def __init__(
        self,
        signed_psbt: _Optional[bytes] = ...,
        signed_inputs: _Optional[_Iterable[int]] = ...,
    ) -> None: ...

class FinalizePsbtRequest(_message.Message):
    __slots__ = ("funded_psbt", "account")
    FUNDED_PSBT_FIELD_NUMBER: _ClassVar[int]
    ACCOUNT_FIELD_NUMBER: _ClassVar[int]
    funded_psbt: bytes
    account: str
    def __init__(
        self, funded_psbt: _Optional[bytes] = ..., account: _Optional[str] = ...
    ) -> None: ...

class FinalizePsbtResponse(_message.Message):
    __slots__ = ("signed_psbt", "raw_final_tx")
    SIGNED_PSBT_FIELD_NUMBER: _ClassVar[int]
    RAW_FINAL_TX_FIELD_NUMBER: _ClassVar[int]
    signed_psbt: bytes
    raw_final_tx: bytes
    def __init__(
        self, signed_psbt: _Optional[bytes] = ..., raw_final_tx: _Optional[bytes] = ...
    ) -> None: ...

class ListLeasesRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListLeasesResponse(_message.Message):
    __slots__ = ("locked_utxos",)
    LOCKED_UTXOS_FIELD_NUMBER: _ClassVar[int]
    locked_utxos: _containers.RepeatedCompositeFieldContainer[UtxoLease]
    def __init__(
        self, locked_utxos: _Optional[_Iterable[_Union[UtxoLease, _Mapping]]] = ...
    ) -> None: ...
