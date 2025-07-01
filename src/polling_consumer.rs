use std::sync::atomic::AtomicUsize;

use zenoh::Wait;

// Shared data
#[repr(C)]
pub struct SharedData {
    pub len: AtomicUsize,
    pub data: [u8; 1024],
}

fn main(){
    let z = zenoh::open(zenoh::Config::default())
        .wait()
        .expect("Failed to open Zenoh session");

    let rs = z
        .get("shm/polling/buffer")
        .wait()
        .expect("Failed to get resource");

    if let Ok(mut r) = rs.recv() {
        if let Ok(res) = r.result_mut() {
            let buf = res.payload_mut();

            let shm = if let Some(sm) = buf.as_shm() {
                sm.as_ptr() as *const SharedData
            } else {                
                std::ptr::null()
            };

            if shm.is_null() {
                println!("Did not receive a SHM buffer");
                return;
            }

            let shared_data: &SharedData = unsafe { &*shm };
            loop {                
                let len = shared_data.len.load(std::sync::atomic::Ordering::Acquire);
                if len > 0 {                     
                    let mut sum: u32 = 0;
                    for i in 0..len {
                        sum += shared_data.data[i] as u32;
                    }
                    println!("Consumed buffer of {len} bytes with sum {sum}");
                    // Just simulate some processing time
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    shared_data.len.store(0, std::sync::atomic::Ordering::Release);
                }
                else {
                    // Wait until the data is set
                    std::thread::sleep(std::time::Duration::from_millis(100));

                }
            }                                            
        }
    }
}
