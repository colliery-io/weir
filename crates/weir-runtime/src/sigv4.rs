//! AWS Signature Version 4 request signing ([[WEIR-T-0109]] / [[WEIR-A-0033]]). The **host** signs
//! each outbound S3/object-store request so the secret key never enters the guest sandbox. Pure +
//! deterministic (the caller supplies the timestamp), so it's verifiable against AWS's published
//! test vectors without any network.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// SHA-256 of an empty body — the payload hash every S3 **GET** carries.
pub const EMPTY_PAYLOAD_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

/// The signer for one (credential, region, service).
pub struct SigV4<'a> {
    pub access_key: &'a str,
    pub secret_key: &'a str,
    pub region: &'a str,
    pub service: &'a str,
}

impl SigV4<'_> {
    /// The `aws4_request` signing key for a date (the HMAC chain over the secret).
    fn signing_key(&self, datestamp: &str) -> Vec<u8> {
        let k_date = hmac(
            format!("AWS4{}", self.secret_key).as_bytes(),
            datestamp.as_bytes(),
        );
        let k_region = hmac(&k_date, self.region.as_bytes());
        let k_service = hmac(&k_region, self.service.as_bytes());
        hmac(&k_service, b"aws4_request")
    }

    /// Build the `Authorization` header value for a request.
    ///
    /// `signed_headers` must be **lowercased names + trimmed values, sorted by name**;
    /// `canonical_query` must be sorted + percent-encoded; `amz_date` is `YYYYMMDDটHHMMSSZ`
    /// and `datestamp` is `YYYYMMDD` (both derived from one instant).
    #[allow(clippy::too_many_arguments)]
    pub fn authorization(
        &self,
        method: &str,
        canonical_uri: &str,
        canonical_query: &str,
        signed_headers: &[(String, String)],
        payload_hash: &str,
        amz_date: &str,
        datestamp: &str,
    ) -> String {
        let canonical_headers: String = signed_headers
            .iter()
            .map(|(k, v)| format!("{k}:{v}\n"))
            .collect();
        let signed_names = signed_headers
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let canonical_request = format!(
            "{method}\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_names}\n{payload_hash}"
        );

        let scope = format!("{datestamp}/{}/{}/aws4_request", self.region, self.service);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );
        let signature = hex::encode(hmac(
            &self.signing_key(datestamp),
            string_to_sign.as_bytes(),
        ));
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_names}, Signature={signature}",
            self.access_key
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_hash_matches_the_known_constant() {
        assert_eq!(sha256_hex(b""), EMPTY_PAYLOAD_SHA256);
    }

    // AWS's published worked example ("Examples of a signed AWS API request"): GET
    // https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08 at 20150830T123600Z.
    // Expected signature is fixed by AWS — proves the whole canonical-request → STS → HMAC chain.
    #[test]
    fn matches_aws_published_test_vector() {
        let signer = SigV4 {
            access_key: "AKIDEXAMPLE",
            secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            region: "us-east-1",
            service: "iam",
        };
        let signed = vec![
            (
                "content-type".to_string(),
                "application/x-www-form-urlencoded; charset=utf-8".to_string(),
            ),
            ("host".to_string(), "iam.amazonaws.com".to_string()),
            ("x-amz-date".to_string(), "20150830T123600Z".to_string()),
        ];
        let auth = signer.authorization(
            "GET",
            "/",
            "Action=ListUsers&Version=2010-05-08",
            &signed,
            EMPTY_PAYLOAD_SHA256,
            "20150830T123600Z",
            "20150830",
        );
        assert!(
            auth.ends_with(
                "Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7"
            ),
            "SigV4 authorization did not match the AWS test vector: {auth}"
        );
        assert!(auth.contains("Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request"));
        assert!(auth.contains("SignedHeaders=content-type;host;x-amz-date"));
    }
}
