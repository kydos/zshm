use std::sync::atomic::AtomicUsize;

use rand::random;
use zenoh::{
    Wait,
    shm::{AllocAlignment, ShmProviderBuilder, ZShm},
};

// Shared data
#[repr(C)]
pub struct SharedData {
    pub len: AtomicUsize,
    pub data: [u8; 1024],
}

fn main() {
    // get alignment for SharedData type by means of new API
    let alignment = AllocAlignment::for_type::<SharedData>();
    let size = std::mem::size_of::<SharedData>();

    let shm_provider = ShmProviderBuilder::default_backend(size)
        .with_alignment(alignment)
        .wait()
        .unwrap();

    let mut buf = shm_provider
        .alloc(size)
        .with_alignment(alignment)
        .wait()
        .unwrap();

    // initialize data
    let shared_data = unsafe {
        let ptr = buf.as_mut_ptr() as *mut SharedData;
        let shared_data = &mut *ptr;

        shared_data
            .len
            .store(0, std::sync::atomic::Ordering::Release);
        shared_data
    };

    // change the morph of buf to be able to make it's copies
    let buf: ZShm = buf.into();

    // shallow copy to move in responder thread
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
