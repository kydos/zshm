use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};

use rand::random;
use zenoh::{
    Wait,
    shm::{AllocAlignment, ShmBuf, ShmProviderBuilder, ZShm},
};

// Shared data
#[repr(C)]
pub struct SharedData {
    pub len: AtomicUsize,
    pub sn: AtomicU64,
    pub read_count: AtomicI32, // How many times the data can be consumed
    pub sub_count: AtomicUsize, // Total number of consumers
    pub data: [u8; 1024],
}

fn main() {
    let mut  matrix:  [[i32; 10]; 10] = [[0; 10]; 10];
    matrix[1][1] = 42; // Example usage of a matrix
    let z = zenoh::open(zenoh::Config::default())
        .wait()
        .expect("Failed to open Zenoh session");

    // get alignment for SharedData type by means of new API
    let alignment = AllocAlignment::for_type::<SharedData>();
    let size = std::mem::size_of::<SharedData>();

    // construct provider using default_backend that now supports alignment setting
    let shm_provider = ShmProviderBuilder::default_backend(size)
        .with_alignment(alignment)
        .wait()
        .unwrap();
    
    let buf = shm_provider
        .alloc(size)
        .with_alignment(alignment)
        .wait()
        .unwrap();

       
    let shared_data = unsafe { 
         let ptr = buf.as_ptr() as *const SharedData;
        &*ptr 
    };
    shared_data.len
        .store(0, Ordering::Release);
    shared_data.sn.store(0, Ordering::Release);
    shared_data.sub_count.store(0, Ordering::Release);
    shared_data.read_count.store(0, Ordering::Release);

    // change the morph of buf to be able to make it's copies
    let buf: ZShm = buf.into();

    // shallow copy to move in producer thread    
    let mut buf_in_thread = buf.clone();
    let tid = std::thread::spawn(move || {
        
        let shared_data = unsafe { 
            let ptr = buf_in_thread.as_mut_unchecked().as_mut_ptr() as *mut SharedData;
            &mut *ptr 
        };

        loop {
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

                println!("{} - Produced buffer of {} bytes with sum of {} for {} subs", 
                    shared_data.sn.load(Ordering::Acquire), len, sum, shared_data.sub_count.load(Ordering::Acquire));
                shared_data.read_count.store(shared_data.sub_count.load(Ordering::Acquire) as i32, Ordering::Release);
                shared_data.len
                    .store(len, Ordering::Release);
                
            } else {
                // Wait until the data is consumed
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });

    let queryable = z
        .declare_queryable("shm/polling/buffer_1n")
        .wait()
        .expect("Failed to declare queryable");

    while let Ok(query) = queryable.recv() {
        query
            .reply("shm/polling/buffer_1n", buf.clone())
            .wait()
            .expect("Failed to reply to query");
    }

    tid.join().expect("Producer thread panicked");
}
