# Copyright 2020 Google
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
Rust Analyzer Bazel rules.

rust_analyzer will generate a rust-project.json file for the
given targets. This file can be consumed by rust-analyzer as an alternative
to Cargo.toml files.
"""

load(
    "//rust/private:utils.bzl",
    "concat",
    "dedent",
    "dedup_expand_location",
    "find_toolchain",
)

_OUTPUT_BASE_TEMPLATE = "__OUTPUT_BASE__/"

def _rust_analyzer_toolchain_impl(ctx):
    toolchain = platform_common.ToolchainInfo(
        proc_macro_srv = ctx.executable.proc_macro_srv,
        rustc = ctx.executable.rustc,
        rustc_srcs = ctx.attr.rustc_srcs,
    )

    return [toolchain]

rust_analyzer_toolchain = rule(
    implementation = _rust_analyzer_toolchain_impl,
    doc = "A toolchain for [rust-analyzer](https://rust-analyzer.github.io/).",
    attrs = {
        "proc_macro_srv": attr.label(
            doc = "The path to a `rust_analyzer_proc_macro_srv` binary.",
            cfg = "exec",
            executable = True,
            allow_single_file = True,
        ),
        "rustc": attr.label(
            doc = "The path to a `rustc` binary.",
            cfg = "exec",
            executable = True,
            allow_single_file = True,
            mandatory = True,
        ),
        "rustc_srcs": attr.label(
            doc = "The source code of rustc.",
            mandatory = True,
        ),
    },
    incompatible_use_toolchain_transition = True,
)

def _rust_analyzer_detect_sysroot_impl(ctx):
    rust_analyzer_toolchain = ctx.toolchains[Label("@rules_rust//rust/rust_analyzer:toolchain_type")]

    if not rust_analyzer_toolchain.rustc_srcs:
        fail(
            "Current Rust-Analyzer toolchain doesn't contain rustc sources in `rustc_srcs` attribute.",
            "These are needed by rust-analyzer. If you are using the default Rust toolchain, add `rust_repositories(include_rustc_srcs = True, ...).` to your WORKSPACE file.",
        )

    rustc_srcs = rust_analyzer_toolchain.rustc_srcs

    sysroot_src = rustc_srcs.label.package + "/library"
    if rustc_srcs.label.workspace_root:
        sysroot_src = _OUTPUT_BASE_TEMPLATE + rustc_srcs.label.workspace_root + "/" + sysroot_src

    rustc = rust_analyzer_toolchain.rustc
    sysroot_dir, _, bin_dir = rustc.dirname.rpartition("/")
    if bin_dir != "bin":
        fail("The rustc path is expected to be relative to the sysroot as `bin/rustc`. Instead got: {}".format(
            rustc.path,
        ))

    sysroot = "{}/{}".format(
        _OUTPUT_BASE_TEMPLATE,
        sysroot_dir,
    )

    toolchain_info = {
        "sysroot": sysroot,
        "sysroot_src": sysroot_src,
    }

    output = ctx.actions.declare_file(ctx.label.name + ".rust_analyzer_toolchain.json")
    ctx.actions.write(
        output = output,
        content = json.encode_indent(toolchain_info, indent = " " * 4),
    )

    return [DefaultInfo(files = depset([output]))]

rust_analyzer_detect_sysroot = rule(
    implementation = _rust_analyzer_detect_sysroot_impl,
    toolchains = [
        "@rules_rust//rust:toolchain_type",
        "@rules_rust//rust/rust_analyzer:toolchain_type",
    ],
    incompatible_use_toolchain_transition = True,
    doc = dedent("""\
        Detect the sysroot and store in a file for use by the gen_rust_project tool.
    """),
)
