#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicI32, AtomicUsize, AtomicU64};

#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::Arc;

#[cfg(target_os = "linux")]
use zenoh::Wait;

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
fn main(){
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
        .get("shm/await/buffer_1n")
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
                log::debug!("Waiting for data to be produced -- futex: {}  / {}", shared_data.futex.value.load(Ordering::SeqCst), next_sn);
                while shared_data.futex.value.load(Ordering::Acquire) == 0 {
                    let _ = shared_data.futex.wait(0);                    
                }
               

                let len = shared_data.len.load(Ordering::Acquire);                
                read_count = shared_data.read_count.load(Ordering::Acquire);
                let sub_count = shared_data.sub_count.load(Ordering::Acquire);
                log::debug!("Read count: {}, Sub count: {}, Length: {}", read_count, sub_count, len);
                
                // The only case in which this could happen is if another consumer was added. 
                if read_count > 0 {
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
                            log::debug!("{} / {} - Last read, resetting length", sn, next_sn);
                            shared_data.futex.value.store(0, Ordering::SeqCst);
                            shared_data.futex.wake(1); // Notify the producer that we are done consuming                            
                            
                        }
                    } else {
                        log::debug!("Waiting for new data, current sn: {}, next sn: {}", sn, next_sn);                        
                    }
                } else {
                    log::debug!("Read count is 0, no data to consume");
                }
            }
            println!("Polling consumer stopped.");                           
            shared_data.sub_count.fetch_sub(1, Ordering::AcqRel);
            if read_count == 1 {                
                shared_data.futex.value.store(0, Ordering::SeqCst);
                shared_data.futex.wake(1); // Notify the producer that we are done consuming
            }
        }   
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("This program only runs on Linux due to futex usage.");
    std::process::exit(1);
}
