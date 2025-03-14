from google.protobuf.internal import containers as _containers
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

class AddAddressFiltersRequest(_message.Message):
    __slots__ = ("addresses",)
    ADDRESSES_FIELD_NUMBER: _ClassVar[int]
    addresses: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, addresses: _Optional[_Iterable[str]] = ...) -> None: ...

class AddAddressFiltersResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetInfoRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetInfoResponse(_message.Message):
    __slots__ = ("block_height", "network")
    BLOCK_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    NETWORK_FIELD_NUMBER: _ClassVar[int]
    block_height: int
    network: str
    def __init__(
        self, block_height: _Optional[int] = ..., network: _Optional[str] = ...
    ) -> None: ...

class GetSwapRequest(_message.Message):
    __slots__ = ("address", "payment_request", "payment_hash")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_REQUEST_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_HASH_FIELD_NUMBER: _ClassVar[int]
    address: str
    payment_request: str
    payment_hash: bytes
    def __init__(
        self,
        address: _Optional[str] = ...,
        payment_request: _Optional[str] = ...,
        payment_hash: _Optional[bytes] = ...,
    ) -> None: ...

class GetSwapResponse(_message.Message):
    __slots__ = ("address", "outputs")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    address: str
    outputs: _containers.RepeatedCompositeFieldContainer[SwapOutput]
    def __init__(
        self,
        address: _Optional[str] = ...,
        outputs: _Optional[_Iterable[_Union[SwapOutput, _Mapping]]] = ...,
    ) -> None: ...

class SwapOutput(_message.Message):
    __slots__ = ("outpoint", "confirmation_height", "block_hash")
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    CONFIRMATION_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    BLOCK_HASH_FIELD_NUMBER: _ClassVar[int]
    outpoint: str
    confirmation_height: int
    block_hash: str
    def __init__(
        self,
        outpoint: _Optional[str] = ...,
        confirmation_height: _Optional[int] = ...,
        block_hash: _Optional[str] = ...,
    ) -> None: ...

class ListClaimableRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListClaimableResponse(_message.Message):
    __slots__ = ("claimables",)
    CLAIMABLES_FIELD_NUMBER: _ClassVar[int]
    claimables: _containers.RepeatedCompositeFieldContainer[ClaimableUtxo]
    def __init__(
        self, claimables: _Optional[_Iterable[_Union[ClaimableUtxo, _Mapping]]] = ...
    ) -> None: ...

class ClaimableUtxo(_message.Message):
    __slots__ = (
        "outpoint",
        "swap_hash",
        "lock_time",
        "confirmation_height",
        "block_hash",
        "blocks_left",
        "paid_with_request",
    )
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    SWAP_HASH_FIELD_NUMBER: _ClassVar[int]
    LOCK_TIME_FIELD_NUMBER: _ClassVar[int]
    CONFIRMATION_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    BLOCK_HASH_FIELD_NUMBER: _ClassVar[int]
    BLOCKS_LEFT_FIELD_NUMBER: _ClassVar[int]
    PAID_WITH_REQUEST_FIELD_NUMBER: _ClassVar[int]
    outpoint: str
    swap_hash: str
    lock_time: int
    confirmation_height: int
    block_hash: str
    blocks_left: int
    paid_with_request: str
    def __init__(
        self,
        outpoint: _Optional[str] = ...,
        swap_hash: _Optional[str] = ...,
        lock_time: _Optional[int] = ...,
        confirmation_height: _Optional[int] = ...,
        block_hash: _Optional[str] = ...,
        blocks_left: _Optional[int] = ...,
        paid_with_request: _Optional[str] = ...,
    ) -> None: ...

class ClaimRequest(_message.Message):
    __slots__ = ("outpoints", "destination_address", "fee_per_kw", "auto_bump")
    OUTPOINTS_FIELD_NUMBER: _ClassVar[int]
    DESTINATION_ADDRESS_FIELD_NUMBER: _ClassVar[int]
    FEE_PER_KW_FIELD_NUMBER: _ClassVar[int]
    AUTO_BUMP_FIELD_NUMBER: _ClassVar[int]
    outpoints: _containers.RepeatedScalarFieldContainer[str]
    destination_address: str
    fee_per_kw: int
    auto_bump: bool
    def __init__(
        self,
        outpoints: _Optional[_Iterable[str]] = ...,
        destination_address: _Optional[str] = ...,
        fee_per_kw: _Optional[int] = ...,
        auto_bump: bool = ...,
    ) -> None: ...

class ClaimResponse(_message.Message):
    __slots__ = ("tx_id", "fee_per_kw")
    TX_ID_FIELD_NUMBER: _ClassVar[int]
    FEE_PER_KW_FIELD_NUMBER: _ClassVar[int]
    tx_id: str
    fee_per_kw: int
    def __init__(
        self, tx_id: _Optional[str] = ..., fee_per_kw: _Optional[int] = ...
    ) -> None: ...

class StopRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StopResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
