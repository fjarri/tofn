use core::{
    array::TryFromSliceError,
    convert::{TryFrom, TryInto},
};
use ecdsa::elliptic_curve::ops::Reduce;
use serde::{Deserialize, Serialize};

/// Sign only 32-byte hash digests
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct MessageDigest([u8; 32]);

impl AsRef<[u8]> for MessageDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<&[u8]> for MessageDigest {
    type Error = TryFromSliceError;
    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self(v.try_into()?))
    }
}

/// Convert a 32-byte hash digest into a scalar as per SEC1:
/// <https://www.secg.org/sec1-v2.pdf< Section 4.1.3 steps 5-6 page 45
///
/// SEC1 specifies to subtract the secp256k1 modulus when the byte array is larger than the modulus.
impl From<&MessageDigest> for k256::Scalar {
    fn from(v: &MessageDigest) -> Self {
        <k256::Scalar as Reduce<k256::U256>>::from_be_bytes_reduced(*k256::FieldBytes::from_slice(
            &v.0[..],
        ))
    }
}
