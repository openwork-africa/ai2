use crate::admin::fake::FakeAdmin;
use crate::admin::HOST_SANDBOX;
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
        admin: "/Users/ada/.ai2/vault/admin.sock".into(),
        dashboard: files::Dashboard::default(),
    }
}

fn linux_options() -> Options {
    Options {
        bin: "/home/ada/.local/bin/ai2".into(),
        home: "/home/ada/.ai2".into(),
        state: "/var/lib/ai2-vault".into(),
        label: files::LABEL.into(),
        keychain: None,
        admin: "/run/ai2-vault/admin.sock".into(),
        dashboard: files::Dashboard::default(),
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
    setup_with_admin(
        host,
        options,
        fake,
        master,
        &FakeAdmin::with(&[HOST_SANDBOX]),
        present,
        check_only,
    )
}

fn setup_with_admin(
    host: &Host,
    options: &Options,
    fake: &Fake,
    master: &dyn MasterKey,
    admin: &FakeAdmin,
    present: bool,
    check_only: bool,
) -> (bool, Value) {
    let presence = move || -> Result<bool> { Ok(present) };
    let ctx = Context {
        host,
        options,
        runner: fake,
        master,
        admin,
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
        )
        .file(
            "/Users/ada/Library/LaunchAgents/africa.openwork.ai2-ui.plist",
            &files::ui_plist(
                &options.bin,
                &options.home,
                files::UI_LABEL,
                files::UI_PORT,
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
fn the_setup_command_takes_headless_and_no_check() {
    let matches = crate::command()
        .try_get_matches_from(["setup", "--headless"])
        .unwrap();
    assert!(matches.get_flag("headless"));
    assert_eq!(crate::command().get_name(), "setup");
    assert!(crate::command()
        .try_get_matches_from(["setup", "--check"])
        .is_err());
}

#[test]
fn without_headless_setup_asks_the_caller_for_the_page_and_runs_nothing() {
    use crate::{outcome, Outcome};
    let options = Options::new("/nowhere/ai2", "/nowhere/.ai2");
    let matches = crate::command().try_get_matches_from(["setup"]).unwrap();
    assert_eq!(outcome(&matches, &options).unwrap(), Outcome::Ui);
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
fn every_step_is_announced_before_it_runs_and_after_it_settles() {
    let fake = Fake::default();
    linux_fresh(&fake);
    let master = MemoryKey::default();
    let (ok, _) = setup(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert!(ok);
    assert_eq!(
        *fake.steps.borrow(),
        [
            "started containers",
            "finished containers \"ready\"",
            "started master-key",
            "finished master-key \"ready\"",
            "started presence",
            "finished presence \"ready\"",
            "started daemon",
            "finished daemon \"ready\"",
            "started dashboard",
            "finished dashboard \"ready\"",
        ]
    );
    assert_eq!(
        *fake.asked.borrow(),
        ["Vault passphrase: ", "The same again: "]
    );
}

#[test]
fn steps_after_a_failure_finish_skipped_without_starting() {
    let fake = Fake::default();
    let master = MemoryKey::default();
    setup(
        &Host::linux(Some(249)),
        &linux_options(),
        &fake,
        &master,
        false,
        false,
    );
    assert_eq!(
        *fake.steps.borrow(),
        [
            "started containers",
            "finished containers \"unsupported\"",
            "started master-key",
            "finished master-key \"failed\"",
            "finished presence \"skipped\"",
            "finished daemon \"skipped\"",
            "finished dashboard \"skipped\"",
        ]
    );
}

#[test]
fn the_default_passphrase_prompt_asks_twice_and_refuses_a_mismatch() {
    use crate::runner::Runner;
    let fake = Fake::default();
    fake.type_in("one").type_in("one");
    assert_eq!(fake.ask_passphrase("A: ", "B: ").unwrap(), "one");
    fake.type_in("one").type_in("two");
    let error = fake.ask_passphrase("A: ", "B: ").unwrap_err();
    assert_eq!(error.to_string(), "the two passphrases differ");
    assert_eq!(*fake.asked.borrow(), ["A: ", "B: ", "A: ", "B: "]);
}

#[test]
fn each_step_has_a_title_a_why_and_a_fact_for_this_kind_of_machine() {
    use crate::{inputs_info, steps_info};
    let order: Vec<_> = all().iter().map(|step| step.id()).collect();
    for host in [
        Host::mac(26, true),
        Host::mac(15, true),
        Host::linux(Some(255)),
        Host::linux(None),
    ] {
        let infos = steps_info(&host.os);
        assert_eq!(infos.iter().map(|info| info.id).collect::<Vec<_>>(), order);
        for info in infos {
            assert!(!info.title.is_empty() && !info.why.is_empty() && !info.fact.is_empty());
        }
    }
    let mac = steps_info(&Host::mac(26, true).os);
    assert_eq!(mac[0].title, "Containers");
    assert!(mac[2].why.contains("Touch ID"));
    assert!(mac[3].fact.contains("launchd"));
    assert_eq!(mac[4].title, "Dashboard");
    assert!(mac[4].fact.contains("LaunchAgent"));
    let linux = steps_info(&Host::linux(Some(255)).os);
    assert_eq!(linux[0].title, "Sandboxes");
    assert!(linux[1].why.contains("systemd-creds"));
    assert!(linux[2].why.contains("passphrase"));
    assert!(linux[4].fact.contains("systemd user unit"));
    let old = steps_info(&Host::mac(15, true).os);
    assert!(old[0].why.contains("cannot keep agents in a container"));
    assert_eq!(
        serde_json::to_value(&mac[1]).unwrap()["title"],
        "Master key"
    );
    let inputs = inputs_info();
    assert_eq!(inputs[0].kind, "passphrase");
    assert_eq!(inputs[0].fields.len(), 2);
    assert!(inputs
        .iter()
        .all(|input| input.fields.iter().all(|field| field.secret)));
    assert_eq!(inputs[1].kind, "sudo");
}

#[test]
fn the_host_is_described_for_the_page() {
    assert_eq!(
        Host::mac(26, true).to_json(),
        serde_json::json!({ "os": "macos", "version": 26, "arm64": true, "systemd": null, "user": "ada", "supports_containers": true })
    );
    assert_eq!(
        Host::linux(Some(249)).to_json(),
        serde_json::json!({ "os": "linux", "version": null, "arm64": null, "systemd": 249, "user": "ada", "supports_containers": false })
    );
}

fn fresh_mac(admin: &FakeAdmin) -> (Fake, bool, Value) {
    let fake = Fake::default();
    container_missing_then_brewed(&fake);
    fake.answer(WHICH_BREW, Ran::ok("/opt/homebrew/bin/brew\n"));
    let master = MemoryKey::default();
    let (ok, report) = setup_with_admin(
        &Host::mac(26, true),
        &mac_options(),
        &fake,
        &master,
        admin,
        true,
        false,
    );
    (fake, ok, report)
}

#[test]
fn the_daemon_step_ends_by_adding_the_host_sandbox_alone_with_touch_id() {
    let admin = FakeAdmin::default();
    let (fake, ok, report) = fresh_mac(&admin);
    assert!(ok, "{report}");
    assert_eq!(*admin.added.borrow(), [(HOST_SANDBOX.to_string(), None)]);
    assert_eq!(report["steps"]["daemon"]["host_sandbox"], "created");
    assert!(report["steps"]["daemon"].get("warnings").is_none());
    let said = fake.said.borrow();
    let asked = said
        .iter()
        .find(|line| line.contains("sandbox host"))
        .unwrap();
    assert!(asked.contains("Touch ID") && asked.contains("this one change"));
    assert!(fake.asked.borrow().is_empty());
    assert_eq!(fake.steps.borrow()[7], "finished daemon \"ready\"");
}

#[test]
fn a_host_sandbox_already_there_is_left_alone() {
    let admin = FakeAdmin::with(&["box", HOST_SANDBOX]);
    let (_, ok, report) = fresh_mac(&admin);
    assert!(ok);
    assert!(admin.added.borrow().is_empty());
    assert_eq!(report["steps"]["daemon"]["host_sandbox"], "present");
}

#[test]
fn an_unreachable_admin_api_skips_the_host_sandbox_with_a_warning() {
    let admin = FakeAdmin::unreachable();
    let (fake, ok, report) = fresh_mac(&admin);
    assert!(ok, "{report}");
    assert_eq!(status(&report, "daemon"), "ready");
    assert_eq!(report["steps"]["daemon"]["host_sandbox"], "skipped");
    let warning = report["steps"]["daemon"]["warnings"][0].as_str().unwrap();
    assert!(
        warning.contains("not reachable") && warning.contains("ai2 vault sandbox add --name host")
    );
    assert!(admin.added.borrow().is_empty());
    assert!(!fake
        .said
        .borrow()
        .iter()
        .any(|line| line.contains("Touch ID")));
}

#[test]
fn a_refused_host_sandbox_is_a_warning_not_a_failure() {
    let admin = FakeAdmin::default().answering(
        401,
        serde_json::json!({ "error": "presence_required", "message": "Add the vault sandbox host: needs you present" }),
    );
    let (_, ok, report) = fresh_mac(&admin);
    assert!(ok);
    assert_eq!(report["steps"]["daemon"]["host_sandbox"], "skipped");
    assert!(report["steps"]["daemon"]["warnings"][0]
        .as_str()
        .unwrap()
        .contains("401: Add the vault sandbox host: needs you present"));
}

#[test]
fn on_linux_the_host_sandbox_asks_for_the_passphrase_once_more() {
    let fake = Fake::default();
    linux_fresh(&fake);
    fake.type_in("a long passphrase");
    let admin = FakeAdmin::default();
    let master = MemoryKey::default();
    let (ok, report) = setup_with_admin(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        &master,
        &admin,
        false,
        false,
    );
    assert!(ok, "{report}");
    assert_eq!(
        *admin.added.borrow(),
        [(
            HOST_SANDBOX.to_string(),
            Some("a long passphrase".to_string())
        )]
    );
    assert_eq!(
        fake.asked.borrow().last().unwrap(),
        crate::steps::HOST_PROMPT
    );
    assert!(crate::steps::HOST_PROMPT.contains("nothing else"));
}

#[test]
fn a_check_never_reaches_for_the_host_sandbox() {
    let fake = Fake::default();
    let admin = FakeAdmin::unreachable();
    let master = MemoryKey::default();
    master.create().unwrap();
    let (_, report) = setup_with_admin(
        &Host::mac(26, true),
        &mac_options(),
        &fake,
        &master,
        &admin,
        true,
        true,
    );
    assert!(report["steps"]["daemon"].get("host_sandbox").is_none());
    assert!(report["steps"]["daemon"].get("warnings").is_none());
    assert!(admin.added.borrow().is_empty());
}

#[test]
fn the_admin_socket_client_lists_and_adds_sandboxes() {
    use crate::admin::{Admin, Socket};
    use std::io::{Read, Write};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("admin.sock");
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let server = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for reply in [
            r#"{"sandboxes":[{"name":"box"},{"name":"host"}]}"#,
            r#"{"sandbox":{"name":"host"}}"#,
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            seen.push(String::from_utf8_lossy(&buf[..n]).into_owned());
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
        }
        seen
    });
    let socket = Socket::new(&path);
    assert_eq!(socket.sandboxes().unwrap(), ["box", "host"]);
    let (status, value) = socket.add_sandbox("host", Some("pass")).unwrap();
    assert_eq!(status, 200);
    assert_eq!(value["sandbox"]["name"], "host");
    let seen = server.join().unwrap();
    assert!(seen[0].starts_with("GET /sandboxes HTTP/1.1\r\n"));
    assert!(seen[1].starts_with("POST /sandboxes HTTP/1.1\r\n"));
    assert!(seen[1].contains("X-Vault-Passphrase: pass\r\n"));
    assert!(seen[1].ends_with(r#"{"name":"host"}"#));
    assert!(socket.add_sandbox("host", Some("a\r\nX: y")).is_err());
    let gone = Socket {
        path: dir.path().join("none.sock"),
        wait: std::time::Duration::ZERO,
    };
    assert!(gone.sandboxes().is_err());
}

#[test]
#[ignore]
fn this_machine_checks_without_changing_anything() {
    let options = Options::new("/usr/local/bin/ai2", "/tmp/ai2-setup-check");
    let (_, report) = crate::setup(&options, true).unwrap();
    println!("{report}");
}

fn dashboard_context<T>(
    host: &Host,
    options: &Options,
    fake: &Fake,
    check_only: bool,
    each: impl FnOnce(&Context) -> T,
) -> T {
    let master = MemoryKey::default();
    let admin = FakeAdmin::with(&[HOST_SANDBOX]);
    let presence = || -> Result<bool> { Ok(true) };
    let ctx = Context {
        host,
        options,
        runner: fake,
        master: &master,
        admin: &admin,
        presence: &presence,
        check_only,
    };
    each(&ctx)
}

fn dashboard(host: &Host, options: &Options, fake: &Fake, check_only: bool) -> crate::State {
    use crate::steps::DashboardStep;
    use crate::Step;
    dashboard_context(host, options, fake, check_only, |ctx| {
        let checked = DashboardStep.check(ctx).unwrap();
        if check_only || checked.is_ready() || checked.status == crate::Status::Unsupported {
            return checked;
        }
        DashboardStep.apply(ctx, &checked).unwrap()
    })
}

const UI_PLIST: &str = "/Users/ada/Library/LaunchAgents/africa.openwork.ai2-ui.plist";
const UI_UNIT: &str = "/home/ada/.config/systemd/user/ai2-ui.service";

#[test]
fn a_fresh_mac_gets_the_dashboard_launch_agent_after_the_vault() {
    let (fake, ok, report) = fresh_mac(&FakeAdmin::with(&[HOST_SANDBOX]));
    assert!(ok, "{report}");
    assert_eq!(status(&report, "dashboard"), "ready");
    assert_eq!(report["steps"]["dashboard"]["port"], 7797);
    assert_eq!(report["steps"]["dashboard"]["plist"], UI_PLIST);
    assert_eq!(
        report["steps"]["dashboard"]["restart"],
        "launchctl kickstart -k gui/501/africa.openwork.ai2-ui"
    );
    let vault = fake
        .at("launchctl bootstrap gui/501 /Users/ada/Library/LaunchAgents/africa.openwork.ai2-vault.plist")
        .unwrap();
    let ui = fake
        .at(&format!("launchctl bootstrap gui/501 {UI_PLIST}"))
        .unwrap();
    assert!(vault < ui);
    let options = mac_options();
    let files = fake.files.borrow();
    let (plist, mode) = &files[Path::new(UI_PLIST)];
    assert_eq!(
        plist,
        &files::ui_plist(&options.bin, &options.home, files::UI_LABEL, 7797, None)
    );
    assert_eq!(*mode, 0o644);
    assert_eq!(files[Path::new("/Users/ada/.ai2/ui")].1, 0o700);
    assert!(!fake.ran_any("root"));
}

#[test]
fn a_dashboard_check_reports_missing_and_changes_nothing() {
    let fake = Fake::default();
    fake.answer("launchctl print", Ran::failed());
    let state = dashboard(&Host::mac(26, true), &mac_options(), &fake, true);
    assert_eq!(state.status, crate::Status::Missing);
    assert_eq!(state.info["written"], false);
    assert_eq!(state.info["loaded"], false);
    assert!(fake.files.borrow().is_empty());
    assert_eq!(
        fake.lines(),
        ["launchctl print gui/501/africa.openwork.ai2-ui"]
    );
}

#[test]
fn a_dashboard_already_in_place_is_ready_and_left_alone() {
    let fake = Fake::default();
    let options = mac_options();
    fake.file(
        UI_PLIST,
        &files::ui_plist(&options.bin, &options.home, files::UI_LABEL, 7797, None),
    );
    let state = dashboard(&Host::mac(26, true), &options, &fake, false);
    assert!(state.is_ready());
    assert!(!fake.ran_any("launchctl bootstrap") && !fake.ran_any("launchctl bootout"));
}

#[test]
fn a_changed_dashboard_is_booted_out_and_loaded_again() {
    let fake = Fake::default();
    fake.file(UI_PLIST, "<plist>old</plist>");
    let state = dashboard(&Host::mac(26, true), &mac_options(), &fake, false);
    assert!(state.is_ready(), "{state:?}");
    let out = fake
        .at("launchctl bootout gui/501/africa.openwork.ai2-ui")
        .unwrap();
    let back = fake
        .at(&format!("launchctl bootstrap gui/501 {UI_PLIST}"))
        .unwrap();
    assert!(out < back);
}

#[test]
fn the_dashboard_never_boots_itself_out_when_it_runs_setup() {
    use crate::runner::Runner;
    let fake = Fake::default();
    fake.file(UI_PLIST, "<plist>old</plist>");
    let mut options = mac_options();
    options.dashboard.in_service = true;
    let state = dashboard(&Host::mac(26, true), &options, &fake, false);
    assert!(state.is_ready());
    assert!(!fake.ran_any("launchctl bootout") && !fake.ran_any("launchctl bootstrap"));
    assert!(state.warnings[0].starts_with(crate::steps::IN_SERVICE));
    assert!(state.warnings[0].contains("launchctl kickstart -k gui/501/africa.openwork.ai2-ui"));
    assert_ne!(
        fake.read(Path::new(UI_PLIST)).unwrap(),
        "<plist>old</plist>"
    );
}

#[test]
fn a_named_dashboard_label_and_port_reach_the_launch_agent() {
    use crate::runner::Runner;
    let fake = Fake::default();
    fake.once("launchctl print", Ran::failed());
    let mut options = mac_options();
    options.dashboard.label = "test.ai2-ui".into();
    options.dashboard.port = 8123;
    let state = dashboard(&Host::mac(26, true), &options, &fake, false);
    assert!(state.is_ready());
    let path = "/Users/ada/Library/LaunchAgents/test.ai2-ui.plist";
    assert!(fake.ran_any(&format!("launchctl bootstrap gui/501 {path}")));
    assert!(!fake.ran_any("launchctl bootout"));
    assert!(fake
        .read(Path::new(path))
        .unwrap()
        .contains("<string>8123</string>"));
}

#[test]
fn a_fresh_linux_gets_the_dashboard_as_a_user_unit() {
    let fake = Fake::default();
    linux_fresh(&fake);
    fake.once("systemctl --user is-active", Ran::failed());
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
    assert_eq!(status(&report, "dashboard"), "ready");
    assert_eq!(report["steps"]["dashboard"]["unit"], UI_UNIT);
    assert_eq!(
        report["steps"]["dashboard"]["restart"],
        "systemctl --user restart ai2-ui.service"
    );
    let options = linux_options();
    let files = fake.files.borrow();
    assert_eq!(
        files[Path::new(UI_UNIT)].0,
        files::ui_unit(&options.bin, &options.home, 7797)
    );
    let order = [
        "root systemctl enable --now ai2-vault.service",
        "systemctl --user daemon-reload",
        "systemctl --user enable --now ai2-ui.service",
    ]
    .map(|line| {
        fake.at(line)
            .unwrap_or_else(|| panic!("{line}: {:?}", fake.lines()))
    });
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!fake.ran_any("root systemctl --user"));
    assert!(!fake.ran_any("systemctl --user restart"));
}

#[test]
fn a_linux_dashboard_check_never_writes_or_starts() {
    let fake = Fake::default();
    fake.answer("systemctl --user is-active", Ran::failed());
    let state = dashboard(&Host::linux(Some(255)), &linux_options(), &fake, true);
    assert_eq!(state.status, crate::Status::Missing);
    assert!(fake.files.borrow().is_empty());
    assert_eq!(
        fake.lines(),
        [
            "systemctl --user is-enabled --quiet ai2-ui.service",
            "systemctl --user is-active --quiet ai2-ui.service"
        ]
    );
}

#[test]
fn a_running_linux_dashboard_restarts_after_its_unit_changes_unless_it_runs_setup() {
    let fake = Fake::default();
    fake.file(UI_UNIT, "[Service]\nExecStart=/old\n");
    let state = dashboard(&Host::linux(Some(255)), &linux_options(), &fake, false);
    assert!(state.is_ready());
    assert!(
        fake.at("systemctl --user restart ai2-ui.service").unwrap()
            > fake
                .at("systemctl --user enable --now ai2-ui.service")
                .unwrap()
    );

    let inside = Fake::default();
    inside.file(UI_UNIT, "[Service]\nExecStart=/old\n");
    let mut options = linux_options();
    options.dashboard.in_service = true;
    let state = dashboard(&Host::linux(Some(255)), &options, &inside, false);
    assert!(state.is_ready());
    assert!(!inside.ran_any("systemctl --user restart"));
    assert!(state.warnings[0].contains("systemctl --user restart ai2-ui.service"));
}

#[test]
fn a_failed_user_unit_start_fails_the_dashboard_step() {
    use crate::steps::DashboardStep;
    use crate::Step;
    let fake = Fake::default();
    fake.answer("systemctl --user enable", Ran::failed())
        .answer("systemctl --user is-active", Ran::failed());
    let error = dashboard_context(
        &Host::linux(Some(255)),
        &linux_options(),
        &fake,
        false,
        |ctx| {
            let checked = DashboardStep.check(ctx).unwrap();
            DashboardStep.apply(ctx, &checked).unwrap_err()
        },
    );
    assert_eq!(error.to_string(), "starting ai2-ui.service failed");
}

#[test]
fn without_systemd_or_on_another_os_the_dashboard_is_unsupported() {
    for host in [
        Host::linux(None),
        Host {
            os: crate::host::Os::Other {
                name: "freebsd".into(),
            },
            ..Host::linux(None)
        },
    ] {
        let fake = Fake::default();
        let state = dashboard(&host, &linux_options(), &fake, false);
        assert_eq!(state.status, crate::Status::Unsupported);
        assert!(fake.lines().is_empty());
    }
}
