# Security policy

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/arocomputer/wove/security/advisories/new)
or email [security@intuitum.sh](mailto:security@intuitum.sh) with SECURITY in the
subject. Do not open a public issue containing an exploit or private information.

Include the affected version or commit, Cargo features, operating system and
terminal, a minimal reproduction, and the expected impact. For SSH reports,
explain whether authentication is required. Use synthetic data and remove tokens,
host private keys, credentials, and application content from attachments.

Maintainers will coordinate reproduction, a fix, and disclosure through the
private report. Do not assume a response deadline or a bounty program.

## Supported versions

Wove has not made its first supported release. Report findings against current
main or an open PR. The earlier `wove` 0.2.0 package is yanked and is not a supported
release. There are no maintained stable branches yet.

## Security boundaries

Core rendering and text editing do not access the network or load application
configuration. Markdown, syntax highlighting, and diff support are optional.
Text writes filter control characters before emitting cell content. They are not
a general document sanitizer: applications still own content limits, URL policy,
Unicode presentation, and the treatment of untrusted files.

The terminal backend acquires raw mode and writes terminal protocol sequences.
It restores modes on normal return and panic unwinding. Process abort, forced
termination, and uncatchable signals cannot run cleanup. Applications must not
change terminal modes behind an active Wove terminal.

SSH accepts connections on a listener supplied by the application. It requires
a host key and public-key authorization policy, and exposes a PTY application
channel. It does not execute shell commands, forward ports, or serve files.
Examples bind loopback. A server's operator owns key storage, network exposure,
and decisions about which users may connect.

The SSH transport bounds input queues, paste size, terminal dimensions, connection
count, and output waits. These limits do not bound all work an application can do.
Callbacks must return promptly; Rust cannot cancel a callback that blocks forever.

Custom elements, Dioxus components, and SSH application callbacks are trusted
in-process Rust code. Wove does not sandbox them. Run untrusted application code
in a separate process with operating-system containment.

## Findings worth reporting

Report terminal control injection through text APIs, unauthenticated access or
authorization bypass in SSH, missing input bounds that permit remote resource
exhaustion, memory-safety defects in dependencies, and unintended credential or
data exposure. Include the trust boundary crossed in the reproduction.

Application code intentionally emitting terminal escapes, an authorization
callback intentionally accepting every key, or a callback running arbitrary code
is not by itself a Wove vulnerability. An implementation that violates the
boundaries above is still a valid report.

## Dependency and automation review

The `security` workflow runs Cargo's advisory audit weekly and on dependency
changes. Dependabot proposes weekly updates for Cargo, Actions, Web, and PTY test
dependencies. Audit warnings still need review; a green run is not proof of security.
Keep optional parsing dependencies disabled when the application does not need them.

Actions use full commit pins and minimal workflow permissions. Pull-request
checks must not receive release credentials or run contributor code with a
privileged token. Review changes to terminal output, SSH, manifests, release
scripts, and workflows as changes to the security boundary.
