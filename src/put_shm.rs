use std::io::Write;

use zenoh::Wait;
use zenoh::shm::{ShmProviderBuilder, ZShm};
fn main() {
    let z = zenoh::open(zenoh::Config::default())
        .wait()
        .expect("Failed to open Zenoh session");

    let provider = ShmProviderBuilder::default_backend(64 * 1024)
        .wait()
        .expect("Failed to create SHM provider");

    let msg = "Hello from Zenoh's Shared Memory!";
    let mut buf = provider.alloc(msg.len()).wait().unwrap();
    buf[..msg.len()].copy_from_slice(msg.as_bytes());

    // Make the buf immutable so that we can (shallow) clone it.
    let data: ZShm = buf.into();

    loop {
        z.put("zenoh/shm/buffer", data.clone())
            .wait()
            .expect("Failed to put SHM buffer");
        print!(".");
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
