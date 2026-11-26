//! `@idempotent` (`spec/rules.md` sections 11.8 to 11.12): the store the application implements,
//! the answer it keeps, a memory store for tests, the fingerprint of a request and the key syntax.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::future::Future;
use std::sync::Mutex;

use serde_json::{Map, Value};

use super::error::Error;

/// The message of the issue at the header of the key when it is not a key.
pub const KEY_MESSAGE: &str = "must be 1 to 255 visible ASCII characters";
/// The message of the `409 Conflict` for a key held with other parameters.
pub const OTHER_MESSAGE: &str = "this idempotency key was used with other parameters";
/// The message of the `409 Conflict` for a key whose first request has not finished.
pub const RUNNING_MESSAGE: &str = "a request with this idempotency key is still running";

/// An answer that is kept and sent again: what was sent, as it was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAnswer {
    pub status: u16,
    /// The bytes of the body that were sent. Empty for an answer without a body.
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub location: Option<String>,
}

/// What the store says about a key (`spec/rules.md` section 11.12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Begin {
    /// The key was not known: it is now held, and the request runs. Call
    /// [`IdempotencyStore::complete`] or [`IdempotencyStore::release`] when it ends.
    Started,
    /// The key was used by a request with the same parameters, and its answer is stored.
    Replay(StoredAnswer),
    /// The key is held by a request that has not finished.
    Running,
    /// The key is held with other parameters, running or stored.
    Other,
}

/// Where the keys and their answers live. How long an entry is kept is the store's business.
///
/// `scope` is one opaque text: the operation signature, and, for an endpoint whose keys are
/// scoped to the caller, a line feed and the caller text. Implement it over your database; use
/// [`MemoryIdempotencyStore`] in tests.
pub trait IdempotencyStore: Send + Sync + 'static {
    /// Looks the key up and, when it is unknown, holds it as running. **Atomic**: of two concurrent
    /// calls for a key the store does not know, exactly one gets [`Begin::Started`]. A key held
    /// with another fingerprint is [`Begin::Other`], whether its request is running or its answer
    /// is stored.
    ///
    /// # Errors
    /// When the store cannot be reached. The request is then answered `500` and does not run.
    fn begin<'a>(
        &'a self,
        scope: &'a str,
        key: &'a str,
        fingerprint: &'a str,
    ) -> impl Future<Output = Result<Begin, Error>> + Send + 'a;

    /// Stores the answer of a started request.
    ///
    /// # Errors
    /// When the store cannot be reached. The answer is sent anyway and the error goes to `on_error`.
    fn complete<'a>(
        &'a self,
        scope: &'a str,
        key: &'a str,
        answer: StoredAnswer,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'a;

    /// Removes the entry of a started request (its answer was a `5xx`).
    ///
    /// # Errors
    /// When the store cannot be reached. The answer is sent anyway and the error goes to `on_error`.
    fn release<'a>(&'a self, scope: &'a str, key: &'a str) -> impl Future<Output = Result<(), Error>> + Send + 'a;
}

#[derive(Debug, Clone)]
enum Entry {
    Running(String),
    Stored(String, StoredAnswer),
}

/// An [`IdempotencyStore`] in memory, for tests. It keeps every entry for as long as it lives.
#[derive(Debug, Default)]
pub struct MemoryIdempotencyStore {
    entries: Mutex<BTreeMap<(String, String), Entry>>,
}

impl MemoryIdempotencyStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn table(&self) -> Result<std::sync::MutexGuard<'_, BTreeMap<(String, String), Entry>>, Error> {
        self.entries.lock().map_err(|_| Error::other("the idempotency store is poisoned"))
    }

    /// [`IdempotencyStore::begin`], without the future.
    ///
    /// # Errors
    /// When the table is poisoned.
    pub fn begin_now(&self, scope: &str, key: &str, fingerprint: &str) -> Result<Begin, Error> {
        let mut table = self.table()?;
        let id = (scope.to_string(), key.to_string());
        let found = match table.get(&id) {
            None => None,
            Some(Entry::Running(held)) => Some(if held == fingerprint { Begin::Running } else { Begin::Other }),
            Some(Entry::Stored(held, answer)) => {
                Some(if held == fingerprint { Begin::Replay(answer.clone()) } else { Begin::Other })
            }
        };
        Ok(found.unwrap_or_else(|| {
            table.insert(id, Entry::Running(fingerprint.to_string()));
            Begin::Started
        }))
    }

    /// [`IdempotencyStore::complete`], without the future.
    ///
    /// # Errors
    /// When the table is poisoned.
    pub fn complete_now(&self, scope: &str, key: &str, answer: StoredAnswer) -> Result<(), Error> {
        let mut table = self.table()?;
        let id = (scope.to_string(), key.to_string());
        if let Some(Entry::Running(held)) = table.get(&id).cloned() {
            table.insert(id, Entry::Stored(held, answer));
        }
        Ok(())
    }

    /// [`IdempotencyStore::release`], without the future.
    ///
    /// # Errors
    /// When the table is poisoned.
    pub fn release_now(&self, scope: &str, key: &str) -> Result<(), Error> {
        self.table()?.remove(&(scope.to_string(), key.to_string()));
        Ok(())
    }

    /// Puts an answer into the store as if a request with `fingerprint` had completed. For tests.
    ///
    /// # Errors
    /// When the table is poisoned.
    pub fn insert_stored(&self, scope: &str, key: &str, fingerprint: &str, answer: StoredAnswer) -> Result<(), Error> {
        let entry = Entry::Stored(fingerprint.to_string(), answer);
        self.table()?.insert((scope.to_string(), key.to_string()), entry);
        Ok(())
    }

    /// Puts a running request with `fingerprint` into the store. For tests.
    ///
    /// # Errors
    /// When the table is poisoned.
    pub fn insert_running(&self, scope: &str, key: &str, fingerprint: &str) -> Result<(), Error> {
        let entry = Entry::Running(fingerprint.to_string());
        self.table()?.insert((scope.to_string(), key.to_string()), entry);
        Ok(())
    }
}

impl IdempotencyStore for MemoryIdempotencyStore {
    fn begin<'a>(
        &'a self,
        scope: &'a str,
        key: &'a str,
        fingerprint: &'a str,
    ) -> impl Future<Output = Result<Begin, Error>> + Send + 'a {
        std::future::ready(self.begin_now(scope, key, fingerprint))
    }

    fn complete<'a>(
        &'a self,
        scope: &'a str,
        key: &'a str,
        answer: StoredAnswer,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'a {
        std::future::ready(self.complete_now(scope, key, answer))
    }

    fn release<'a>(&'a self, scope: &'a str, key: &'a str) -> impl Future<Output = Result<(), Error>> + Send + 'a {
        std::future::ready(self.release_now(scope, key))
    }
}

/// Whether `key` can be a key: 1 to 255 visible ASCII characters.
#[must_use]
pub fn is_valid_key(key: &str) -> bool {
    (1..=255).contains(&key.len()) && key.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

/// The fingerprint of a request: the first 16 hex characters of the SHA-256 of the RFC 8785 JSON
/// of the bound parameters in wire form, keyed by parameter name. The key's own header is not a
/// parameter.
#[must_use]
pub fn fingerprint(params: &Map<String, Value>) -> String {
    let text = super::canonical::canonical(&Value::Object(params.clone()));
    let digest = sha256(text.as_bytes());
    let mut out = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

const K: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5,
    0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
    0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
    0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
    0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85,
    0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
    0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
    0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
];

fn compress(h: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];
    for (i, word) in block.chunks_exact(4).enumerate().take(16) {
        w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
        *slot = slot.wrapping_add(v);
    }
}

/// SHA-256 (FIPS 180-4) of `data`.
#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] =
        [0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(u64::try_from(data.len()).unwrap_or(u64::MAX).wrapping_mul(8)).to_be_bytes());
    for block in message.chunks_exact(64) {
        compress(&mut h, block);
    }
    let mut out = [0u8; 32];
    for (chunk, v) in out.chunks_exact_mut(4).zip(h) {
        chunk.copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn decode_b64url(text: &str) -> Option<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).ok()?);
        }
    }
    Some(out)
}

/// The answer a `Veld-Test-Idempotency` header stores (`spec/rules.md` section 11.9): status, the
/// error it names by signature (`null` for a success) and its body in wire form.
fn seeded_answer(
    ep: &super::spec::EndpointSpec,
    etag: Option<&str>,
    state: &Map<String, Value>,
) -> Result<StoredAnswer, Error> {
    let bad = |what: &str| Error::other(format!("the header Veld-Test-Idempotency: {what}"));
    let status = state
        .get("status")
        .and_then(Value::as_u64)
        .and_then(|s| u16::try_from(s).ok())
        .ok_or_else(|| bad("`status` is not a status"))?;
    let data = state.get("data").cloned().unwrap_or(Value::Null);
    let _ = etag;
    match state.get("type").and_then(Value::as_str) {
        Some(sig) => {
            let spec = ep.errors.iter().find(|e| e.sig == sig).ok_or_else(|| bad("`type` is not an error of the endpoint"))?;
            let problem = super::error::Problem {
                sig: spec.sig.to_string(),
                title: spec.name.to_string(),
                status,
                op: Some(ep.sig.to_string()),
                data: spec.schema.to_wire(&spec.schema.from_wire(&data, false), false, false),
                schema: Some(spec.schema),
                // #if rules
                rulings: None,
                // #endif
            };
            Ok(StoredAnswer {
                status,
                body: problem.to_text().into_bytes(),
                content_type: Some("application/problem+json".to_string()),
                etag: None,
                location: None,
            })
        }
        None => {
            let wire = match (ep.success, &data) {
                (Some(schema), value) if !value.is_null() => Some(schema.to_wire(&schema.from_wire(value, false), false, false)),
                _ => None,
            };
            // @if version
            let etag = etag.and_then(|field| wire.as_ref().and_then(|w| super::conditional::etag_of(w, field)));
            // @end
            // @if !version
            let etag: Option<String> = None;
            // @end
            Ok(StoredAnswer {
                status,
                body: wire.map(|w| serde_json::to_vec(&w).unwrap_or_default()).unwrap_or_default(),
                content_type: (!data.is_null()).then_some("application/json".to_string()),
                etag,
                location: None,
            })
        }
    }
}

/// What the memory store of one request of a test handler says about `key`: the store holds what
/// `state` (the value of `Veld-Test-Idempotency`, unpadded base64url JSON) says, under the
/// fingerprint of the request (`other`: under another one). Without the header the store is empty.
///
/// # Errors
/// When the header is not what section 11.9 says.
pub fn seeded_begin(
    ep: &super::spec::EndpointSpec,
    etag: Option<&str>,
    state: Option<&str>,
    (scope, key, fingerprint): (&str, &str, &str),
) -> Result<Begin, Error> {
    let store = MemoryIdempotencyStore::new();
    if let Some(text) = state {
        let bad = |what: &str| Error::other(format!("the header Veld-Test-Idempotency: {what}"));
        let bytes = decode_b64url(text).ok_or_else(|| bad("is not base64url"))?;
        let Value::Object(object) = serde_json::from_slice(&bytes).map_err(|e| bad(&format!("is not JSON: {e}")))? else {
            return Err(bad("is not an object"));
        };
        match object.get("state").and_then(Value::as_str) {
            Some("stored") => store.insert_stored(scope, key, fingerprint, seeded_answer(ep, etag, &object)?)?,
            Some("other") => store.insert_running(scope, key, &format!("other:{fingerprint}"))?,
            Some("running") => store.insert_running(scope, key, fingerprint)?,
            _ => return Err(bad("`state` is not `stored`, `other` or `running`")),
        }
    }
    store.begin_now(scope, key, fingerprint)
}
