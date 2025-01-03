# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# NO CHECKED-IN PROTOBUF GENCODE
# source: swap.proto
# Protobuf Python Version: 5.28.1
"""Generated protocol buffer code."""
from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import runtime_version as _runtime_version
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder

_runtime_version.ValidateProtobufRuntimeVersion(
    _runtime_version.Domain.PUBLIC, 5, 28, 1, "", "swap.proto"
)
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(
    b'\n\nswap.proto\x12\x04swap"]\n\x12\x41\x64\x64\x46undInitRequest\x12\x0e\n\x06nodeID\x18\x01 \x01(\t\x12\x19\n\x11notificationToken\x18\x02 \x01(\t\x12\x0e\n\x06pubkey\x18\x03 \x01(\x0c\x12\x0c\n\x04hash\x18\x04 \x01(\x0c"\x93\x01\n\x10\x41\x64\x64\x46undInitReply\x12\x0f\n\x07\x61\x64\x64ress\x18\x01 \x01(\t\x12\x0e\n\x06pubkey\x18\x02 \x01(\x0c\x12\x12\n\nlockHeight\x18\x03 \x01(\x03\x12\x19\n\x11maxAllowedDeposit\x18\x04 \x01(\x03\x12\x14\n\x0c\x65rrorMessage\x18\x05 \x01(\t\x12\x19\n\x11minAllowedDeposit\x18\x07 \x01(\x03"D\n\x14\x41\x64\x64\x46undStatusRequest\x12\x11\n\taddresses\x18\x01 \x03(\t\x12\x19\n\x11notificationToken\x18\x02 \x01(\t"\x94\x01\n\x12\x41\x64\x64\x46undStatusReply\x12\x38\n\x08statuses\x18\x01 \x03(\x0b\x32&.swap.AddFundStatusReply.StatusesEntry\x1a\x44\n\rStatusesEntry\x12\x0b\n\x03key\x18\x01 \x01(\t\x12"\n\x05value\x18\x02 \x01(\x0b\x32\x13.swap.AddressStatus:\x02\x38\x01"Q\n\rAddressStatus\x12\n\n\x02tx\x18\x01 \x01(\t\x12\x0e\n\x06\x61mount\x18\x02 \x01(\x03\x12\x11\n\tconfirmed\x18\x03 \x01(\x08\x12\x11\n\tblockHash\x18\x04 \x01(\t"/\n\x15GetSwapPaymentRequest\x12\x16\n\x0epaymentRequest\x18\x01 \x01(\t"P\n\x13GetSwapPaymentReply\x12\x14\n\x0cpaymentError\x18\x01 \x01(\t\x12#\n\nswap_error\x18\x03 \x01(\x0e\x32\x0f.swap.SwapError*r\n\tSwapError\x12\x0c\n\x08NO_ERROR\x10\x00\x12\x16\n\x12\x46UNDS_EXCEED_LIMIT\x10\x01\x12\x10\n\x0cTX_TOO_SMALL\x10\x02\x12\x1b\n\x17INVOICE_AMOUNT_MISMATCH\x10\x03\x12\x10\n\x0cSWAP_EXPIRED\x10\x04\x32\xe1\x01\n\x07Swapper\x12\x41\n\x0b\x41\x64\x64\x46undInit\x12\x18.swap.AddFundInitRequest\x1a\x16.swap.AddFundInitReply"\x00\x12G\n\rAddFundStatus\x12\x1a.swap.AddFundStatusRequest\x1a\x18.swap.AddFundStatusReply"\x00\x12J\n\x0eGetSwapPayment\x12\x1b.swap.GetSwapPaymentRequest\x1a\x19.swap.GetSwapPaymentReply"\x00\x62\x06proto3'
)

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, "swap_pb2", _globals)
if not _descriptor._USE_C_DESCRIPTORS:
    DESCRIPTOR._loaded_options = None
    _globals["_ADDFUNDSTATUSREPLY_STATUSESENTRY"]._loaded_options = None
    _globals["_ADDFUNDSTATUSREPLY_STATUSESENTRY"]._serialized_options = b"8\001"
    _globals["_SWAPERROR"]._serialized_start = 700
    _globals["_SWAPERROR"]._serialized_end = 814
    _globals["_ADDFUNDINITREQUEST"]._serialized_start = 20
    _globals["_ADDFUNDINITREQUEST"]._serialized_end = 113
    _globals["_ADDFUNDINITREPLY"]._serialized_start = 116
    _globals["_ADDFUNDINITREPLY"]._serialized_end = 263
    _globals["_ADDFUNDSTATUSREQUEST"]._serialized_start = 265
    _globals["_ADDFUNDSTATUSREQUEST"]._serialized_end = 333
    _globals["_ADDFUNDSTATUSREPLY"]._serialized_start = 336
    _globals["_ADDFUNDSTATUSREPLY"]._serialized_end = 484
    _globals["_ADDFUNDSTATUSREPLY_STATUSESENTRY"]._serialized_start = 416
    _globals["_ADDFUNDSTATUSREPLY_STATUSESENTRY"]._serialized_end = 484
    _globals["_ADDRESSSTATUS"]._serialized_start = 486
    _globals["_ADDRESSSTATUS"]._serialized_end = 567
    _globals["_GETSWAPPAYMENTREQUEST"]._serialized_start = 569
    _globals["_GETSWAPPAYMENTREQUEST"]._serialized_end = 616
    _globals["_GETSWAPPAYMENTREPLY"]._serialized_start = 618
    _globals["_GETSWAPPAYMENTREPLY"]._serialized_end = 698
    _globals["_SWAPPER"]._serialized_start = 817
    _globals["_SWAPPER"]._serialized_end = 1042
# @@protoc_insertion_point(module_scope)
