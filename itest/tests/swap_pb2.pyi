from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import (
    ClassVar as _ClassVar,
    Mapping as _Mapping,
    Optional as _Optional,
    Union as _Union,
)

DESCRIPTOR: _descriptor.FileDescriptor

class CreateSwapRequest(_message.Message):
    __slots__ = ("hash", "refund_pubkey")
    HASH_FIELD_NUMBER: _ClassVar[int]
    REFUND_PUBKEY_FIELD_NUMBER: _ClassVar[int]
    hash: bytes
    refund_pubkey: bytes
    def __init__(
        self, hash: _Optional[bytes] = ..., refund_pubkey: _Optional[bytes] = ...
    ) -> None: ...

class CreateSwapResponse(_message.Message):
    __slots__ = ("address", "claim_pubkey", "lock_height", "parameters")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    CLAIM_PUBKEY_FIELD_NUMBER: _ClassVar[int]
    LOCK_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    PARAMETERS_FIELD_NUMBER: _ClassVar[int]
    address: str
    claim_pubkey: bytes
    lock_height: int
    parameters: SwapParameters
    def __init__(
        self,
        address: _Optional[str] = ...,
        claim_pubkey: _Optional[bytes] = ...,
        lock_height: _Optional[int] = ...,
        parameters: _Optional[_Union[SwapParameters, _Mapping]] = ...,
    ) -> None: ...

class PaySwapRequest(_message.Message):
    __slots__ = ("payment_request",)
    PAYMENT_REQUEST_FIELD_NUMBER: _ClassVar[int]
    payment_request: str
    def __init__(self, payment_request: _Optional[str] = ...) -> None: ...

class PaySwapResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class RefundSwapRequest(_message.Message):
    __slots__ = ("address", "transaction", "input_index", "pub_nonce")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_FIELD_NUMBER: _ClassVar[int]
    INPUT_INDEX_FIELD_NUMBER: _ClassVar[int]
    PUB_NONCE_FIELD_NUMBER: _ClassVar[int]
    address: str
    transaction: bytes
    input_index: int
    pub_nonce: bytes
    def __init__(
        self,
        address: _Optional[str] = ...,
        transaction: _Optional[bytes] = ...,
        input_index: _Optional[int] = ...,
        pub_nonce: _Optional[bytes] = ...,
    ) -> None: ...

class RefundSwapResponse(_message.Message):
    __slots__ = ("pub_nonce", "partial_signature")
    PUB_NONCE_FIELD_NUMBER: _ClassVar[int]
    PARTIAL_SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    pub_nonce: bytes
    partial_signature: bytes
    def __init__(
        self,
        pub_nonce: _Optional[bytes] = ...,
        partial_signature: _Optional[bytes] = ...,
    ) -> None: ...

class SwapParameters(_message.Message):
    __slots__ = (
        "lock_time",
        "max_swap_amount_sat",
        "min_swap_amount_sat",
        "min_utxo_amount_sat",
    )
    LOCK_TIME_FIELD_NUMBER: _ClassVar[int]
    MAX_SWAP_AMOUNT_SAT_FIELD_NUMBER: _ClassVar[int]
    MIN_SWAP_AMOUNT_SAT_FIELD_NUMBER: _ClassVar[int]
    MIN_UTXO_AMOUNT_SAT_FIELD_NUMBER: _ClassVar[int]
    lock_time: int
    max_swap_amount_sat: int
    min_swap_amount_sat: int
    min_utxo_amount_sat: int
    def __init__(
        self,
        lock_time: _Optional[int] = ...,
        max_swap_amount_sat: _Optional[int] = ...,
        min_swap_amount_sat: _Optional[int] = ...,
        min_utxo_amount_sat: _Optional[int] = ...,
    ) -> None: ...

class SwapParametersRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class SwapParametersResponse(_message.Message):
    __slots__ = ("parameters",)
    PARAMETERS_FIELD_NUMBER: _ClassVar[int]
    parameters: SwapParameters
    def __init__(
        self, parameters: _Optional[_Union[SwapParameters, _Mapping]] = ...
    ) -> None: ...
