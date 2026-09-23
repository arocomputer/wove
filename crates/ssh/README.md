# SSH

Serve [Wove](https://wovetui.com) apps over SSH. Each connection gets its own
copy of your app.

```sh
ssh-keygen -t ed25519 -f /tmp/wove-host -N ''
cargo run -p wove-ssh --example server -- /tmp/wove-host ~/.ssh/id_ed25519.pub
ssh -p 2222 -i ~/.ssh/id_ed25519 guest@localhost
```

The example listens on localhost and accepts only the public key you pass it.
Keep the host key out of your repository.

See the [SSH guide](https://wovetui.com/docs/ssh/) for the full
example and the connection limits.
