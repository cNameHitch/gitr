//! Git URL parsing.
//!
//! Supports all git URL formats:
//! - ssh://[user@]host[:port]/path
//! - git://host[:port]/path
//! - http[s]://[user@]host[:port]/path
//! - file:///path
//! - /local/path
//! - user@host:path (SCP-like SSH)

use crate::{GitUrl, Scheme, TransportError};

impl GitUrl {
    /// Parse a git URL string into a GitUrl.
    ///
    /// Handles all standard git URL formats, including the SCP-like SSH syntax
    /// (`user@host:path`) which has no explicit scheme.
    pub fn parse(url: &str) -> Result<Self, TransportError> {
        let url = url.trim();
        if url.is_empty() {
            return Err(TransportError::InvalidUrl("empty URL".into()));
        }

        // Check for scheme://... format
        if let Some(rest) = url.strip_prefix("ssh://") {
            return parse_standard(Scheme::Ssh, rest);
        }
        if let Some(rest) = url.strip_prefix("git://") {
            return parse_standard(Scheme::Git, rest);
        }
        if let Some(rest) = url.strip_prefix("http://") {
            return parse_standard(Scheme::Http, rest);
        }
        if let Some(rest) = url.strip_prefix("https://") {
            return parse_standard(Scheme::Https, rest);
        }
        if let Some(rest) = url.strip_prefix("file://") {
            return Ok(GitUrl {
                scheme: Scheme::File,
                host: None,
                port: None,
                user: None,
                path: rest.to_string(),
            });
        }

        // Check for absolute local paths (Unix or Windows)
        if url.starts_with('/')
            || url.starts_with('.')
            || (url.len() >= 2 && url.as_bytes()[1] == b':')
        {
            return Ok(GitUrl {
                scheme: Scheme::Local,
                host: None,
                port: None,
                user: None,
                path: url.to_string(),
            });
        }

        // SCP-like syntax: [user@]host:path
        // Must not be confused with Windows drive letters (C:\path)
        if let Some(colon_pos) = url.find(':') {
            // Windows drive letter check: single letter before colon
            if colon_pos == 1 && url.as_bytes()[0].is_ascii_alphabetic() {
                return Ok(GitUrl {
                    scheme: Scheme::Local,
                    host: None,
                    port: None,
                    user: None,
                    path: url.to_string(),
                });
            }

            let host_part = &url[..colon_pos];
            let path = &url[colon_pos + 1..];

            // Parse user@host
            let (user, host) = if let Some(at_pos) = host_part.find('@') {
                let user = &host_part[..at_pos];
                let host = &host_part[at_pos + 1..];
                (Some(user.to_string()), host.to_string())
            } else {
                (None, host_part.to_string())
            };

            if host.is_empty() {
                return Err(TransportError::InvalidUrl(
                    format!("empty host in SCP-like URL: {}", url),
                ));
            }

            return Ok(GitUrl {
                scheme: Scheme::Ssh,
                host: Some(host),
                port: None,
                user,
                path: path.to_string(),
            });
        }

        Err(TransportError::InvalidUrl(format!(
            "could not parse URL: {}",
            url
        )))
    }
}

/// Parse a URL with scheme already stripped: [user@]host[:port]/path
fn parse_standard(scheme: Scheme, rest: &str) -> Result<GitUrl, TransportError> {
    // Split into host+port and path at the first /
    let (authority, path) = if let Some(slash_pos) = rest.find('/') {
        (&rest[..slash_pos], &rest[slash_pos..])
    } else {
        (rest, "/")
    };

    // Parse user@host:port from authority
    let (user, host_port) = if let Some(at_pos) = authority.find('@') {
        let user = &authority[..at_pos];
        let host_port = &authority[at_pos + 1..];
        (Some(user.to_string()), host_port)
    } else {
        (None, authority)
    };

    // Parse host:port
    // Handle IPv6: [host]:port
    let (host, port) = if host_port.starts_with('[') {
        // IPv6 address
        if let Some(bracket_end) = host_port.find(']') {
            let host = &host_port[1..bracket_end];
            let after_bracket = &host_port[bracket_end + 1..];
            let port = if let Some(port_str) = after_bracket.strip_prefix(':') {
                Some(port_str.parse::<u16>().map_err(|_| {
                    TransportError::InvalidUrl(format!("invalid port: {}", port_str))
                })?)
            } else {
                None
            };
            (host.to_string(), port)
        } else {
            return Err(TransportError::InvalidUrl(
                "unclosed IPv6 bracket".into(),
            ));
        }
    } else if let Some(colon_pos) = host_port.rfind(':') {
        let host = &host_port[..colon_pos];
        let port_str = &host_port[colon_pos + 1..];
        let port = port_str.parse::<u16>().map_err(|_| {
            TransportError::InvalidUrl(format!("invalid port: {}", port_str))
        })?;
        (host.to_string(), Some(port))
    } else {
        (host_port.to_string(), None)
    };

    if host.is_empty() {
        return Err(TransportError::InvalidUrl("empty host".into()));
    }

    Ok(GitUrl {
        scheme,
        host: Some(host),
        port,
        user,
        path: path.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ssh_url() {
        let url = GitUrl::parse("ssh://git@github.com/user/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Ssh);
        assert_eq!(url.host.as_deref(), Some("github.com"));
        assert_eq!(url.user.as_deref(), Some("git"));
        assert_eq!(url.path, "/user/repo.git");
        assert_eq!(url.port, None);
    }

    #[test]
    fn parse_ssh_url_with_port() {
        let url = GitUrl::parse("ssh://git@github.com:2222/user/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Ssh);
        assert_eq!(url.port, Some(2222));
    }

    #[test]
    fn parse_scp_like_url() {
        let url = GitUrl::parse("git@github.com:user/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Ssh);
        assert_eq!(url.host.as_deref(), Some("github.com"));
        assert_eq!(url.user.as_deref(), Some("git"));
        assert_eq!(url.path, "user/repo.git");
    }

    #[test]
    fn parse_scp_like_no_user() {
        let url = GitUrl::parse("github.com:user/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Ssh);
        assert_eq!(url.host.as_deref(), Some("github.com"));
        assert_eq!(url.user, None);
        assert_eq!(url.path, "user/repo.git");
    }

    #[test]
    fn parse_https_url() {
        let url = GitUrl::parse("https://github.com/user/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Https);
        assert_eq!(url.host.as_deref(), Some("github.com"));
        assert_eq!(url.path, "/user/repo.git");
    }

    #[test]
    fn parse_http_url() {
        let url = GitUrl::parse("http://example.com/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Http);
        assert_eq!(url.host.as_deref(), Some("example.com"));
    }

    #[test]
    fn parse_git_url() {
        let url = GitUrl::parse("git://example.com/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Git);
        assert_eq!(url.host.as_deref(), Some("example.com"));
    }

    #[test]
    fn parse_file_url() {
        let url = GitUrl::parse("file:///tmp/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::File);
        assert_eq!(url.path, "/tmp/repo.git");
    }

    #[test]
    fn parse_local_absolute_path() {
        let url = GitUrl::parse("/tmp/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Local);
        assert_eq!(url.path, "/tmp/repo.git");
    }

    #[test]
    fn parse_local_relative_path() {
        let url = GitUrl::parse("./repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Local);
        assert_eq!(url.path, "./repo.git");
    }

    #[test]
    fn parse_https_with_port() {
        let url = GitUrl::parse("https://example.com:8443/repo.git").unwrap();
        assert_eq!(url.scheme, Scheme::Https);
        assert_eq!(url.port, Some(8443));
    }

    #[test]
    fn parse_https_with_user() {
        let url = GitUrl::parse("https://user@example.com/repo.git").unwrap();
        assert_eq!(url.user.as_deref(), Some("user"));
    }

    #[test]
    fn parse_empty_url_fails() {
        assert!(GitUrl::parse("").is_err());
    }

    #[test]
    fn display_scp_like() {
        let url = GitUrl {
            scheme: Scheme::Ssh,
            host: Some("github.com".into()),
            port: None,
            user: Some("git".into()),
            path: "user/repo.git".into(),
        };
        assert_eq!(url.to_string(), "git@github.com:user/repo.git");
    }

    #[test]
    fn display_https() {
        let url = GitUrl {
            scheme: Scheme::Https,
            host: Some("github.com".into()),
            port: None,
            user: None,
            path: "/user/repo.git".into(),
        };
        assert_eq!(url.to_string(), "https://github.com/user/repo.git");
    }
}
