//! Capability parsing and negotiation for git protocol.
//!
//! Git servers advertise capabilities in the first line of ref advertisement.
//! In v1, capabilities are appended after a NUL byte on the first ref line.
//! In v2, capabilities are sent as a separate section.

/// Parsed set of server capabilities.
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    /// Raw capability strings.
    entries: Vec<CapabilityEntry>,
}

/// A single capability, optionally with a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEntry {
    pub name: String,
    pub value: Option<String>,
}

impl Capabilities {
    /// Parse capabilities from a NUL-separated capability string (v1 format).
    ///
    /// In v1, the first ref line looks like:
    /// `<oid> HEAD\0multi_ack thin-pack side-band-64k ofs-delta symref=HEAD:refs/heads/main agent=git/2.39.0`
    pub fn parse_v1(caps_str: &str) -> Self {
        let mut entries = Vec::new();
        for cap in caps_str.split_whitespace() {
            if let Some(eq_pos) = cap.find('=') {
                entries.push(CapabilityEntry {
                    name: cap[..eq_pos].to_string(),
                    value: Some(cap[eq_pos + 1..].to_string()),
                });
            } else {
                entries.push(CapabilityEntry {
                    name: cap.to_string(),
                    value: None,
                });
            }
        }
        Self { entries }
    }

    /// Parse capabilities from v2 capability advertisement lines.
    ///
    /// Each line is a capability, optionally with `=value`.
    pub fn parse_v2(lines: &[Vec<u8>]) -> Self {
        let mut entries = Vec::new();
        for line in lines {
            let s = String::from_utf8_lossy(line);
            let s = s.trim_end_matches('\n');
            if let Some(eq_pos) = s.find('=') {
                entries.push(CapabilityEntry {
                    name: s[..eq_pos].to_string(),
                    value: Some(s[eq_pos + 1..].to_string()),
                });
            } else {
                entries.push(CapabilityEntry {
                    name: s.to_string(),
                    value: None,
                });
            }
        }
        Self { entries }
    }

    /// Check if a capability is advertised.
    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.name == name)
    }

    /// Get the value of a capability (e.g., `symref=HEAD:refs/heads/main`).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .and_then(|e| e.value.as_deref())
    }

    /// Get all values for a capability that may appear multiple times.
    pub fn get_all(&self, name: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|e| e.name == name)
            .filter_map(|e| e.value.as_deref())
            .collect()
    }

    /// Get all capability entries.
    pub fn entries(&self) -> &[CapabilityEntry] {
        &self.entries
    }

}

impl std::fmt::Display for Capabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for e in &self.entries {
            if !first {
                write!(f, " ")?;
            }
            first = false;
            if let Some(ref v) = e.value {
                write!(f, "{}={}", e.name, v)?;
            } else {
                write!(f, "{}", e.name)?;
            }
        }
        Ok(())
    }
}

/// Select the best sideband mode from server capabilities.
pub fn select_sideband(caps: &Capabilities) -> SidebandMode {
    if caps.has("side-band-64k") {
        SidebandMode::Band64k
    } else if caps.has("side-band") {
        SidebandMode::Band
    } else {
        SidebandMode::None
    }
}

/// Sideband mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebandMode {
    /// No sideband — raw pack data.
    None,
    /// side-band (1000 byte max per packet).
    Band,
    /// side-band-64k (65516 byte max per packet).
    Band64k,
}

/// Negotiate capabilities for fetch (client side).
///
/// Given the server's capabilities, produce the list of capabilities
/// the client wants to request.
pub fn negotiate_fetch_capabilities(server: &Capabilities) -> Vec<String> {
    let mut client = Vec::new();

    // Prefer side-band-64k, fall back to side-band
    if server.has("side-band-64k") {
        client.push("side-band-64k".into());
    } else if server.has("side-band") {
        client.push("side-band".into());
    }

    if server.has("ofs-delta") {
        client.push("ofs-delta".into());
    }

    if server.has("thin-pack") {
        client.push("thin-pack".into());
    }

    // Don't negotiate multi_ack or no-done — keep simple protocol
    // to avoid complex ACK/NAK state machine handling.

    if server.has("include-tag") {
        client.push("include-tag".into());
    }

    // Always send agent
    client.push("agent=gitr/0.1".into());

    client
}

/// Negotiate capabilities for push (client side).
pub fn negotiate_push_capabilities(server: &Capabilities) -> Vec<String> {
    let mut client = Vec::new();

    if server.has("report-status") {
        client.push("report-status".into());
    }

    if server.has("ofs-delta") {
        client.push("ofs-delta".into());
    }

    if server.has("side-band-64k") {
        client.push("side-band-64k".into());
    }

    // agent
    client.push("agent=gitr/0.1".into());

    client
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_v1_capabilities() {
        let caps = Capabilities::parse_v1(
            "multi_ack thin-pack side-band side-band-64k ofs-delta symref=HEAD:refs/heads/main agent=git/2.39.0"
        );
        assert!(caps.has("multi_ack"));
        assert!(caps.has("thin-pack"));
        assert!(caps.has("side-band-64k"));
        assert!(caps.has("ofs-delta"));
        assert_eq!(caps.get("symref"), Some("HEAD:refs/heads/main"));
        assert_eq!(caps.get("agent"), Some("git/2.39.0"));
        assert!(!caps.has("nonexistent"));
    }

    #[test]
    fn parse_v2_capabilities() {
        let lines = vec![
            b"agent=git/2.39.0\n".to_vec(),
            b"ls-refs\n".to_vec(),
            b"fetch=shallow wait-for-done filter\n".to_vec(),
        ];
        let caps = Capabilities::parse_v2(&lines);
        assert_eq!(caps.get("agent"), Some("git/2.39.0"));
        assert!(caps.has("ls-refs"));
        assert_eq!(caps.get("fetch"), Some("shallow wait-for-done filter"));
    }

    #[test]
    fn select_sideband_prefers_64k() {
        let caps = Capabilities::parse_v1("side-band side-band-64k");
        assert_eq!(select_sideband(&caps), SidebandMode::Band64k);
    }

    #[test]
    fn select_sideband_falls_back() {
        let caps = Capabilities::parse_v1("side-band");
        assert_eq!(select_sideband(&caps), SidebandMode::Band);
    }

    #[test]
    fn select_no_sideband() {
        let caps = Capabilities::parse_v1("thin-pack");
        assert_eq!(select_sideband(&caps), SidebandMode::None);
    }

    #[test]
    fn negotiate_fetch_caps() {
        let server = Capabilities::parse_v1(
            "multi_ack_detailed thin-pack side-band-64k ofs-delta no-done include-tag"
        );
        let client = negotiate_fetch_capabilities(&server);
        assert!(client.contains(&"side-band-64k".to_string()));
        assert!(client.contains(&"ofs-delta".to_string()));
        assert!(client.contains(&"include-tag".to_string()));
        // multi_ack and no-done are intentionally not negotiated to keep simple protocol
        assert!(!client.contains(&"multi_ack_detailed".to_string()));
        assert!(!client.contains(&"no-done".to_string()));
    }
}
