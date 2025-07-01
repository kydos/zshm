#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};

#[cfg(target_os = "linux")]
use rand::random;
#[cfg(target_os = "linux")]
use zenoh::{
    Wait,
    shm::{AllocAlignment, ShmBuf, ShmProviderBuilder, ZShm},
};

#[cfg(target_os = "linux")]
use linux_futex::*;

#[cfg(target_os = "linux")]
// Shared data
#[repr(C)]
pub struct SharedData {
    pub futex: Futex<Shared>, 
    pub len: AtomicUsize,
    pub sn: AtomicU64,
    pub read_count: AtomicI32, // How many times the data can be consumed
    pub sub_count: AtomicUsize, // Total number of consumers
    pub data: [u8; 1024],
}

#[cfg(target_os = "linux")]
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
    shared_data.futex.value.store(0, Ordering::Release);

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

            log::debug!("Waiting for data to be consumed, futex: {}", shared_data.futex.value.load(Ordering::Acquire));
            while shared_data.futex.value.load(Ordering::Acquire) == 1     {
                let _ = shared_data.futex.wait(1);
            }
            log::debug!("Done Waiting...");
            
            let sn = shared_data.sn.fetch_add(1, Ordering::AcqRel) + 1;
            let mut sum: usize = 0;
            let len = (512 + random::<u32>() % 513) as usize; // 
            for i in 0..len {
                let r: u8 = rand::random();
                shared_data.data[i] = r;
                sum += r as usize;
            }
                                    
            
            shared_data.read_count.store(std::cmp::max(shared_data.sub_count.load(Ordering::Acquire), 1) as i32, Ordering::Release);
            shared_data.len.store(len, Ordering::Release);            
            println!("{} - Produced buffer of {} bytes with sum of {} for {} subs with read count {}", 
                sn, len, sum, shared_data.sub_count.load(Ordering::Acquire), shared_data.read_count.load(Ordering::Acquire));

            log::debug!("{} - Data ready, waking up consumers", sn);
            shared_data.futex.value.store(1, Ordering::Release);
            shared_data.futex.wake(shared_data.sub_count.load(Ordering::Acquire) as i32); // Notify all consumers that data is ready
        }
    });

    let queryable = z
        .declare_queryable("shm/await/buffer_1n")
        .wait()
        .expect("Failed to declare queryable");

    while let Ok(query) = queryable.recv() {
        query
            .reply("shm/await/buffer_1n", buf.clone())
            .wait()
            .expect("Failed to reply to query");
    }

    tid.join().expect("Producer thread panicked");
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("This program only runs on Linux due to futex usage.");
    std::process::exit(1);
}
