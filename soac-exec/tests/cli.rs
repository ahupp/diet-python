use assert_cmd::Command;

#[test]
fn prints_hello_world() {
    let mut cmd = Command::cargo_bin("soac-exec").unwrap();
    cmd.assert().success().stdout("hello, world");
}
