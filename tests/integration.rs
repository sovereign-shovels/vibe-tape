use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vibe-tape"))
}

#[test]
#[cfg(unix)]
fn test_record_and_replay_echo() {
    let dir = tempfile::tempdir().unwrap();
    let tape = dir.path().join("session.tape");

    let output = Command::new(bin())
        .args([
            "run",
            "--output",
            tape.to_str().unwrap(),
            "/bin/echo",
            "hello-vibe-tape",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run vibe-tape");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        tape.exists(),
        "tape file should be created. stderr: {}",
        stderr
    );

    let replay = Command::new(bin())
        .args(["replay", tape.to_str().unwrap()])
        .output()
        .expect("failed to replay");

    let replay_stdout = String::from_utf8_lossy(&replay.stdout);
    assert!(
        replay_stdout.contains("hello-vibe-tape"),
        "replay should contain command output. stdout: {}",
        replay_stdout
    );
    assert!(replay_stdout.contains("START"));
    assert!(replay_stdout.contains("END"));
}

#[test]
#[cfg(unix)]
fn test_record_captures_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let tape = dir.path().join("session.tape");

    // Use a shell script that creates a file after a short delay
    let script = r#"
        sleep 0.3
        echo "created-by-test" > test_file.txt
        sleep 0.3
    "#;

    let output = Command::new(bin())
        .args([
            "run",
            "--output",
            tape.to_str().unwrap(),
            "/bin/sh",
            "-c",
            script,
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run vibe-tape");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        tape.exists(),
        "tape file should be created. stderr: {}",
        stderr
    );

    let replay = Command::new(bin())
        .args(["replay", tape.to_str().unwrap()])
        .output()
        .expect("failed to replay");

    let replay_stdout = String::from_utf8_lossy(&replay.stdout);
    assert!(
        replay_stdout.contains("CREATE") || replay_stdout.contains("WRITE"),
        "replay should show file creation. stdout: {}",
        replay_stdout
    );
}
