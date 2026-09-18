# Security

Report vulnerabilities through GitHub private vulnerability reporting on this
repository. Please avoid public issues containing exploitable details until a
fix is available.

Text writes reject control characters so element content cannot directly emit
terminal escape sequences. This is not a general untrusted-document sanitizer.
Applications remain responsible for content limits and Unicode presentation.

Wove performs no network access. Its terminal backend changes terminal modes
and writes output. It restores modes on normal exit and panic unwinding;
abort, forced termination, and uncatchable signals cannot run cleanup.
