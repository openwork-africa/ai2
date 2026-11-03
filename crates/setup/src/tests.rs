use crate::files::{self, SERVICE_BIN, UNIT_PATH};
use crate::host::Host;
use crate::runner::fake::Fake;
use crate::runner::Ran;
use crate::{all, run_steps, Context, Options, NOT_SAFE};
use anyhow::Result;
use keys::{MasterKey, MemoryKey};
use serde_json::Value;
use std::path::Path;

const WHICH_CONTAINER: &str = "/bin/sh -c command -v container";
const WHICH_BREW: &str = "/bin/sh -c command -v brew";
const CONTAINER: &str = "/opt/homebrew/bin/container";
const SIGNED: &str = "Package \"c.pkg\":\n   Status: signed by a developer certificate issued by Apple for distribution\n   Notarization: trusted by the Apple notary service\n   Certificate Chain:\n    1. Developer ID Installer: Apple Inc. - Containerization (UPBK2H6LZM)\n";

fn mac_options() -> Options {
    Options {
        bin: "/Users/ada/.local/bin/ai2".into(),
        home: "/Users/ada/.ai2".into(),
        state: "/Users/ada/.ai2/vault".into(),
        label: files::LABEL.into(),
        keychain: None,
    }
}

fn linux_options() -> Options {
    Options {
        bin: "/home/ada/.local/bin/ai2".into(),
        home: "/home/ada/.ai2".into(),
        state: "/var/lib/ai2-vault".into(),
        label: files::LABEL.into(),
        keychain: None,
    }
}

fn setup(
    host: &Host,
    options: &Options,
    fake: &Fake,
    master: &dyn MasterKey,
    present: bool,
    check_only: bool,
) -> (bool, Value) {
    let presence = move || -> Result<bool> { Ok(present) };
    let ctx = Context {
        host,
        options,
        runner: fake,
        master,
        presence: &presence,
        check_only,
    };
    run_steps(&all(), &ctx)
}

fn status(report: &Value, step: &str) -> String {
    report["steps"][step]["status"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn container_missing_then_brewed(fake: &Fake) {
    fake.answer(WHICH_CONTAINER, Ran::ok(&format!("{CONTAINER}\n")))
        .once(WHICH_CONTAINER, Ran::failed())
        .answer(
            &format!("{CONTAINER} system status"),
            Ran::ok("status running"),
        )
        .answer("launchctl print", Ran::ok(""))
        .once("launchctl print", Ran::failed());
}

#[test]
fn a_fresh_mac_gets_container_a_key_and_the_launch_agent() {
    let fake = Fake::default();
    container_missing_then_brewed(&fake);
    fake.answer(WHICH_BREW, Ran::ok("/opt/homebrew/bin/brew\n"));
    let master = MemoryKey::default();
    let options = mac_options();
    let (ok, report) = setup(&Host::mac(26, true), &options, &fake, &master, true, false);
    assert!(ok, "{report}");
    for step in ["containers", "master-key", "presence", "daemon"] {
        assert_eq!(status(&report, step), "ready", "{step}: {report}");
    }
    assert_eq!(report["steps"]["master-key"]["store"], "keychain");
    assert!(master.exists().unwrap());
    let brew = fake.at("brew install container").unwrap();
    let start = fake
        .at(&format!("{CONTAINER} system start --enable-kernel-install"))
        .unwrap();
    let bootstrap = fake
        .at("launchctl bootstrap gui/501 /Users/ada/Library/LaunchAgents/africa.openwork.ai2-vault.plist")
        .unwrap();
    assert!(brew < start && start < bootstrap);
    assert!(!fake.ran_any("root"), "{:?}", fake.lines());
    let files = fake.files.borrow();
    let (plist, mode) = &files[&files::plist_path(Path::new("/Users/ada"), files::LABEL)];
    assert_eq!(
        plist,
        &files::plist(
            &options.bin,
            &options.home,
            &options.state,
            files::LABEL,
            None
        )
    );
    assert_eq!(*mode, 0o644);
    assert_eq!(files[Path::new("/Users/ada/.ai2/vault")].1, 0o700);
}

#[test]
fn without_brew_the_signed_pkg_is_checked_then_installed_as_root() {
    let fake = Fake::default();
    container_missing_then_brewed(&fake);
    fake.answer(WHICH_BREW, Ran::failed())
        .answer(
            "curl -fsSLI",
            Ran::ok("https://github.com/apple/container/releases/tag/1.6.1"),
        )
        .answer("pkgutil --check-signature", Ran::ok(SIGNED));
    let master = MemoryKey::default();
    let (ok, report) = setup(
        &Host::mac(26, true),
        &mac_options(),
        &fake,
        &master,
        true,
        false,
    );
    assert!(ok, "{report}");
    let download = fake.at("curl -fsSL -o ").expect("downloads the pkg");
    assert!(fake.lines()[download].ends_with(
        "https://github.com/apple/container/releases/download/1.6.1/container-1.6.1-installer-signed.pkg"
    ));
    let check = fake.at("pkgutil --check-signature").unwrap();
    let install = fake.at("root installer -pkg ").unwrap();
    assert!(download < check && check < install);
    assert!(fake.lines()[install].ends_with("ai2-container-1.6.1.pkg -target /"));
}

#[test]
fn a_pkg_not_signed_by_apple_is_never_installed() {
    let fake = Fake::default();
    container_missing_then_brewed(&fake);
    fake.answer(WHICH_BREW, Ran::failed())
        .answer(
            "curl -fsSLI",
            Ran::ok("https://github.com/apple/container/releases/tag/1.6.1"),
        )
        .answer(
            "pkgutil --check-signature",
            Ran::ok("Status: signed by a developer certificate\n    1. Developer ID Installer: Someone Else (ABCDEFGHIJ)\n"),
        );
    let master = MemoryKey::default();
    let (ok, report) = setup(
        &Host::mac(26, true),
        &mac_options(),
        &fake,
        &master,
        true,
        false,
    );
    assert!(!ok);
    assert_eq!(status(&report, "containers"), "failed");
    assert!(report["steps"]["containers"]["error"]
        .as_str()
        .unwrap()
        .contains("not signed"));
    for step in ["master-key", "presence", "daemon"] {
        assert_eq!(status(&report, step), "skipped");
    }
    assert!(!fake.ran_any("root installer"));
    assert!(!master.exists().unwrap());
}

#[test]
fn an_intel_or_old_mac_is_warned_and_set_up_without_containers() {
    for host in [Host::mac(26, false), Host::mac(15, true)] {
        let fake = Fake::default();
        fake.answer("launchctl print", Ran::ok(""))
            .once("launchctl print", Ran::failed());
        let master = MemoryKey::default();
        let (ok, report) = setup(&host, &mac_options(), &fake, &master, true, false);
        assert!(ok, "{report}");
        assert_eq!(status(&report, "containers"), "unsupported");
        assert_eq!(report["steps"]["containers"]["warnings"][0], NOT_SAFE);
        assert!(fake.said.borrow().iter().any(|said| said == NOT_SAFE));
        assert_eq!(status(&report, "daemon"), "ready");
        assert!(!fake.ran_any(WHICH_BREW) && !fake.ran_any("brew"));
    }
}

#[test]
fn check_only_reports_and_changes_nothing() {
    let fake = Fake::default();
    fake.answer("launchctl print", Ran::failed());
    let master = MemoryKey::default();
    let (ok, report) = setup(
        &Host::mac(26, true),
        &mac_options(),
        &fake,
        &master,
        false,
        true,
    );
    assert!(!ok);
    assert_eq!(report["check"], true);
    for step in ["containers", "master-key", "presence", "daemon"] {
        assert_eq!(status(&report, step), "missing", "{step}");
    }
    assert!(!master.exists().unwrap());
    assert!(fake.files.borrow().is_empty());
    assert!(!fake.ran_any("brew") && !fake.ran_any("launchctl bootstrap"));
}

#[test]
fn a_mac_without_a_way_to_confirm_a_human_stops_before_the_daemon() {
    let fake = Fake::default();
    fake.answer(WHICH_CONTAINER, Ran::ok(CONTAINER)).answer(
        &format!("{CONTAINER} system status"),
        Ran::ok("status running"),
    );
    let master = MemoryKey::default();
    let (ok, report) = setup(
        &Host::mac(26, true),
        &mac_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert!(!ok);
    assert_eq!(status(&report, "presence"), "failed");
    assert_eq!(status(&report, "daemon"), "skipped");
}

#[test]
fn a_set_up_mac_is_left_alone() {
    let fake = Fake::default();
    let options = mac_options();
    fake.answer(WHICH_CONTAINER, Ran::ok(CONTAINER))
        .answer(
            &format!("{CONTAINER} system status"),
            Ran::ok("status running"),
        )
        .file(
            "/Users/ada/Library/LaunchAgents/africa.openwork.ai2-vault.plist",
            &files::plist(
                &options.bin,
                &options.home,
                &options.state,
                files::LABEL,
                None,
            ),
        );
    let master = MemoryKey::default();
    master.create().unwrap();
    let before = master.load().unwrap();
    let (ok, _) = setup(&Host::mac(26, true), &options, &fake, &master, true, false);
    assert!(ok);
    assert_eq!(*master.load().unwrap(), *before);
    assert!(!fake.ran_any("brew") && !fake.ran_any("launchctl bootstrap"));
    assert!(!fake.ran_any(&format!("{CONTAINER} system start")));
}

fn linux_fresh(fake: &Fake) {
    fake.answer("root sh -c if test -s", Ran::ok("yes\n"))
        .once("root sh -c if test -s", Ran::ok("no\n"))
        .answer("stat -c", Ran::ok("600 root\n"))
        .once("stat -c", Ran::failed())
        .answer("getent passwd ai2-vault", Ran::ok("ai2-vault:x:990"))
        .once("getent passwd ai2-vault", Ran::failed())
        .once("getent group ai2-vault-admin", Ran::failed())
        .answer("id -nG ada", Ran::ok("ada sudo ai2-vault-admin\n"))
        .once("id -nG ada", Ran::ok("ada sudo\n"))
        .answer("cmp -s", Ran::ok(""))
        .once("cmp -s", Ran::failed())
        .answer("systemctl is-active", Ran::ok(""))
        .once("systemctl is-active", Ran::failed())
        .type_in("a long passphrase")
        .type_in("a long passphrase");
}

#[test]
fn a_fresh_linux_gets_a_passphrase_a_user_and_a_locked_down_service() {
    let fake = Fake::default();
    linux_fresh(&fake);
    let master = MemoryKey::default();
    let (ok, report) = setup(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert!(ok, "{report} {:?}", fake.lines());
    assert_eq!(report["steps"]["containers"]["runtime"], "systemd");
    assert_eq!(report["steps"]["master-key"]["store"], "systemd-creds");
    assert_eq!(report["steps"]["master-key"]["status"], "ready");
    let inputs = fake.inputs.borrow();
    assert_eq!(inputs[0].len(), 32);
    let files = fake.files.borrow();
    let hash = &files[Path::new("/var/lib/ai2-vault/passphrase")].0;
    assert!(keys::passphrase::verify(hash.trim(), "a long passphrase").unwrap());
    assert_eq!(
        files[Path::new(UNIT_PATH)].0,
        files::unit(Path::new(SERVICE_BIN), "/etc/ai2-vault/master.cred", 1000)
    );
    let order = [
        "root install -d -m 0755 /etc/ai2-vault",
        "root systemd-creds encrypt --name=master - /etc/ai2-vault/master.cred",
        "root chmod 0600 /etc/ai2-vault/master.cred",
        "root chown root:root /etc/ai2-vault/master.cred",
        "root install -d -m 0700 /var/lib/ai2-vault",
        "root useradd --system --user-group --no-create-home --home-dir /var/lib/ai2-vault --shell /usr/sbin/nologin ai2-vault",
        "root groupadd --system ai2-vault-admin",
        "root usermod -aG ai2-vault-admin ada",
        "root install -D -m 0755 -o root -g root /home/ada/.local/bin/ai2 /usr/local/libexec/ai2-vault/ai2",
        "root install -d -o ai2-vault -g ai2-vault -m 0700 /var/lib/ai2-vault",
        "root chown -R ai2-vault:ai2-vault /var/lib/ai2-vault",
        "root systemctl daemon-reload",
        "root systemctl enable --now ai2-vault.service",
    ]
    .map(|line| fake.at(line).unwrap_or_else(|| panic!("{line}: {:?}", fake.lines())));
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!fake.ran_any("root systemctl restart"));
    assert!(report["steps"]["daemon"]["warnings"][0]
        .as_str()
        .unwrap()
        .contains("Log out and back in"));
}

#[test]
fn a_master_key_file_wider_than_0600_is_tightened_not_remade() {
    let fake = Fake::default();
    fake.answer("root sh -c if test -s", Ran::ok("yes\n"))
        .answer("id -nG ada", Ran::ok("ada ai2-vault-admin"))
        .answer("stat -c", Ran::ok("600 root\n"))
        .once("stat -c", Ran::ok("644 root\n"))
        .once("stat -c", Ran::ok("644 root\n"));
    let master = MemoryKey::default();
    let (_, report) = setup(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        &master,
        false,
        true,
    );
    assert_eq!(status(&report, "master-key"), "missing");
    let (_, report) = setup(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert_eq!(status(&report, "master-key"), "ready");
    assert!(fake.ran_any("root chmod 0600 /etc/ai2-vault/master.cred"));
    assert!(!fake.ran_any("root systemd-creds"));
}

#[test]
fn old_systemd_warns_and_stops_at_the_master_key_with_what_to_do() {
    let fake = Fake::default();
    let master = MemoryKey::default();
    let (ok, report) = setup(
        &Host::linux(Some(249)),
        &linux_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert!(!ok);
    assert_eq!(status(&report, "containers"), "unsupported");
    assert_eq!(report["steps"]["containers"]["warnings"][0], NOT_SAFE);
    assert_eq!(status(&report, "master-key"), "failed");
    let error = report["steps"]["master-key"]["error"].as_str().unwrap();
    assert!(error.contains("systemd 250 or later") && error.contains("systemd 249"));
    assert_eq!(status(&report, "presence"), "skipped");
    assert_eq!(status(&report, "daemon"), "skipped");
    assert!(!master.exists().unwrap());
}

#[test]
fn passphrases_that_differ_are_not_stored() {
    let fake = Fake::default();
    fake.answer("root sh -c if test -s", Ran::ok("no\n"))
        .answer("stat -c", Ran::ok("600 root\n"))
        .type_in("a long passphrase")
        .type_in("another passphrase");
    let master = MemoryKey::default();
    let (ok, report) = setup(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert!(!ok);
    assert!(report["steps"]["presence"]["error"]
        .as_str()
        .unwrap()
        .contains("differ"));
    assert!(fake.inputs.borrow().is_empty());
}

#[test]
fn a_linux_check_never_prompts_for_sudo() {
    let fake = Fake::default();
    fake.answer("root? sh -c if test -s", Ran::failed());
    let master = MemoryKey::default();
    let (_, report) = setup(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        &master,
        false,
        true,
    );
    assert_eq!(status(&report, "presence"), "unknown");
    assert!(fake.lines().iter().all(|line| !line.starts_with("root ")));
}

#[test]
fn a_running_service_is_restarted_after_its_unit_changes() {
    let fake = Fake::default();
    fake.answer("root sh -c if test -s", Ran::ok("yes\n"))
        .answer("stat -c", Ran::ok("600 root\n"))
        .answer("id -nG ada", Ran::ok("ada ai2-vault-admin"))
        .file(UNIT_PATH, "[Service]\nExecStart=/old\n");
    let master = MemoryKey::default();
    master.create().unwrap();
    let (ok, report) = setup(
        &Host::linux(Some(256)),
        &linux_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert!(ok, "{report}");
    assert!(!fake.ran_any("root useradd") && !fake.ran_any("root usermod"));
    assert!(
        fake.at("root systemctl restart ai2-vault.service").unwrap()
            > fake.at("root systemctl enable --now").unwrap()
    );
}

#[test]
fn the_setup_command_takes_check() {
    let matches = crate::command()
        .try_get_matches_from(["setup", "--check"])
        .unwrap();
    assert!(matches.get_flag("check"));
    assert_eq!(crate::command().get_name(), "setup");
}

#[test]
fn release_urls_and_signatures_are_read_strictly() {
    use crate::steps::{latest_version, pkg_url, signed_by_apple};
    assert_eq!(
        latest_version("https://github.com/apple/container/releases/tag/1.6.1").as_deref(),
        Some("1.6.1")
    );
    assert_eq!(
        latest_version("https://github.com/apple/container/releases"),
        None
    );
    assert_eq!(latest_version("https://x/tag/1.0;rm -rf /"), None);
    assert_eq!(
        pkg_url("1.6.1"),
        "https://github.com/apple/container/releases/download/1.6.1/container-1.6.1-installer-signed.pkg"
    );
    assert!(signed_by_apple(SIGNED));
    assert!(!signed_by_apple(
        "Developer ID Installer: Apple Inc. - Containerization (UPBK2H6LZM)"
    ));
}

#[test]
#[ignore]
fn this_machine_checks_without_changing_anything() {
    let options = Options::new("/usr/local/bin/ai2", "/tmp/ai2-setup-check");
    let matches = crate::command()
        .try_get_matches_from(["setup", "--check"])
        .unwrap();
    crate::run(&matches, &options).unwrap();
}
