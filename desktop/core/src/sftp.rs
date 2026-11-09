//! SFTP upload. The server's host key must match the fingerprint the user trusted once
//! (trust on first use); a changed key stops every connection.

use crate::config::{AuthKind, SftpConfig};
use crate::paths::expand_home;
use base64::Engine;
use sha2::{Digest, Sha256};
use ssh2::{MethodType, OpenFlags, OpenType, RenameFlags, Session, Sftp};
use std::fmt;
use std::fs;
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub enum ConnectError {
    /// First connection: the user must confirm this fingerprint.
    UnknownHostKey(String),
    /// The server presents a different key than the trusted one.
    HostKeyChanged { trusted: String, presented: String },
    Other(String),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectError::UnknownHostKey(fp) => write!(f, "Unbekannter Server-Schlüssel {fp}"),
            ConnectError::HostKeyChanged { trusted, presented } => write!(
                f,
                "Der Server-Schlüssel hat sich geändert (vertraut: {trusted}, jetzt: {presented}). Verbindung abgebrochen."
            ),
            ConnectError::Other(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ConnectError {}

fn other(context: &str, e: impl fmt::Display) -> ConnectError {
    ConnectError::Other(format!("{context}: {e}"))
}

pub struct Connection {
    _session: Session,
    sftp: Sftp,
    root: String,
}

/// `SHA256:<base64 without padding>`, as `ssh-keygen -lf` shows it.
fn fingerprint(key: &[u8]) -> String {
    format!("SHA256:{}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(Sha256::digest(key)))
}

/// Host key algorithms in the order the libssh2 bundled with libssh2-sys 0.3.3 used by default.
/// Newer libssh2 prefers Ed25519 over ECDSA, so a server holding both would present a
/// different key and every stored `host_key` fingerprint would read as "changed".
/// Pinning the old order keeps existing fingerprints valid. SHA-1 `ssh-rsa` and `ssh-dss`
/// are left out: the bundled libssh2 no longer supports them.
const HOSTKEY_PREF: &str = "ecdsa-sha2-nistp256,ecdsa-sha2-nistp384,ecdsa-sha2-nistp521,\
ecdsa-sha2-nistp256-cert-v01@openssh.com,ecdsa-sha2-nistp384-cert-v01@openssh.com,\
ecdsa-sha2-nistp521-cert-v01@openssh.com,\
ssh-ed25519,ssh-ed25519-cert-v01@openssh.com,\
rsa-sha2-512,rsa-sha2-256,\
rsa-sha2-512-cert-v01@openssh.com,rsa-sha2-256-cert-v01@openssh.com";

fn new_session() -> Result<Session, ConnectError> {
    let session = Session::new().map_err(|e| other("SSH", e))?;
    session
        .method_pref(MethodType::HostKey, HOSTKEY_PREF)
        .map_err(|e| other("SSH-Host-Key-Algorithmen", e))?;
    Ok(session)
}

/// `LIBSSH2_ERROR_KEX_FAILURE`: client and server share no key exchange, cipher or host key method.
const KEX_FAILURE: ssh2::ErrorCode = ssh2::ErrorCode::Session(-5);

/// A failed key exchange most often means a very old server that only offers SHA-1 `ssh-rsa`
/// or `ssh-dss` host keys, which the bundled libssh2 no longer supports. Say so instead of
/// only passing on libssh2's generic message.
fn handshake_error(e: ssh2::Error) -> ConnectError {
    if e.code() == KEX_FAILURE {
        ConnectError::Other(format!(
            "SSH-Handshake: {e}. Server und App haben keine gemeinsamen Verfahren für den Schlüsselaustausch. \
Sehr alte Server, die nur veraltete Host-Keys (SHA-1 ssh-rsa oder ssh-dss) anbieten, werden nicht mehr unterstützt."
        ))
    } else {
        other("SSH-Handshake", e)
    }
}

pub fn connect(cfg: &SftpConfig, secret: Option<&str>) -> Result<Connection, ConnectError> {
    let host = cfg.host.trim();
    let addr = (host, cfg.port)
        .to_socket_addrs()
        .map_err(|e| other(&format!("{host} nicht gefunden"), e))?
        .next()
        .ok_or_else(|| ConnectError::Other(format!("{host} nicht gefunden")))?;
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(15)).map_err(|e| other(&format!("Verbindung zu {host}:{}", cfg.port), e))?;
    let mut session = new_session()?;
    session.set_tcp_stream(tcp);
    session.set_timeout(60_000);
    session.handshake().map_err(handshake_error)?;

    let (key, _) = session.host_key().ok_or_else(|| ConnectError::Other("Server liefert keinen Host-Key".into()))?;
    let presented = fingerprint(key);
    if cfg.host_key.is_empty() {
        return Err(ConnectError::UnknownHostKey(presented));
    }
    if cfg.host_key != presented {
        return Err(ConnectError::HostKeyChanged { trusted: cfg.host_key.clone(), presented });
    }

    let user = cfg.user.trim();
    match cfg.auth {
        AuthKind::Password => {
            let password = secret.ok_or_else(|| ConnectError::Other("Kein Passwort gespeichert".into()))?;
            session.userauth_password(user, password).map_err(|e| other("Anmeldung fehlgeschlagen", e))?;
        }
        AuthKind::Key => {
            let key_path = expand_home(cfg.key_path.trim());
            session
                .userauth_pubkey_file(user, None, &key_path, secret.filter(|s| !s.is_empty()))
                .map_err(|e| other("Anmeldung mit Schlüssel fehlgeschlagen", e))?;
        }
    }
    if !session.authenticated() {
        return Err(ConnectError::Other("Anmeldung fehlgeschlagen".into()));
    }
    let sftp = session.sftp().map_err(|e| other("SFTP nicht verfügbar", e))?;
    let root = cfg.remote_dir.trim().trim_end_matches('/').to_owned();
    Ok(Connection { _session: session, sftp, root })
}

impl Connection {
    fn remote(&self, relative: &str) -> String {
        if self.root.is_empty() {
            relative.to_owned()
        } else {
            format!("{}/{}", self.root, relative)
        }
    }

    /// Names in the target folder (for the precheck), sorted; folders end with `/`.
    pub fn list_root(&self) -> anyhow::Result<Vec<String>> {
        let root = if self.root.is_empty() { "." } else { self.root.as_str() };
        let mut names: Vec<String> = self
            .sftp
            .readdir(Path::new(root))
            .map_err(|e| anyhow::anyhow!("Zielordner {root} nicht lesbar: {e}"))?
            .into_iter()
            .filter_map(|(path, stat)| {
                let name = path.file_name()?.to_string_lossy().into_owned();
                Some(if stat.is_dir() { format!("{name}/") } else { name })
            })
            .collect();
        names.sort();
        Ok(names)
    }

    fn ensure_dirs(&self, relative_file: &str) -> anyhow::Result<()> {
        let mut current = self.root.clone();
        let parts: Vec<&str> = relative_file.split('/').collect();
        for part in &parts[..parts.len().saturating_sub(1)] {
            current = if current.is_empty() { (*part).to_owned() } else { format!("{current}/{part}") };
            if self.sftp.stat(Path::new(&current)).is_err() {
                self.sftp
                    .mkdir(Path::new(&current), 0o755)
                    .map_err(|e| anyhow::anyhow!("Ordner {current} anlegen: {e}"))?;
            }
        }
        Ok(())
    }

    /// Writes to a temporary name and renames, so visitors never get a half-written file.
    pub fn upload(&self, local: &Path, relative: &str) -> anyhow::Result<()> {
        self.ensure_dirs(relative)?;
        let target = self.remote(relative);
        let temp = format!("{target}.orp-upload");
        let data = fs::read(local)?;
        {
            let mut file = self
                .sftp
                .open_mode(Path::new(&temp), OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE, 0o644, OpenType::File)
                .map_err(|e| anyhow::anyhow!("{relative} hochladen: {e}"))?;
            file.write_all(&data).map_err(|e| anyhow::anyhow!("{relative} hochladen: {e}"))?;
        }
        let flags = Some(RenameFlags::OVERWRITE | RenameFlags::ATOMIC | RenameFlags::NATIVE);
        if self.sftp.rename(Path::new(&temp), Path::new(&target), flags).is_err() {
            // SFTP v3 servers refuse to rename onto an existing file.
            let _ = self.sftp.unlink(Path::new(&target));
            self.sftp
                .rename(Path::new(&temp), Path::new(&target), None)
                .map_err(|e| anyhow::anyhow!("{relative} ersetzen: {e}"))?;
        }
        Ok(())
    }

    pub fn delete(&self, relative: &str) -> anyhow::Result<()> {
        match self.sftp.unlink(Path::new(&self.remote(relative))) {
            Ok(()) => Ok(()),
            // Already gone is fine.
            Err(e) if e.code() == ssh2::ErrorCode::SFTP(2) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("{relative} entfernen: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, handshake_error, new_session, ConnectError, HOSTKEY_PREF, KEX_FAILURE};
    use ssh2::MethodType;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn fingerprint_matches_openssh_format() {
        assert_eq!(fingerprint(b"abc"), "SHA256:ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0");
    }

    #[test]
    fn hostkey_pref_is_accepted_and_fully_supported() {
        // libssh2 silently drops unknown names from a preference list, so check each one.
        let session = new_session().expect("method_pref rejects HOSTKEY_PREF");
        let supported = session.supported_algs(MethodType::HostKey).unwrap();
        for name in HOSTKEY_PREF.split(',') {
            assert!(supported.contains(&name), "{name} is not supported by libssh2 (supported: {supported:?})");
        }
    }

    #[test]
    fn kex_failure_explains_outdated_host_keys() {
        let kex = handshake_error(ssh2::Error::new(KEX_FAILURE, "Unable to exchange encryption keys"));
        assert!(matches!(kex, ConnectError::Other(_)));
        let message = kex.to_string();
        assert!(message.starts_with("SSH-Handshake: "), "{message}");
        assert!(message.contains("SHA-1 ssh-rsa"), "{message}");

        let other = handshake_error(ssh2::Error::new(ssh2::ErrorCode::Session(-13), "Socket disconnected"));
        assert!(!other.to_string().contains("SHA-1"), "{other}");
    }

    fn read_u32(buf: &[u8], pos: &mut usize) -> usize {
        let value = u32::from_be_bytes(buf[*pos..*pos + 4].try_into().unwrap()) as usize;
        *pos += 4;
        value
    }

    /// Captures the host key algorithm list the client sends in its KEXINIT, over loopback
    /// against a fake server that stops after the client's first packet (no sshd needed).
    #[test]
    fn handshake_offers_hostkey_algorithms_in_old_order() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.write_all(b"SSH-2.0-OpenSSH_test\r\n").unwrap();
            // Client identification line.
            let mut line = Vec::new();
            let mut byte = [0u8; 1];
            while !line.ends_with(b"\r\n") {
                stream.read_exact(&mut byte).unwrap();
                line.push(byte[0]);
            }
            assert!(line.starts_with(b"SSH-2.0-"));
            // First binary packet (unencrypted): length, padding length, payload, padding.
            let mut len = [0u8; 4];
            stream.read_exact(&mut len).unwrap();
            let mut packet = vec![0u8; u32::from_be_bytes(len) as usize];
            stream.read_exact(&mut packet).unwrap();
            let payload = &packet[1..packet.len() - packet[0] as usize];
            assert_eq!(payload[0], 20, "expected SSH_MSG_KEXINIT");
            let mut pos = 1 + 16; // message type + cookie
            let kex_len = read_u32(payload, &mut pos);
            pos += kex_len;
            let hostkey_len = read_u32(payload, &mut pos);
            String::from_utf8(payload[pos..pos + hostkey_len].to_vec()).unwrap()
        });

        let mut session = new_session().unwrap();
        session.set_tcp_stream(TcpStream::connect(addr).unwrap());
        session.set_timeout(10_000);
        // The fake server hangs up after the KEXINIT, so the handshake itself fails.
        assert!(session.handshake().is_err());
        assert_eq!(server.join().unwrap(), HOSTKEY_PREF);
    }
}
