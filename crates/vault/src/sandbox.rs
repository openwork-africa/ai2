use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;

pub const SOCKET_IN_CONTAINER: &str = "/run/ai2-vault.sock";
pub const ENDPOINT_ENV: &str = "AI2_VAULT_ENDPOINT";
pub const SUN_PATH_MAX: usize = 103;

pub fn attach(socket: &Path) -> Result<Vec<String>> {
    if !socket.is_absolute() {
        bail!(
            "the sandbox socket {} must be an absolute path",
            socket.display()
        );
    }
    let path = socket.display().to_string();
    if path.contains(':') || path.contains(',') {
        bail!("the sandbox socket {path} cannot hold ':' or ','");
    }
    if path.len() > SUN_PATH_MAX {
        bail!(
            "the sandbox socket {path} is {} bytes; a Unix socket path holds at most {SUN_PATH_MAX}",
            path.len()
        );
    }
    Ok(vec![
        "--volume".into(),
        format!("{path}:{SOCKET_IN_CONTAINER}"),
        "--env".into(),
        format!("{ENDPOINT_ENV}=unix:{SOCKET_IN_CONTAINER}"),
    ])
}

pub fn container_run(socket: &Path, rest: &[&str]) -> Result<Command> {
    let mut command = Command::new("container");
    command.arg("run").args(attach(socket)?);
    command.args(rest);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sandbox_socket_is_mounted_as_a_file() {
        let args = attach(Path::new("/Users/ada/.ai2/vault/sandboxes/s1.sock")).unwrap();
        assert_eq!(
            args,
            [
                "--volume",
                "/Users/ada/.ai2/vault/sandboxes/s1.sock:/run/ai2-vault.sock",
                "--env",
                "AI2_VAULT_ENDPOINT=unix:/run/ai2-vault.sock"
            ]
        );
    }

    #[test]
    fn relative_and_too_long_socket_paths_are_refused() {
        assert!(attach(Path::new("s1.sock")).is_err());
        assert!(attach(Path::new("/tmp/a:b.sock")).is_err());
        let long = format!("/{}", "a".repeat(SUN_PATH_MAX));
        assert!(attach(Path::new(&long)).is_err());
    }

    #[test]
    fn the_run_command_puts_the_attach_before_the_image() {
        let command = container_run(Path::new("/tmp/s.sock"), &["--rm", "alpine", "true"]).unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "run",
                "--volume",
                "/tmp/s.sock:/run/ai2-vault.sock",
                "--env",
                "AI2_VAULT_ENDPOINT=unix:/run/ai2-vault.sock",
                "--rm",
                "alpine",
                "true"
            ]
        );
    }

    #[test]
    #[ignore]
    fn a_host_socket_is_reachable_from_inside_an_apple_container() {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener;
        let dir = tempfile::Builder::new()
            .prefix("ai2s")
            .tempdir_in("/tmp")
            .unwrap();
        let socket = dir.path().join("gw.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut got = [0u8; 5];
            stream.read_exact(&mut got).unwrap();
            stream.write_all(b"pong\n").unwrap();
            got
        });
        let script = r#"perl -MIO::Socket::UNIX -e 'my $s = IO::Socket::UNIX->new(Peer => $ENV{AI2_VAULT_ENDPOINT} =~ s/^unix://r) or die "connect: $!\n"; print $s "ping\n"; print scalar <$s>;'"#;
        let output = container_run(
            &socket,
            &[
                "--rm",
                "--progress",
                "none",
                "debian:trixie-slim",
                "sh",
                "-c",
                script,
            ],
        )
        .unwrap()
        .output()
        .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "pong\n");
        assert_eq!(&served.join().unwrap(), b"ping\n");
    }

    #[test]
    #[ignore]
    fn a_vault_gateway_answers_from_inside_an_apple_container() {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let (Some(socket), Some(token), Some(path), Some(name)) = (
            var("AI2_E2E_SOCKET"),
            var("AI2_E2E_TOKEN"),
            var("AI2_E2E_PATH"),
            var("AI2_E2E_CONTAINER"),
        ) else {
            eprintln!("set AI2_E2E_SOCKET, AI2_E2E_TOKEN, AI2_E2E_PATH and AI2_E2E_CONTAINER, as tests/macos/vault-e2e.sh does");
            return;
        };
        let script = r#"perl -MIO::Socket::UNIX -e 'my $s = IO::Socket::UNIX->new(Peer => $ENV{AI2_VAULT_ENDPOINT} =~ s/^unix://r) or die "connect: $!\n"; print $s "GET $ENV{AI2_E2E_PATH} HTTP/1.1\r\nHost: v\r\nAuthorization: Bearer $ENV{AI2_VAULT_TOKEN}\r\nConnection: close\r\n\r\n"; print while <$s>;'"#;
        let token = format!("AI2_VAULT_TOKEN={token}");
        let path = format!("AI2_E2E_PATH={path}");
        let output = container_run(
            Path::new(&socket),
            &[
                "--rm",
                "--progress",
                "none",
                "--name",
                &name,
                "--env",
                &token,
                "--env",
                &path,
                "debian:trixie-slim",
                "sh",
                "-c",
                script,
            ],
        )
        .unwrap()
        .output()
        .unwrap();
        let out = String::from_utf8_lossy(&output.stdout);
        println!("{out}");
        assert!(
            out.starts_with("HTTP/1.1 200"),
            "{out}{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
