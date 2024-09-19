from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Iterable as _Iterable, Optional as _Optional

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
    def __init__(self, block_height: _Optional[int] = ..., network: _Optional[str] = ...) -> None: ...

class GetSwapRequest(_message.Message):
    __slots__ = ("address", "payment_request", "payment_hash", "destination")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_REQUEST_FIELD_NUMBER: _ClassVar[int]
    PAYMENT_HASH_FIELD_NUMBER: _ClassVar[int]
    DESTINATION_FIELD_NUMBER: _ClassVar[int]
    address: str
    payment_request: str
    payment_hash: bytes
    destination: str
    def __init__(self, address: _Optional[str] = ..., payment_request: _Optional[str] = ..., payment_hash: _Optional[bytes] = ..., destination: _Optional[str] = ...) -> None: ...

class GetSwapReply(_message.Message):
    __slots__ = ("address", "confirmation_height")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    CONFIRMATION_HEIGHT_FIELD_NUMBER: _ClassVar[int]
    address: str
    confirmation_height: int
    def __init__(self, address: _Optional[str] = ..., confirmation_height: _Optional[int] = ...) -> None: ...

class StopRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StopReply(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
