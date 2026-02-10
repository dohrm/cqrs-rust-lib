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

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
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
}
