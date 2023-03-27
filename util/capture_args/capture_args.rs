// Copyright 2023 The Bazel Authors. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;
use std::fs::File;
use std::io::Write;

// Simple utility to capture command line arguments and write them to a file.
pub fn main() {
    let file_name = env::var("OUTPUT_FILE").expect("OUTPUT_FILE environment variable is not set");
    let mut file = File::create(file_name).expect("Can't open OUTPUT_FILE");

    // Write command line args, skipping the first (our executable path), to
    // `OUTPUT_FILE` separated by newlines.
    let args: Vec<String> = env::args().skip(1).collect();
    writeln!(&mut file, "{}", args.join("\n")).expect("Unable to write to OUTPUT_FILE");
}
