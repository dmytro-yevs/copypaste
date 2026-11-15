//! Hash transforms over the exact UTF-8 bytes of the text, with no newline
//! added. A hash is not marked sensitive by itself; the runtime passes on
//! the source's sensitivity, since hashes of short secrets can be reversed.

use ::base64::engine::general_purpose::STANDARD;
use ::base64::Engine;
use md5::Md5;
use sha1::Sha1;
use sha2::Digest;
use sha2::{Sha256, Sha512};

use crate::catalogue::{Catalogue, Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};

pub(crate) fn register(catalogue: &mut Catalogue) {
    catalogue.add(&MD5);
    catalogue.add(&SHA1);
    catalogue.add(&SHA256);
    catalogue.add(&SHA512);
    catalogue.add(&CRC32);
}

static DIGEST_PARAMS: [ParamSpec; 1] = [ParamSpec::choice(
    "output",
    &["hex_lower", "hex_upper", "base64"],
    "hex_lower",
)];

static CRC32_PARAMS: [ParamSpec; 1] = [ParamSpec::choice("output", &["hex", "decimal"], "hex")];

const fn digest(
    id: &'static str,
    modifier: &'static str,
    example: &'static str,
    run: crate::catalogue::Runner,
) -> TransformDef {
    TransformDef {
        id,
        version: 1,
        category: Category::Hash,
        params: &DIGEST_PARAMS,
        flags: Flags::PLAIN,
        modifier: Some(modifier),
        example: ("abc", example),
        run,
    }
}

static MD5: TransformDef = digest(
    "hash.md5",
    "md5",
    "900150983cd24fb0d6963f7d28e17f72",
    hash::<Md5>,
);

static SHA1: TransformDef = digest(
    "hash.sha1",
    "sha1",
    "a9993e364706816aba3e25717850c26c9cd0d89d",
    hash::<Sha1>,
);

static SHA256: TransformDef = digest(
    "hash.sha256",
    "sha256",
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    hash::<Sha256>,
);

static SHA512: TransformDef = digest(
    "hash.sha512",
    "sha512",
    "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
     2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
    hash::<Sha512>,
);

static CRC32: TransformDef = TransformDef {
    id: "hash.crc32",
    version: 1,
    category: Category::Hash,
    params: &CRC32_PARAMS,
    flags: Flags::PLAIN,
    modifier: Some("crc32"),
    example: ("abc", "352441c2"),
    run: run_crc32,
};

/// Bytes hashed between cancellation checks.
const CHUNK: usize = 64 * 1024;

fn hash<D: Digest>(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut hasher = D::new();
    for chunk in input.as_bytes().chunks(CHUNK) {
        ctx.check()?;
        hasher.update(chunk);
    }
    let bytes = hasher.finalize();
    let text = match params.choice("output") {
        "base64" => STANDARD.encode(bytes),
        "hex_upper" => hex::encode_upper(bytes),
        _ => hex::encode(bytes),
    };
    Ok(Output::new(text))
}

/// IEEE CRC-32, the variant used by zip and PNG.
fn run_crc32(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut hasher = crc32fast::Hasher::new();
    for chunk in input.as_bytes().chunks(CHUNK) {
        ctx.check()?;
        hasher.update(chunk);
    }
    let value = hasher.finalize();
    let text = match params.choice("output") {
        "decimal" => value.to_string(),
        _ => format!("{value:08x}"),
    };
    Ok(Output::new(text))
}
