mod data_frame;
mod frame;
mod link;
mod session;

#[cfg(test)]
mod behavior_tests;

use link::{FrameLink, next_initial_seq_number};
use std::{
    any::Any,
    io,
    num::NonZeroU8,
    panic::{self, AssertUnwindSafe},
    sync::Arc,
    thread,
};

use log::{error, info};
use parking_lot::Mutex;

use crate::db;
use crate::shared::{FrameError, bitmap::IdMap8};
use crate::transport::Transport;
use crate::vfs::VfsDirManager;
use session::Session;
#[cfg(test)]
pub(crate) use session::{authenticate, sign_on_properties};

/// A connection id only has to be unique among the clients currently attached,
/// so it is handed back to the pool as soon as the session holding it is gone.
struct ConnectionId {
    id: NonZeroU8,
    pool: Arc<Mutex<IdMap8>>,
}

impl ConnectionId {
    fn acquire(pool: &Arc<Mutex<IdMap8>>) -> Option<Self> {
        let mut ids = pool.lock();
        let id = ids.find_free()?;
        ids.set(id);

        Some(Self {
            id,
            pool: pool.clone(),
        })
    }

    fn get(&self) -> u8 {
        self.id.get()
    }
}

impl Drop for ConnectionId {
    fn drop(&mut self) {
        self.pool.lock().clear(self.id);
    }
}

#[derive(Clone)]
pub struct Server {
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
    connection_ids: Arc<Mutex<IdMap8>>,
}

impl Server {
    pub fn new(conn: Arc<db::Database>, vfs_root: Arc<VfsDirManager>) -> Self {
        Self {
            conn,
            vfs_root,
            connection_ids: Arc::new(Mutex::new(IdMap8::new())),
        }
    }

    pub fn spawn(&self, transport: impl Transport, peer: String) {
        let Some(connection_id) = self.accept(&peer) else {
            return;
        };

        let conn = self.conn.clone();
        let vfs_root = self.vfs_root.clone();
        thread::spawn(move || {
            let mut transport = transport;
            worker(&mut transport, peer, connection_id, conn, vfs_root)
        });
    }

    pub fn serve(&self, mut transport: impl Transport, peer: String) {
        let Some(connection_id) = self.accept(&peer) else {
            return;
        };

        worker(
            &mut transport,
            peer,
            connection_id,
            self.conn.clone(),
            self.vfs_root.clone(),
        );
    }

    fn accept(&self, peer: &str) -> Option<ConnectionId> {
        let Some(connection_id) = ConnectionId::acquire(&self.connection_ids) else {
            error!(target: "server", "rejected client {peer}: no free connection id");
            return None;
        };

        info!(target: "server", "accepted client {peer} as connection {}", connection_id.get());

        Some(connection_id)
    }
}

fn worker(
    transport: &mut impl Transport,
    peer: String,
    connection_id: ConnectionId,
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
) {
    // A session is the only thing a bug in request handling is allowed to cost:
    // unwinding here drops the transport and returns the connection id, and the
    // client is free to connect again.
    let served = panic::catch_unwind(AssertUnwindSafe(|| {
        try_worker(peer.clone(), transport, connection_id, conn, vfs_root)
    }));

    match served {
        Ok(Ok(())) => {}
        Ok(Err(err)) => error!(target: "server", "worker({peer}): fatal error: {err}"),
        Err(payload) => {
            error!(target: "server", "worker({peer}): panicked: {}", panic_reason(&payload));
        }
    }
}

fn panic_reason(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown reason")
}

fn try_worker(
    peer: String,
    transport: &mut impl Transport,
    connection_id: ConnectionId,
    conn: Arc<db::Database>,
    vfs_root: Arc<VfsDirManager>,
) -> io::Result<()> {
    let initial_seq_number = next_initial_seq_number();

    let link = FrameLink::new(transport, connection_id, initial_seq_number);
    let session = Session::new(conn, vfs_root);
    run_session(&peer, link, session)
}

fn run_session(peer: &str, mut link: FrameLink<'_>, mut session: Session) -> io::Result<()> {
    let mut message = Vec::new();
    let mut body = Vec::new();
    let result = (|| -> Result<(), FrameError> {
        while let Some(data) = link.receive_data()? {
            for response in session.process_data_frame(&data)? {
                response.write_into(&mut message, &mut body)?;
                link.send_data(&body)?;
            }
        }
        info!(target: "server", "worker({peer}): connection closed");
        Ok(())
    })();
    result.map_err(|err| match err {
        FrameError::Io(err) => err,
        err => io::Error::other(err),
    })
}
