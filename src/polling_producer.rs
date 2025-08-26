use std::sync::atomic::AtomicUsize;

use rand::random;
use zenoh::{
    shm::{BuildLayout, ResideInShm, ShmBufIntoImmut, ShmBufUnsafeMut, ShmProviderBuilder},
    Wait,
};

// Shared data
#[repr(C)]
pub struct SharedData {
    pub len: AtomicUsize,
    pub data: [u8; 1024],
}

// #SAFETY: this is safe because SharedData is safe to be shared
unsafe impl ResideInShm for SharedData {}

fn main() {
    let typed_layout = BuildLayout::for_type::<SharedData>();

    let shm_provider = ShmProviderBuilder::default_backend(typed_layout.layout())
        .wait()
        .unwrap();

    // allocate typed SHM buffer
    let buf = shm_provider.alloc(typed_layout).wait().unwrap();

    // initialize data
    buf.len.store(0, std::sync::atomic::Ordering::Release);

    // change the morph of buf to be able to make it's shallow copies
    let mut buf = buf.into_immut();

    // shallow copy `buf` to move it in responder thread
    let buf_in_thread = buf.clone();
    let tid = std::thread::spawn(move || {
        let z = zenoh::open(zenoh::Config::default())
            .wait()
            .expect("Failed to open Zenoh session");

        let queryable = z
            .declare_queryable("shm/polling/buffer")
            .wait()
            .expect("Failed to declare queryable");

        while let Ok(query) = queryable.recv() {
            query
                .reply("shm/polling/buffer", buf_in_thread.clone())
                .wait()
                .expect("Failed to reply to query");
        }
    });

    // get mutable shared data
    let shared_data = unsafe { buf.as_mut_unchecked() };

    // producer loop
    while !tid.is_finished() {
        let len = shared_data.len.load(std::sync::atomic::Ordering::Acquire);
        if len == 0 {
            let mut sum: usize = 0;
            let len = (512 + random::<u32>() % 513) as usize; //
            for i in 0..len {
                let r: u8 = rand::random();
                shared_data.data[i] = r;
                sum += r as usize;
            }

            println!("Produced buffer of {len} bytes with sum of {sum}");
            shared_data
                .len
                .store(len, std::sync::atomic::Ordering::Release);
        } else {
            // Wait until the data is consumed
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    tid.join().expect("Responder thread panicked");
}
