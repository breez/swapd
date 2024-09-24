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

class AddAddressFiltersReply(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetInfoRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class GetInfoReply(_message.Message):
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

class GetSwapReply(_message.Message):
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
    __slots__ = ("outpoint", "confirmation_height")
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    CONFIRMATION_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    outpoint: str
    confirmation_height: int
    def __init__(
        self, outpoint: _Optional[str] = ..., confirmation_height: _Optional[int] = ...
    ) -> None: ...

class ListRedeemableRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListRedeemableReply(_message.Message):
    __slots__ = ("redeemables",)
    REDEEMABLES_FIELD_NUMBER: _ClassVar[int]
    redeemables: _containers.RepeatedCompositeFieldContainer[Redeemable]
    def __init__(
        self, redeemables: _Optional[_Iterable[_Union[Redeemable, _Mapping]]] = ...
    ) -> None: ...

class Redeemable(_message.Message):
    __slots__ = ("blocks_left", "swap_hash", "lock_time", "utxos")
    BLOCKS_LEFT_FIELD_NUMBER: _ClassVar[int]
    SWAP_HASH_FIELD_NUMBER: _ClassVar[int]
    LOCK_TIME_FIELD_NUMBER: _ClassVar[int]
    UTXOS_FIELD_NUMBER: _ClassVar[int]
    blocks_left: int
    swap_hash: str
    lock_time: int
    utxos: _containers.RepeatedCompositeFieldContainer[RedeemableUtxo]
    def __init__(
        self,
        blocks_left: _Optional[int] = ...,
        swap_hash: _Optional[str] = ...,
        lock_time: _Optional[int] = ...,
        utxos: _Optional[_Iterable[_Union[RedeemableUtxo, _Mapping]]] = ...,
    ) -> None: ...

class RedeemableUtxo(_message.Message):
    __slots__ = ("outpoint", "confirmation_height")
    OUTPOINT_FIELD_NUMBER: _ClassVar[int]
    CONFIRMATION_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    outpoint: str
    confirmation_height: int
    def __init__(
        self, outpoint: _Optional[str] = ..., confirmation_height: _Optional[int] = ...
    ) -> None: ...

class StopRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StopReply(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
