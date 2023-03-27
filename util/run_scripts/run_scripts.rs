use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::Command;

// Simple utility to run commands from files.  Each command/command line
// argument in the file is on a separate line.
pub fn main() {
    // Skip the first arg (our executable path).
    let scripts = env::args().skip(1);

    for script in scripts {
        let script_file = BufReader::new(File::open(&script).expect("unable to open file"));
        let mut lines = script_file.lines().map(|l| {
            l.map_err(|e| format!("{script}: error reading line: {e}"))
                .unwrap()
        });

        // First line of the file is the command to run.
        let command = lines
            .next()
            .ok_or_else(|| format!("{script}: no command in file"))
            .unwrap();

        // Subsequent lines are arguments.
        let args = lines.collect::<Vec<_>>();

        // Run the command and wait for it to finish.
        let mut child = Command::new(&command)
            .args(args)
            .spawn()
            .map_err(|e| format!("{script}: failed to spawn child process {command}: {e}"))
            .unwrap();
        let status = child.wait().expect("");

        if !status.success() {
            panic!("{}: error running script: {}", script, status);
        }
    }
}
