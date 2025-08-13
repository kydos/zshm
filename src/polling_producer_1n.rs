use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};

use rand::random;
use zenoh::{
    shm::{BuildLayout, ResideInShm, ShmBufUnsafeMut, ShmProviderBuilder, Typed, ZShm},
    Wait,
};

// Shared data
#[repr(C)]
pub struct SharedData {
    pub len: AtomicUsize,
    pub sn: AtomicU64,
    pub read_count: AtomicI32,  // How many times the data can be consumed
    pub sub_count: AtomicUsize, // Total number of consumers
    pub data: [u8; 1024],
}

// #SAFETY: this is safe because SharedData is safe to be shared
unsafe impl ResideInShm for SharedData {}

fn main() {
    let typed_layout = BuildLayout::for_type::<SharedData>();

    let shm_provider = ShmProviderBuilder::default_backend(&typed_layout)
        .wait()
        .unwrap();

    // allocate typed SHM buffer
    let mut buf = shm_provider.alloc(typed_layout).wait().unwrap();

    // initialize data
    {
        let shared_data = buf.as_mut();
        shared_data.len.store(0, Ordering::Release);
        shared_data.sn.store(0, Ordering::Release);
        shared_data.sub_count.store(0, Ordering::Release);
        shared_data.read_count.store(0, Ordering::Release);
    };

    // change the morph of buf to be able to make it's shallow copies
    let mut buf: Typed<SharedData, ZShm> = buf.into();

    // shallow copy to move in responder thread
    let buf_in_thread = buf.clone();
    let tid = std::thread::spawn(move || {
        let z = zenoh::open(zenoh::Config::default())
            .wait()
            .expect("Failed to open Zenoh session");

        let queryable = z
            .declare_queryable("shm/polling/buffer_1n")
            .wait()
            .expect("Failed to declare queryable");

        while let Ok(query) = queryable.recv() {
            query
                .reply("shm/polling/buffer_1n", buf_in_thread.clone())
                .wait()
                .expect("Failed to reply to query");
        }
    });

    // get mutable shared data
    let shared_data = unsafe { buf.as_mut_unchecked() };

    // producer loop
    while !tid.is_finished() {
        // Wait until the subscriber is ready
        while shared_data.sub_count.load(Ordering::Acquire) == 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let len = shared_data.len.load(Ordering::Acquire);
        if len == 0 {
            shared_data.sn.fetch_add(1, Ordering::AcqRel);
            let mut sum: usize = 0;
            let len = (512 + random::<u32>() % 513) as usize; //
            for i in 0..len {
                let r: u8 = rand::random();
                shared_data.data[i] = r;
                sum += r as usize;
            }

            println!(
                "{} - Produced buffer of {} bytes with sum of {} for {} subs",
                shared_data.sn.load(Ordering::Acquire),
                len,
                sum,
                shared_data.sub_count.load(Ordering::Acquire)
            );
            shared_data.read_count.store(
                shared_data.sub_count.load(Ordering::Acquire) as i32,
                Ordering::Release,
            );
            shared_data.len.store(len, Ordering::Release);
        } else {
            // Wait until the data is consumed
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    tid.join().expect("Responder thread panicked");
}
