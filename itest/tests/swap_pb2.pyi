from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Iterable as _Iterable, Mapping as _Mapping, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class SwapError(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    NO_ERROR: _ClassVar[SwapError]
    FUNDS_EXCEED_LIMIT: _ClassVar[SwapError]
    TX_TOO_SMALL: _ClassVar[SwapError]
    INVOICE_AMOUNT_MISMATCH: _ClassVar[SwapError]
    SWAP_EXPIRED: _ClassVar[SwapError]
NO_ERROR: SwapError
FUNDS_EXCEED_LIMIT: SwapError
TX_TOO_SMALL: SwapError
INVOICE_AMOUNT_MISMATCH: SwapError
SWAP_EXPIRED: SwapError

class AddFundInitRequest(_message.Message):
    __slots__ = ("nodeID", "notificationToken", "pubkey", "hash")
    NODEID_FIELD_NUMBER: _ClassVar[int]
    NOTIFICATIONTOKEN_FIELD_NUMBER: _ClassVar[int]
    PUBKEY_FIELD_NUMBER: _ClassVar[int]
    HASH_FIELD_NUMBER: _ClassVar[int]
    nodeID: str
    notificationToken: str
    pubkey: bytes
    hash: bytes
    def __init__(self, nodeID: _Optional[str] = ..., notificationToken: _Optional[str] = ..., pubkey: _Optional[bytes] = ..., hash: _Optional[bytes] = ...) -> None: ...

class AddFundInitReply(_message.Message):
    __slots__ = ("address", "pubkey", "lockHeight", "maxAllowedDeposit", "errorMessage", "minAllowedDeposit")
    ADDRESS_FIELD_NUMBER: _ClassVar[int]
    PUBKEY_FIELD_NUMBER: _ClassVar[int]
    LOCKHEIGHT_FIELD_NUMBER: _ClassVar[int]
    MAXALLOWEDDEPOSIT_FIELD_NUMBER: _ClassVar[int]
    ERRORMESSAGE_FIELD_NUMBER: _ClassVar[int]
    MINALLOWEDDEPOSIT_FIELD_NUMBER: _ClassVar[int]
    address: str
    pubkey: bytes
    lockHeight: int
    maxAllowedDeposit: int
    errorMessage: str
    minAllowedDeposit: int
    def __init__(self, address: _Optional[str] = ..., pubkey: _Optional[bytes] = ..., lockHeight: _Optional[int] = ..., maxAllowedDeposit: _Optional[int] = ..., errorMessage: _Optional[str] = ..., minAllowedDeposit: _Optional[int] = ...) -> None: ...

class AddFundStatusRequest(_message.Message):
    __slots__ = ("addresses", "notificationToken")
    ADDRESSES_FIELD_NUMBER: _ClassVar[int]
    NOTIFICATIONTOKEN_FIELD_NUMBER: _ClassVar[int]
    addresses: _containers.RepeatedScalarFieldContainer[str]
    notificationToken: str
    def __init__(self, addresses: _Optional[_Iterable[str]] = ..., notificationToken: _Optional[str] = ...) -> None: ...

class AddFundStatusReply(_message.Message):
    __slots__ = ("statuses",)
    class StatusesEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: AddressStatus
        def __init__(self, key: _Optional[str] = ..., value: _Optional[_Union[AddressStatus, _Mapping]] = ...) -> None: ...
    STATUSES_FIELD_NUMBER: _ClassVar[int]
    statuses: _containers.MessageMap[str, AddressStatus]
    def __init__(self, statuses: _Optional[_Mapping[str, AddressStatus]] = ...) -> None: ...

class AddressStatus(_message.Message):
    __slots__ = ("tx", "amount", "confirmed", "blockHash")
    TX_FIELD_NUMBER: _ClassVar[int]
    AMOUNT_FIELD_NUMBER: _ClassVar[int]
    CONFIRMED_FIELD_NUMBER: _ClassVar[int]
    BLOCKHASH_FIELD_NUMBER: _ClassVar[int]
    tx: str
    amount: int
    confirmed: bool
    blockHash: str
    def __init__(self, tx: _Optional[str] = ..., amount: _Optional[int] = ..., confirmed: bool = ..., blockHash: _Optional[str] = ...) -> None: ...

class GetSwapPaymentRequest(_message.Message):
    __slots__ = ("paymentRequest",)
    PAYMENTREQUEST_FIELD_NUMBER: _ClassVar[int]
    paymentRequest: str
    def __init__(self, paymentRequest: _Optional[str] = ...) -> None: ...

class GetSwapPaymentReply(_message.Message):
    __slots__ = ("paymentError", "swap_error")
    PAYMENTERROR_FIELD_NUMBER: _ClassVar[int]
    SWAP_ERROR_FIELD_NUMBER: _ClassVar[int]
    paymentError: str
    swap_error: SwapError
    def __init__(self, paymentError: _Optional[str] = ..., swap_error: _Optional[_Union[SwapError, str]] = ...) -> None: ...
