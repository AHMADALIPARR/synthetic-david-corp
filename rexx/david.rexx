/* Fixed-action build/launch glue. Run from the repository root. */
parse arg action rest
action = translate(strip(action))
if rest <> '' then do
  say 'REXX accepts one fixed action. Pass files or prompts directly to david.'
  exit 2
end
if stream('rust/Cargo.toml','c','query exists') = '' then do
  say 'Run from the Synthetic David repository root.'
  exit 2
end
parse source platform .
windows = pos('WIN', translate(platform)) > 0
base = directory()
if windows then do
  separator = ';'
  suffix = '.exe'
end
else do
  separator = ':'
  suffix = ''
end
if windows & stream('.tools/cargo/bin/cargo.exe','c','query exists') <> '' then do
  call value 'CARGO_HOME', base || '/.tools/cargo', 'ENVIRONMENT'
  call value 'RUSTUP_HOME', base || '/.tools/rustup', 'ENVIRONMENT'
  call value 'PATH', base || '/.tools/cargo/bin;' || base || '/.tools/w64devkit/bin;' || value('PATH',,'ENVIRONMENT'), 'ENVIRONMENT'
  linker = base || '/.tools/rustup/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe'
  call value 'CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER', linker, 'ENVIRONMENT'
  call value 'RUSTFLAGS', '-C link-self-contained=yes', 'ENVIRONMENT'
end
select
  when action = 'BUILD' then command = 'cargo build --manifest-path rust/Cargo.toml --locked -j 2'
  when action = 'TEST' then command = 'cargo test --manifest-path rust/Cargo.toml --locked -j 2'
  when action = 'CHECK' then command = 'cargo check --manifest-path rust/Cargo.toml --locked -j 2'
  when action = 'FMT' then command = 'cargo fmt --manifest-path rust/Cargo.toml --all -- --check'
  when action = 'DEMO' then command = 'rust/target/debug/david' || suffix || ' demo'
  when action = 'VERSION' then command = 'rust/target/debug/david' || suffix || ' version'
  when action = 'BROKER-BUILD' then command = 'rust/target/debug/david' || suffix || ' broker-build'
  when action = 'AUDIT' then command = 'rust/target/debug/david' || suffix || ' audit'
  when action = 'LICENSE-STATUS' then command = 'rust/target/debug/david' || suffix || ' license-status'
  when action = 'DEPLOYMENT-ID' then command = 'rust/target/debug/david' || suffix || ' deployment-id'
  when action = 'QWEN-START' then command = 'rust/target/debug/david' || suffix || ' qwen-start'
  when action = 'QWEN-STATUS' then command = 'rust/target/debug/david' || suffix || ' qwen-status'
  when action = 'QWEN-VERIFY' then command = 'rust/target/debug/david' || suffix || ' qwen-verify'
  when action = 'PUBLICATION-CHECK' then command = 'rust/target/debug/david' || suffix || ' publication-check'
  when action = 'HELP' | action = '' then do
    say 'build test check fmt version broker-build demo audit license-status deployment-id qwen-start qwen-status qwen-verify publication-check'
    exit 0
  end
  otherwise do
    say 'Unknown action. Arbitrary commands are forbidden.'
    exit 2
  end
end
/* No user text is interpolated into this command. */
if windows & left(command,5) = 'rust/' then command = translate(command, '\', '/')
address system command
exit rc
