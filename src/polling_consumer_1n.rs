use std::sync::atomic::{AtomicI32, AtomicUsize, AtomicU64};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use zenoh::Wait;

// Shared data
#[repr(C)]
pub struct SharedData {
    pub len: AtomicUsize,
    pub sn: AtomicU64,
    pub read_count: AtomicI32, // How many times the data can be consumed
    pub sub_count: AtomicUsize, // Total number of consumers
    pub data: [u8; 1024],
}

fn main(){
    // Set up Ctrl-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl-C! Shutting down gracefully...");
        r.store(false, Ordering::Release);
    }).expect("Error setting Ctrl-C handler");

    let z = zenoh::open(zenoh::Config::default())
        .wait()
        .expect("Failed to open Zenoh session");

    let rs = z
        .get("shm/polling/buffer_1n")
        .wait()
        .expect("Failed to get resource");

    if let Ok(mut r) = rs.recv() {
        if let Ok(res) = r.result_mut() {
            let buf = res.payload_mut();

            let shm = if let Some(sm) = buf.as_shm() {
                println!("Received SHM buffer");
                sm.as_ptr() as *const SharedData
            } else {
                println!("Received non-SHM buffer");
                std::ptr::null()
            };

            if shm.is_null() {
                println!("Failed to get SHM pointer");
                return;
            }

            let shared_data: &SharedData = unsafe { &*shm };
            shared_data.sub_count.fetch_add(1, Ordering::AcqRel);
            let mut read_count = -1;
            let mut next_sn = 0u64;            
            while running.load(Ordering::Acquire) {
                let len = shared_data.len.load(Ordering::Acquire);                
                read_count = shared_data.read_count.load(Ordering::Acquire);
                
                if len > 0 && read_count > 0 {
                    // There is some data to read, if the SN is higher than what we read last time
                    let sn = shared_data.sn.load(Ordering::Acquire);                       
                    if sn == next_sn || next_sn == 0 {
                        // If we are here, it means we can read the data
                        read_count = shared_data.read_count.fetch_sub(1, Ordering::AcqRel); 
                        next_sn = sn + 1;
                        let mut sum: u32 = 0;
                        for i in 0..len {
                            sum += shared_data.data[i] as u32;
                        }
                        println!("{} / {} - Consumed buffer of {} bytes with sum {} remaining {} reads ", sn, next_sn, len, sum, read_count -1);
                        // Just simulate some processing time
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        
                        if read_count == 1 {
                            log::debug!("{sn} / {next_sn} - Last read, resetting length");
                            shared_data.len.store(0, Ordering::Release);
                        }
                    } else {
                        log::debug!("Waiting for new data, current sn: {sn}, next sn: {next_sn}");
                    }
                } else {
                    // No data to read, wait for a while
                    // println!("No data to read, waiting for sample {}", next_sn);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            println!("Polling consumer stopped.");                           
            shared_data.sub_count.fetch_sub(1, Ordering::AcqRel);
            if read_count == 1 {                
                shared_data.len.store(0, Ordering::Release);            
            }
        }   
    }
}
