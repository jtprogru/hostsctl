//! Конфиг, разложенный по файлам-зонам.

mod common;
use common::Sandbox;

#[test]
fn group_lives_in_its_own_yaml_zone() {
    let s = Sandbox::new();
    s.init();
    s.ok(&["group", "add", "work", "--file", "zones/work.yaml", "-d", "Work stands"]);
    s.ok(&["add", "10.0.0.7", "stand.local", "--group", "work"]);
    s.ok(&["add", "127.0.0.1", "dev.local", "--group", "local"]);

    let zone = std::fs::read_to_string(s.path("zones/work.yaml")).unwrap();
    assert!(zone.contains("stand.local"));
    assert!(zone.contains("Work stands"));

    // В основном конфиге записи зоны не дублируются.
    let main = std::fs::read_to_string(s.path("config.yaml")).unwrap();
    assert!(!main.contains("stand.local"), "a zone entry leaked into config.yaml:\n{main}");
    assert!(main.contains("dev.local"));

    s.ok(&["apply", "-y"]);
    let hosts = s.hosts();
    assert!(hosts.contains("stand.local"));
    assert!(hosts.contains("dev.local"));
    common::assert_system_lines_intact(&hosts);
}

#[test]
fn hosts_zone_is_plain_and_hand_editable() {
    let s = Sandbox::new();
    s.init();
    std::fs::create_dir_all(s.path("zones")).unwrap();
    // Файл написан руками в обычном hosts-синтаксисе.
    std::fs::write(
        s.path("zones/10-local.hosts"),
        "# Local development\n127.0.0.1   k8s.orb.local\n10.30.13.37 sre-mcp.local  # stand\n# 127.0.0.1 old.local\n",
    )
    .unwrap();

    let list = s.ok(&["list", "--all"]);
    assert!(list.contains("k8s.orb.local"), "{list}");
    assert!(list.contains("Local development"));
    assert!(list.contains("old.local"), "a commented-out line is a disabled entry");

    s.ok(&["apply", "-y"]);
    let hosts = s.hosts();
    assert!(hosts.contains("sre-mcp.local"));
    assert!(!hosts.contains("old.local"));

    // Правка через CLI возвращается в тот же файл и тем же форматом.
    s.ok(&["disable", "sre-mcp.local"]);
    let zone = std::fs::read_to_string(s.path("zones/10-local.hosts")).unwrap();
    assert!(zone.contains("# Local development"));
    assert!(zone.contains("# 10.30.13.37"), "a disabled entry is written as a comment:\n{zone}");
    assert!(zone.contains("# stand"), "the comment was lost:\n{zone}");
    assert!(!zone.contains("ip:"), "a hosts zone must not turn into yaml:\n{zone}");

    // И перечитывается обратно без потерь.
    let list = s.ok(&["list", "--all"]);
    assert!(list.contains("sre-mcp.local"));
}

#[test]
fn existing_hosts_files_can_be_attached_as_zones() {
    let s = Sandbox::new();
    s.init();
    // Файлы лежат где-то в стороне, как у старого hosts-sync.
    std::fs::create_dir_all(s.path("legacy")).unwrap();
    std::fs::write(s.path("legacy/10-local.hosts"), "127.0.0.1 a.local\n").unwrap();
    std::fs::write(s.path("legacy/20-block.hosts"), "0.0.0.0 ads.example\n").unwrap();

    let out = s.ok(&["zone", "add", "legacy/*.hosts"]);
    assert!(out.contains("2 files"), "{out}");

    let list = s.ok(&["zone", "list"]);
    assert!(list.contains("legacy/10-local.hosts"));
    assert!(list.contains("legacy/20-block.hosts"));

    s.ok(&["apply", "-y"]);
    let hosts = s.hosts();
    assert!(hosts.contains("a.local"));
    assert!(hosts.contains("ads.example"));

    // Отключение шаблона убирает записи из hosts, но не файлы с диска.
    s.ok(&["zone", "rm", "legacy/*.hosts"]);
    s.ok(&["apply", "-y"]);
    let hosts = s.hosts();
    assert!(!hosts.contains("a.local"));
    assert!(
        s.path("legacy/10-local.hosts").exists(),
        "the file was deleted and should not have been"
    );
}

#[test]
fn group_moves_between_files() {
    let s = Sandbox::new();
    s.init();
    s.ok(&["add", "127.0.0.1", "movable.local", "--group", "local"]);
    assert!(std::fs::read_to_string(s.path("config.yaml")).unwrap().contains("movable.local"));

    s.ok(&["group", "move", "local", "--file", "zones/local.hosts"]);
    let main = std::fs::read_to_string(s.path("config.yaml")).unwrap();
    let zone = std::fs::read_to_string(s.path("zones/local.hosts")).unwrap();
    assert!(!main.contains("movable.local"), "still in config.yaml:\n{main}");
    assert!(zone.contains("movable.local"), "{zone}");

    // И обратно.
    s.ok(&["group", "move", "local", "--file", "main"]);
    assert!(std::fs::read_to_string(s.path("config.yaml")).unwrap().contains("movable.local"));
    let zone = std::fs::read_to_string(s.path("zones/local.hosts")).unwrap();
    assert!(!zone.contains("movable.local"), "the zone was not emptied:\n{zone}");
}

#[test]
fn duplicate_group_across_files_is_an_error() {
    let s = Sandbox::new();
    s.init();
    s.ok(&["add", "127.0.0.1", "first.local", "--group", "local"]);
    std::fs::create_dir_all(s.path("zones")).unwrap();
    // Та же группа 'local' объявлена ещё и файлом-зоной.
    std::fs::write(s.path("zones/local.hosts"), "127.0.0.1 dup.local\n").unwrap();
    let err = s.fails(&["list"]);
    assert!(err.contains("declared twice"), "{err}");
    assert!(err.contains("config.yaml") && err.contains("local.hosts"), "{err}");
}

#[test]
fn zone_file_is_not_touched_when_nothing_changed() {
    let s = Sandbox::new();
    s.init();
    s.ok(&["group", "add", "work", "--file", "zones/work.yaml"]);
    s.ok(&["add", "10.0.0.7", "stand.local", "--group", "work"]);
    let before = std::fs::metadata(s.path("zones/work.yaml")).unwrap().modified().unwrap();

    // Правка в другой группе не должна дёргать чужой файл.
    s.ok(&["add", "127.0.0.1", "other.local", "--group", "local"]);
    let after = std::fs::metadata(s.path("zones/work.yaml")).unwrap().modified().unwrap();
    assert_eq!(before, after, "the zone was rewritten without a change");
}

#[test]
fn remote_source_can_live_in_a_yaml_zone_but_not_hosts() {
    let s = Sandbox::new();
    s.init();
    let err = s.fails(&[
        "source",
        "add",
        "https://example.invalid/list",
        "--group",
        "ads",
        "--file",
        "zones/ads.hosts",
    ]);
    assert!(err.contains("hosts zone") || err.contains("is a remote list"), "{err}");

    s.ok(&[
        "source",
        "add",
        "https://example.invalid/list",
        "--group",
        "ads",
        "--file",
        "zones/ads.yaml",
    ]);
    let zone = std::fs::read_to_string(s.path("zones/ads.yaml")).unwrap();
    assert!(zone.contains("example.invalid"));
}

#[test]
fn unknown_zone_path_is_added_to_include() {
    let s = Sandbox::new();
    s.init();
    // 'custom/' под шаблоны по умолчанию не попадает.
    s.ok(&["group", "add", "misc", "--file", "custom/misc.yaml"]);
    let main = std::fs::read_to_string(s.path("config.yaml")).unwrap();
    assert!(main.contains("custom/misc.yaml"), "include was not extended:\n{main}");

    s.ok(&["add", "127.0.0.1", "misc.local", "--group", "misc"]);
    // Перечитывание с диска находит группу.
    assert!(s.ok(&["list"]).contains("misc.local"));
}

#[test]
fn yaml_zone_shorthand_is_accepted() {
    let s = Sandbox::new();
    s.init();
    std::fs::create_dir_all(s.path("zones")).unwrap();
    // Одна группа без имени: имя берётся из имени файла.
    std::fs::write(
        s.path("zones/30-work.yaml"),
        "description: Stands\nentries:\n  - ip: 10.0.0.9\n    hostnames: [w.local]\n",
    )
    .unwrap();
    let list = s.ok(&["group", "list"]);
    assert!(list.contains("work"), "{list}");
    assert!(list.contains("30-work.yaml"), "the group file is not shown:\n{list}");
    s.ok(&["apply", "-y"]);
    assert!(s.hosts().contains("w.local"));
}

#[test]
fn config_path_lists_zones() {
    let s = Sandbox::new();
    s.init();
    s.ok(&["group", "add", "work", "--file", "zones/work.yaml"]);
    let out = s.ok(&["config-path", "--all"]);
    assert!(out.contains("config.yaml"));
    assert!(out.contains("zones/work.yaml"));
}
