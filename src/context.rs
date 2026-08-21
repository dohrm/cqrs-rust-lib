use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct CqrsContext {
    current_user: Option<String>,
    metadata: Option<serde_json::Value>,
    request_id: String,
    now: DateTime<Utc>,
    rand_bytes: Option<[u8; 16]>,
}

impl CqrsContext {
    pub fn new(current_user: Option<String>) -> Self {
        Self {
            current_user,
            metadata: None,
            request_id: "".to_string(),
            now: Utc::now(),
            rand_bytes: None,
        }
    }

    pub fn current_user(&self) -> String {
        self.current_user.clone().unwrap_or("anonymous".to_string())
    }

    pub fn request_id(&self) -> String {
        self.request_id.clone()
    }

    pub fn with_next_request_id(self) -> Self {
        Self {
            request_id: self.next_uuid(),
            ..self
        }
    }

    pub fn with_request_id(self, request_id: String) -> Self {
        Self { request_id, ..self }
    }

    /// Replaces the whole metadata bag.
    ///
    /// Two callers each setting one key with this method means the second erases the
    /// first. Use [`with_metadata_entry`](Self::with_metadata_entry) to add a key
    /// without discarding what is already there.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Adds or replaces a single metadata key, keeping every other key.
    ///
    /// Reads are keyed — [`metadata`](Self::metadata) takes an `Option<&str>` — so
    /// writes should be too. When the bag is absent, or holds something that is not a
    /// JSON object, it is replaced by a fresh object holding just this entry: a scalar
    /// has no key to preserve — the discard is logged at `warn`.
    ///
    /// The bag is visible to command handlers through [`metadata`](Self::metadata). It
    /// is not persisted on the event envelope: `CQRSWriteRouter` forwards only
    /// `user_id` and `request_id`.
    ///
    /// ```rust
    /// use cqrs_rust_lib::CqrsContext;
    /// use serde_json::json;
    ///
    /// let context = CqrsContext::default()
    ///     .with_metadata_entry("tenant_id", json!("acme"))
    ///     .with_metadata_entry("locale", json!("fr-CH"));
    ///
    /// assert_eq!(context.metadata(Some("tenant_id")), Some(json!("acme")));
    /// assert_eq!(context.metadata(Some("locale")), Some(json!("fr-CH")));
    /// ```
    pub fn with_metadata_entry(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        let key_name = key.into();
        let mut bag = match self.metadata.take() {
            Some(serde_json::Value::Object(bag)) => bag,
            Some(other) => {
                tracing::warn!(
                    discarded = %other,
                    key = %key_name,
                    "context metadata was not a JSON object; replacing it"
                );
                serde_json::Map::new()
            }
            None => serde_json::Map::new(),
        };
        bag.insert(key_name, value);
        self.metadata = Some(serde_json::Value::Object(bag));
        self
    }

    pub fn metadata(&self, key: Option<&str>) -> Option<serde_json::Value> {
        if let Some(key) = key {
            self.metadata.as_ref().and_then(|v| v.get(key).cloned())
        } else {
            self.metadata.clone()
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.now
    }

    /// # with_rand_bytes
    ///
    /// ⚠️ **WARNING: FOR TESTING PURPOSES ONLY** ⚠️
    ///
    /// This function overrides the default random bytes generation used for UUID creation. It should be used
    /// exclusively in testing environments as it breaks the uniqueness guarantee of UUID generation.
    ///
    /// ## Purpose
    /// - Allows deterministic UUID generation for testing scenarios
    /// - Enables predictable test outcomes
    /// - Facilitates testing of UUID-dependent code paths
    ///
    /// ## Usage Restrictions
    /// - **DO NOT USE IN PRODUCTION CODE**
    /// - Only use in test modules and test environments
    /// - Should never be included in release builds
    ///
    /// ## Example Usage
    /// ```rust
    /// use cqrs_rust_lib::CqrsContext;
    ///
    /// let context = CqrsContext::default().with_rand_bytes([0; 16]);
    /// // Will always generate: "00000000-0000-4000-8000-000000000000"
    /// let uuid = context.next_uuid();
    /// ```
    ///
    /// ## Technical Details
    /// - Replaces the cryptographically secure random number generator
    /// - Uses a fixed byte array instead of random generation
    /// - Generates UUIDs in a deterministic manner
    ///
    /// ## Side Effects
    /// - Breaks UUID v4 specification compliance
    /// - Removes randomness from UUID generation
    /// - May produce duplicate UUIDs if used in concurrent contexts
    ///
    /// ## Best Practices
    /// 1. Always wrap usage with `#[cfg(test)]`
    /// 2. Document test cases using this function clearly
    /// 3. Consider using different byte patterns for different test cases
    /// 4. Reset the context after testing if necessary
    ///
    /// ## Security Considerations
    /// Using this function in production would severely compromise the security and
    /// uniqueness guarantees of UUID generation. It could lead to:
    /// - Duplicate UUIDs
    /// - Predictable identifier patterns
    /// - Potential security vulnerabilities
    ///
    /// ## Parameters
    /// - `bytes`: Fixed array of 16 bytes to use for UUID generation
    ///
    /// ## Returns
    /// - A new Context instance with overridden random byte generation
    ///
    /// ## Related
    /// - `Context::new()`
    /// - `Context::next_uuid()`
    pub fn with_rand_bytes(mut self, bytes: [u8; 16]) -> Self {
        self.rand_bytes = Some(bytes);
        self
    }
    pub fn next_uuid(&self) -> String {
        let bytes = if let Some(b) = self.rand_bytes {
            b
        } else {
            rand::random::<[u8; 16]>()
        };
        uuid::Builder::from_random_bytes(bytes)
            .as_uuid()
            .to_string()
    }
}

impl Default for CqrsContext {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_next_uuid() {
        let context = CqrsContext::default();
        let uuid = context.next_uuid();
        assert_eq!(uuid.len(), 36);
    }

    #[test]
    fn test_next_uuid_with_rand_bytes() {
        let context = CqrsContext::default().with_rand_bytes([0; 16]);
        let uuid = context.next_uuid();
        assert_eq!(uuid, "00000000-0000-4000-8000-000000000000".to_string());
    }

    /// Two middlewares each writing one key, in whichever order they run: both
    /// entries must survive.
    #[test]
    fn test_metadata_entries_accumulate() {
        let context = CqrsContext::default()
            .with_metadata_entry("tenant_id", json!("acme"))
            .with_metadata_entry("locale", json!("fr-CH"));

        assert_eq!(context.metadata(Some("tenant_id")), Some(json!("acme")));
        assert_eq!(context.metadata(Some("locale")), Some(json!("fr-CH")));
        assert_eq!(
            context.metadata(None),
            Some(json!({"tenant_id": "acme", "locale": "fr-CH"}))
        );
    }

    #[test]
    fn test_metadata_entry_preserves_an_existing_bag() {
        let context = CqrsContext::default()
            .with_metadata(json!({"tenant_id": "acme", "trace_id": "abc"}))
            .with_metadata_entry("locale", json!("fr-CH"));

        assert_eq!(
            context.metadata(None),
            Some(json!({"tenant_id": "acme", "trace_id": "abc", "locale": "fr-CH"}))
        );
    }

    #[test]
    fn test_metadata_entry_overwrites_only_its_own_key() {
        let context = CqrsContext::default()
            .with_metadata(json!({"tenant_id": "acme", "locale": "en-GB"}))
            .with_metadata_entry("locale", json!("fr-CH"));

        assert_eq!(
            context.metadata(None),
            Some(json!({"tenant_id": "acme", "locale": "fr-CH"}))
        );
    }

    /// `with_metadata` still replaces — that is the distinction the two methods draw,
    /// and it is what makes the choice visible at the call site.
    #[test]
    fn test_with_metadata_still_replaces_the_whole_bag() {
        let context = CqrsContext::default()
            .with_metadata_entry("tenant_id", json!("acme"))
            .with_metadata(json!({"locale": "fr-CH"}));

        assert_eq!(context.metadata(Some("tenant_id")), None);
        assert_eq!(context.metadata(None), Some(json!({"locale": "fr-CH"})));
    }

    #[test]
    fn test_metadata_entry_on_a_non_object_bag_starts_clean() {
        for bag in [json!("a scalar"), json!(["an", "array"]), json!(null)] {
            let context = CqrsContext::default()
                .with_metadata(bag.clone())
                .with_metadata_entry("locale", json!("fr-CH"));

            assert_eq!(
                context.metadata(None),
                Some(json!({"locale": "fr-CH"})),
                "a {bag} bag has no key to preserve"
            );
        }
    }

    #[test]
    fn test_metadata_entry_on_an_absent_bag_starts_clean() {
        let context = CqrsContext::default();
        assert_eq!(context.metadata(None), None, "precondition: no bag yet");

        let context = context.with_metadata_entry("locale", json!("fr-CH"));
        assert_eq!(context.metadata(None), Some(json!({"locale": "fr-CH"})));
    }
}
