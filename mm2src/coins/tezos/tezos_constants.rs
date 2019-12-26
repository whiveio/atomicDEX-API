/// Base58 encoding prefixes
/// https://gitlab.com/tezos/tezos/blob/master/src/lib_crypto/base58.ml#L347
pub const BLOCK_HASH_PREFIX: [u8; 2] = [1, 52];
pub const ED_SIG_PREFIX: [u8; 5] = [9, 245, 205, 134, 18];
pub const ED_SK_PREFIX: [u8; 4] = [13, 15, 58, 7];
pub const ED_PK_PREFIX: [u8; 4] = [13, 15, 37, 217];
pub const SECP_PK_PREFIX: [u8; 4] = [3, 254, 226, 86];
pub const P256_PK_PREFIX: [u8; 4] = [3, 178, 139, 127];
/// ed25519_public_key_hash, https://gitlab.com/tezos/tezos/blob/master/src/lib_crypto/base58.ml#L362
pub const TZ1_ADDR_PREFIX: [u8; 3] = [6, 161, 159];
/// secp256k1_public_key_hash, https://gitlab.com/tezos/tezos/blob/master/src/lib_crypto/base58.ml#L364
pub const TZ2_ADDR_PREFIX: [u8; 3] = [6, 161, 161];
/// p256_public_key_hash, https://gitlab.com/tezos/tezos/blob/master/src/lib_crypto/base58.ml#L366
pub const TZ3_ADDR_PREFIX: [u8; 3] = [6, 161, 164];