# SSH

Serve Wove applications through SSH with `wove-ssh`. Supply a persistent host key,
a public-key authorization policy, and a factory that creates an app per peer.
Each app owns its tree and input state. No local terminal is acquired.

```sh
ssh-keygen -t ed25519 -f /tmp/wove-host -N ''
cargo run -p wove-ssh --example server -- /tmp/wove-host ~/.ssh/id_ed25519.pub
ssh -p 2222 -i ~/.ssh/id_ed25519 guest@localhost
```

The example listens on loopback and accepts only the public key supplied on the
command line. Ctrl-C exits a client; Ctrl-C in the server stops all connections.
The host key file is private. Keep it outside the repository.

See the [SSH guide](https://github.com/intuitums/wove/blob/main/crates/web/src/content/docs/ssh.mdx)
for the application contract and limits.
