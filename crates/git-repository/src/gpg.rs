//! GPG signing delegation.

use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpgFormat {
    OpenPGP,
    X509,
}

pub struct GpgSigner {
    program: String,
    _format: GpgFormat,
    key: Option<String>,
}

pub struct GpgSignature {
    pub signature: Vec<u8>,
}

pub struct GpgVerifyResult {
    pub valid: bool,
    pub key_id: Option<String>,
    pub signer: Option<String>,
}

impl GpgSigner {
    pub fn from_config(config: &git_config::ConfigSet) -> Self {
        let program = config
            .get_string("gpg.program")
            .ok()
            .flatten()
            .unwrap_or_else(|| "gpg".to_string());

        let format = match config.get_string("gpg.format").ok().flatten().as_deref() {
            Some("x509") => GpgFormat::X509,
            _ => GpgFormat::OpenPGP,
        };

        let key = config.get_string("user.signingKey").ok().flatten();

        Self {
            program,
            _format: format,
            key,
        }
    }

    pub fn sign(&self, data: &[u8]) -> Result<GpgSignature, std::io::Error> {
        let mut cmd = Command::new(&self.program);
        cmd.args(["--status-fd=2", "-bsau"]);

        if let Some(ref key) = self.key {
            cmd.arg(key);
        } else {
            return Err(std::io::Error::other(
                "gpg: no signing key configured (set user.signingKey)",
            ));
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(data)?;
        }
        drop(child.stdin.take());

        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(std::io::Error::other(
                "gpg failed to sign the data",
            ));
        }

        Ok(GpgSignature {
            signature: output.stdout,
        })
    }

    pub fn verify(
        &self,
        data: &[u8],
        signature: &[u8],
    ) -> Result<GpgVerifyResult, std::io::Error> {
        // Write signature to a temp file, pass data on stdin
        let sig_file = tempfile::NamedTempFile::new()?;
        std::fs::write(sig_file.path(), signature)?;

        let mut cmd = Command::new(&self.program);
        cmd.args(["--status-fd=1", "--keyid-format", "long", "--verify"]);
        cmd.arg(sig_file.path());
        cmd.arg("-");
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(data)?;
        }
        drop(child.stdin.take());

        let output = child.wait_with_output()?;
        let status_output = String::from_utf8_lossy(&output.stdout);

        let valid = output.status.success();
        let key_id = status_output
            .lines()
            .find(|l| l.contains("GOODSIG") || l.contains("VALIDSIG"))
            .and_then(|l| l.split_whitespace().nth(2))
            .map(|s| s.to_string());

        Ok(GpgVerifyResult {
            valid,
            key_id,
            signer: None,
        })
    }
}
