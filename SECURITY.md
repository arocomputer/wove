# Security

Report vulnerabilities through GitHub private vulnerability reporting on this
repository. Please avoid public issues containing exploitable details until a
fix is available.

Text writes reject control characters so element content cannot directly emit
terminal escape sequences. This is not a general untrusted-document sanitizer.
Applications remain responsible for content limits and Unicode presentation.

The core library performs no network access. Its terminal backend changes terminal modes
and writes output. It restores modes on normal exit and panic unwinding;
abort, forced termination, and uncatchable signals cannot run cleanup.

The optional SSH package accepts connections only on the listener the application
supplies. It requires public-key authorization and a host key. Keep that key
persistent and private, and restrict the authorization callback to trusted users.
It accepts a PTY and interactive application channel, not commands, forwarding,
or file transfers. Its example listens on loopback only.

SSH input queues, paste size, terminal dimensions, connection count, and output
waits are bounded. Application callbacks must return promptly; Rust cannot cancel
a callback that blocks forever. Limits do not replace application-specific quotas.
