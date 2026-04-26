use crate::state::SharedState;
use tokio::net::TcpStream;

pub async fn handle_connection(mut client_stream: TcpStream, state: SharedState) -> anyhow::Result<()> {
    // Select a backend
    let backend_addr = {
        let mut state_guard = state.write().await;
        state_guard.next_backend_addr()
    };

    match backend_addr {
        Some(addr) => {
            tracing::info!("Proxying to {}", addr);
            let mut backend_stream = TcpStream::connect(addr).await?;

            // Proxy data bidirectionally between client and backend
            let (from_client, from_backend) =
                tokio::io::copy_bidirectional(&mut client_stream, &mut backend_stream).await?;

            tracing::info!(
                "Connection closed. Sent {} bytes, received {} bytes from {}",
                from_client,
                from_backend,
                addr
            );
        }
        None => {
            tracing::error!("No healthy backends available");
        }
    }

    Ok(())
}
