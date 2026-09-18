//! Serve a separate input field to each authenticated SSH connection.
use wove::{
    elements::{Input, Text},
    Tree,
};
use wove_ssh::{Error, Server};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: server <host-key> <authorized-public-key>".into());
    }
    let host = russh::keys::load_secret_key(&args[0], None)?;
    let allowed = russh::keys::load_public_key(&args[1])?;
    let server = Server::new(
        host,
        move |_, key| key == &allowed,
        |peer| {
            let mut tree = Tree::new();
            tree.add(
                tree.root(),
                Text::new(format!("Welcome, {}. Ctrl-C to leave.", peer.user)),
            )?;
            let input = tree.add(tree.root(), Input::default())?;
            tree.focus(Some(input))?;
            Ok(tree)
        },
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:2222").await?;
    eprintln!("Listening on {}", listener.local_addr()?);
    server
        .serve(listener, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
