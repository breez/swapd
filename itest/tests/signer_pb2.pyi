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

class SignMethod(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SIGN_METHOD_WITNESS_V0: _ClassVar[SignMethod]
    SIGN_METHOD_TAPROOT_KEY_SPEND_BIP0086: _ClassVar[SignMethod]
    SIGN_METHOD_TAPROOT_KEY_SPEND: _ClassVar[SignMethod]
    SIGN_METHOD_TAPROOT_SCRIPT_SPEND: _ClassVar[SignMethod]

class MuSig2Version(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    MUSIG2_VERSION_UNDEFINED: _ClassVar[MuSig2Version]
    MUSIG2_VERSION_V040: _ClassVar[MuSig2Version]
    MUSIG2_VERSION_V100RC2: _ClassVar[MuSig2Version]

SIGN_METHOD_WITNESS_V0: SignMethod
SIGN_METHOD_TAPROOT_KEY_SPEND_BIP0086: SignMethod
SIGN_METHOD_TAPROOT_KEY_SPEND: SignMethod
SIGN_METHOD_TAPROOT_SCRIPT_SPEND: SignMethod
MUSIG2_VERSION_UNDEFINED: MuSig2Version
MUSIG2_VERSION_V040: MuSig2Version
MUSIG2_VERSION_V100RC2: MuSig2Version

class KeyLocator(_message.Message):
    __slots__ = ("key_family", "key_index")
    KEY_FAMILY_FIELD_NUMBER: _ClassVar[int]
    KEY_INDEX_FIELD_NUMBER: _ClassVar[int]
    key_family: int
    key_index: int
    def __init__(
        self, key_family: _Optional[int] = ..., key_index: _Optional[int] = ...
    ) -> None: ...

class KeyDescriptor(_message.Message):
    __slots__ = ("raw_key_bytes", "key_loc")
    RAW_KEY_BYTES_FIELD_NUMBER: _ClassVar[int]
    KEY_LOC_FIELD_NUMBER: _ClassVar[int]
    raw_key_bytes: bytes
    key_loc: KeyLocator
    def __init__(
        self,
        raw_key_bytes: _Optional[bytes] = ...,
        key_loc: _Optional[_Union[KeyLocator, _Mapping]] = ...,
    ) -> None: ...

class TxOut(_message.Message):
    __slots__ = ("value", "pk_script")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    PK_SCRIPT_FIELD_NUMBER: _ClassVar[int]
    value: int
    pk_script: bytes
    def __init__(
        self, value: _Optional[int] = ..., pk_script: _Optional[bytes] = ...
    ) -> None: ...

class SignDescriptor(_message.Message):
    __slots__ = (
        "key_desc",
        "single_tweak",
        "double_tweak",
        "tap_tweak",
        "witness_script",
        "output",
        "sighash",
        "input_index",
        "sign_method",
    )
    KEY_DESC_FIELD_NUMBER: _ClassVar[int]
    SINGLE_TWEAK_FIELD_NUMBER: _ClassVar[int]
    DOUBLE_TWEAK_FIELD_NUMBER: _ClassVar[int]
    TAP_TWEAK_FIELD_NUMBER: _ClassVar[int]
    WITNESS_SCRIPT_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_FIELD_NUMBER: _ClassVar[int]
    SIGHASH_FIELD_NUMBER: _ClassVar[int]
    INPUT_INDEX_FIELD_NUMBER: _ClassVar[int]
    SIGN_METHOD_FIELD_NUMBER: _ClassVar[int]
    key_desc: KeyDescriptor
    single_tweak: bytes
    double_tweak: bytes
    tap_tweak: bytes
    witness_script: bytes
    output: TxOut
    sighash: int
    input_index: int
    sign_method: SignMethod
    def __init__(
        self,
        key_desc: _Optional[_Union[KeyDescriptor, _Mapping]] = ...,
        single_tweak: _Optional[bytes] = ...,
        double_tweak: _Optional[bytes] = ...,
        tap_tweak: _Optional[bytes] = ...,
        witness_script: _Optional[bytes] = ...,
        output: _Optional[_Union[TxOut, _Mapping]] = ...,
        sighash: _Optional[int] = ...,
        input_index: _Optional[int] = ...,
        sign_method: _Optional[_Union[SignMethod, str]] = ...,
    ) -> None: ...

class SignReq(_message.Message):
    __slots__ = ("raw_tx_bytes", "sign_descs", "prev_outputs")
    RAW_TX_BYTES_FIELD_NUMBER: _ClassVar[int]
    SIGN_DESCS_FIELD_NUMBER: _ClassVar[int]
    PREV_OUTPUTS_FIELD_NUMBER: _ClassVar[int]
    raw_tx_bytes: bytes
    sign_descs: _containers.RepeatedCompositeFieldContainer[SignDescriptor]
    prev_outputs: _containers.RepeatedCompositeFieldContainer[TxOut]
    def __init__(
        self,
        raw_tx_bytes: _Optional[bytes] = ...,
        sign_descs: _Optional[_Iterable[_Union[SignDescriptor, _Mapping]]] = ...,
        prev_outputs: _Optional[_Iterable[_Union[TxOut, _Mapping]]] = ...,
    ) -> None: ...

class SignResp(_message.Message):
    __slots__ = ("raw_sigs",)
    RAW_SIGS_FIELD_NUMBER: _ClassVar[int]
    raw_sigs: _containers.RepeatedScalarFieldContainer[bytes]
    def __init__(self, raw_sigs: _Optional[_Iterable[bytes]] = ...) -> None: ...

class InputScript(_message.Message):
    __slots__ = ("witness", "sig_script")
    WITNESS_FIELD_NUMBER: _ClassVar[int]
    SIG_SCRIPT_FIELD_NUMBER: _ClassVar[int]
    witness: _containers.RepeatedScalarFieldContainer[bytes]
    sig_script: bytes
    def __init__(
        self,
        witness: _Optional[_Iterable[bytes]] = ...,
        sig_script: _Optional[bytes] = ...,
    ) -> None: ...

class InputScriptResp(_message.Message):
    __slots__ = ("input_scripts",)
    INPUT_SCRIPTS_FIELD_NUMBER: _ClassVar[int]
    input_scripts: _containers.RepeatedCompositeFieldContainer[InputScript]
    def __init__(
        self, input_scripts: _Optional[_Iterable[_Union[InputScript, _Mapping]]] = ...
    ) -> None: ...

class SignMessageReq(_message.Message):
    __slots__ = (
        "msg",
        "key_loc",
        "double_hash",
        "compact_sig",
        "schnorr_sig",
        "schnorr_sig_tap_tweak",
        "tag",
    )
    MSG_FIELD_NUMBER: _ClassVar[int]
    KEY_LOC_FIELD_NUMBER: _ClassVar[int]
    DOUBLE_HASH_FIELD_NUMBER: _ClassVar[int]
    COMPACT_SIG_FIELD_NUMBER: _ClassVar[int]
    SCHNORR_SIG_FIELD_NUMBER: _ClassVar[int]
    SCHNORR_SIG_TAP_TWEAK_FIELD_NUMBER: _ClassVar[int]
    TAG_FIELD_NUMBER: _ClassVar[int]
    msg: bytes
    key_loc: KeyLocator
    double_hash: bool
    compact_sig: bool
    schnorr_sig: bool
    schnorr_sig_tap_tweak: bytes
    tag: bytes
    def __init__(
        self,
        msg: _Optional[bytes] = ...,
        key_loc: _Optional[_Union[KeyLocator, _Mapping]] = ...,
        double_hash: bool = ...,
        compact_sig: bool = ...,
        schnorr_sig: bool = ...,
        schnorr_sig_tap_tweak: _Optional[bytes] = ...,
        tag: _Optional[bytes] = ...,
    ) -> None: ...

class SignMessageResp(_message.Message):
    __slots__ = ("signature",)
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    signature: bytes
    def __init__(self, signature: _Optional[bytes] = ...) -> None: ...

class VerifyMessageReq(_message.Message):
    __slots__ = ("msg", "signature", "pubkey", "is_schnorr_sig", "tag")
    MSG_FIELD_NUMBER: _ClassVar[int]
    SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    PUBKEY_FIELD_NUMBER: _ClassVar[int]
    IS_SCHNORR_SIG_FIELD_NUMBER: _ClassVar[int]
    TAG_FIELD_NUMBER: _ClassVar[int]
    msg: bytes
    signature: bytes
    pubkey: bytes
    is_schnorr_sig: bool
    tag: bytes
    def __init__(
        self,
        msg: _Optional[bytes] = ...,
        signature: _Optional[bytes] = ...,
        pubkey: _Optional[bytes] = ...,
        is_schnorr_sig: bool = ...,
        tag: _Optional[bytes] = ...,
    ) -> None: ...

class VerifyMessageResp(_message.Message):
    __slots__ = ("valid",)
    VALID_FIELD_NUMBER: _ClassVar[int]
    valid: bool
    def __init__(self, valid: bool = ...) -> None: ...

class SharedKeyRequest(_message.Message):
    __slots__ = ("ephemeral_pubkey", "key_loc", "key_desc")
    EPHEMERAL_PUBKEY_FIELD_NUMBER: _ClassVar[int]
    KEY_LOC_FIELD_NUMBER: _ClassVar[int]
    KEY_DESC_FIELD_NUMBER: _ClassVar[int]
    ephemeral_pubkey: bytes
    key_loc: KeyLocator
    key_desc: KeyDescriptor
    def __init__(
        self,
        ephemeral_pubkey: _Optional[bytes] = ...,
        key_loc: _Optional[_Union[KeyLocator, _Mapping]] = ...,
        key_desc: _Optional[_Union[KeyDescriptor, _Mapping]] = ...,
    ) -> None: ...

class SharedKeyResponse(_message.Message):
    __slots__ = ("shared_key",)
    SHARED_KEY_FIELD_NUMBER: _ClassVar[int]
    shared_key: bytes
    def __init__(self, shared_key: _Optional[bytes] = ...) -> None: ...

class TweakDesc(_message.Message):
    __slots__ = ("tweak", "is_x_only")
    TWEAK_FIELD_NUMBER: _ClassVar[int]
    IS_X_ONLY_FIELD_NUMBER: _ClassVar[int]
    tweak: bytes
    is_x_only: bool
    def __init__(
        self, tweak: _Optional[bytes] = ..., is_x_only: bool = ...
    ) -> None: ...

class TaprootTweakDesc(_message.Message):
    __slots__ = ("script_root", "key_spend_only")
    SCRIPT_ROOT_FIELD_NUMBER: _ClassVar[int]
    KEY_SPEND_ONLY_FIELD_NUMBER: _ClassVar[int]
    script_root: bytes
    key_spend_only: bool
    def __init__(
        self, script_root: _Optional[bytes] = ..., key_spend_only: bool = ...
    ) -> None: ...

class MuSig2CombineKeysRequest(_message.Message):
    __slots__ = ("all_signer_pubkeys", "tweaks", "taproot_tweak", "version")
    ALL_SIGNER_PUBKEYS_FIELD_NUMBER: _ClassVar[int]
    TWEAKS_FIELD_NUMBER: _ClassVar[int]
    TAPROOT_TWEAK_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    all_signer_pubkeys: _containers.RepeatedScalarFieldContainer[bytes]
    tweaks: _containers.RepeatedCompositeFieldContainer[TweakDesc]
    taproot_tweak: TaprootTweakDesc
    version: MuSig2Version
    def __init__(
        self,
        all_signer_pubkeys: _Optional[_Iterable[bytes]] = ...,
        tweaks: _Optional[_Iterable[_Union[TweakDesc, _Mapping]]] = ...,
        taproot_tweak: _Optional[_Union[TaprootTweakDesc, _Mapping]] = ...,
        version: _Optional[_Union[MuSig2Version, str]] = ...,
    ) -> None: ...

class MuSig2CombineKeysResponse(_message.Message):
    __slots__ = ("combined_key", "taproot_internal_key", "version")
    COMBINED_KEY_FIELD_NUMBER: _ClassVar[int]
    TAPROOT_INTERNAL_KEY_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    combined_key: bytes
    taproot_internal_key: bytes
    version: MuSig2Version
    def __init__(
        self,
        combined_key: _Optional[bytes] = ...,
        taproot_internal_key: _Optional[bytes] = ...,
        version: _Optional[_Union[MuSig2Version, str]] = ...,
    ) -> None: ...

class MuSig2SessionRequest(_message.Message):
    __slots__ = (
        "key_loc",
        "all_signer_pubkeys",
        "other_signer_public_nonces",
        "tweaks",
        "taproot_tweak",
        "version",
        "pregenerated_local_nonce",
    )
    KEY_LOC_FIELD_NUMBER: _ClassVar[int]
    ALL_SIGNER_PUBKEYS_FIELD_NUMBER: _ClassVar[int]
    OTHER_SIGNER_PUBLIC_NONCES_FIELD_NUMBER: _ClassVar[int]
    TWEAKS_FIELD_NUMBER: _ClassVar[int]
    TAPROOT_TWEAK_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    PREGENERATED_LOCAL_NONCE_FIELD_NUMBER: _ClassVar[int]
    key_loc: KeyLocator
    all_signer_pubkeys: _containers.RepeatedScalarFieldContainer[bytes]
    other_signer_public_nonces: _containers.RepeatedScalarFieldContainer[bytes]
    tweaks: _containers.RepeatedCompositeFieldContainer[TweakDesc]
    taproot_tweak: TaprootTweakDesc
    version: MuSig2Version
    pregenerated_local_nonce: bytes
    def __init__(
        self,
        key_loc: _Optional[_Union[KeyLocator, _Mapping]] = ...,
        all_signer_pubkeys: _Optional[_Iterable[bytes]] = ...,
        other_signer_public_nonces: _Optional[_Iterable[bytes]] = ...,
        tweaks: _Optional[_Iterable[_Union[TweakDesc, _Mapping]]] = ...,
        taproot_tweak: _Optional[_Union[TaprootTweakDesc, _Mapping]] = ...,
        version: _Optional[_Union[MuSig2Version, str]] = ...,
        pregenerated_local_nonce: _Optional[bytes] = ...,
    ) -> None: ...

class MuSig2SessionResponse(_message.Message):
    __slots__ = (
        "session_id",
        "combined_key",
        "taproot_internal_key",
        "local_public_nonces",
        "have_all_nonces",
        "version",
    )
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    COMBINED_KEY_FIELD_NUMBER: _ClassVar[int]
    TAPROOT_INTERNAL_KEY_FIELD_NUMBER: _ClassVar[int]
    LOCAL_PUBLIC_NONCES_FIELD_NUMBER: _ClassVar[int]
    HAVE_ALL_NONCES_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    session_id: bytes
    combined_key: bytes
    taproot_internal_key: bytes
    local_public_nonces: bytes
    have_all_nonces: bool
    version: MuSig2Version
    def __init__(
        self,
        session_id: _Optional[bytes] = ...,
        combined_key: _Optional[bytes] = ...,
        taproot_internal_key: _Optional[bytes] = ...,
        local_public_nonces: _Optional[bytes] = ...,
        have_all_nonces: bool = ...,
        version: _Optional[_Union[MuSig2Version, str]] = ...,
    ) -> None: ...

class MuSig2RegisterNoncesRequest(_message.Message):
    __slots__ = ("session_id", "other_signer_public_nonces")
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    OTHER_SIGNER_PUBLIC_NONCES_FIELD_NUMBER: _ClassVar[int]
    session_id: bytes
    other_signer_public_nonces: _containers.RepeatedScalarFieldContainer[bytes]
    def __init__(
        self,
        session_id: _Optional[bytes] = ...,
        other_signer_public_nonces: _Optional[_Iterable[bytes]] = ...,
    ) -> None: ...

class MuSig2RegisterNoncesResponse(_message.Message):
    __slots__ = ("have_all_nonces",)
    HAVE_ALL_NONCES_FIELD_NUMBER: _ClassVar[int]
    have_all_nonces: bool
    def __init__(self, have_all_nonces: bool = ...) -> None: ...

class MuSig2SignRequest(_message.Message):
    __slots__ = ("session_id", "message_digest", "cleanup")
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    CLEANUP_FIELD_NUMBER: _ClassVar[int]
    session_id: bytes
    message_digest: bytes
    cleanup: bool
    def __init__(
        self,
        session_id: _Optional[bytes] = ...,
        message_digest: _Optional[bytes] = ...,
        cleanup: bool = ...,
    ) -> None: ...

class MuSig2SignResponse(_message.Message):
    __slots__ = ("local_partial_signature",)
    LOCAL_PARTIAL_SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    local_partial_signature: bytes
    def __init__(self, local_partial_signature: _Optional[bytes] = ...) -> None: ...

class MuSig2CombineSigRequest(_message.Message):
    __slots__ = ("session_id", "other_partial_signatures")
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    OTHER_PARTIAL_SIGNATURES_FIELD_NUMBER: _ClassVar[int]
    session_id: bytes
    other_partial_signatures: _containers.RepeatedScalarFieldContainer[bytes]
    def __init__(
        self,
        session_id: _Optional[bytes] = ...,
        other_partial_signatures: _Optional[_Iterable[bytes]] = ...,
    ) -> None: ...

class MuSig2CombineSigResponse(_message.Message):
    __slots__ = ("have_all_signatures", "final_signature")
    HAVE_ALL_SIGNATURES_FIELD_NUMBER: _ClassVar[int]
    FINAL_SIGNATURE_FIELD_NUMBER: _ClassVar[int]
    have_all_signatures: bool
    final_signature: bytes
    def __init__(
        self, have_all_signatures: bool = ..., final_signature: _Optional[bytes] = ...
    ) -> None: ...

class MuSig2CleanupRequest(_message.Message):
    __slots__ = ("session_id",)
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    session_id: bytes
    def __init__(self, session_id: _Optional[bytes] = ...) -> None: ...

class MuSig2CleanupResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...
